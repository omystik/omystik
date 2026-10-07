use anyhow::Result;
use compute_abi::{
    ComputeGraphFieldKindV1, ComputeGraphInputBindingV1, ComputeGraphInputSourceV1,
    ComputeGraphOpV1, ComputeGraphOutputBindingV1, ComputeGraphV1Package, Hash32, ProgramId,
    ProgramPackageManifest, ProgramRuntimeKind, ProgramVersion,
};
use mesh_gateway_wire::encode;

use crate::computation::{PROGRAM_ID, PROGRAM_VERSION};

pub const GRAPH_ABI_VERSION: &str = "compute-graph/v1";
pub const GRAPH_ENTRYPOINT: &str = "main";

pub const SESSION_INPUT_KIND_CONFIG_V1: &str = "alert_config_v1";
pub const SESSION_INPUT_KIND_CONVOY_V1: &str = "convoy_payload_v1";
pub const SESSION_INPUT_KIND_SATELLITE_V1: &str = "satellite_payload_v1";

pub const ENCODING_CONFIG_V1: &str = "osatcon.alert_math.config.v1";
pub const ENCODING_ENCRYPTED_CONVOY_POINT_V1: &str =
    "osatcon.alert_math.encrypted_convoy_point.v1";
pub const ENCODING_ENCRYPTED_SATELLITE_BATCH_V1: &str =
    "osatcon.alert_math.encrypted_satellite_batch.v1";
pub const ENCODING_ALERT_OUTPUT_V1: &str = "osatcon.alert_math.output.v1";

pub const SLOT_CONFIG: &str = "alert_config";
pub const SLOT_CONVOY_PAYLOAD: &str = "convoy_payload";
pub const SLOT_SATELLITE_PAYLOAD: &str = "satellite_payload";
pub const SLOT_ALERT_OUTPUT: &str = "alert_output";

pub const VAR_RADIUS_SQ: &str = "radius_sq";
pub const VAR_MAX_SATS_SCAN: &str = "max_sats_scan";

pub const VAR_CONVOY_X_CT_BYTES: &str = "convoy_x_ct_bytes";
pub const VAR_CONVOY_Y_CT_BYTES: &str = "convoy_y_ct_bytes";
pub const VAR_CONVOY_X: &str = "convoy_x";
pub const VAR_CONVOY_Y: &str = "convoy_y";

pub const VAR_SATELLITE_BATCH_BYTES: &str = "satellite_batch_bytes";

pub const VAR_EXPOSED_CT: &str = "exposed_ct";
pub const VAR_EXPOSED_CT_BYTES: &str = "exposed_ct_bytes";

/// Stable graph id for satcon.alert_math@0.1.0.
///
/// This is intentionally constant for the demo program version.
/// If the graph changes, bump PROGRAM_VERSION and graph id together.
pub const GRAPH_ID_V1: Hash32 = [
    0x73, 0x61, 0x74, 0x63, 0x6f, 0x6e, 0x2e, 0x61,
    0x6c, 0x65, 0x72, 0x74, 0x2e, 0x6d, 0x61, 0x74,
    0x68, 0x2e, 0x30, 0x2e, 0x31, 0x2e, 0x30, 0x00,
    0x43, 0x47, 0x56, 0x31, 0x00, 0x00, 0x00, 0x01,
];

/// Build the production deployable ComputeGraphV1 package for the alert program.
///
/// This package is domain-owned by `smart-program-alert`, but it is runtime-generic:
/// Ypolo receives encoded graph bytes and interprets them through its generic
/// `ComputeGraphV1` executor.
pub fn build_compute_graph_v1_package() -> ComputeGraphV1Package {
    ComputeGraphV1Package {
        graph_id: GRAPH_ID_V1,
        program_id: ProgramId(PROGRAM_ID.to_string()),
        program_version: ProgramVersion(PROGRAM_VERSION.to_string()),
        inputs: vec![
            ComputeGraphInputBindingV1 {
                slot_name: SLOT_CONFIG.to_string(),
                source: ComputeGraphInputSourceV1::SessionInput {
                    input_kind: SESSION_INPUT_KIND_CONFIG_V1.to_string(),
                },
                encoding: ENCODING_CONFIG_V1.to_string(),
                encrypted: false,
            },
            ComputeGraphInputBindingV1 {
                slot_name: SLOT_CONVOY_PAYLOAD.to_string(),
                source: ComputeGraphInputSourceV1::SessionInput {
                    input_kind: SESSION_INPUT_KIND_CONVOY_V1.to_string(),
                },
                encoding: ENCODING_ENCRYPTED_CONVOY_POINT_V1.to_string(),
                encrypted: true,
            },
            ComputeGraphInputBindingV1 {
                slot_name: SLOT_SATELLITE_PAYLOAD.to_string(),
                source: ComputeGraphInputSourceV1::SessionInput {
                    input_kind: SESSION_INPUT_KIND_SATELLITE_V1.to_string(),
                },
                encoding: ENCODING_ENCRYPTED_SATELLITE_BATCH_V1.to_string(),
                encrypted: true,
            },
        ],
        outputs: vec![ComputeGraphOutputBindingV1 {
            slot_name: SLOT_ALERT_OUTPUT.to_string(),
            encoding: ENCODING_ALERT_OUTPUT_V1.to_string(),
            encrypted: true,
        }],
        ops: build_compute_graph_v1_ops(),
        metadata: vec![
            ("graph_abi_version".into(), GRAPH_ABI_VERSION.into()),
            ("program_family".into(), "osatcon".into()),
            ("program_name".into(), "alert_math".into()),
            ("semantic_contract".into(), "satellite_convoy_alert".into()),
            ("numeric_contract".into(), "fixed_point_planar_sqdist".into()),
            ("coordinate_scalar".into(), "100000".into()),
            ("output_semantics".into(), "encrypted_exposure_boolean".into()),
        ],
    }
}

/// Encode the graph package into deployable package bytes.
pub fn build_compute_graph_v1_package_bytes() -> Result<Vec<u8>> {
    Ok(encode(&build_compute_graph_v1_package())?)
}

/// Build the package manifest that corresponds to the encoded graph package.
pub fn build_compute_graph_v1_package_manifest(
    manifest_blob_id: [u8; 32],
) -> ProgramPackageManifest {
    ProgramPackageManifest {
        program_id: ProgramId(PROGRAM_ID.to_string()),
        program_version: ProgramVersion(PROGRAM_VERSION.to_string()),
        manifest_blob_id,
        runtime_kind: ProgramRuntimeKind::ComputeGraphV1,
        entrypoint: GRAPH_ENTRYPOINT.to_string(),
        metadata: vec![
            ("package_kind".into(), "compute_graph_v1".into()),
            ("graph_abi_version".into(), GRAPH_ABI_VERSION.into()),
            ("graph_id".into(), hex::encode(GRAPH_ID_V1)),
        ],
    }
}

/// Required session input declaration for this graph.
///
/// The simulation uses this in `ComputeJobSpec.metadata`.
/// Ypolo reads it generically; Ypolo does not know what the names mean.
pub fn required_session_inputs_v1() -> String {
    [
        SESSION_INPUT_KIND_CONFIG_V1,
        SESSION_INPUT_KIND_CONVOY_V1,
        SESSION_INPUT_KIND_SATELLITE_V1,
    ]
    .join(",")
}

fn build_compute_graph_v1_ops() -> Vec<ComputeGraphOpV1> {
    vec![
        ComputeGraphOpV1::DecodeStructFields {
            input_slot: SLOT_CONFIG.to_string(),
            encoding: ENCODING_CONFIG_V1.to_string(),
            fields: vec![
                field(VAR_RADIUS_SQ, VAR_RADIUS_SQ, ComputeGraphFieldKindV1::I64),
                field(
                    VAR_MAX_SATS_SCAN,
                    VAR_MAX_SATS_SCAN,
                    ComputeGraphFieldKindV1::Usize,
                ),
            ],
        },
        ComputeGraphOpV1::DecodeStructFields {
            input_slot: SLOT_CONVOY_PAYLOAD.to_string(),
            encoding: ENCODING_ENCRYPTED_CONVOY_POINT_V1.to_string(),
            fields: vec![
                field("x_ct", VAR_CONVOY_X_CT_BYTES, ComputeGraphFieldKindV1::Bytes),
                field("y_ct", VAR_CONVOY_Y_CT_BYTES, ComputeGraphFieldKindV1::Bytes),
            ],
        },
        ComputeGraphOpV1::DeserializeFheInt64 {
            input_var: VAR_CONVOY_X_CT_BYTES.to_string(),
            output_var: VAR_CONVOY_X.to_string(),
        },
        ComputeGraphOpV1::DeserializeFheInt64 {
            input_var: VAR_CONVOY_Y_CT_BYTES.to_string(),
            output_var: VAR_CONVOY_Y.to_string(),
        },
        ComputeGraphOpV1::BindInputSlotBytes {
            input_slot: SLOT_SATELLITE_PAYLOAD.to_string(),
            output_var: VAR_SATELLITE_BATCH_BYTES.to_string(),
        },
        ComputeGraphOpV1::AnySquaredDistanceLeRadiusI64 {
            point_x: VAR_CONVOY_X.to_string(),
            point_y: VAR_CONVOY_Y.to_string(),
            candidate_batch: VAR_SATELLITE_BATCH_BYTES.to_string(),
            radius_sq_clear: VAR_RADIUS_SQ.to_string(),
            max_scan_clear: VAR_MAX_SATS_SCAN.to_string(),
            output: VAR_EXPOSED_CT.to_string(),
        },
        ComputeGraphOpV1::SerializeFheBool {
            input_var: VAR_EXPOSED_CT.to_string(),
            output_var: VAR_EXPOSED_CT_BYTES.to_string(),
        },
        ComputeGraphOpV1::EncodeStructFields {
            output_slot: SLOT_ALERT_OUTPUT.to_string(),
            encoding: ENCODING_ALERT_OUTPUT_V1.to_string(),
            fields: vec![encoded_field(
                "exposed_ct",
                VAR_EXPOSED_CT_BYTES,
                ComputeGraphFieldKindV1::Bytes,
            )],
        },
    ]
}

fn field(
    field_name: impl Into<String>,
    output_var: impl Into<String>,
    field_kind: ComputeGraphFieldKindV1,
) -> compute_abi::ComputeGraphDecodedFieldV1 {
    compute_abi::ComputeGraphDecodedFieldV1 {
        field_name: field_name.into(),
        output_var: output_var.into(),
        field_kind,
    }
}

fn encoded_field(
    field_name: impl Into<String>,
    input_var: impl Into<String>,
    field_kind: ComputeGraphFieldKindV1,
) -> compute_abi::ComputeGraphEncodedFieldV1 {
    compute_abi::ComputeGraphEncodedFieldV1 {
        field_name: field_name.into(),
        input_var: input_var.into(),
        field_kind,
    }
}