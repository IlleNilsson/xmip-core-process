//! The BPMN 2.0 XML Schemas, shipped inside Xmip and resolved here alone
//! (the owner, 2026-10-22: *We can't be dependent on resources online*).
//!
//! The five files of OMG's BPMN 2.0.2 publication, as published and
//! unmodified, in `schema/`: `BPMN20.xsd` and the `Semantic.xsd` it
//! includes, `BPMNDI.xsd`, and the Diagram Definition's `DC.xsd` and
//! `DI.xsd`. Each namespace below is a name a design binds a prefix to; a
//! `schemaLocation` given for one of them is answered by the file shipped
//! here, and the location itself is never read, local or not.

/// The BPMN 2.0 model.
pub const MODEL: &str = "http://www.omg.org/spec/BPMN/20100524/MODEL";
/// BPMN's diagram interchange.
pub const BPMN_DI: &str = "http://www.omg.org/spec/BPMN/20100524/DI";
/// The Diagram Definition's common elements.
pub const DC: &str = "http://www.omg.org/spec/DD/20100524/DC";
/// The Diagram Definition's diagram interchange.
pub const DI: &str = "http://www.omg.org/spec/DD/20100524/DI";
/// XML Schema instance: where `schemaLocation` is written.
pub const INSTANCE: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Each shipped schema: the namespace it defines, its file and its text.
pub const SHIPPED: [(&str, &str, &str); 5] = [
    (MODEL, "BPMN20.xsd", include_str!("../schema/BPMN20.xsd")),
    (
        MODEL,
        "Semantic.xsd",
        include_str!("../schema/Semantic.xsd"),
    ),
    (BPMN_DI, "BPMNDI.xsd", include_str!("../schema/BPMNDI.xsd")),
    (DC, "DC.xsd", include_str!("../schema/DC.xsd")),
    (DI, "DI.xsd", include_str!("../schema/DI.xsd")),
];

/// The shipped schema a design's `schemaLocation` for `namespace` is
/// answered by, or `None` where Xmip ships none for it.
#[must_use]
pub fn shipped(namespace: &str) -> Option<&'static str> {
    SHIPPED
        .iter()
        .find(|(space, _, _)| *space == namespace)
        .map(|(_, _, text)| *text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_schema_names_only_its_siblings() {
        for (_, file, text) in SHIPPED {
            for (at, _) in text.match_indices("schemaLocation=\"") {
                let rest = &text[at + "schemaLocation=\"".len()..];
                let location = &rest[..rest.find('"').expect("closed")];
                assert!(
                    SHIPPED.iter().any(|(_, sibling, _)| *sibling == location),
                    "{file} names {location}, which Xmip does not ship"
                );
            }
        }
        assert!(shipped(MODEL).is_some_and(|text| text.contains("tDefinitions")));
    }
}
