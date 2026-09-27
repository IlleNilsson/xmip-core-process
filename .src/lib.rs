#![forbid(unsafe_code)]

use message::Message;
use xcore::{Failure, PartyId};

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessOutcome {
    Message(Message),
    NoMessage,
    Waiting(String),
}

pub trait XmipProcess: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    /// The Party this Process runs as.
    ///
    /// Consequential beyond the Process itself. ADR-0022 clause 3 gives a host
    /// process the work of exactly one identity context, so this decides which
    /// host process the Process can be placed in — and an estate with eight
    /// distinct identities runs at least eight host processes on any node
    /// serving all eight. That cost belongs in capacity planning rather than
    /// being discovered in production.
    ///
    /// `None` runs as the Host Service's own identity, which is the ordinary
    /// case and still an identity context like any other.
    fn runs_as(&self) -> Option<PartyId> {
        None
    }

    /// Run the Process on `message`.
    ///
    /// # Errors
    /// Why it did not run, and whether trying again could change that.
    fn execute(&self, message: &Message) -> Result<ProcessOutcome, Failure>;
}

pub trait ProcessRegistry: Send + Sync {
    fn resolve(&self, name: &str, version: &str) -> Option<&dyn XmipProcess>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Orders;

    impl XmipProcess for Orders {
        fn name(&self) -> &str {
            "orders"
        }

        fn version(&self) -> &str {
            "2"
        }

        fn execute(&self, _: &Message) -> Result<ProcessOutcome, Failure> {
            Ok(ProcessOutcome::NoMessage)
        }
    }

    struct Registry(Vec<Orders>);

    impl ProcessRegistry for Registry {
        fn resolve(&self, name: &str, version: &str) -> Option<&dyn XmipProcess> {
            self.0
                .iter()
                .find(|p| p.name() == name && p.version() == version)
                .map(|p| p as &dyn XmipProcess)
        }
    }

    #[test]
    fn a_registry_resolves_by_name_and_version() {
        let registry = Registry(vec![Orders]);
        let found = registry.resolve("orders", "2").expect("resolved");
        assert_eq!(found.name(), "orders");
        assert_eq!(
            found.runs_as(),
            None,
            "a process runs as the node unless it says otherwise"
        );
        assert!(registry.resolve("orders", "1").is_none());
    }

    #[test]
    fn an_outcome_and_an_error_carry_what_a_caller_decides_on() {
        assert_eq!(
            ProcessOutcome::Waiting("invoice".to_string()),
            ProcessOutcome::Waiting("invoice".to_string())
        );
        assert_ne!(
            ProcessOutcome::NoMessage,
            ProcessOutcome::Waiting(String::new())
        );
        assert!(Failure::retryable("later").retryable);
        assert!(!Failure::permanent("never").retryable);
    }
}
