use anyhow::{anyhow, Context, Result};
use compute_core::{ComputeContext, ComputeExecutor, ComputeOutput, ExecutorInputs};
use std::io::Cursor;
use std::path::PathBuf;
use tfhe::safe_serialization::safe_deserialize;
use tfhe::set_server_key;
use tfhe::ServerKey;

use crate::programs::dispatch_program;

const SAFE_SER_SIZE_LIMIT: u64 = 1024 * 1024 * 1024;

pub struct TfheExecutor {
    server_key_path: PathBuf,
}

impl TfheExecutor {
    pub fn new(server_key_path: PathBuf) -> Self {
        Self { server_key_path }
    }
}

#[async_trait::async_trait]
impl ComputeExecutor for TfheExecutor {
    async fn execute(
        &self,
        _ctx: ComputeContext,
        exec_inputs: ExecutorInputs,
    ) -> Result<ComputeOutput> {
        let server_key_bytes = tokio::fs::read(&self.server_key_path)
            .await
            .with_context(|| {
                format!(
                    "failed to read local ServerKey at {}",
                    self.server_key_path.display()
                )
            })?;

        let server_key = decode_server_key(&server_key_bytes)?;
        set_server_key(server_key);

        Err(anyhow!(
            "generic TFHE runtime execution is not implemented yet; \
             received {} direct inputs and {} session inputs for {}@{}",
            exec_inputs.inputs.len(),
            exec_inputs.session_inputs.len(),
            exec_inputs.spec.program_id.0,
            exec_inputs.spec.program_version.0,
        ))
    }
}

fn decode_server_key(bytes: &[u8]) -> Result<ServerKey> {
    let mut cursor = Cursor::new(bytes);
    let sk: ServerKey =
        safe_deserialize(&mut cursor, SAFE_SER_SIZE_LIMIT)
            .map_err(|e| anyhow!(e.to_string()))?;
    Ok(sk)
}