use anyhow::{anyhow,Context, Result};
use compute_abi::{
    ProgramId, ProgramManifest, ProgramSlot,
    ProgramVersion,
};

use compute_core::{ComputeOutput, ExecutorInputs};
use compute_program::{ProgramDescriptor, SmartProgram};

use mesh_gateway_wire::{decode, encode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use tfhe::prelude::*;
use tfhe::{CompactCiphertextList,
    FheBool, FheUint64,
    safe_serialization::{safe_deserialize, safe_serialize}
};

use std::io::Cursor;

// Program identifier chosen for the SatCon alert smart program.
///
/// This crate owns that identity and its typed ABI.
pub const PROGRAM_ID: &str = "satcon.alert_math";
pub const PROGRAM_VERSION: &str = "0.1.0";

use crate::graph::{
    SESSION_INPUT_KIND_CONFIG_V1,
    SESSION_INPUT_KIND_CONVOY_V1,
    SESSION_INPUT_KIND_SATELLITE_V1,
};

pub const SESSION_INPUT_CONFIG_V1: &str = SESSION_INPUT_KIND_CONFIG_V1;
pub const SESSION_INPUT_CONVOY_V1: &str = SESSION_INPUT_KIND_CONVOY_V1;
pub const SESSION_INPUT_SATELLITE_V1: &str = SESSION_INPUT_KIND_SATELLITE_V1;

const SAFE_SER_SIZE_LIMIT: u64 = 1024 * 1024 * 1024;

/// Domain-owned smart program implementation.
///
/// Important:
/// - this crate owns the typed ABI and manifest of the SatCon alert program
/// - the generic compute layer should not contain alert-specific domain logic
pub struct SmartProgramAlert;

impl SmartProgram for SmartProgramAlert {
    fn descriptor(&self) -> ProgramDescriptor {
        ProgramDescriptor {
            program_id: ProgramId(PROGRAM_ID.to_string()),
            program_version: ProgramVersion(PROGRAM_VERSION.to_string()),
        }
    }

    fn manifest(&self) -> ProgramManifest {
        manifest_v1()
    }

    fn execute(&self, exec_inputs: ExecutorInputs) -> Result<ComputeOutput> {
        execute_v1(exec_inputs)
    }

    fn try_build_session_inputs(
        &self,
        session_inputs: &HashMap<String, Vec<u8>>,
    ) -> Result<Option<Vec<Vec<u8>>>> {
        Ok(try_build_session_input_blob_v1(session_inputs)?
            .map(|input_blob| vec![input_blob]))
    }
}

/// Program manifest for the first owned SatCon alert program version.
///
/// This is intentionally domain-owned here rather than in a generic crate.
pub fn manifest_v1() -> ProgramManifest {
    ProgramManifest {
        program_id: ProgramId(PROGRAM_ID.to_string()),
        program_version: ProgramVersion(PROGRAM_VERSION.to_string()),
        abi_version: "compute-abi/v1".into(),
        executor_backend: "external-process-v1".into(),
        required_artifacts: vec![],
        input_slots: vec![ProgramSlot {
            name: "alert_math_input_v1".into(),
            encoding: "osatcon.alert_math.input.v1".into(),
            encrypted: true,
        }],
        output_slots: vec![ProgramSlot {
            name: "alert_math_output_v1".into(),
            encoding: "osatcon.alert_math.output.v1".into(),
            encrypted: true,
        }],
        compatibility: vec![
            ("program_family".into(), "osatcon".into()),
            ("program_name".into(), "alert_math".into()),
            ("semantic_contract".into(), "satellite_convoy_alert".into()),
            ("numeric_contract".into(), "local_projected_km_sqdist_u64".into()),
        ],
    }
}

/// Encrypted execution/configuration parameters.
///
/// These values are supplied by the caller and remain encrypted during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMathEncryptedConfigV1 {
    /// Squared proximity threshold in the same fixed-point units used by the
    /// encrypted coordinates.
    ///
    /// This is typically produced by:
    /// `ofield_core::geo::encode_distance_threshold_sq(radius_m)`.
    pub radius_sq: Vec<u8>,

    /// The caller may submit more satellites, but only the first `max_sats_scan`
    /// are evaluated.
    /// Serialized CompactCiphertextList containing one encrypted u64.
    ///
    /// Semantics:
    /// - value is the max number of satellites to consider
    /// - execution still loops over the submitted batch length
    /// - each satellite index is compared to this encrypted value
    pub max_sats_scan: Vec<u8>,
}

/// Program-specific encrypted input envelope.
///
/// This crate owns the program-side input contract.
/// Edge nodes and submitters should assemble this payload, then hand it
/// to the generic compute submission layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMathInputV1 {
    pub encrypted_config: AlertMathEncryptedConfigV1,
    pub encrypted_fields: AlertMathEncryptedFieldsV1,
    pub metadata: Vec<(String, String)>,
}

/// Program-specific encrypted output envelope.
///
/// This stays program-owned and can evolve version-by-version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMathOutputV1 {
    pub encrypted_outputs: AlertMathEncryptedOutputsV1,
    pub metadata: Vec<(String, String)>,
}

/// Program-owned encrypted field bundle.
///
/// We keep two opaque sub-envelopes at the outer ABI boundary so submitters
/// only need one canonical input blob, while the smart program still owns the
/// internal encrypted structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMathEncryptedFieldsV1 {
    /// Serialized `EncryptedConvoyPointV1`
    pub convoy_payload_ct: Vec<u8>,

    /// Serialized `EncryptedSatelliteBatchV1`
    pub satellite_payload_ct: Vec<u8>,
}

/// First executable encrypted output contract.
///
/// V1 intentionally emits only the encrypted exposure predicate.
/// This keeps the first TFHE kernel compact and avoids encrypted argmin/sort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMathEncryptedOutputsV1 {
    /// Serialized `FheBool`
    pub exposed_ct: Vec<u8>,

    /// Encrypted per-satellite distances for the scanned candidate set.
    pub distance_context: EncryptedDistanceBatchV1,

    /// encrypted context used by UI as expanded cyphertexts
    /// warning:
    /// - input `AlertMathEncryptedFieldsV1` carries compact-list ciphertext bytes
    /// - output `context_of_exposition` should carry expanded ciphertext bytes
    pub context_of_exposition: AlertMathEncryptedFieldsV1, 
}

/// Encrypted convoy point in fixed-point planar coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedConvoyPointV1 {
    /// Serialized encrypted u64 coordinate.
    ///
    /// In input payloads this is a safe-serialized `CompactCiphertextList`.
    /// In output context payloads this is a safe-serialized expanded `FheUint64`.
    pub x_ct: Vec<u8>,

    /// Serialized encrypted u64 coordinate.
    ///
    /// In input payloads this is a safe-serialized `CompactCiphertextList`.
    /// In output context payloads this is a safe-serialized expanded `FheUint64`.
    pub y_ct: Vec<u8>,
}

/// Encrypted satellite point in fixed-point planar coordinates.
///
/// `sat_id` is left cleartext in V1 because the V1 kernel only computes
/// an encrypted boolean exposure flag and does not emit per-satellite results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSatellitePointV1 {
    pub sat_id: i32,
    /// Serialized encrypted u64 coordinate.
    ///
    /// In input payloads this is a safe-serialized `CompactCiphertextList`.
    /// In output context payloads this is a safe-serialized expanded `FheUint64`.
    pub x_ct: Vec<u8>,
    /// Serialized encrypted u64 coordinate.
    ///
    /// In input payloads this is a safe-serialized `CompactCiphertextList`.
    /// In output context payloads this is a safe-serialized expanded `FheUint64`.
    pub y_ct: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSatelliteBatchV1 {
    pub sats: Vec<EncryptedSatellitePointV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedDistancePointV1 {
    pub sat_id: i32,

    /// Serialized expanded `FheUint64`.
    ///
    /// This is d2 = dx² + dy² in projected fixed-point space.
    pub d2_ct: Vec<u8>,

    /// Serialized expanded `FheBool`.
    pub within_radius_ct: Vec<u8>,

    /// Serialized expanded `FheBool`.
    pub within_scan_ct: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedDistanceBatchV1 {
    pub sats: Vec<EncryptedDistancePointV1>,
}

/// Canonical builder for the alert input blob.
///
/// Submitters should use this instead of crafting ad hoc payloads.
pub fn build_input_blob_v1(input: AlertMathInputV1) -> Result<Vec<u8>> {
    Ok(encode(&input)?)
}

/// Decode helper for workers / tests.
pub fn decode_input_blob_v1(bytes: &[u8]) -> Result<AlertMathInputV1> {
    decode(bytes).map_err(|e| anyhow!("decode AlertMathInputV1 failed: {e}"))
}

/// Canonical builder for the output blob.
///
/// The program crate owns the output envelope format.
pub fn build_output_blob_v1(output: AlertMathOutputV1) -> Result<Vec<u8>> {
    Ok(encode(&output)?)
}

pub fn decode_output_blob_v1(bytes: &[u8]) -> Result<AlertMathOutputV1> {
    decode(bytes).map_err(|e| anyhow!("decode AlertMathOutputV1 failed: {e}"))
}

/// Helper for building the inner encrypted convoy payload.
pub fn build_encrypted_convoy_payload_v1(payload: EncryptedConvoyPointV1) -> Result<Vec<u8>> {
    Ok(encode(&payload)?)
}

/// Helper for decoding the inner encrypted convoy payload.
pub fn decode_encrypted_convoy_payload_v1(bytes: &[u8]) -> Result<EncryptedConvoyPointV1> {
    decode(bytes).map_err(|e| anyhow!("decode EncryptedConvoyPointV1 failed: {e}"))
}

/// Helper for building the inner encrypted satellite payload.
pub fn build_encrypted_satellite_payload_v1(payload: EncryptedSatelliteBatchV1) -> Result<Vec<u8>> {
    Ok(encode(&payload)?)
}

/// Helper for decoding the inner encrypted satellite payload.
pub fn decode_encrypted_satellite_payload_v1(bytes: &[u8]) -> Result<EncryptedSatelliteBatchV1> {
    decode(bytes).map_err(|e| anyhow!("decode EncryptedSatelliteBatchV1 failed: {e}"))
}

/// First execution entrypoint owned by the SatCon program crate.
///
/// V1 kernel semantics:
/// - convoy point is encrypted
/// - satellite points are encrypted
/// - threshold radius_sq is cleartext config
/// - output is a single encrypted boolean:
///     exposed = OR_i( squared_distance(convoy, sat_i) <= radius_sq )
///
/// No plaintext convoy/satellite coordinates are ever materialized in the
/// compute node execution path.
pub fn execute_v1(exec_inputs: ExecutorInputs) -> Result<ComputeOutput> {
    if exec_inputs.inputs.len() != 1 {
        return Err(anyhow!(
            "smart-program-alert expects exactly 1 input blob, got {}",
            exec_inputs.inputs.len()
        ));
    }

    let input = decode_input_blob_v1(&exec_inputs.inputs[0])?;
    validate_encrypted_config(&input.encrypted_config)?;

    let radius_sq = decode_compact_fhe_uint64(&input.encrypted_config.radius_sq)
        .context("failed to decode encrypted radius_sq")?;
    let max_sat_scan = decode_compact_fhe_uint64(&input.encrypted_config.max_sats_scan)
        .context("failed to decode encrypted max_sats_scan")?;
    
    let convoy = decode_encrypted_convoy_payload_v1(&input.encrypted_fields.convoy_payload_ct)?;
    let sats = decode_encrypted_satellite_payload_v1(&input.encrypted_fields.satellite_payload_ct)?;

    let convoy_x =
        decode_compact_fhe_uint64(&convoy.x_ct).context("failed to decode convoy x compact ciphertext")?;
    let convoy_y =
        decode_compact_fhe_uint64(&convoy.y_ct).context("failed to decode convoy y compact ciphertext")?;

    let output_convoy_context = EncryptedConvoyPointV1 {
        x_ct: serialize_tfhe(&convoy_x)
            .context("failed to serialize expanded convoy x ciphertext")?,
        y_ct: serialize_tfhe(&convoy_y)
            .context("failed to serialize expanded convoy y ciphertext")?,
    };

    let mut acc = FheBool::encrypt_trivial(false);
    let mut output_sat_context = Vec::with_capacity(sats.sats.len());
    let mut distance_context = Vec::with_capacity(sats.sats.len());

    for (idx, sat) in sats.sats.iter().enumerate() {
        let sat_x =
            decode_compact_fhe_uint64(&sat.x_ct).context("failed to decode satellite x compact ciphertext")?;
        let sat_y =
            decode_compact_fhe_uint64(&sat.y_ct).context("failed to decode satellite y compact ciphertext")?;

        output_sat_context.push(EncryptedSatellitePointV1 {
            sat_id: sat.sat_id,
            x_ct: serialize_tfhe(&sat_x)
                .context("failed to serialize expanded satellite x ciphertext")?,
            y_ct: serialize_tfhe(&sat_y)
                .context("failed to serialize expanded satellite y ciphertext")?,
        });

        let dx = abs_diff_u64(&sat_x, &convoy_x);
        let dy = abs_diff_u64(&sat_y, &convoy_y);

        // IMPORTANT NUMERIC CONTRACT:
        // This V1 kernel assumes the encoded coordinate domain is bounded such
        // that dx*dx + dy*dy fits in signed 64-bit space.
        //
        // With centimeter scaling, this is safe for tactical/local alerting
        // windows and comfortably below Earth-scale axis deltas.
        let dx2 = &dx * &dx;
        let dy2 = &dy * &dy;
        let d2 = &dx2 + &dy2;

        let within_radius = d2.le(&radius_sq);
        
        // ** Public rank compared against encrypted max_sats_scan.
        // This does not reveal max_sats_scan.
        let rank = (idx as u64) + 1;
        let within_scan = max_sat_scan.ge(rank);

        let accepted = within_radius.clone() & within_scan.clone();

        acc = acc | accepted;

        distance_context.push(EncryptedDistancePointV1 {
            sat_id: sat.sat_id,
            d2_ct: serialize_tfhe(&d2)
                .context("failed to serialize encrypted d2")?,
            within_radius_ct: serialize_tfhe(&within_radius)
                .context("failed to serialize encrypted within_radius")?,
            within_scan_ct: serialize_tfhe(&within_scan)
                .context("failed to serialize encrypted within_scan")?,
        });
    }

    
    let output = AlertMathOutputV1 {
        encrypted_outputs: AlertMathEncryptedOutputsV1 {
            exposed_ct: serialize_tfhe(&acc)
                .context("failed to serialize exposed ciphertext")?,

            distance_context: EncryptedDistanceBatchV1 {
                sats: distance_context,
            },

            context_of_exposition: AlertMathEncryptedFieldsV1 {
                convoy_payload_ct: build_encrypted_convoy_payload_v1(output_convoy_context)
                    .context("failed to build expanded convoy context payload")?,
                satellite_payload_ct: build_encrypted_satellite_payload_v1(
                    EncryptedSatelliteBatchV1 {
                        sats: output_sat_context,
                    },
                )
                .context("failed to build expanded satellite context payload")?,
            },
        },
        metadata: vec![
            ("program_id".into(), PROGRAM_ID.into()),
            ("program_version".into(), PROGRAM_VERSION.into()),
            ("semantic_contract".into(), "satellite_convoy_alert".into()),
            ("numeric_contract".into(), "local_projected_km_sqdist_u64".into()),
            ("output_semantics".into(), "encrypted_exposure_boolean".into()),
            ("distance_semantics".into(), "encrypted_projected_distance_sq_per_candidate".into()),
            ("submitted_satellite_count".into(), sats.sats.len().to_string()),
            ("max_sats_scan".into(), "encrypted".into()),
            ("radius_sq".into(), "encrypted".into()),
            ("context_of_exposition".into(), "expanded_encrypted_u64_context".into()),
        ],
    };

    Ok(ComputeOutput {
        outputs: vec![build_output_blob_v1(output)?],
    })
}

fn validate_encrypted_config(cfg: &AlertMathEncryptedConfigV1) -> Result<()> {
    if cfg.radius_sq.is_empty() {
        return Err(anyhow!("encrypted radius_sq must be non-negative"));
    }

    if cfg.max_sats_scan.is_empty() {
        return Err(anyhow!("encrypted max_sats_scan must be > 0"));
    }

    Ok(())
}

fn deserialize_tfhe<T>(bytes: &[u8]) -> Result<T>
where
    T: DeserializeOwned
        + tfhe::Versionize
        + tfhe::Unversionize
        + tfhe::named::Named,
{
    let cursor = Cursor::new(bytes);

    safe_deserialize::<T>(cursor, SAFE_SER_SIZE_LIMIT)
        .map_err(|e| anyhow!("safe_deserialize TFHE value failed: {e}"))
}

fn serialize_tfhe<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize + tfhe::Versionize + tfhe::named::Named,
{
    let mut bytes = Vec::new();

    safe_serialize(value, &mut bytes, SAFE_SER_SIZE_LIMIT)
        .map_err(|e| anyhow!("safe_serialize TFHE value failed: {e}"))?;

    Ok(bytes)
}

pub fn build_encrypted_config_payload_v1(
    payload: AlertMathEncryptedConfigV1,
) -> Result<Vec<u8>> {
    Ok(encode(&payload)?)
}

pub fn decode_encrypted_config_payload_v1(
    bytes: &[u8],
) -> Result<AlertMathEncryptedConfigV1> {
    decode(bytes).map_err(|e| anyhow!("decode AlertMathCleartextConfigV1 failed: {e}"))
}

fn decode_compact_fhe_uint64(bytes: &[u8]) -> Result<FheUint64> {
    let list: CompactCiphertextList = deserialize_tfhe(bytes)
        .context("failed to deserialize CompactCiphertextList")?;

    let expanded = list
        .expand()
        .map_err(|e| anyhow!("expand CompactCiphertextList failed: {e}"))?;

    expanded
        .get::<FheUint64>(0)
        .map_err(|e| anyhow!("extract FheUint64 from CompactCiphertextList failed: {e}"))?
        .ok_or_else(|| anyhow!("missing FheUint64 at CompactCiphertextList index 0"))
}

pub fn try_build_session_input_blob_v1(
    session_inputs: &HashMap<String, Vec<u8>>,
) -> Result<Option<Vec<u8>>> {
    let Some(config_bytes) = session_inputs.get(SESSION_INPUT_CONFIG_V1) else {
        return Ok(None);
    };

    let Some(convoy_payload_ct) = session_inputs.get(SESSION_INPUT_CONVOY_V1) else {
        return Ok(None);
    };

    let Some(satellite_payload_ct) = session_inputs.get(SESSION_INPUT_SATELLITE_V1) else {
        return Ok(None);
    };

    let encrypted_config = decode_encrypted_config_payload_v1(config_bytes)?;

    let input = AlertMathInputV1 {
        encrypted_config,
        encrypted_fields: AlertMathEncryptedFieldsV1 {
            convoy_payload_ct: convoy_payload_ct.clone(),
            satellite_payload_ct: satellite_payload_ct.clone(),
        },
        metadata: vec![
            ("session_input_model".into(), "three_stream_v1".into()),
            ("config_kind".into(), SESSION_INPUT_CONFIG_V1.into()),
            ("convoy_kind".into(), SESSION_INPUT_CONVOY_V1.into()),
            ("satellite_kind".into(), SESSION_INPUT_SATELLITE_V1.into()),
        ],
    };

    Ok(Some(build_input_blob_v1(input)?))
}

fn abs_diff_u64(a: &FheUint64, b: &FheUint64) -> FheUint64 {
    let a_ge_b = a.ge(b);

    let a_minus_b = a - b;
    let b_minus_a = b - a;

    a_ge_b.if_then_else(&a_minus_b, &b_minus_a)
}