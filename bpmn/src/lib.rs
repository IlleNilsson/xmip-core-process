#![forbid(unsafe_code)]

//! BPMN 2.0, the default Work Process design format (ADR-0066, amendment
//! 2026-10-22).
//!
//! The owner, 2026-10-22: *there are BPEL solutions out there, no need to
//! invent the wheel unless we have to*, then *go with the best
//! functionality agreeing with Xmip licensing*; and of the choice, *It can
//! not be the only choice, just the default one.* BPMN 2.0 is OMG's
//! standard, whose license lets anyone build software on it, and its XML
//! with BPMN DI is what every BPMN tool opens: a design drawn in Xmip's
//! VS Code designer is a standard `.bpmn` file, and one drawn elsewhere
//! compiles here.
//!
//! [`Bpmn`] reads a design ([`document`]), resolves what it names of other
//! documents on this machine alone ([`resources`], [`schema`]), and
//! compiles its one executable process to the Work Process program
//! ([`compile`]): what it does not compile is refused in words
//! ([`refused`]), and what Xmip adds is in BPMN's extension elements
//! ([`extension`]). Nothing here opens a connection.

pub mod compile;
pub mod diagram;
pub mod document;
pub mod edit;
pub mod extension;
mod leads;
pub mod refused;
pub mod resources;
pub mod schema;
pub mod splice;
pub mod timer;

use process::{DesignFormat, LocalResources, Program, Refusal};

/// The BPMN 2.0 design format.
pub struct Bpmn;

/// The word a design names this format by.
pub const FORMAT: &str = "bpmn";

impl DesignFormat for Bpmn {
    fn format(&self) -> &'static str {
        FORMAT
    }

    fn compile(&self, design: &str, local: &dyn LocalResources) -> Result<Program, Vec<Refusal>> {
        compile::compile(design, local)
    }
}
