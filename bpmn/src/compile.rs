//! A BPMN 2.0 design compiled to a Work Process program: its one
//! executable process walked element by element, each flow node made a
//! step and each embedded subprocess, and each task a boundary event is
//! attached to, made a scope ([`process::program`]).

use std::collections::{HashMap, HashSet};

mod shapes;

use process::program::{Node, Program, Scope, Step, Work};
use process::{LocalResources, Refusal};

use crate::document::{self, Element};
use crate::extension;
use crate::leads::Lead;
use crate::refused::{KEPT, READ, why};
use crate::resources;
use crate::schema::MODEL;

/// `design` compiled.
///
/// # Errors
/// Every refusal: the XML, what it names of other documents, what this
/// version does not compile, and what does not hold together.
pub fn compile(design: &str, local: &dyn LocalResources) -> Result<Program, Vec<Refusal>> {
    let root = document::read(design).map_err(|why| vec![Refusal::new("the design", why)])?;
    if !root.is(MODEL, "definitions") {
        return Err(vec![Refusal::new(
            root.label(),
            "a BPMN 2.0 design is a bpmn:definitions document",
        )]);
    }
    let mut refusals = resources::refusals(&root, local);
    let executable: Vec<&Element> = root
        .children_named(MODEL, "process")
        .filter(|process| process.attribute("isExecutable") == Some("true"))
        .collect();
    let [process] = executable.as_slice() else {
        refusals.push(Refusal::new(
            root.label(),
            format!(
                "marks {} processes executable, where a design is one Work Process",
                executable.len()
            ),
        ));
        return Err(refusals);
    };
    let mut builder = Builder::new(&root);
    builder.gather(process);
    builder.container(process, None);
    builder.boundaries();
    let program = builder.finish(process, &mut refusals);
    match program.check() {
        Ok(()) if refusals.is_empty() => Ok(program),
        Ok(()) => Err(refusals),
        Err(problems) => {
            refusals.extend(
                problems
                    .into_iter()
                    .map(|why| Refusal::new(process.label(), why)),
            );
            Err(refusals)
        }
    }
}

/// What is known as a process is compiled.
pub(crate) struct Builder<'d> {
    /// Each `bpmn:error`'s code, by its identifier.
    pub(crate) errors: HashMap<&'d str, String>,
    pub(crate) nodes: Vec<Node>,
    /// How each node's next is found, beside it.
    pub(crate) leads: Vec<Lead<'d>>,
    pub(crate) scopes: Vec<Scope>,
    /// Where a flow into each flow node lands.
    pub(crate) entry: HashMap<&'d str, u32>,
    pub(crate) flows: HashMap<&'d str, &'d Element>,
    pub(crate) outgoing: HashMap<&'d str, Vec<&'d str>>,
    pub(crate) incoming: HashMap<&'d str, u32>,
    /// The activities a boundary event is attached to.
    attached: HashSet<&'d str>,
    pub(crate) scope_of: HashMap<&'d str, u32>,
    boundary: Vec<&'d Element>,
    /// Every flow node walked, at every depth.
    pub(crate) walked: Vec<&'d Element>,
    /// Each subprocess's start, once walked.
    inner_start: HashMap<&'d str, Vec<u32>>,
    starts: Vec<u32>,
    /// The Correlations its steps follow, in the order first named.
    sets: Vec<String>,
    pub(crate) refusals: Vec<Refusal>,
}

impl<'d> Builder<'d> {
    fn new(root: &'d Element) -> Self {
        let errors = root
            .children_named(MODEL, "error")
            .filter_map(|error| {
                let id = error.attribute("id")?;
                let code = error.attribute("errorCode").unwrap_or(id);
                Some((id, code.to_string()))
            })
            .collect();
        Self {
            errors,
            nodes: Vec::new(),
            leads: Vec::new(),
            scopes: Vec::new(),
            entry: HashMap::new(),
            flows: HashMap::new(),
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            attached: HashSet::new(),
            scope_of: HashMap::new(),
            boundary: Vec::new(),
            walked: Vec::new(),
            inner_start: HashMap::new(),
            starts: Vec::new(),
            sets: Vec::new(),
            refusals: Vec::new(),
        }
    }

    /// Every sequence flow and boundary attachment, at every depth.
    fn gather(&mut self, container: &'d Element) {
        for child in container.children.iter().filter(|c| c.namespace == MODEL) {
            match child.name.as_str() {
                "sequenceFlow" => {
                    let (Some(id), Some(source), Some(target)) = (
                        child.attribute("id"),
                        child.attribute("sourceRef"),
                        child.attribute("targetRef"),
                    ) else {
                        self.refuse(child, "a sequence flow needs an id, a source and a target");
                        continue;
                    };
                    self.flows.insert(id, child);
                    self.outgoing.entry(source).or_default().push(id);
                    *self.incoming.entry(target).or_default() += 1;
                }
                "boundaryEvent" => {
                    if let Some(attached) = child.attribute("attachedToRef") {
                        self.attached.insert(attached);
                    }
                    self.boundary.push(child);
                }
                "subProcess" => self.gather(child),
                _ => {}
            }
        }
    }

    pub(crate) fn refuse(&mut self, at: &Element, why: impl Into<String>) {
        self.refusals.push(Refusal::new(at.label(), why));
    }

    /// Every flow node of `container`, standing in `scope`.
    fn container(&mut self, container: &'d Element, scope: Option<u32>) {
        for child in &container.children {
            if child.namespace != MODEL {
                continue;
            }
            let name = child.name.as_str();
            if let Some(why) = why(name) {
                self.refuse(child, why);
                continue;
            }
            if KEPT.contains(&name) || matches!(name, "sequenceFlow" | "boundaryEvent") {
                continue;
            }
            if !READ.contains(&name) {
                self.refuse(
                    child,
                    format!("a bpmn:{name} is not part of what Xmip compiles"),
                );
                continue;
            }
            if let Some(refused) = child.children.iter().find_map(|inner| why(&inner.name)) {
                self.refuse(child, refused);
                continue;
            }
            if child.attribute("isForCompensation") == Some("true") {
                self.refuse(child, why("compensateEventDefinition").unwrap_or_default());
                continue;
            }
            self.walked.push(child);
            self.flow_node(child, container, scope);
        }
    }

    fn flow_node(&mut self, element: &'d Element, container: &'d Element, scope: Option<u32>) {
        let Some(id) = element.attribute("id") else {
            if !matches!(element.name.as_str(), "extensionElements" | "documentation") {
                self.refuse(element, "a flow node needs an id");
            }
            return;
        };
        let scope = if self.attached.contains(id) && element.name != "subProcess" {
            Some(self.scope(id, scope))
        } else {
            scope
        };
        let inner = container.name == "subProcess";
        let shape = match element.name.as_str() {
            "startEvent" => self.start(element, container, inner),
            "endEvent" => self.end(element, container, inner),
            "task" => Some(match extension::assignments(element) {
                assignments if assignments.is_empty() => pass(),
                assignments => (
                    Step::Call {
                        work: Work::Assign(assignments),
                        next: 0,
                    },
                    Lead::Out(id),
                ),
            }),
            "serviceTask" => self.service(element),
            "sendTask" => Some((Step::Publish { next: 0 }, Lead::Out(id))),
            "receiveTask" => self.receive(element),
            "intermediateCatchEvent" => self.catch_event(element),
            "intermediateThrowEvent" => self.throw_event(element),
            "exclusiveGateway" => Some(self.exclusive(id)),
            "parallelGateway" => {
                self.parallel(element, scope);
                return;
            }
            "subProcess" => {
                self.subprocess(element, scope);
                return;
            }
            _ => return,
        };
        if let Some((step, lead)) = shape {
            let lead = match lead {
                Lead::Out(_) => Lead::Out(id),
                other => other,
            };
            let takes = matches!(step, Step::Wait { .. } | Step::Batch { .. })
                || element.name == "startEvent";
            let place = self.add(id, scope, step, lead);
            self.entry.insert(id, place);
            match extension::set(element) {
                Ok(Some(set)) if takes => {
                    let number = self.sets.iter().position(|known| *known == set);
                    let number = number.unwrap_or_else(|| {
                        self.sets.push(set);
                        self.sets.len() - 1
                    });
                    self.nodes[place as usize].set = u32::try_from(number).ok();
                }
                Ok(Some(_)) => self.refuse(element, "only a step taking a Message correlates"),
                Ok(None) => {}
                Err(refused) => self.refusals.push(refused),
            }
        }
    }

    pub(crate) fn add(&mut self, id: &str, scope: Option<u32>, step: Step, lead: Lead<'d>) -> u32 {
        let place = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        self.nodes.push(Node {
            set: None,
            id: id.to_string(),
            scope,
            step,
        });
        self.leads.push(lead);
        place
    }

    fn scope(&mut self, id: &'d str, parent: Option<u32>) -> u32 {
        let place = u32::try_from(self.scopes.len()).unwrap_or(u32::MAX);
        self.scopes.push(Scope {
            id: id.to_string(),
            parent,
            catches: Vec::new(),
            timeout: None,
        });
        self.scope_of.insert(id, place);
        place
    }

    fn finish(mut self, process: &Element, refusals: &mut Vec<Refusal>) -> Program {
        self.resolve();
        refusals.append(&mut self.refusals);
        Program {
            name: process.label().to_string(),
            nodes: self.nodes,
            scopes: self.scopes,
            starts: self.starts,
            sets: self.sets,
        }
    }
}

/// What a flow node becomes: its step, its next to be found as its lead
/// says.
pub(crate) type Shaped<'d> = (Step, Lead<'d>);

/// A step that only goes on.
pub(crate) const fn pass<'d>() -> Shaped<'d> {
    (Step::Start { next: 0 }, Lead::Out(""))
}

/// An event's definitions.
pub(crate) fn definitions(event: &Element) -> Vec<&Element> {
    event
        .children
        .iter()
        .filter(|child| child.namespace == MODEL && child.name.ends_with("EventDefinition"))
        .collect()
}
