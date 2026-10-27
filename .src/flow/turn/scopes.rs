//! A turn's scopes: an error thrown and caught by the innermost scope that
//! catches its code, a scope's flows interrupted, a flow gone on or added,
//! and the timers a scope or a step starts, fired once due.

use super::Turn;
use crate::instance::{Outcome, Owner, Standing, Timer, Token};
use crate::program::{Due, Program, Step};

impl Turn<'_> {
    /// Every timer due by `now` fired, earliest first.
    pub(super) fn due(&mut self, now: i128) {
        let mut due: Vec<Timer> = self
            .state
            .timers
            .iter()
            .copied()
            .filter(|timer| timer.due_unix_nanos <= now)
            .collect();
        due.sort_by_key(|timer| (timer.due_unix_nanos, timer.id));
        for timer in due {
            // An earlier timer's interruption may have taken it.
            if !self.state.timers.contains(&timer) {
                continue;
            }
            self.state.timers.retain(|kept| kept.id != timer.id);
            match timer.owner {
                Owner::Node(node) => self.woken(node, now),
                Owner::Scope(scope) => {
                    let Some(timeout) = self.program.scope(scope).and_then(|s| s.timeout) else {
                        continue;
                    };
                    self.interrupt(scope);
                    self.add(timeout.next, now);
                }
            }
        }
    }

    /// An error of `code` thrown at `node`: caught by the innermost scope
    /// catching it, every flow in that scope ended and one going on from
    /// its catch; or, caught by none, the Instance failed.
    pub(super) fn throw(&mut self, node: u32, code: &str, reason: &str) {
        for scope in self.program.scopes_of(node) {
            let Some(catches) = self.program.scope(scope).map(|s| &s.catches) else {
                continue;
            };
            let caught = catches
                .iter()
                .find(|catch| catch.code.as_deref().is_none_or(|caught| caught == code));
            if let Some(catch) = caught {
                let next = catch.next;
                self.interrupt(scope);
                self.add(next, self.now);
                return;
            }
        }
        self.state.tokens.clear();
        self.state.timers.clear();
        self.state.joins.clear();
        self.state.batch.clear();
        self.state.ended = Some(Outcome::Failed {
            code: code.to_string(),
            reason: reason.to_string(),
        });
    }

    /// Every flow standing in `scope` ended, with its timers and Joins.
    pub(super) fn interrupt(&mut self, scope: u32) {
        let program = self.program;
        let inside = |node: u32| program.scopes_of(node).contains(&scope);
        if self
            .state
            .tokens
            .iter()
            .any(|token| inside(token.at) && matches!(self.step(token.at), Step::Batch { .. }))
        {
            self.state.batch.clear();
        }
        self.state.tokens.retain(|token| !inside(token.at));
        self.state.joins.retain(|(join, _)| !inside(*join));
        self.state.timers.retain(|timer| match timer.owner {
            Owner::Node(node) => !inside(node),
            Owner::Scope(owner) => owner != scope && !nested(program, owner, scope),
        });
    }

    /// The flow at `index` gone on to `next`.
    pub(super) fn go(&mut self, index: usize, next: u32, now: i128) {
        self.state.tokens[index] = Token {
            at: next,
            standing: Standing::Ready,
        };
        self.entered(next, now);
    }

    /// A new flow at `at`.
    pub(super) fn add(&mut self, at: u32, now: i128) {
        self.state.tokens.push(Token {
            at,
            standing: Standing::Ready,
        });
        self.entered(at, now);
    }

    /// Each scope `at` stands in with a timer, its timer started, where it
    /// was not running already.
    pub(super) fn entered(&mut self, at: u32, now: i128) {
        for scope in self.program.scopes_of(at) {
            if let Some(timeout) = self.program.scope(scope).and_then(|s| s.timeout) {
                self.timer(Owner::Scope(scope), timeout.due, now);
            }
        }
    }

    pub(super) fn timer(&mut self, owner: Owner, due: Due, now: i128) {
        if self.state.timers.iter().any(|timer| timer.owner == owner) {
            return;
        }
        let due_unix_nanos = match due {
            Due::After { nanos } => now.saturating_add(i128::from(nanos)),
            Due::At { unix_nanos } => unix_nanos,
        };
        self.state.timers.push(Timer {
            id: self.state.next_timer,
            owner,
            due_unix_nanos,
        });
        self.state.next_timer += 1;
    }
}

/// Whether `inner` stands, at any depth, inside `outer`.
fn nested(program: &Program, inner: u32, outer: u32) -> bool {
    let mut at = program.scope(inner).and_then(|scope| scope.parent);
    while let Some(scope) = at {
        if scope == outer {
            return true;
        }
        at = program.scope(scope).and_then(|scope| scope.parent);
    }
    false
}
