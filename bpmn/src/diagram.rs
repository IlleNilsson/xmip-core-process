//! A BPMN design as the Work Process designer draws it: every flow node
//! with where its BPMN DI shape stands, every sequence flow with its
//! waypoints, what Xmip adds to each, and what keeps the design from
//! compiling, each by the element it names (ADR-0064 clause 5: the rules
//! and the layout are here, the drawing in the extension).

use serde::Serialize;

use process::{NoResources, Refusal};

use crate::compile;
use crate::document::{self, Element};
use crate::extension::{self, XMIP};
use crate::schema::{BPMN_DI, DC, DI, MODEL};

/// The kinds the designer adds, by their element names, in palette order,
/// each with what a person calls it.
pub const KINDS: [(&str, &str); 14] = [
    ("startEvent", "Start"),
    ("endEvent", "End"),
    ("task", "Task"),
    ("serviceTask", "Call a unit"),
    ("sendTask", "Publish"),
    ("receiveTask", "Wait for a Message"),
    ("timerCatch", "Wait for a time"),
    ("messageCatch", "Wait for a Message event"),
    ("intermediateThrowEvent", "Throw"),
    ("exclusiveGateway", "Decide"),
    ("parallelGateway", "Split or join"),
    ("subProcess", "Subprocess"),
    ("errorBoundary", "Catch an error"),
    ("timerBoundary", "Time out"),
];

/// The design as drawn.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Diagram {
    /// The executable process's identifier.
    pub process: String,
    pub nodes: Vec<Shape>,
    pub flows: Vec<Flow>,
    /// What keeps it from compiling, each by the element it names.
    pub problems: Vec<Problem>,
    /// The kinds the palette offers: element name and words.
    pub kinds: Vec<(String, String)>,
    pub width: f64,
    pub height: f64,
}

/// One flow node and its shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Shape {
    pub id: String,
    /// Its element name: `serviceTask`, `exclusiveGateway`; a catch event's
    /// is `timerCatch` or `messageCatch`, a boundary's `errorBoundary` or
    /// `timerBoundary`.
    pub kind: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// The subprocess it stands in.
    pub parent: Option<String>,
    /// The activity a boundary event is attached to.
    pub attached: Option<String>,
    pub call: Option<CallView>,
    pub debatch: Option<CallView>,
    pub batch: Option<BatchView>,
    /// The correlation set it takes its Message by.
    pub set: Option<String>,
    /// A timer's duration or date, as written.
    pub timer: Option<String>,
    /// An error event's code.
    pub error: Option<String>,
}

/// A unit a step calls.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CallView {
    pub module: String,
    pub unit: String,
    pub settings: Vec<(String, String)>,
}

/// A receive task's batch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BatchView {
    pub most: String,
    pub within: Option<String>,
}

/// One sequence flow.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Flow {
    pub id: String,
    pub source: String,
    pub target: String,
    pub condition: Option<String>,
    /// Whether it is its gateway's default.
    pub default: bool,
    pub waypoints: Vec<(f64, f64)>,
}

/// A refusal, for the designer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub at: String,
    pub why: String,
}

/// `text` read as the designer draws it.
///
/// # Errors
/// It is not XML, or no BPMN definitions.
pub fn diagram(text: &str) -> Result<Diagram, String> {
    let root = document::read(text)?;
    if !root.is(MODEL, "definitions") {
        return Err("a BPMN 2.0 design is a bpmn:definitions document".to_string());
    }
    let process = root
        .children_named(MODEL, "process")
        .find(|process| process.attribute("isExecutable") == Some("true"))
        .or_else(|| root.child(MODEL, "process"));
    let mut drawn = Diagram {
        kinds: KINDS
            .iter()
            .map(|(kind, words)| ((*kind).to_string(), (*words).to_string()))
            .collect(),
        ..Diagram::default()
    };
    let Some(process) = process else {
        return Ok(drawn);
    };
    drawn.process = process.label().to_string();
    let plane = root
        .children_named(BPMN_DI, "BPMNDiagram")
        .find_map(|diagram| diagram.child(BPMN_DI, "BPMNPlane"));
    collect(process, None, plane, &mut drawn);
    drawn.problems = match compile::compile(text, &NoResources) {
        Ok(_) => Vec::new(),
        Err(refusals) => refusals.into_iter().map(problem).collect(),
    };
    let right = drawn
        .nodes
        .iter()
        .map(|n| n.x + n.width)
        .fold(0.0, f64::max);
    let bottom = drawn
        .nodes
        .iter()
        .map(|n| n.y + n.height)
        .fold(0.0, f64::max);
    drawn.width = right + 40.0;
    drawn.height = bottom + 40.0;
    Ok(drawn)
}

fn problem(refusal: Refusal) -> Problem {
    Problem {
        at: refusal.at,
        why: refusal.why,
    }
}

fn collect(
    container: &Element,
    parent: Option<&str>,
    plane: Option<&Element>,
    drawn: &mut Diagram,
) {
    for child in container.children.iter().filter(|c| c.namespace == MODEL) {
        let Some(id) = child.attribute("id") else {
            continue;
        };
        if child.name == "sequenceFlow" {
            drawn.flows.push(flow(child, id, plane, container));
            continue;
        }
        if !is_flow_node(&child.name) {
            continue;
        }
        let at = drawn.nodes.len();
        drawn.nodes.push(shape(child, id, parent, plane, at));
        if child.name == "subProcess" {
            collect(child, Some(id), plane, drawn);
        }
    }
}

fn is_flow_node(name: &str) -> bool {
    name.ends_with("Event")
        || name.ends_with("Task")
        || name.ends_with("Gateway")
        || matches!(name, "task" | "subProcess" | "transaction" | "callActivity")
}

fn shape(
    element: &Element,
    id: &str,
    parent: Option<&str>,
    plane: Option<&Element>,
    at: usize,
) -> Shape {
    let definition = element
        .children
        .iter()
        .find(|c| c.namespace == MODEL && c.name.ends_with("EventDefinition"));
    let kind = match (element.name.as_str(), definition.map(|d| d.name.as_str())) {
        ("intermediateCatchEvent", Some("timerEventDefinition")) => "timerCatch",
        ("intermediateCatchEvent", _) => "messageCatch",
        ("boundaryEvent", Some("timerEventDefinition")) => "timerBoundary",
        ("boundaryEvent", _) => "errorBoundary",
        (name, _) => name,
    };
    let (default_width, default_height) = size(kind);
    let bounds = plane
        .and_then(|plane| di_of(plane, "BPMNShape", id))
        .and_then(|shape| shape.child(DC, "Bounds"));
    let number = |name: &str, fallback: f64| {
        bounds
            .and_then(|b| b.attribute(name))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    let column = f64::from(u32::try_from(at).unwrap_or(0));
    Shape {
        id: id.to_string(),
        kind: kind.to_string(),
        name: element.attribute("name").unwrap_or_default().to_string(),
        x: number("x", 40.0 + column * 140.0),
        y: number("y", 80.0),
        width: number("width", default_width),
        height: number("height", default_height),
        parent: parent.map(str::to_string),
        attached: element.attribute("attachedToRef").map(str::to_string),
        call: extension::extension(element, "call").map(call_view),
        debatch: extension::extension(element, "debatch").map(call_view),
        batch: extension::extension(element, "batch").map(|batch| BatchView {
            most: batch.attribute("most").unwrap_or_default().to_string(),
            within: batch.attribute("within").map(str::to_string),
        }),
        set: extension::extension(element, "correlate")
            .and_then(|c| c.attribute("set"))
            .map(str::to_string),
        timer: definition.and_then(timer_text),
        error: definition
            .filter(|d| d.name == "errorEventDefinition")
            .and_then(|d| d.attribute("errorRef"))
            .map(str::to_string),
    }
}

/// The size a kind is drawn at where its design gives none.
#[must_use]
pub fn size(kind: &str) -> (f64, f64) {
    if kind.ends_with("Gateway") {
        (50.0, 50.0)
    } else if kind.ends_with("Event") || kind.ends_with("Catch") || kind.ends_with("Boundary") {
        (36.0, 36.0)
    } else if kind == "subProcess" {
        (350.0, 200.0)
    } else {
        (100.0, 80.0)
    }
}

fn timer_text(definition: &Element) -> Option<String> {
    ["timeDuration", "timeDate", "timeCycle"]
        .iter()
        .find_map(|name| definition.child(MODEL, name))
        .map(|time| time.text.trim().to_string())
}

fn call_view(called: &Element) -> CallView {
    CallView {
        module: called.attribute("module").unwrap_or_default().to_string(),
        unit: called.attribute("unit").unwrap_or_default().to_string(),
        settings: called
            .children_named(XMIP, "setting")
            .map(|s| {
                let name = s.attribute("name").unwrap_or_default().to_string();
                (name, s.attribute("value").unwrap_or_default().to_string())
            })
            .collect(),
    }
}

fn flow(element: &Element, id: &str, plane: Option<&Element>, container: &Element) -> Flow {
    let source = element.attribute("sourceRef").unwrap_or_default();
    let gateway_default = container
        .children
        .iter()
        .find(|c| c.attribute("id") == Some(source))
        .and_then(|gateway| gateway.attribute("default"));
    Flow {
        id: id.to_string(),
        source: source.to_string(),
        target: element
            .attribute("targetRef")
            .unwrap_or_default()
            .to_string(),
        condition: element
            .child(MODEL, "conditionExpression")
            .map(|c| c.text.trim().to_string()),
        default: gateway_default == Some(id),
        waypoints: plane
            .and_then(|plane| di_of(plane, "BPMNEdge", id))
            .map(|edge| {
                edge.children_named(DI, "waypoint")
                    .filter_map(|point| {
                        let x = point.attribute("x")?.parse().ok()?;
                        Some((x, point.attribute("y")?.parse().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The DI element of `kind` drawing the element `id`.
pub(crate) fn di_of<'a>(plane: &'a Element, kind: &str, id: &str) -> Option<&'a Element> {
    plane
        .children
        .iter()
        .find(|c| c.is(BPMN_DI, kind) && c.attribute("bpmnElement") == Some(id))
}
