use anyhow::{anyhow, Context, Result};
use compute_abi::{ExternalProcessInvocationV1, ExternalProcessPackageV1,
    ExternalProcessSessionInputV1, ExternalProcessArtifactInputV1, ExternalProcessResponseV1,
    ProgramRuntimeKind};
use compute_core::{ComputeContext, ComputeExecutor, ComputeOutput, ExecutorInputs};
use mesh_gateway_wire::{decode, encode, JobStore};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ExternalProcessExecutor {
    store: JobStore,
    server_key_path: PathBuf,
    work_root: PathBuf,
}

impl ExternalProcessExecutor {
    pub fn new(
        store: JobStore,
        server_key_path: PathBuf,
        work_root: PathBuf,
    ) -> Self {
        Self {
            store,
            server_key_path,
            work_root,
        }
    }
}

#[async_trait::async_trait]
impl ComputeExecutor for ExternalProcessExecutor {
    async fn execute(
        &self,
        ctx: ComputeContext,
        exec_inputs: ExecutorInputs,
    ) -> Result<ComputeOutput> {
        let installed = self
            .store
            .get_installed_program(&exec_inputs.spec.program_id, &exec_inputs.spec.program_version)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "program is not installed: {}@{}",
                    exec_inputs.spec.program_id.0,
                    exec_inputs.spec.program_version.0
                )
            })?;

        if installed.runtime_kind != ProgramRuntimeKind::ExternalProcessV1 {
            return Err(anyhow!(
                "ExternalProcessExecutor cannot execute runtime kind {:?}",
                installed.runtime_kind
            ));
        }

        let package_bytes = self
            .store
            .read_blob(&installed.package_blob_id)
            .await
            .with_context(|| {
                format!(
                    "failed to read installed package blob {}",
                    hex::encode(installed.package_blob_id)
                )
            })?;

        let package: ExternalProcessPackageV1 = decode(&package_bytes)
            .map_err(|e| anyhow!("decode ExternalProcessPackageV1 failed: {e}"))?;

        validate_package_command(&package.command)?;

        let invocation_dir = self
            .work_root
            .join(hex::encode(ctx.job_id));

        if invocation_dir.exists() {
            tokio::fs::remove_dir_all(&invocation_dir).await.ok();
        }

        tokio::fs::create_dir_all(&invocation_dir)
            .await
            .with_context(|| format!("create invocation dir {}", invocation_dir.display()))?;

        let request_path = invocation_dir.join("request.bin");
        let response_path = invocation_dir.join("response.bin");

        let request = ExternalProcessInvocationV1 {
            job_id: ctx.job_id,
            program_id: exec_inputs.spec.program_id.0.clone(),
            program_version: exec_inputs.spec.program_version.0.clone(),
            server_key_path: self.server_key_path.clone(),
            inputs: exec_inputs.inputs,
            session_inputs: exec_inputs
                .session_inputs
                .into_iter()
                .map(|i| ExternalProcessSessionInputV1 {
                    input_kind: i.input_kind,
                    bytes: i.bytes,
                    observed_unix_ms: i.observed_unix_ms,
                    sequence: i.sequence,
                })
                .collect(),
            artifacts: exec_inputs
                .artifacts
                .into_iter()
                .map(|a| ExternalProcessArtifactInputV1 {
                    artifact_id: a.artifact_id,
                    artifact_kind: a.artifact_kind,
                    bytes: a.bytes,
                })
                .collect(),
            metadata: exec_inputs.spec.metadata,
        };

        tokio::fs::write(&request_path, encode(&request)?)
            .await
            .with_context(|| format!("write {}", request_path.display()))?;

        let mut command = Command::new(&package.command);
        command
            .arg("--request")
            .arg(&request_path)
            .arg("--response")
            .arg(&response_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in package.env {
            command.env(k, v);
        }

        let output = command
            .output()
            .await
            .with_context(|| {
                format!(
                    "spawn external program runner {}",
                    package.command.display()
                )
            })?;

        if !output.status.success() {
            return Err(anyhow!(
                "external program runner failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let response_bytes = tokio::fs::read(&response_path)
            .await
            .with_context(|| format!("read {}", response_path.display()))?;

        let response: ExternalProcessResponseV1 = decode(&response_bytes)
            .map_err(|e| anyhow!("decode ExternalProcessResponseV1 failed: {e}"))?;

        if response.outputs.is_empty() {
            return Err(anyhow!("external runner returned no outputs"));
        }

        Ok(ComputeOutput {
            outputs: response.outputs,
        })
    }
}

fn validate_package_command(command: &Path) -> Result<()> {
    if command.as_os_str().is_empty() {
        return Err(anyhow!("external package command path is empty"));
    }

    if !command.exists() {
        return Err(anyhow!(
            "external package command does not exist: {}",
            command.display()
        ));
    }

    let meta = fs::metadata(command)
        .with_context(|| format!("stat external package command {}", command.display()))?;

    if !meta.is_file() {
        return Err(anyhow!(
            "external package command is not a file: {}",
            command.display()
        ));
    }

    Ok(())
}