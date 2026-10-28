//! Debatch, the process unit, as data (ADR-0066, amendment 2026-10-22): a
//! batch split into one Message per item, at receive, streamed, and in an
//! Xmip Work Process, through one splitting implementation (the owner,
//! 2026-10-22). Declared, not built (open problem 47), and its settings not
//! chosen, so it declares none. Batching, correlating and waiting are a
//! Work Process's own acts, not units.

use node::Stage;
use node::unit::Declaration;
use xcore::settings::Settings;

/// Debatch: a batch split into one Message per item.
pub const DEBATCH: &Declaration = &Declaration {
    name: "Debatch",
    capability: "work-process",
    stages: &[Stage::Receive, Stage::Process],
    settings: &Settings::none("Debatch"),
};

/// Every process unit's declaration, in the order a person reads them.
pub const DECLARATIONS: [&Declaration; 1] = [DEBATCH];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_process_unit_is_declared_soundly() {
        for declaration in DECLARATIONS {
            assert!(
                declaration.problems().is_empty(),
                "{:?}",
                declaration.problems()
            );
        }
    }
}
