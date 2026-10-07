use anyhow::{anyhow, Result};
use compute_abi::{ProgramId, ProgramManifest, ProgramRuntimeKind, ProgramVersion};
use compute_core::{ComputeOutput, ExecutorInputs};
use std::collections::HashMap;
use std::sync::Arc;

/// Stable identity for a smart program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProgramDescriptor {
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,
}

/// Generic smart program trait.
///
/// Domain crates implement this trait and own:
/// - manifest
/// - typed ABI
/// - execution kernel
pub trait SmartProgram: Send + Sync + 'static {
    fn descriptor(&self) -> ProgramDescriptor;
    fn manifest(&self) -> ProgramManifest;
    fn execute(&self, exec_inputs: ExecutorInputs) -> Result<ComputeOutput>; 

    /// optional hook for session oriented-programs.
    /// 
    /// Ypolo calls this when live session inputs arrive.
    /// Return:
    /// - `Ok(None)` if the program does not yet have enough inputs
    /// - `Ok(Some(inputs))` when the program can run
    fn try_build_session_inputs(
        &self,
        _session_inputs: &HashMap<String, Vec<u8>>,
    ) -> Result<Option<Vec<Vec<u8>>>> {
            Ok(None)
    }
}

pub fn matches_descriptor(
    descriptor: &ProgramDescriptor,
    program_id: &ProgramId,
    program_version: &ProgramVersion,
) -> bool {
    &descriptor.program_id == program_id && &descriptor.program_version == program_version
}

#[derive(Default, Clone)]
pub struct ProgramRegistry {
    programs: HashMap<ProgramDescriptor, Arc<dyn SmartProgram>>,
}

impl ProgramRegistry {
    pub fn new() -> Self {
        Self {
            programs: HashMap::new(),
        }
    }

    pub fn register<P>(&mut self, program: P)
    where
        P: SmartProgram,
    {
        let descriptor = program.descriptor();
        self.programs.insert(descriptor, Arc::new(program));
    }

    pub fn register_arc(&mut self, program: Arc<dyn SmartProgram>) {
        let descriptor = program.descriptor();
        self.programs.insert(descriptor, program);
    }

    pub fn get(
        &self,
        program_id: &ProgramId,
        program_version: &ProgramVersion,
    ) -> Option<Arc<dyn SmartProgram>> {
        self.programs
            .get(&ProgramDescriptor {
                program_id: program_id.clone(),
                program_version: program_version.clone(),
            })
            .cloned()
    }

    pub fn has(
        &self,
        program_id: &ProgramId,
        program_version: &ProgramVersion,
    ) -> bool {
        self.programs.contains_key(&ProgramDescriptor {
            program_id: program_id.clone(),
            program_version: program_version.clone(),
        })
    }

    pub fn manifest(
        &self,
        program_id: &ProgramId,
        program_version: &ProgramVersion,
    ) -> Option<ProgramManifest> {
        self.get(program_id, program_version).map(|p| p.manifest())
    }

    pub fn manifests(&self) -> Vec<ProgramManifest> {
        self.programs.values().map(|p| p.manifest()).collect()
    }

    pub fn execute(&self, exec_inputs: ExecutorInputs) -> Result<ComputeOutput> {
        let program = self
            .get(&exec_inputs.spec.program_id, &exec_inputs.spec.program_version)
            .ok_or_else(|| {
                anyhow!(
                    "program not registered: {}@{}",
                    exec_inputs.spec.program_id.0,
                    exec_inputs.spec.program_version.0
                )
            })?;

        program.execute(exec_inputs)
    }
}

pub fn runtime_kind_is_supported(runtime_kind: &ProgramRuntimeKind) -> bool {
    matches!(
        runtime_kind,
            ProgramRuntimeKind::WasmModule
            | ProgramRuntimeKind::ComputeGraphV1
            | ProgramRuntimeKind::ExternalProcessV1
    )
}