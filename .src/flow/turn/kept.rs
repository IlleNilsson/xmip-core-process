//! What a turn's decisions and the values it keeps read, and a
//! [`Step::Keep`](crate::program::Step::Keep) taken (ADR-0066, amendment
//! 2026-11-25; open problem 191).
//!
//! A name the design declares as a value of the Instance's own is read from
//! the values the Instance keeps across every Message; any other from the
//! current Message. So a decision reads `Accepted >= Ordered` over values
//! counted Message by Message, where until this amendment each Message a
//! wait took replaced everything its decisions read.

use expression::Names;

use super::{NO_VALUE, Turn};
use crate::instance::Instance;
use crate::program::{Keep, Program};

/// What an Instance's decisions and values read: its own values by the
/// names its program declares, the current Message's by any other.
pub(super) struct Reading<'a> {
    pub(super) program: &'a Program,
    pub(super) instance: &'a Instance,
}

impl Names for Reading<'_> {
    fn value(&self, name: &str) -> Option<&str> {
        if self.program.keeps(name) {
            self.instance.value(name)
        } else {
            self.instance.read_value(name)
        }
    }

    fn absent(&self, name: &str) -> String {
        if self.program.keeps(name) {
            format!("the Instance keeps no {name} yet")
        } else {
            format!("the current Message carries no {name}")
        }
    }
}

impl Turn<'_> {
    /// The values `keeps` sets at `node`, in order, each computed over the
    /// Instance as those before it left it, and the flow at `index` gone
    /// on to `next`; a value that cannot be computed throws [`NO_VALUE`]
    /// with why, for the scopes to catch.
    pub(super) fn keep(&mut self, (index, node): (usize, u32), keeps: &[Keep], next: u32) {
        let program = self.program;
        let computations = self.computations;
        let compiled = &computations[usize::try_from(node).unwrap_or(usize::MAX)];
        for (keep, computed) in keeps.iter().zip(compiled) {
            let name = program
                .values
                .get(usize::try_from(keep.value).unwrap_or(usize::MAX))
                .map_or("?", String::as_str);
            let reading = Reading {
                program,
                instance: &self.state,
            };
            match computed.compute(&reading) {
                Ok(value) => self.state.keep(name, value),
                Err(why) => {
                    let reason = format!("'{}' cannot keep {name}: {why}", self.id(node));
                    self.throw(node, NO_VALUE, &reason);
                    return;
                }
            }
        }
        self.go(index, next, self.now);
    }
}
