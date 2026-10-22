//! What a Work Process unit is to the runtime: a [`Flow`], advanced one
//! turn at a time (ADR-0066, amendment 2026-10-22).
//!
//! A turn takes the Instance as the Ledger holds it and one [`Event`] — a
//! Message of its key arrived, work it asked for done or failed, a time
//! due — and answers the [`Act`]s the runtime carries out: work to do on a
//! Message, a Message to publish, when it is next due, how it ended. The
//! runtime writes the Instance back with all of it in one hand-on; between
//! turns nothing holds a thread (`runtime-model.md` section 22).
//!
//! A unit compiled from a design is a [`Compiled`] program; a unit written
//! by hand implements [`Flow`] itself. Either is opened as
//! `node::unit::Opener<dyn Flow>`, linked or loaded through the C ABI
//! ([`crate::export`]), and the runtime never knows which, nor which design
//! format a program was drawn in.

#[cfg(test)]
mod tests;
mod turn;

pub use turn::{LOOPS, NO_BRANCH};

use path::expression::Expression;

use crate::instance::{Instance, Outcome};
use crate::program::{Program, Step, Work};

/// A Work Process unit, advanced a turn at a time.
pub trait Flow: Send + Sync {
    /// What the runtime reads before a turn: its name, the correlation
    /// sets it follows and every name its decisions read.
    fn declared(&self) -> &Declared;

    /// One turn of `instance` on `event`, at `now_unix_nanos`.
    ///
    /// # Errors
    /// The event is not one the Instance can take now ([`Refused`]); the
    /// Instance is left as it was.
    fn advance(
        &self,
        instance: &mut Instance,
        event: &Event,
        now_unix_nanos: i128,
    ) -> Result<Vec<Act>, Refused>;
}

/// What a [`Flow`] declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    pub name: String,
    /// The correlation sets it follows, by name, in the order of the
    /// numbers its steps give them.
    pub sets: Vec<String>,
    /// Every name read of a Message before it is handed on.
    pub reads: Vec<String>,
}

/// What happens to an Instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A Message of its key: its first, which starts it, or one a step
    /// waits for.
    Arrived(Arrival),
    /// The work a step asked for is done: what it made, the Message the
    /// flow goes on with first, a split's parts all of them.
    Done { node: u32, made: Vec<Arrival> },
    /// The work failed: thrown as an error of `code`.
    Threw {
        node: u32,
        code: String,
        reason: String,
    },
    /// Its time came: every timer due by now fires.
    Due,
}

/// A Message as a turn sees it: its identifier, the values read of it, and
/// its key in each correlation set it has every key part of, by the set's
/// number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrival {
    pub message: u128,
    pub values: Vec<(String, String)>,
    pub keys: Vec<(u32, String)>,
}

/// What the runtime carries out after a turn.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// Do `work` on `message`, and answer [`Event::Done`] or
    /// [`Event::Threw`] for `node`.
    Work {
        node: u32,
        work: Work,
        message: u128,
    },
    /// Make one Message of `messages`, their Sections in order, and answer
    /// [`Event::Done`] for `node`.
    Combine { node: u32, messages: Vec<u128> },
    /// Publish `message` back through routing.
    Publish { message: u128 },
    /// The Instance ended.
    Ended(Outcome),
}

/// Why a turn did not take its event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A Message came and no step waits for one: it waits its turn in the
    /// Ledger, before every later Message of its key.
    NotWaiting,
    /// The Instance has ended.
    Ended,
    /// The event does not fit the Instance: said in words.
    Unfit(String),
}

/// A unit compiled from a design: its program, its conditions compiled
/// once as it opens.
pub struct Compiled {
    program: Program,
    declared: Declared,
    conditions: Vec<Vec<Expression>>,
}

impl Compiled {
    /// `program` checked and its conditions compiled.
    ///
    /// # Errors
    /// Every problem [`Program::check`] finds.
    pub fn new(program: Program) -> Result<Self, Vec<String>> {
        program.check()?;
        let mut conditions = Vec::with_capacity(program.nodes.len());
        for node in &program.nodes {
            let compiled = match &node.step {
                Step::Decide { branches, .. } => branches
                    .iter()
                    .map(|branch| Expression::parse(&branch.condition))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|why| vec![format!("'{}': {why}", node.id)])?,
                _ => Vec::new(),
            };
            conditions.push(compiled);
        }
        let declared = Declared {
            name: program.name.clone(),
            sets: program.sets.clone(),
            reads: program.reads(),
        };
        Ok(Self {
            program,
            declared,
            conditions,
        })
    }

    /// Its program.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }
}

impl Flow for Compiled {
    fn declared(&self) -> &Declared {
        &self.declared
    }

    fn advance(
        &self,
        instance: &mut Instance,
        event: &Event,
        now_unix_nanos: i128,
    ) -> Result<Vec<Act>, Refused> {
        if instance.ended.is_some() {
            return Err(Refused::Ended);
        }
        let state = instance.clone();
        let mut turn = turn::Turn::new(&self.program, &self.conditions, state, now_unix_nanos);
        turn.take(event, now_unix_nanos)?;
        turn.run(now_unix_nanos);
        let (state, acts) = turn.finish();
        *instance = state;
        instance.turns += 1;
        Ok(acts)
    }
}
