use cairo_vm_base::vm::cairo_vm::{
    air_public_input::PublicInputError,
    cairo_run::EncodeTraceError,
    types::errors::program_errors::ProgramError,
    vm::errors::{
        cairo_run_errors::CairoRunError, memory_errors::MemoryError, runner_errors::RunnerError,
        trace_errors::TraceError, vm_errors::VirtualMachineError,
    },
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JSON(#[from] serde_json::Error),
    #[error(transparent)]
    CairoRunError(#[from] CairoRunError),
    #[error(transparent)]
    EncodeTrace(#[from] EncodeTraceError),
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error("Runner Error: {0}")]
    Runner(#[from] RunnerError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    VirtualMachine(#[from] VirtualMachineError),
    #[error(transparent)]
    PublicInput(#[from] PublicInputError),
    #[error(transparent)]
    Program(#[from] ProgramError),
}
