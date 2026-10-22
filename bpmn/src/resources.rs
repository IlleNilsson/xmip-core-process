//! What a design names of other documents, resolved on this machine or
//! refused (the owner, 2026-10-22: *We can't be dependent on resources
//! online, check that out!*).
//!
//! - A `schemaLocation` for a namespace Xmip ships a schema of is answered
//!   by that schema ([`crate::schema`]), and its location is never read.
//! - Any other location, in a `schemaLocation` or a `bpmn:import`, that
//!   names a network resource is refused in words, before anything is
//!   asked of anyone ([`process::remote`]).
//! - A local one is read through [`process::LocalResources`], which answers
//!   files beside the design; a BPMN document imported for its definitions
//!   is refused in this version, as one design is one document.

use process::{LocalResources, Refusal, remote};

use crate::document::Element;
use crate::schema::{INSTANCE, MODEL, shipped};

/// What imports BPMN definitions, as `importType` names it.
const BPMN_IMPORT: &str = "http://www.omg.org/spec/BPMN/20100524/MODEL";

/// Every refusal of what `definitions` names of other documents.
pub fn refusals(definitions: &Element, local: &dyn LocalResources) -> Vec<Refusal> {
    let mut refusals = Vec::new();
    if let Some(locations) = definitions.attribute_in(INSTANCE, "schemaLocation") {
        let words: Vec<&str> = locations.split_whitespace().collect();
        for pair in words.chunks(2) {
            let [namespace, location] = pair else {
                refusals.push(Refusal::new(
                    definitions.label(),
                    "its schemaLocation pairs a namespace with no location",
                ));
                continue;
            };
            if shipped(namespace).is_some() {
                continue;
            }
            refusals.extend(located(definitions, location, local, "a schema"));
        }
    }
    for import in definitions.children_named(MODEL, "import") {
        let location = import.attribute("location").unwrap_or_default();
        let kind = import.attribute("importType").unwrap_or_default();
        if let Some(refused) = located(import, location, local, "a document")
            .into_iter()
            .next()
        {
            refusals.push(refused);
        } else if kind == BPMN_IMPORT {
            refusals.push(Refusal::new(
                import.label(),
                format!(
                    "imports the definitions of '{location}'; one design is one document \
                     in this version"
                ),
            ));
        }
    }
    refusals
}

/// The refusal of `location`, where it is remote or not beside the design.
fn located(
    at: &Element,
    location: &str,
    local: &dyn LocalResources,
    what: &str,
) -> Option<Refusal> {
    if location.trim().is_empty() {
        return Some(Refusal::new(
            at.label(),
            format!("names {what} with no location"),
        ));
    }
    if remote(location) {
        return Some(Refusal::new(
            at.label(),
            format!(
                "names {what} on a network, '{location}'; a design is compiled from what is \
                 on this machine and Xmip never fetches one"
            ),
        ));
    }
    local
        .read(location)
        .err()
        .map(|why| Refusal::new(at.label(), format!("names {what} '{location}': {why}")))
}
