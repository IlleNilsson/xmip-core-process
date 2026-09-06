#![forbid(unsafe_code)]

use message::Message;
use xcore::PartyId;

xcore::declare_retryable_error!(ProcessError);

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

    fn execute(&self, message: &Message) -> Result<ProcessOutcome, ProcessError>;
}

pub trait ProcessRegistry: Send + Sync {
    fn resolve(&self, name: &str, version: &str) -> Option<&dyn XmipProcess>;
}
