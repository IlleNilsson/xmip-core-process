#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;
use xmip_message::Message;

#[derive(Debug)]
pub struct ProcessError {
    pub retryable: bool,
    pub message: String,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.message) }
}
impl Error for ProcessError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessOutcome {
    Message(Message),
    NoMessage,
    Waiting(String),
}

pub trait XmipProcess: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn execute(&self, message: &Message) -> Result<ProcessOutcome, ProcessError>;
}

pub trait ProcessRegistry: Send + Sync {
    fn resolve(&self, name: &str, version: &str) -> Option<&dyn XmipProcess>;
}
