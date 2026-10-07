use anyhow::{anyhow, Context, Result};
use compute_abi::{
    ComputeJobSpec, ExternalProcessInvocationV1, ExternalProcessResponseV1, ProgramId,
    ProgramVersion,
};
use compute_core::ExecutorInputs;
use mesh_gateway_wire::{decode, encode};
use std::{env, path::PathBuf, io::Cursor};

use smart_program_alert::computation::{
    build_input_blob_v1, decode_encrypted_config_payload_v1, execute_v1,
    AlertMathEncryptedFieldsV1, AlertMathInputV1, PROGRAM_ID, PROGRAM_VERSION,
};

use smart_program_alert::graph::{
    SESSION_INPUT_KIND_CONFIG_V1, SESSION_INPUT_KIND_CONVOY_V1,
    SESSION_INPUT_KIND_SATELLITE_V1,
};

use tfhe::{
    ServerKey,
    set_server_key,
    safe_serialization::safe_deserialize,
};

const SAFE_SER_SIZE_LIMIT: u64 = 1024 * 1024 * 1024;

fn main() -> Result<()> {
    let args = RunnerArgs::parse()?;

    let request_bytes = std::fs::read(&args.request)
        .with_context(|| format!("read request {}", args.request.display()))?;

    let invocation: ExternalProcessInvocationV1 = decode(&request_bytes)
        .map_err(|e| anyhow!("decode ExternalProcessInvocationV1 failed: {e}"))?;

    eprintln!(
        "smart-program-alert runner: session_inputs={:?}",
        invocation
            .session_inputs
            .iter()
            .map(|i| (i.input_kind.as_str(), i.sequence, i.bytes.len()))
            .collect::<Vec<_>>()
    );
    
    install_server_key(&invocation.server_key_path)?;
    
    let response = run_alert(invocation)?;

    std::fs::write(&args.response, encode(&response)?)
        .with_context(|| format!("write response {}", args.response.display()))?;

    Ok(())
}

fn decode_server_key(bytes: &[u8]) -> Result<ServerKey> {
    let mut cursor = Cursor::new(bytes);

    let server_key: ServerKey =
        safe_deserialize(&mut cursor, SAFE_SER_SIZE_LIMIT)
            .map_err(|e| anyhow!("safe_deserialize ServerKey failed: {e}"))?;

    Ok(server_key)
}

fn install_server_key(server_key_path: &std::path::Path) -> Result<()> {
    let bytes = std::fs::read(server_key_path)
        .with_context(|| format!("failed to read ServerKey at {}", server_key_path.display()))?;

    let server_key = decode_server_key(&bytes)
        .with_context(|| format!("failed to decode ServerKey at {}", server_key_path.display()))?;

    set_server_key(server_key);

    Ok(())
}

fn run_alert(invocation: ExternalProcessInvocationV1) -> Result<ExternalProcessResponseV1> {
    if invocation.program_id != PROGRAM_ID {
        return Err(anyhow!(
            "program_id mismatch: expected {}, got {}",
            PROGRAM_ID,
            invocation.program_id
        ));
    }

    if invocation.program_version != PROGRAM_VERSION {
        return Err(anyhow!(
            "program_version mismatch: expected {}, got {}",
            PROGRAM_VERSION,
            invocation.program_version
        ));
    }

    let config_bytes = latest_session_input(&invocation, SESSION_INPUT_KIND_CONFIG_V1)?;
    let convoy_payload_ct = latest_session_input(&invocation, SESSION_INPUT_KIND_CONVOY_V1)?;
    let satellite_payload_ct = latest_session_input(&invocation, SESSION_INPUT_KIND_SATELLITE_V1)?;

    let encrypted_config = decode_encrypted_config_payload_v1(config_bytes)
        .context("decode alert config session input")?;

    let input = AlertMathInputV1 {
        encrypted_config,
        encrypted_fields: AlertMathEncryptedFieldsV1 {
            convoy_payload_ct: convoy_payload_ct.to_vec(),
            satellite_payload_ct: satellite_payload_ct.to_vec(),
        },
        metadata: vec![
            ("runner".into(), "smart-program-alert".into()),
            ("runtime_kind".into(), "external-process-v1".into()),
            ("session_input_model".into(), "three_stream_v1".into()),
        ],
    };

    let input_blob = build_input_blob_v1(input)?;

    let spec = ComputeJobSpec {
        program_id: ProgramId(PROGRAM_ID.to_string()),
        program_version: ProgramVersion(PROGRAM_VERSION.to_string()),
        inputs: vec![],
        artifacts: vec![],
        expected_outputs: 1,
        metadata: invocation.metadata.clone(),
    };

    let output = execute_v1(ExecutorInputs {
        spec,
        inputs: vec![input_blob],
        session_inputs: vec![],
        artifacts: vec![],
    })?;

    Ok(ExternalProcessResponseV1 {
        outputs: output.outputs,
        metadata: vec![
            ("runner".into(), "smart-program-alert".into()),
            ("program_id".into(), PROGRAM_ID.into()),
            ("program_version".into(), PROGRAM_VERSION.into()),
        ],
    })
}

fn latest_session_input<'a>(
    invocation: &'a ExternalProcessInvocationV1,
    input_kind: &str,
) -> Result<&'a [u8]> {
    invocation
        .session_inputs
        .iter()
        .filter(|input| input.input_kind == input_kind)
        .max_by_key(|input| input.sequence)
        .map(|input| input.bytes.as_slice())
        .ok_or_else(|| anyhow!("missing required session input: {input_kind}"))
}

struct RunnerArgs {
    request: PathBuf,
    response: PathBuf,
}

impl RunnerArgs {
    fn parse() -> Result<Self> {
        let mut request: Option<PathBuf> = None;
        let mut response: Option<PathBuf> = None;

        let mut args = env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--request" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--request requires a path"))?;
                    request = Some(PathBuf::from(value));
                }

                "--response" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--response requires a path"))?;
                    response = Some(PathBuf::from(value));
                }

                other => {
                    return Err(anyhow!("unknown argument: {other}"));
                }
            }
        }

        Ok(Self {
            request: request.ok_or_else(|| anyhow!("missing --request <path>"))?,
            response: response.ok_or_else(|| anyhow!("missing --response <path>"))?,
        })
    }
}