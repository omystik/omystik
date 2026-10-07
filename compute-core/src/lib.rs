use anyhow::Result;
use compute_abi::ComputeJobSpec;

pub struct ComputeContext {
    pub job_id: [u8; 32],
}

pub struct ComputeArtifactInput {
    pub artifact_id: [u8; 32],
    pub artifact_kind: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ComputeSessionInput {
    pub input_kind: String,
    pub bytes: Vec<u8>,
    pub observed_unix_ms: u64,
    pub sequence: u64,
}

pub struct ComputeOutput {
    pub outputs: Vec<Vec<u8>>,
}

pub struct ExecutorInputs {
    pub spec: ComputeJobSpec,
    pub inputs: Vec<Vec<u8>>,
    pub session_inputs: Vec<ComputeSessionInput>,
    pub artifacts: Vec<ComputeArtifactInput>,
}

#[async_trait::async_trait]
pub trait ComputeExecutor: Send + Sync {
    async fn execute(
        &self,
        ctx: ComputeContext,
        exe_inputs: ExecutorInputs,
    ) -> Result<ComputeOutput>;
}