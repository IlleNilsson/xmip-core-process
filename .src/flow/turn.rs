//! One turn of a compiled program: the event taken, then every flow that
//! can go on run until each waits or ends (`super::Flow::advance`).

use path::expression::{Expression, Truth};

mod scopes;

use super::{Act, Arrival, Event, Refused};
use crate::instance::{Instance, Outcome, Owner, Standing};
use crate::program::{Ending, Program, Step, Work};

/// The most steps one turn takes: a flow that loops without waiting on
/// anything is thrown as an error, never run forever.
const MOST_STEPS: u32 = 10_000;

/// The code a decision throws where no condition holds and no default
/// flow is drawn.
pub const NO_BRANCH: &str = "no-branch";

/// The code a turn throws where its flows loop without waiting.
pub const LOOPS: &str = "loops";

pub(super) struct Turn<'a> {
    program: &'a Program,
    conditions: &'a [Vec<Expression>],
    state: Instance,
    acts: Vec<Act>,
    /// When the turn is taken: what a timer it starts is due after.
    now: i128,
}

impl<'a> Turn<'a> {
    pub(super) const fn new(
        program: &'a Program,
        conditions: &'a [Vec<Expression>],
        state: Instance,
        now: i128,
    ) -> Self {
        Self {
            program,
            conditions,
            state,
            acts: Vec::new(),
            now,
        }
    }

    /// `event` taken into the Instance.
    pub(super) fn take(&mut self, event: &Event, now: i128) -> Result<(), Refused> {
        match event {
            Event::Arrived(arrival) => self.arrived(arrival, now),
            Event::Done { node, made } => self.done(*node, made, now),
            Event::Threw { node, code, reason } => {
                self.working(*node)?;
                self.throw(*node, code, reason);
                Ok(())
            }
            Event::Due => {
                self.due(now);
                Ok(())
            }
        }
    }

    fn arrived(&mut self, arrival: &Arrival, now: i128) -> Result<(), Refused> {
        if self.state.is_new() && self.state.tokens.is_empty() {
            let start = self
                .program
                .starts
                .iter()
                .copied()
                .find(|start| self.fits(*start, arrival));
            let Some(start) = start else {
                return Err(Refused::Unfit(
                    "it starts no Instance: it has no key in the set a start takes".to_string(),
                ));
            };
            self.taken(start, arrival);
            self.current(arrival);
            self.add(start, now);
            return Ok(());
        }
        let waiting =
            self.state.tokens.iter().position(|token| {
                token.standing == Standing::Message && self.fits(token.at, arrival)
            });
        let Some(index) = waiting else {
            return Err(Refused::NotWaiting);
        };
        let node = self.state.tokens[index].at;
        self.taken(node, arrival);
        match self.step(node) {
            Step::Wait { next } => {
                self.current(arrival);
                self.go(index, next, now);
            }
            Step::Batch { most, .. } => {
                self.state.batch.push(arrival.message);
                if self.state.batch.len() >= usize::try_from(most).unwrap_or(usize::MAX) {
                    self.combine(index, node);
                }
            }
            _ => {
                return Err(Refused::Unfit(format!(
                    "'{}' waits for no Message",
                    self.id(node)
                )));
            }
        }
        Ok(())
    }

    fn done(&mut self, node: u32, made: &[Arrival], now: i128) -> Result<(), Refused> {
        let index = self.working(node)?;
        match self.step(node) {
            Step::Call {
                work: Work::Debatch(_),
                next,
            } => {
                for part in made {
                    self.acts.push(Act::Publish {
                        message: part.message,
                    });
                }
                self.go(index, next, now);
            }
            Step::Call { next, .. } | Step::Batch { next, .. } => {
                if let Some(first) = made.first() {
                    self.current(first);
                }
                self.state.batch.clear();
                self.go(index, next, now);
            }
            _ => {
                return Err(Refused::Unfit(format!(
                    "'{}' asked for no work",
                    self.id(node)
                )));
            }
        }
        Ok(())
    }

    /// The flow at `node` waiting for its work.
    fn working(&self, node: u32) -> Result<usize, Refused> {
        self.state
            .tokens
            .iter()
            .position(|token| token.at == node && token.standing == Standing::Work)
            .ok_or_else(|| Refused::Unfit(format!("'{}' waits for no work", self.id(node))))
    }

    /// The Timer or Batch at `node` woken by its time.
    fn woken(&mut self, node: u32, now: i128) {
        let Some(index) = self.state.tokens.iter().position(|token| token.at == node) else {
            return;
        };
        match self.step(node) {
            Step::Timer { next, .. } => self.go(index, next, now),
            Step::Batch { next, .. } if self.state.batch.is_empty() => self.go(index, next, now),
            Step::Batch { .. } => self.combine(index, node),
            _ => {}
        }
    }

    fn combine(&mut self, index: usize, node: u32) {
        self.state.tokens[index].standing = Standing::Work;
        self.state
            .timers
            .retain(|timer| timer.owner != Owner::Node(node));
        self.acts.push(Act::Combine {
            node,
            messages: self.state.batch.clone(),
        });
    }

    /// Every flow that is ready run until it waits or ends.
    pub(super) fn run(&mut self, now: i128) {
        let mut steps = 0;
        while let Some(index) = self.ready() {
            steps += 1;
            if steps > MOST_STEPS {
                let node = self.state.tokens[index].at;
                let reason = format!("its flows ran {MOST_STEPS} steps without waiting");
                self.throw(node, LOOPS, &reason);
                continue;
            }
            self.stand(index, now);
        }
    }

    fn ready(&self) -> Option<usize> {
        self.state
            .tokens
            .iter()
            .position(|token| token.standing == Standing::Ready)
    }

    /// The step the flow at `index` stands on, taken.
    fn stand(&mut self, index: usize, now: i128) {
        let node = self.state.tokens[index].at;
        match self.step(node) {
            Step::Start { next } | Step::Publish { next } => {
                if matches!(self.step(node), Step::Publish { .. }) {
                    let message = self.state.message.unwrap_or_default();
                    self.acts.push(Act::Publish { message });
                }
                self.go(index, next, now);
            }
            Step::Call { work, .. } => {
                self.state.tokens[index].standing = Standing::Work;
                let message = self.state.message.unwrap_or_default();
                self.acts.push(Act::Work {
                    node,
                    work,
                    message,
                });
            }
            Step::Wait { .. } => self.state.tokens[index].standing = Standing::Message,
            Step::Batch { within, .. } => {
                self.state.tokens[index].standing = Standing::Message;
                if let Some(due) = within {
                    self.timer(Owner::Node(node), due, now);
                }
            }
            Step::Timer { due, .. } => {
                self.state.tokens[index].standing = Standing::Timer;
                self.timer(Owner::Node(node), due, now);
            }
            Step::Decide {
                branches,
                otherwise,
            } => self.decide(index, node, &branches, otherwise, now),
            Step::Fork { next } => {
                let mut leads = next.into_iter();
                if let Some(first) = leads.next() {
                    self.go(index, first, now);
                }
                for lead in leads {
                    self.add(lead, now);
                }
            }
            Step::Join { from, next } => {
                self.state.tokens.remove(index);
                let arrived = self.joined(node);
                if arrived >= from {
                    self.state.joins.retain(|(join, _)| *join != node);
                    self.add(next, now);
                }
            }
            Step::End { ending } => self.end(index, node, &ending),
        }
    }

    fn decide(
        &mut self,
        index: usize,
        node: u32,
        branches: &[crate::program::Branch],
        otherwise: Option<u32>,
        now: i128,
    ) {
        let compiled = &self.conditions[usize::try_from(node).unwrap_or(usize::MAX)];
        let taken = branches
            .iter()
            .zip(compiled)
            .find(|(_, condition)| condition.evaluate(&self.state) == Truth::True)
            .map(|(branch, _)| branch.next)
            .or(otherwise);
        match taken {
            Some(next) => self.go(index, next, now),
            None => {
                let reason = format!(
                    "no condition of '{}' holds and it has no default flow",
                    self.id(node)
                );
                self.throw(node, NO_BRANCH, &reason);
            }
        }
    }

    fn end(&mut self, index: usize, node: u32, ending: &Ending) {
        match ending {
            Ending::Flow => {
                self.state.tokens.remove(index);
            }
            Ending::Terminate => {
                self.state.tokens.clear();
                self.state.timers.clear();
                self.state.joins.clear();
                self.state.ended = Some(Outcome::Completed);
            }
            Ending::Error { code } => {
                let reason = format!("the error end '{}' threw it", self.id(node));
                self.throw(node, code, &reason);
            }
        }
    }

    /// One more flow arrived at the Join at `node`: how many have.
    fn joined(&mut self, node: u32) -> u32 {
        if let Some((_, arrived)) = self.state.joins.iter_mut().find(|(join, _)| *join == node) {
            *arrived += 1;
            return *arrived;
        }
        self.state.joins.push((node, 1));
        1
    }

    /// The set the step at `node` takes its Message by.
    fn set_of(&self, node: u32) -> Option<u32> {
        self.program.node(node).and_then(|node| node.set)
    }

    /// Whether the step at `node` takes `arrival`: it follows no set, or
    /// the Message has a key in its set, the Instance's key where it has
    /// one.
    fn fits(&self, node: u32, arrival: &Arrival) -> bool {
        let Some(set) = self.set_of(node) else {
            return true;
        };
        let Some((_, key)) = arrival.keys.iter().find(|(known, _)| *known == set) else {
            return false;
        };
        self.state.key(set).is_none_or(|kept| kept == key)
    }

    /// The Instance's key in the set the step at `node` follows, taken from
    /// `arrival` where it had none: the set initialized.
    fn taken(&mut self, node: u32, arrival: &Arrival) {
        let Some(set) = self.set_of(node) else {
            return;
        };
        if self.state.key(set).is_some() {
            return;
        }
        if let Some((_, key)) = arrival.keys.iter().find(|(known, _)| *known == set) {
            self.state.keys.push((set, key.clone()));
        }
    }

    fn current(&mut self, arrival: &Arrival) {
        self.state.message = Some(arrival.message);
        self.state.values.clone_from(&arrival.values);
    }

    fn step(&self, node: u32) -> Step {
        self.program.node(node).map_or(
            Step::End {
                ending: Ending::Flow,
            },
            |node| node.step.clone(),
        )
    }

    fn id(&self, node: u32) -> &str {
        self.program.node(node).map_or("?", |node| node.id.as_str())
    }

    /// The Instance as the turn left it, and what the runtime carries out:
    /// a scope's timer stopped where no flow stands in it any more, and
    /// the Instance ended where no flow is left.
    pub(super) fn finish(mut self) -> (Instance, Vec<Act>) {
        let program = self.program;
        let tokens = self.state.tokens.clone();
        self.state.timers.retain(|timer| match timer.owner {
            Owner::Scope(scope) => tokens
                .iter()
                .any(|token| program.scopes_of(token.at).contains(&scope)),
            Owner::Node(_) => true,
        });
        if self.state.ended.is_none() && self.state.tokens.is_empty() {
            self.state.ended = Some(Outcome::Completed);
        }
        if let Some(ended) = &self.state.ended {
            self.acts.push(Act::Ended(ended.clone()));
        }
        (self.state, self.acts)
    }
}
