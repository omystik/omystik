use anyhow::Result;
use compute_abi::{ExternalProcessPackageV1, ProgramRuntimeKind};
use compute_client::deploy::{deploy_program, DeployedProgram};
use libp2p::PeerId;
use libp2p_common::komvos::Client as FaceAClient;
use std::path::PathBuf;

use crate::computation::manifest_v1;
use mesh_gateway_wire::encode;

/// Deploy satcon.alert_math@0.1.0 as an ExternalProcessV1 package.
///
/// Ypolo remains agnostic:
/// - it receives manifest bytes
/// - it receives package bytes
/// - it stores InstalledProgramRecord
/// - it launches the external runner command at execution time
pub async fn deploy_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    runner_command: PathBuf,
    metadata: Vec<(String, String)>,
) -> Result<DeployedProgram> {
    let package = ExternalProcessPackageV1 {
        command: runner_command,
        env: vec![],
        metadata: vec![
            ("package_kind".into(), "external-process-v1".into()),
            ("program_id".into(), crate::computation::PROGRAM_ID.into()),
            ("program_version".into(), crate::computation::PROGRAM_VERSION.into()),
        ],
    };

    let package_bytes = encode(&package)?;

    deploy_program(
        net_client,
        ypolo_peer,
        manifest_v1(),
        package_bytes,
        ProgramRuntimeKind::ExternalProcessV1,
        metadata,
    )
    .await
}

// WARNING,  at dev PathBuf::from("./target/debug/smart-program-alert")
// VERSUS
// at production PathBuf::from("/opt/osatcon/bin/smart-program-alert")