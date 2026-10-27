//! A Process Instance's state: everything needed to go on after a wait, a
//! timeout or the loss of the node that ran it (`runtime-model.md` section
//! 22, *Process State belongs to the cluster*). It lives in the Ledger, in
//! its binary form ([`crate::form`]), and is read and written whole at
//! every turn; nothing of it is kept in a thread.

/// One Instance of a Work Process.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instance {
    /// Its key in each Correlation it has taken a Message by, by the
    /// set's number: what a later Message of the set must carry to be
    /// handed to it.
    pub keys: Vec<(u32, String)>,
    /// Where its flows stand.
    pub tokens: Vec<Token>,
    /// How many flows each Join has received so far.
    pub joins: Vec<(u32, u32)>,
    /// What it waits for in time.
    pub timers: Vec<Timer>,
    /// The Messages a Batch has collected, in the order received.
    pub batch: Vec<u128>,
    /// The Message it works on now, and the values read from it.
    pub message: Option<u128>,
    pub values: Vec<(String, String)>,
    /// How it ended, once it has.
    pub ended: Option<Outcome>,
    /// The next timer's number.
    pub next_timer: u64,
    /// How many turns it has taken: what a write of it is checked against,
    /// so two turns never both write from the same state.
    pub turns: u64,
}

/// Where one flow stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub at: u32,
    pub standing: Standing,
}

/// What a flow waits for, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Ready to go on at once.
    Ready,
    /// For the next Message of its key.
    Message,
    /// For a timer it started.
    Timer,
    /// For the work it asked to be done.
    Work,
}

/// A timer: who it belongs to and when it is due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timer {
    pub id: u64,
    pub owner: Owner,
    pub due_unix_nanos: i128,
}

/// What a timer interrupts or wakes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// A Timer or a Batch step.
    Node(u32),
    /// A scope's boundary timer.
    Scope(u32),
}

/// How an Instance ended (`runtime-model.md` section 22, *Process
/// outcome*).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    /// An error no scope caught: its code and why.
    Failed {
        code: String,
        reason: String,
    },
}

impl Instance {
    /// A new Instance, not started.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Its key in the set numbered `set`, where it has taken one.
    #[must_use]
    pub fn key(&self, set: u32) -> Option<&str> {
        self.keys
            .iter()
            .find(|(known, _)| *known == set)
            .map(|(_, key)| key.as_str())
    }

    /// Whether it has not started yet: no turn taken.
    #[must_use]
    pub const fn is_new(&self) -> bool {
        self.turns == 0
    }

    /// When it is next due, where a timer waits.
    #[must_use]
    pub fn next_due(&self) -> Option<i128> {
        self.timers.iter().map(|timer| timer.due_unix_nanos).min()
    }

    /// Whether a flow waits for the next Message of its key.
    #[must_use]
    pub fn waits_for_a_message(&self) -> bool {
        self.tokens
            .iter()
            .any(|token| token.standing == Standing::Message)
    }

    /// The value `name` read from the current Message.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, value)| value.as_str())
    }
}

impl path::expression::Names for Instance {
    fn value(&self, name: &str) -> Option<&str> {
        Instance::value(self, name)
    }

    fn absent(&self, name: &str) -> String {
        format!("the current Message carries no {name}")
    }
}
