//! A Work Process design format: a technology behind one trait, reading a
//! design and compiling it to the one [`Program`] form (ADR-0066,
//! amendment 2026-10-22).
//!
//! The owner, 2026-10-22, of BPMN 2.0: *It can not be the only choice, just
//! the default one.* So a design names its format, BPMN 2.0 is assumed
//! where it names none ([`DEFAULT_FORMAT`]), and every format compiles to
//! the same Program; the engine never learns which one a unit came from.
//!
//! **Offline.** The owner, 2026-10-22: *We can't be dependent on resources
//! online*, and *We can only use standards, code and compiled units that
//! explicitly work offline!* A design is compiled from what is on this
//! machine: what it names of another document is read through
//! [`LocalResources`], which answers files beside the design and nothing
//! else, and a location naming a network resource is refused in words
//! ([`remote`]) before anything is asked. No format opens a connection.

use crate::program::Program;

/// The format a design is read as where it names none.
pub const DEFAULT_FORMAT: &str = "bpmn";

/// A design format: reads a design and compiles it.
pub trait DesignFormat: Send + Sync {
    /// The word a design names it by: `bpmn`.
    fn format(&self) -> &'static str;

    /// `design` compiled, reading what it names of other documents through
    /// `local`.
    ///
    /// # Errors
    /// Every [`Refusal`]: what the format does not support, what does not
    /// hold together, what names a resource that is not local.
    fn compile(&self, design: &str, local: &dyn LocalResources) -> Result<Program, Vec<Refusal>>;
}

/// What a design may read of other documents: files beside it, by a
/// location relative to it. Never the network.
pub trait LocalResources {
    /// The document at `location`, relative to the design.
    ///
    /// # Errors
    /// Why it is not there, in words.
    fn read(&self, location: &str) -> Result<String, String>;
}

/// No document beside the design: what a design compiled from text alone
/// reads.
pub struct NoResources;

impl LocalResources for NoResources {
    fn read(&self, location: &str) -> Result<String, String> {
        Err(format!("'{location}' is not given beside the design"))
    }
}

/// Why a design does not compile: where, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The element's identifier, or its name where it has none.
    pub at: String,
    pub why: String,
}

impl Refusal {
    #[must_use]
    pub fn new(at: impl Into<String>, why: impl Into<String>) -> Self {
        Self {
            at: at.into(),
            why: why.into(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.at, self.why)
    }
}

/// Whether `location` names a resource on a network: anything with a
/// scheme — `https:`, `ftp:`, `urn:` — or a host (`//host/…`). A namespace
/// is a name and is never asked for; a location that is one of these is
/// refused, and nothing is fetched to decide it. A Windows drive letter
/// (`C:\…`) is local.
#[must_use]
pub fn remote(location: &str) -> bool {
    let location = location.trim();
    if location.starts_with("//") || location.starts_with("\\\\") {
        return true;
    }
    let Some((scheme, rest)) = location.split_once(':') else {
        return false;
    };
    let drive = scheme.len() == 1 && scheme.chars().all(|c| c.is_ascii_alphabetic());
    let named = !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if scheme.eq_ignore_ascii_case("file") {
        // `file:///path` is this machine's; `file://host/path` is not.
        return rest.starts_with("//") && !rest.starts_with("///");
    }
    named && !drive
}

/// The format of `formats` a design names, `named`, or the default where
/// it names none.
///
/// # Errors
/// No format of that word is carried, named with every one that is.
pub fn chosen<'a>(
    formats: &[&'a dyn DesignFormat],
    named: Option<&str>,
) -> Result<&'a dyn DesignFormat, String> {
    let word = named.unwrap_or(DEFAULT_FORMAT);
    formats
        .iter()
        .find(|format| format.format().eq_ignore_ascii_case(word))
        .copied()
        .ok_or_else(|| {
            let known: Vec<&str> = formats.iter().map(|format| format.format()).collect();
            format!(
                "no Work Process design format '{word}' is carried; there is {}",
                known.join(", ")
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_location_on_a_network_is_remote_and_a_file_beside_the_design_is_not() {
        for location in [
            "https://www.omg.org/spec/BPMN/20100501/BPMN20.xsd",
            "http://example.org/orders.bpmn",
            "ftp://partner-x/process.bpmn",
            "urn:example:process",
            "//server/share/process.bpmn",
            "\\\\server\\share\\process.bpmn",
            "file://server/share/process.bpmn",
        ] {
            assert!(remote(location), "{location}");
        }
        for location in [
            "orders.bpmn",
            "schema/BPMN20.xsd",
            "C:\\designs\\a.bpmn",
            "file:///C:/designs/a.bpmn",
        ] {
            assert!(!remote(location), "{location}");
        }
    }
}
