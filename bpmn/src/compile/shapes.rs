//! What each kind of flow node becomes: its step and where it leads, the
//! events by their definitions, a subprocess made a scope, and each
//! boundary event made its activity's timer or catch (`super`).

use process::program::{Catch, Ending, Step, Timeout, Work};

use super::{Builder, Shaped, definitions, pass};
use crate::document::Element;
use crate::extension;
use crate::leads::Lead;
use crate::timer;

impl<'d> Builder<'d> {
    pub(super) fn start(
        &mut self,
        element: &'d Element,
        container: &'d Element,
        inner: bool,
    ) -> Option<Shaped<'d>> {
        let definitions = definitions(element);
        if inner && !definitions.is_empty() {
            self.refuse(element, "a subprocess starts with a none start event");
            return None;
        }
        if let Some(other) = definitions
            .iter()
            .find(|d| d.name != "messageEventDefinition")
        {
            self.refuse(
                element,
                format!("a start event of a {} is not supported", other.name),
            );
            return None;
        }
        let place = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        match container.attribute("id") {
            Some(sub) if inner => self.inner_start.entry(sub).or_default().push(place),
            _ => self.starts.push(place),
        }
        Some(pass())
    }

    pub(super) fn end(
        &mut self,
        element: &'d Element,
        container: &'d Element,
        inner: bool,
    ) -> Option<Shaped<'d>> {
        let definitions = definitions(element);
        let ending = match definitions.as_slice() {
            [] if inner => {
                let sub = container.attribute("id").unwrap_or_default();
                return Some((Step::Start { next: 0 }, Lead::Exit(sub)));
            }
            [] => Ending::Flow,
            [only] if only.name == "terminateEventDefinition" && !inner => Ending::Terminate,
            [only] if only.name == "errorEventDefinition" => Ending::Error {
                code: self
                    .code(only)
                    .unwrap_or_else(|| element.label().to_string()),
            },
            [only] if only.name == "messageEventDefinition" && !inner => {
                let id = element.attribute("id").unwrap_or_default();
                let end = u32::try_from(self.nodes.len() + 1).unwrap_or(u32::MAX);
                let publish = self.add(id, None, Step::Publish { next: end }, Lead::Fixed);
                self.entry.insert(id, publish);
                let ending = Step::End {
                    ending: Ending::Flow,
                };
                self.add(&format!("{id} (end)"), None, ending, Lead::Ends);
                return None;
            }
            _ => {
                self.refuse(element, "this end event is not supported in this version");
                return None;
            }
        };
        Some((Step::End { ending }, Lead::Ends))
    }

    /// The code of the error `definition` names, where it names one.
    pub(crate) fn code(&self, definition: &Element) -> Option<String> {
        let error = definition.attribute("errorRef")?;
        Some(
            self.errors
                .get(error)
                .cloned()
                .unwrap_or_else(|| error.to_string()),
        )
    }

    pub(super) fn service(&mut self, element: &'d Element) -> Option<Shaped<'d>> {
        if !extension::assignments(element).is_empty() {
            self.refuse(
                element,
                "a service task calls a unit; draw a task for assignments",
            );
            return None;
        }
        let work = match (
            extension::extension(element, "call"),
            extension::extension(element, "debatch"),
        ) {
            (Some(called), None) => extension::call(element, called).map(Work::Unit),
            (None, Some(split)) => extension::call(element, split).map(Work::Debatch),
            _ => {
                self.refuse(
                    element,
                    "a service task names one xmip:call or one xmip:debatch",
                );
                return None;
            }
        };
        match work {
            Ok(work) => Some((Step::Call { work, next: 0 }, Lead::Out(""))),
            Err(refused) => {
                self.refusals.push(refused);
                None
            }
        }
    }

    pub(super) fn receive(&mut self, element: &'d Element) -> Option<Shaped<'d>> {
        let Some(batch) = extension::extension(element, "batch") else {
            return Some((Step::Wait { next: 0 }, Lead::Out("")));
        };
        match extension::batch(element, batch) {
            Ok((most, within)) => Some((
                Step::Batch {
                    most,
                    within,
                    next: 0,
                },
                Lead::Out(""),
            )),
            Err(refused) => {
                self.refusals.push(refused);
                None
            }
        }
    }

    pub(super) fn catch_event(&mut self, element: &'d Element) -> Option<Shaped<'d>> {
        match definitions(element).as_slice() {
            [only] if only.name == "messageEventDefinition" => {
                Some((Step::Wait { next: 0 }, Lead::Out("")))
            }
            [only] if only.name == "timerEventDefinition" => match timer::due(element, only) {
                Ok(due) => Some((Step::Timer { due, next: 0 }, Lead::Out(""))),
                Err(refused) => {
                    self.refusals.push(refused);
                    None
                }
            },
            _ => {
                self.refuse(element, "a catch event waits for a message or a timer");
                None
            }
        }
    }

    pub(super) fn throw_event(&mut self, element: &'d Element) -> Option<Shaped<'d>> {
        match definitions(element).as_slice() {
            [] => Some(pass()),
            [only] if only.name == "messageEventDefinition" => {
                Some((Step::Publish { next: 0 }, Lead::Out("")))
            }
            _ => {
                self.refuse(element, "a throw event publishes a message or passes");
                None
            }
        }
    }

    pub(super) fn subprocess(&mut self, element: &'d Element, parent: Option<u32>) {
        let id = element.attribute("id").unwrap_or_default();
        if element.attribute("triggeredByEvent") == Some("true") {
            self.refuse(
                element,
                "an event subprocess is not supported in this version",
            );
            return;
        }
        let scope = self.scope(id, parent);
        self.container(element, Some(scope));
        match self.inner_start.get(id).map(Vec::as_slice) {
            Some([start]) => {
                self.entry.insert(id, *start);
            }
            _ => self.refuse(element, "a subprocess has exactly one none start event"),
        }
    }

    /// Each boundary event made its activity's scope's timer or catch.
    pub(super) fn boundaries(&mut self) {
        for boundary in std::mem::take(&mut self.boundary) {
            let attached = boundary.attribute("attachedToRef").unwrap_or_default();
            let Some(&scope) = self.scope_of.get(attached) else {
                self.refuse(
                    boundary,
                    format!("is attached to '{attached}', which is not drawn"),
                );
                continue;
            };
            if boundary.attribute("cancelActivity") == Some("false") {
                self.refuse(
                    boundary,
                    "a boundary event that does not interrupt is not supported",
                );
                continue;
            }
            let Some(next) = self.out(boundary) else {
                continue;
            };
            let definitions = definitions(boundary);
            match definitions.as_slice() {
                [only] if only.name == "timerEventDefinition" => {
                    let due = match timer::due(boundary, only) {
                        Ok(due) => due,
                        Err(refused) => {
                            self.refusals.push(refused);
                            continue;
                        }
                    };
                    if self.scopes[scope as usize].timeout.is_some() {
                        self.refuse(boundary, "an activity has one boundary timer");
                    }
                    self.scopes[scope as usize].timeout = Some(Timeout { due, next });
                }
                [only] if only.name == "errorEventDefinition" => {
                    let code = self.code(only);
                    self.scopes[scope as usize]
                        .catches
                        .push(Catch { code, next });
                }
                _ => self.refuse(boundary, "a boundary event catches an error or a timer"),
            }
        }
    }
}
