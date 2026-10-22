//! What the Work Process designer does to a design, made to its text in
//! place ([`crate::splice`]): add a step with its shape, move it, connect
//! two, rename one, set what a step calls, waits by or is timed by, and
//! take one out with its flows and its drawing. Each edit reads the design
//! again before every splice, so no position is ever used after the text
//! moved under it.

mod drawing;

use serde::Deserialize;

use crate::document::{self, Element};
use crate::extension::{EXPRESSION, XMIP};
use crate::schema::MODEL;
use crate::splice::{self, appended, attributed, find, parent_of, prepended, removed};

/// One edit the designer asks for.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "edit", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Edit {
    /// A step of `kind` ([`crate::diagram::KINDS`]) at `x`, `y`, in the
    /// subprocess `within` or the process, or on the activity `attached`.
    Add {
        kind: String,
        x: f64,
        y: f64,
        #[serde(default)]
        within: Option<String>,
        #[serde(default)]
        attached: Option<String>,
    },
    Move {
        id: String,
        x: f64,
        y: f64,
    },
    Connect {
        source: String,
        target: String,
    },
    Rename {
        id: String,
        name: String,
    },
    /// A flow's condition, or none.
    Condition {
        flow: String,
        #[serde(default)]
        condition: Option<String>,
    },
    /// A gateway's default flow, or none.
    Default {
        gateway: String,
        #[serde(default)]
        flow: Option<String>,
    },
    /// What Xmip adds to a step, or that taken away where `value` is none.
    Extension {
        id: String,
        name: Extended,
        #[serde(default)]
        value: Option<Value>,
    },
    /// A timer's duration.
    Timer {
        id: String,
        duration: String,
    },
    Remove {
        id: String,
    },
}

/// The Xmip extensions a step may carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Extended {
    Call,
    Debatch,
    Batch,
    Correlate,
}

/// An extension's values.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Value {
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub settings: Vec<(String, String)>,
    #[serde(default)]
    pub most: String,
    #[serde(default)]
    pub within: Option<String>,
    #[serde(default)]
    pub set: String,
}

impl Extended {
    const fn word(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Debatch => "debatch",
            Self::Batch => "batch",
            Self::Correlate => "correlate",
        }
    }
}

/// `text` with `edit` made.
///
/// # Errors
/// The design does not read, or names nothing the edit names, in words.
pub fn apply(text: &str, edit: &Edit) -> Result<String, String> {
    match edit {
        Edit::Add {
            kind,
            x,
            y,
            within,
            attached,
        } => drawing::add(text, kind, (*x, *y), within.as_deref(), attached.as_deref()),
        Edit::Move { id, x, y } => drawing::moved(text, id, (*x, *y)),
        Edit::Connect { source, target } => drawing::connect(text, source, target),
        Edit::Rename { id, name } => {
            let root = document::read(text)?;
            let element = named(&root, id)?;
            let name = (!name.is_empty()).then_some(name.as_str());
            Ok(attributed(text, element, "name", name))
        }
        Edit::Condition { flow, condition } => conditioned(text, flow, condition.as_deref()),
        Edit::Default { gateway, flow } => {
            let root = document::read(text)?;
            Ok(attributed(
                text,
                named(&root, gateway)?,
                "default",
                flow.as_deref(),
            ))
        }
        Edit::Extension { id, name, value } => extended(text, id, *name, value.as_ref()),
        Edit::Timer { id, duration } => timed(text, id, duration),
        Edit::Remove { id } => drawing::remove(text, id),
    }
}

pub(crate) fn named<'a>(root: &'a Element, id: &str) -> Result<&'a Element, String> {
    find(root, id).ok_or_else(|| format!("the design has no element '{id}'"))
}

/// The prefix the design binds `namespace` to, bound first where it binds
/// none: the text, perhaps with its declaration added, and the prefix.
pub(crate) fn bound(text: &str, namespace: &str, wanted: &str) -> Result<(String, String), String> {
    let root = document::read(text)?;
    if let Some(prefix) = splice::prefix(&root, namespace) {
        return Ok((text.to_string(), prefix));
    }
    let declaration = format!(" xmlns:{wanted}=\"{namespace}\"");
    let at = root.open_end - 1;
    Ok((
        splice::replaced(text, (at, at), &declaration),
        wanted.to_string(),
    ))
}

fn conditioned(text: &str, flow: &str, condition: Option<&str>) -> Result<String, String> {
    let root = document::read(text)?;
    let element = named(&root, flow)?;
    let mut text = match element.child(MODEL, "conditionExpression") {
        Some(old) => removed(text, old),
        None => text.to_string(),
    };
    if let Some(condition) = condition {
        let (bound, prefix) = bound(&text, MODEL, "bpmn")?;
        text = bound;
        let root = document::read(&text)?;
        let name = splice::qualified(&prefix, "conditionExpression");
        let snippet = format!(
            "<{name} language=\"{EXPRESSION}\">{}</{name}>",
            codec::xml::escape(condition)
        );
        text = appended(&text, named(&root, flow)?, &snippet);
    }
    Ok(text)
}

fn extended(text: &str, id: &str, name: Extended, value: Option<&Value>) -> Result<String, String> {
    let (text, xmip) = bound(text, XMIP, "xmip")?;
    let (text, model) = bound(&text, MODEL, "bpmn")?;
    let root = document::read(&text)?;
    let element = named(&root, id)?;
    let holder = element.child(MODEL, "extensionElements");
    let mut text = match holder.and_then(|h| h.child(XMIP, name.word())) {
        Some(old) => removed(&text, old),
        None => text.clone(),
    };
    let Some(value) = value else {
        return Ok(text);
    };
    let tag = splice::qualified(&xmip, name.word());
    let escape = codec::xml::escape;
    let snippet = match name {
        Extended::Call | Extended::Debatch => {
            let setting = splice::qualified(&xmip, "setting");
            let settings: String = value
                .settings
                .iter()
                .map(|(n, v)| {
                    format!(
                        "<{setting} name=\"{}\" value=\"{}\"/>",
                        escape(n),
                        escape(v)
                    )
                })
                .collect();
            let (module, unit) = (escape(&value.module), escape(&value.unit));
            if settings.is_empty() {
                format!("<{tag} module=\"{module}\" unit=\"{unit}\"/>")
            } else {
                format!("<{tag} module=\"{module}\" unit=\"{unit}\">{settings}</{tag}>")
            }
        }
        Extended::Batch => match &value.within {
            Some(within) => format!(
                "<{tag} most=\"{}\" within=\"{}\"/>",
                escape(&value.most),
                escape(within)
            ),
            None => format!("<{tag} most=\"{}\"/>", escape(&value.most)),
        },
        Extended::Correlate => format!("<{tag} set=\"{}\"/>", escape(&value.set)),
    };
    let root = document::read(&text)?;
    let element = named(&root, id)?;
    text = match element.child(MODEL, "extensionElements") {
        Some(holder) => appended(&text, holder, &snippet),
        None => {
            let holder = splice::qualified(&model, "extensionElements");
            prepended(&text, element, &format!("<{holder}>{snippet}</{holder}>"))
        }
    };
    Ok(text)
}

fn timed(text: &str, id: &str, duration: &str) -> Result<String, String> {
    if codec::civil::read_duration(duration).is_none() {
        return Err(format!("'{duration}' is no ISO 8601 duration"));
    }
    let (text, model) = bound(text, MODEL, "bpmn")?;
    let root = document::read(&text)?;
    if named(&root, id)?
        .child(MODEL, "timerEventDefinition")
        .is_none()
    {
        return Err(format!("'{id}' is no timer"));
    }
    let mut text = text.clone();
    for old in ["timeDuration", "timeDate", "timeCycle"] {
        let root = document::read(&text)?;
        let definition = named(&root, id)?.child(MODEL, "timerEventDefinition");
        if let Some(old) = definition.and_then(|d| d.child(MODEL, old)) {
            text = removed(&text, old);
        }
    }
    let root = document::read(&text)?;
    let definition = named(&root, id)?
        .child(MODEL, "timerEventDefinition")
        .ok_or_else(|| format!("'{id}' is no timer"))?;
    let name = splice::qualified(&model, "timeDuration");
    Ok(appended(
        &text,
        definition,
        &format!("<{name}>{}</{name}>", codec::xml::escape(duration)),
    ))
}

/// The process or subprocess `within` names, where a step is added.
pub(crate) fn container<'a>(
    root: &'a Element,
    within: Option<&str>,
) -> Result<&'a Element, String> {
    match within {
        Some(id) => named(root, id),
        None => root
            .children_named(MODEL, "process")
            .find(|p| p.attribute("isExecutable") == Some("true"))
            .or_else(|| root.child(MODEL, "process"))
            .ok_or_else(|| "the design has no process".to_string()),
    }
}

/// The element holding `id`.
pub(crate) fn holder_of<'a>(root: &'a Element, id: &str) -> Result<&'a Element, String> {
    parent_of(root, id).ok_or_else(|| format!("the design has no element '{id}'"))
}
