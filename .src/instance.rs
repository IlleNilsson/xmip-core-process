//! A Process Instance's state: everything needed to go on after a wait, a
//! timeout or the loss of the node that ran it (`runtime-model.md` section
//! 22, *Process State belongs to the cluster*). It lives in the Ledger, in
//! its binary form ([`crate::form`]), and is read and written whole at
//! every turn; nothing of it is kept in a thread.
//!
//! **Its own values** are kept across every Message it takes (ADR-0066,
//! amendment 2026-11-25; open problem 191): what its design declares and
//! sets, an order's lines counted against those delivered and accepted, as
//! an orchestration keeps its variables. The values read from the current
//! Message are apart from them, replaced by each Message it takes.
//!
//! **Its history of Messages** is kept with it, the newest [`MOST_HISTORY`]
//! of them, each the Message a turn took, made or published, and the
//! Journey that carried it (open problem 192): the runtime, which carries
//! the turn out, writes it, so a unit written by hand has one too.

/// The most entries of its history an Instance keeps: past it the oldest
/// is let go of and counted ([`Instance::earlier`]).
pub const MOST_HISTORY: usize = 64;

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
    pub read: Vec<(String, String)>,
    /// Its own values, by name, kept across every Message it takes.
    pub values: Vec<(String, String)>,
    /// How it ended, once it has.
    pub ended: Option<Outcome>,
    /// The next timer's number.
    pub next_timer: u64,
    /// How many turns it has taken: what a write of it is checked against,
    /// so two turns never both write from the same state.
    pub turns: u64,
    /// The Messages it handled, oldest first, the newest [`MOST_HISTORY`].
    pub history: Vec<Handled>,
    /// How many entries of its history came before those kept.
    pub earlier: u64,
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

/// One entry of an Instance's history: when, what happened to which
/// Message, and the Journey that carried it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handled {
    pub at_unix_nanos: i128,
    pub what: Handling,
    pub message: u128,
    pub journey: u128,
}

/// What happened to a Message an Instance handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handling {
    /// Its first: it started the Instance.
    Started,
    /// A Message of its key, taken by a step waiting for one.
    Arrived,
    /// Its time came, on the Journey that waits for it.
    Due,
    /// Made by work it asked for: an Assignment, a unit, a Batch, a split.
    Made,
    /// Published back through routing.
    Published,
    /// Its turn threw an error no scope caught: the Instance failed.
    Failed,
    /// Retried by an operator: its turn taken again.
    Retried,
    /// Dismissed by an operator: the Instance it failed ended.
    Dismissed,
}

/// Every [`Handling`], in the order of their numbers.
pub const HANDLINGS: [Handling; 8] = [
    Handling::Started,
    Handling::Arrived,
    Handling::Due,
    Handling::Made,
    Handling::Published,
    Handling::Failed,
    Handling::Retried,
    Handling::Dismissed,
];

impl Handling {
    /// How a surface says it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Arrived => "arrived",
            Self::Due => "due",
            Self::Made => "made",
            Self::Published => "published",
            Self::Failed => "failed",
            Self::Retried => "retried",
            Self::Dismissed => "dismissed",
        }
    }
}

/// Where an Instance stands, as a surface says it (open problem 192).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Its flows wait for neither a Message nor a time: a turn under way.
    Running,
    /// Waiting: for a Message of its key where `message`, until `until`
    /// where a timer waits — its earliest due time.
    Waiting { message: bool, until: Option<i128> },
    /// An error no scope caught, waiting for an operator: its code and why.
    Failed { code: String, reason: String },
    /// Ended, every flow done.
    Completed,
}

impl State {
    /// How a surface says it: `running`, `waiting`, `failed` or `completed`.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Waiting { .. } => "waiting",
            Self::Failed { .. } => "failed",
            Self::Completed => "completed",
        }
    }
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
    pub fn read_value(&self, name: &str) -> Option<&str> {
        named(&self.read, name)
    }

    /// Its own value `name`, where it keeps one.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        named(&self.values, name)
    }

    /// Its own value `name` set to `value`, kept where it was or after the
    /// rest.
    pub fn keep(&mut self, name: &str, value: String) {
        match self.values.iter_mut().find(|(known, _)| known == name) {
            Some((_, kept)) => *kept = value,
            None => self.values.push((name.to_string(), value)),
        }
    }

    /// Where it stands.
    #[must_use]
    pub fn state(&self) -> State {
        match &self.ended {
            Some(Outcome::Completed) => State::Completed,
            Some(Outcome::Failed { code, reason }) => State::Failed {
                code: code.clone(),
                reason: reason.clone(),
            },
            None => {
                let message = self.waits_for_a_message();
                let until = self.next_due();
                if message || until.is_some() {
                    State::Waiting { message, until }
                } else {
                    State::Running
                }
            }
        }
    }

    /// `entry` added to its history, the oldest let go of past
    /// [`MOST_HISTORY`] and counted.
    pub fn handled(&mut self, entry: Handled) {
        self.history.push(entry);
        if self.history.len() > MOST_HISTORY {
            let over = self.history.len() - MOST_HISTORY;
            self.history.drain(..over);
            self.earlier += u64::try_from(over).unwrap_or(u64::MAX);
        }
    }

    /// The Journey whose turn failed it, where it failed: what an
    /// operator's Retry or Dismiss of that Journey acts on.
    #[must_use]
    pub fn failed_by(&self) -> Option<u128> {
        if !matches!(self.ended, Some(Outcome::Failed { .. })) {
            return None;
        }
        self.history
            .iter()
            .rev()
            .find(|entry| entry.what == Handling::Failed)
            .map(|entry| entry.journey)
    }
}

fn named<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(known, _)| known == name)
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(what: Handling, journey: u128) -> Handled {
        Handled {
            at_unix_nanos: 0,
            what,
            message: journey,
            journey,
        }
    }

    #[test]
    fn its_history_keeps_the_newest_and_counts_what_it_let_go() {
        let mut instance = Instance::new();
        for journey in 0..(MOST_HISTORY as u128 + 3) {
            instance.handled(entry(Handling::Arrived, journey));
        }
        assert_eq!(instance.history.len(), MOST_HISTORY);
        assert_eq!(instance.earlier, 3);
        assert_eq!(instance.history[0].journey, 3, "the oldest let go of first");
    }

    #[test]
    fn it_says_where_it_stands_and_which_journey_failed_it() {
        let mut instance = Instance::new();
        assert_eq!(instance.state(), State::Running);
        instance.tokens.push(Token {
            at: 1,
            standing: Standing::Message,
        });
        instance.timers.push(Timer {
            id: 0,
            owner: Owner::Scope(0),
            due_unix_nanos: 9,
        });
        assert_eq!(
            instance.state(),
            State::Waiting {
                message: true,
                until: Some(9)
            }
        );
        instance.handled(entry(Handling::Failed, 4));
        assert_eq!(instance.failed_by(), None, "failed only once it ended so");
        instance.ended = Some(Outcome::Failed {
            code: "late".into(),
            reason: "overdue".into(),
        });
        assert_eq!(instance.state().word(), "failed");
        assert_eq!(instance.failed_by(), Some(4));
        instance.keep("Delivered", "2".into());
        instance.keep("Delivered", "3".into());
        assert_eq!(instance.value("Delivered"), Some("3"));
        assert_eq!(instance.values.len(), 1, "set again where it was");
    }
}
