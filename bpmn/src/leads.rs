//! Where each step leads: a BPMN sequence flow resolved to the number of
//! the step it reaches, once every flow node has its number; and the
//! gateways, whose leads are their flows.

use process::program::{Branch, Step};

use crate::compile::{Builder, Shaped, pass};
use crate::document::Element;
use crate::extension;

/// How a step's next is found.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Lead<'d> {
    /// The one outgoing flow of the flow node of that identifier.
    Out(&'d str),
    /// The one outgoing flow of the subprocess of that identifier: where a
    /// flow ending inside it goes on.
    Exit(&'d str),
    /// An exclusive gateway's flows, each a branch, its default otherwise.
    Branches(&'d str),
    /// A parallel gateway's flows, every one.
    Fork(&'d str),
    /// A step already numbered.
    Fixed,
    /// An end leads nowhere.
    Ends,
}

impl<'d> Builder<'d> {
    /// An exclusive gateway: a decision where it splits, a step that only
    /// goes on where it merges.
    pub(crate) fn exclusive(&self, id: &'d str) -> Shaped<'d> {
        if self.outgoing.get(id).is_some_and(|flows| flows.len() > 1) {
            let step = Step::Decide {
                branches: Vec::new(),
                otherwise: None,
            };
            (step, Lead::Branches(id))
        } else {
            pass()
        }
    }

    /// A parallel gateway: a Join where flows meet, a Fork where they
    /// split, both where they do both.
    pub(crate) fn parallel(&mut self, element: &'d Element, scope: Option<u32>) {
        let id = element.attribute("id").unwrap_or_default();
        let from = self.incoming.get(id).copied().unwrap_or(0);
        let splits = self.outgoing.get(id).is_some_and(|flows| flows.len() > 1);
        let fork = Step::Fork { next: Vec::new() };
        let place = match (from > 1, splits) {
            (true, true) => {
                let fork_place = u32::try_from(self.nodes.len() + 1).unwrap_or(u32::MAX);
                let join = Step::Join {
                    from,
                    next: fork_place,
                };
                let place = self.add(id, scope, join, Lead::Fixed);
                self.add(&format!("{id} (split)"), scope, fork, Lead::Fork(id));
                place
            }
            (true, false) => self.add(id, scope, Step::Join { from, next: 0 }, Lead::Out(id)),
            (false, true) => self.add(id, scope, fork, Lead::Fork(id)),
            (false, false) => self.add(id, scope, Step::Start { next: 0 }, Lead::Out(id)),
        };
        self.entry.insert(id, place);
    }

    /// Where `flow` leads, or a refusal.
    fn target(&mut self, flow: &'d str) -> Option<u32> {
        let element = *self.flows.get(flow)?;
        let target = element.attribute("targetRef").unwrap_or_default();
        let found = self.entry.get(target).copied();
        if found.is_none() {
            self.refuse(
                element,
                format!("leads to '{target}', which is not a step Xmip runs"),
            );
        }
        found
    }

    /// The one step `element`'s one outgoing flow leads to.
    pub(crate) fn out(&mut self, element: &'d Element) -> Option<u32> {
        let id = element.attribute("id").unwrap_or_default();
        let flows = self.outgoing.get(id).cloned().unwrap_or_default();
        match flows.as_slice() {
            [flow] => {
                if let Some(flowed) = self.flows.get(flow).copied()
                    && flowed
                        .child(crate::schema::MODEL, "conditionExpression")
                        .is_some()
                {
                    self.refuse(flowed, "a condition stands on a gateway's flow; draw one");
                }
                self.target(flow)
            }
            [] => {
                self.refuse(element, "leads nowhere");
                None
            }
            _ => {
                self.refuse(element, "has more than one outgoing flow; draw a gateway");
                None
            }
        }
    }

    /// Every step's next filled in from its lead.
    pub(crate) fn resolve(&mut self) {
        let elements = self.elements();
        for place in 0..self.nodes.len() {
            let lead = self.leads[place];
            let step = match (lead, self.nodes[place].step.clone()) {
                (Lead::Fixed | Lead::Ends, step) => step,
                (Lead::Out(id) | Lead::Exit(id), step) => {
                    let Some(element) = elements.get(id).copied() else {
                        continue;
                    };
                    let Some(next) = self.out(element) else {
                        continue;
                    };
                    with_next(step, next)
                }
                (Lead::Branches(id), _) => self.branches(id),
                (Lead::Fork(id), _) => {
                    let flows = self.outgoing.get(id).cloned().unwrap_or_default();
                    let next = flows.iter().filter_map(|flow| self.target(flow)).collect();
                    Step::Fork { next }
                }
            };
            self.nodes[place].step = step;
        }
    }

    fn branches(&mut self, id: &'d str) -> Step {
        let flows = self.outgoing.get(id).cloned().unwrap_or_default();
        let default = self
            .elements()
            .get(id)
            .and_then(|gateway| gateway.attribute("default"));
        let mut branches = Vec::new();
        let mut otherwise = None;
        for flow in flows {
            let Some(element) = self.flows.get(flow).copied() else {
                continue;
            };
            let Some(next) = self.target(flow) else {
                continue;
            };
            if default == Some(flow) {
                otherwise = Some(next);
                continue;
            }
            match extension::condition(element) {
                Ok(Some(condition)) => branches.push(Branch { condition, next }),
                Ok(None) => self.refuse(element, "a flow out of a decision needs a condition"),
                Err(refused) => self.refusals.push(refused),
            }
        }
        Step::Decide {
            branches,
            otherwise,
        }
    }

    /// Every flow node by its identifier, with the gateways and
    /// subprocesses among them.
    fn elements(&self) -> std::collections::HashMap<&'d str, &'d Element> {
        let mut found = std::collections::HashMap::new();
        for element in &self.walked {
            if let Some(id) = element.attribute("id") {
                found.insert(id, *element);
            }
        }
        found
    }
}

/// `step` leading to `next`.
fn with_next(step: Step, next: u32) -> Step {
    match step {
        Step::Start { .. } => Step::Start { next },
        Step::Call { work, .. } => Step::Call { work, next },
        Step::Publish { .. } => Step::Publish { next },
        Step::Wait { .. } => Step::Wait { next },
        Step::Batch { most, within, .. } => Step::Batch { most, within, next },
        Step::Timer { due, .. } => Step::Timer { due, next },
        Step::Join { from, .. } => Step::Join { from, next },
        other => other,
    }
}
