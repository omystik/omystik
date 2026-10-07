use anyhow::Result;
use compute_core::{ComputeOutput, ExecutorInputs};
use compute_program::ProgramRegistry;

pub fn dispatch_program(
    exec_inputs: ExecutorInputs,
    registry: &ProgramRegistry,
) -> Result<ComputeOutput> {
    registry.execute(exec_inputs)
}
