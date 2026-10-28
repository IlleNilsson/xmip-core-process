// deny, not forbid: `export` stands on the far side of the C boundary and
// allows it there alone, each block with a SAFETY comment.
#![deny(unsafe_code)]

//! Xmip Work Processes: designed in VS Code, compiled at design time,
//! executed when called upon (ADR-0066, amendment 2026-10-22).
//!
//! The owner, 2026-10-22: *I would like a Work Process to be designed in VS
//! Code, but at the end of the day it is a compiled piece executed when
//! called upon.* So:
//!
//! - [`design`]: a design format is a technology behind [`DesignFormat`],
//!   reading a design and compiling it to the one [`Program`] form; BPMN
//!   2.0 is the default (`xmip-core-process-bpmn`, mounted beside this
//!   source).
//! - [`program`]: the compiled form, a graph of steps a design becomes.
//! - [`generate`]: the Work Process unit a program is built into, a Rust
//!   crate exporting it through the C ABI ([`export`], `xmip_module.h`
//!   section 13), and the module entry naming it.
//! - [`flow`]: what a unit is to the runtime, a [`Flow`] advanced one turn
//!   at a time over an [`Instance`] the Ledger keeps; [`Compiled`] runs a
//!   program, and a unit written by hand implements [`Flow`] itself.
//! - [`form`]: the binary forms a program, an Instance and a turn cross in.
//!
//! The engine runs the compiled form only and never knows which format a
//! program came from; the runtime (`xmip-core-runtime`) carries out the
//! acts a turn answers and keeps the Instance in the Ledger, so a waiting
//! Instance holds no thread (`runtime-model.md` section 22).

pub mod declaration;
pub mod design;
pub mod export;
pub mod flow;
pub mod form;
pub mod generate;
pub mod instance;
pub mod program;

pub use declaration::DECLARATIONS;
pub use design::{DEFAULT_FORMAT, DesignFormat, LocalResources, NoResources, Refusal, remote};
pub use flow::{Act, Arrival, Compiled, Declared, Event, Flow, Refused};
pub use instance::{Instance, Outcome};
pub use program::{Program, Step};
