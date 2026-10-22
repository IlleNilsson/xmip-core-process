//! The edits that change what is drawn: a step added with its BPMN DI
//! shape, a shape moved and the flows touching it routed again, two steps
//! connected by a flow and its edge, and a step taken out with its flows,
//! the boundary events on it and every DI element drawing them.

use super::{bound, container, holder_of, named};
use crate::diagram::{di_of, size};
use crate::document::{self, Element};
use crate::schema::{BPMN_DI, DC, DI, MODEL};
use crate::splice::{self, appended, attributed, every, qualified, removed, unused_id};

/// A step of `kind` added at `at`.
pub(super) fn add(
    text: &str,
    kind: &str,
    at: (f64, f64),
    within: Option<&str>,
    attached: Option<&str>,
) -> Result<String, String> {
    let (text, model) = bound(text, MODEL, "bpmn")?;
    let text = with_process(&text, &model, within)?;
    let root = document::read(&text)?;
    let holder = match attached {
        Some(activity) => holder_of(&root, activity)?,
        None => container(&root, within)?,
    };
    let id = unused_id(&root, kind);
    let element = |name: &str, inner: &str| {
        let name = qualified(&model, name);
        let opened = format!("<{name} id=\"{id}\"");
        let attached = attached
            .map(|a| format!(" attachedToRef=\"{}\"", codec::xml::escape(a)))
            .unwrap_or_default();
        if inner.is_empty() {
            format!("{opened}{attached}/>")
        } else {
            format!("{opened}{attached}>{inner}</{name}>")
        }
    };
    let definition = |name: &str, inner: &str| {
        let name = qualified(&model, name);
        if inner.is_empty() {
            format!("<{name}/>")
        } else {
            format!("<{name}>{inner}</{name}>")
        }
    };
    let duration = format!("<{0}>PT1M</{0}>", qualified(&model, "timeDuration"));
    let snippet = match kind {
        "startEvent"
        | "endEvent"
        | "task"
        | "serviceTask"
        | "sendTask"
        | "receiveTask"
        | "intermediateThrowEvent"
        | "exclusiveGateway"
        | "parallelGateway"
        | "subProcess" => element(kind, ""),
        "timerCatch" => element(
            "intermediateCatchEvent",
            &definition("timerEventDefinition", &duration),
        ),
        "messageCatch" => element(
            "intermediateCatchEvent",
            &definition("messageEventDefinition", ""),
        ),
        "errorBoundary" | "timerBoundary" if attached.is_none() => {
            return Err("a boundary event is added on an activity".to_string());
        }
        "errorBoundary" => element("boundaryEvent", &definition("errorEventDefinition", "")),
        "timerBoundary" => element(
            "boundaryEvent",
            &definition("timerEventDefinition", &duration),
        ),
        other => return Err(format!("the designer adds no '{other}'")),
    };
    let text = appended(&text, holder, &snippet);
    shaped(&text, &id, kind, at)
}

/// The executable process, added where the design has none and the step
/// is not added in a subprocess.
fn with_process(text: &str, model: &str, within: Option<&str>) -> Result<String, String> {
    let root = document::read(text)?;
    if within.is_some() || container(&root, None).is_ok() {
        return Ok(text.to_string());
    }
    let process = qualified(model, "process");
    let snippet = format!("<{process} id=\"Process_1\" isExecutable=\"true\"/>");
    match root.child(BPMN_DI, "BPMNDiagram") {
        Some(diagram) => {
            let indent = splice::indent_of(text, diagram.start);
            let placed = format!("{snippet}\n{indent}");
            Ok(splice::replaced(
                text,
                (diagram.start, diagram.start),
                &placed,
            ))
        }
        None => Ok(appended(text, &root, &snippet)),
    }
}

/// The plane every shape is drawn on, added where the design has none.
fn with_plane(text: &str) -> Result<(String, String, String), String> {
    let (text, di) = bound(text, BPMN_DI, "bpmndi")?;
    let (text, dc) = bound(&text, DC, "dc")?;
    let root = document::read(&text)?;
    if plane(&root).is_some() {
        return Ok((text, di, dc));
    }
    let process = container(&root, None)?.label().to_string();
    let (diagram, plane) = (qualified(&di, "BPMNDiagram"), qualified(&di, "BPMNPlane"));
    let snippet = format!(
        "<{diagram} id=\"Diagram_1\"><{plane} id=\"Plane_1\" \
         bpmnElement=\"{process}\"/></{diagram}>"
    );
    Ok((appended(&text, &root, &snippet), di, dc))
}

fn plane(root: &Element) -> Option<&Element> {
    root.children_named(BPMN_DI, "BPMNDiagram")
        .find_map(|diagram| diagram.child(BPMN_DI, "BPMNPlane"))
}

/// The shape of `id`, of `kind`, drawn at `at`.
fn shaped(text: &str, id: &str, kind: &str, (x, y): (f64, f64)) -> Result<String, String> {
    let (text, di, dc) = with_plane(text)?;
    let root = document::read(&text)?;
    let plane = plane(&root).ok_or("the design has no BPMN plane")?;
    let (width, height) = size(kind);
    let expanded = if kind == "subProcess" {
        " isExpanded=\"true\""
    } else {
        ""
    };
    let (shape, bounds) = (qualified(&di, "BPMNShape"), qualified(&dc, "Bounds"));
    let snippet = format!(
        "<{shape} id=\"{id}_di\" bpmnElement=\"{id}\"{expanded}><{bounds} x=\"{x}\" y=\"{y}\" \
         width=\"{width}\" height=\"{height}\"/></{shape}>"
    );
    Ok(appended(&text, plane, &snippet))
}

/// The shape of `id` moved to `at`, and every flow touching it routed again.
pub(super) fn moved(text: &str, id: &str, at: (f64, f64)) -> Result<String, String> {
    let (mut text, _, _) = with_plane(text)?;
    let root = document::read(&text)?;
    let element = named(&root, id)?;
    let kind = element.name.clone();
    let bounds = plane(&root)
        .and_then(|plane| di_of(plane, "BPMNShape", id))
        .and_then(|shape| shape.child(DC, "Bounds"))
        .cloned();
    text = match bounds {
        Some(bounds) => {
            let text = attributed(&text, &bounds, "y", Some(&at.1.to_string()));
            let root = document::read(&text)?;
            let bounds = plane(&root)
                .and_then(|plane| di_of(plane, "BPMNShape", id))
                .and_then(|shape| shape.child(DC, "Bounds"))
                .ok_or("the shape went missing")?;
            attributed(&text, bounds, "x", Some(&at.0.to_string()))
        }
        None => shaped(&text, id, &kind, at)?,
    };
    for flow in touching(&document::read(&text)?, id) {
        text = routed(&text, &flow)?;
    }
    Ok(text)
}

/// Every sequence flow from or to `id`.
fn touching(root: &Element, id: &str) -> Vec<String> {
    every(root)
        .into_iter()
        .filter(|e| e.is(MODEL, "sequenceFlow"))
        .filter(|e| e.attribute("sourceRef") == Some(id) || e.attribute("targetRef") == Some(id))
        .filter_map(|e| e.attribute("id").map(str::to_string))
        .collect()
}

/// The bounds `id` is drawn at.
fn bounds_of(root: &Element, id: &str) -> Option<(f64, f64, f64, f64)> {
    let bounds = plane(root)
        .and_then(|plane| di_of(plane, "BPMNShape", id))
        .and_then(|shape| shape.child(DC, "Bounds"))?;
    let read = |name: &str| bounds.attribute(name).and_then(|v| v.parse::<f64>().ok());
    Some((read("x")?, read("y")?, read("width")?, read("height")?))
}

/// The edge of `flow` drawn again, straight from its source's right side
/// to its target's left.
fn routed(text: &str, flow: &str) -> Result<String, String> {
    let (mut text, di, _) = with_plane(text)?;
    let (text_di, waypoint_prefix) = bound(&text, DI, "di")?;
    text = text_di;
    loop {
        let root = document::read(&text)?;
        let old = plane(&root)
            .and_then(|plane| di_of(plane, "BPMNEdge", flow))
            .and_then(|edge| edge.child(DI, "waypoint"));
        match old {
            Some(old) => text = removed(&text, old),
            None => break,
        }
    }
    let root = document::read(&text)?;
    let element = named(&root, flow)?;
    let ends = (
        element
            .attribute("sourceRef")
            .and_then(|s| bounds_of(&root, s)),
        element
            .attribute("targetRef")
            .and_then(|t| bounds_of(&root, t)),
    );
    let (Some((sx, sy, sw, sh)), Some((tx, ty, _, th))) = ends else {
        return Ok(text);
    };
    let point = qualified(&waypoint_prefix, "waypoint");
    let points = format!(
        "<{point} x=\"{}\" y=\"{}\"/><{point} x=\"{tx}\" y=\"{}\"/>",
        sx + sw,
        sy + sh / 2.0,
        ty + th / 2.0
    );
    let plane = plane(&root).ok_or("the design has no BPMN plane")?;
    Ok(match di_of(plane, "BPMNEdge", flow) {
        Some(edge) => appended(&text, edge, &points),
        None => {
            let edge = qualified(&di, "BPMNEdge");
            let snippet =
                format!("<{edge} id=\"{flow}_di\" bpmnElement=\"{flow}\">{points}</{edge}>");
            appended(&text, plane, &snippet)
        }
    })
}

/// `source` connected to `target` by a new sequence flow.
pub(super) fn connect(text: &str, source: &str, target: &str) -> Result<String, String> {
    let (text, model) = bound(text, MODEL, "bpmn")?;
    let root = document::read(&text)?;
    named(&root, target)?;
    let holder = holder_of(&root, source)?;
    let id = unused_id(&root, "Flow");
    let flow = qualified(&model, "sequenceFlow");
    let escape = codec::xml::escape;
    let snippet = format!(
        "<{flow} id=\"{id}\" sourceRef=\"{}\" targetRef=\"{}\"/>",
        escape(source),
        escape(target)
    );
    let text = appended(&text, holder, &snippet);
    routed(&text, &id)
}

/// `id` taken out with its flows, the boundary events on it and every DI
/// element drawing any of them; a gateway whose default was one of its
/// flows has none.
pub(super) fn remove(text: &str, id: &str) -> Result<String, String> {
    let root = document::read(text)?;
    named(&root, id)?;
    let mut gone = vec![id.to_string()];
    for element in every(&root) {
        let on_it = element.attribute("attachedToRef") == Some(id);
        if on_it && let Some(boundary) = element.attribute("id") {
            gone.push(boundary.to_string());
        }
    }
    for removed_id in gone.clone() {
        gone.extend(touching(&root, &removed_id));
    }
    let mut text = text.to_string();
    for removed_id in &gone {
        loop {
            let root = document::read(&text)?;
            let found = every(&root).into_iter().find(|e| {
                e.attribute("id") == Some(removed_id.as_str())
                    || e.attribute("bpmnElement") == Some(removed_id.as_str())
                    || (e.attribute("default") == Some(removed_id.as_str()))
            });
            let Some(found) = found.cloned() else {
                break;
            };
            text = if found.attribute("default") == Some(removed_id.as_str()) {
                attributed(&text, &found, "default", None)
            } else {
                removed(&text, &found)
            };
        }
    }
    Ok(text)
}
