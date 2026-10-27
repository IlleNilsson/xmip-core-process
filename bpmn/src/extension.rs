//! What Xmip adds to a BPMN element, in the extension elements BPMN keeps
//! for it (`bpmn:extensionElements`), so every other BPMN tool opens the
//! design and keeps them as written:
//!
//! ```xml
//! <bpmn:process id="OrderApproval" isExecutable="true">
//!   <bpmn:startEvent id="Received">
//!     <bpmn:extensionElements>
//!       <xmip:correlate set="Order"/>
//!     </bpmn:extensionElements>
//!   </bpmn:startEvent>
//!   <bpmn:serviceTask id="Check">
//!     <bpmn:extensionElements>
//!       <xmip:call module="orders" unit="Check">
//!         <xmip:setting name="level" value="2"/>
//!       </xmip:call>
//!     </bpmn:extensionElements>
//!   </bpmn:serviceTask>
//! ```
//!
//! `xmip:debatch` names the splitting unit as `xmip:call` names any, and
//! `xmip:batch most="100" within="PT5M"` makes a receive task collect.
//! `xmip:correlate set="…"` on a start event, a receive task or a message
//! catch event names the Correlation the step takes its Message by:
//! the start initializes it, a later step follows it. The owner,
//! 2026-10-22: *Correlations should be definable, like in BizTalk*, and
//! *As long as the correlation sets are cluster wide*: a set's keys are
//! declared once, in the cluster's `[[correlations]]`, never in a
//! design, so every Work Process following it agrees on them.

use assign::{Assignment, AssignmentValue};
use path::expression::Value;
use process::Refusal;
use process::program::{Call, Due};
use xcore::ScalarValue;

use crate::document::Element;
use crate::schema::MODEL;
use crate::timer;

/// Xmip's namespace for what it adds: a name, never fetched.
pub const XMIP: &str = "urn:xmip:process:1";

/// The language a condition is written in: Xmip's expression language.
pub const EXPRESSION: &str = "urn:xmip:expression";

/// The extension of `element` named `name`, where it has one.
pub fn extension<'a>(element: &'a Element, name: &str) -> Option<&'a Element> {
    element.child(MODEL, "extensionElements")?.child(XMIP, name)
}

/// The Correlation `element` takes its Message by, as the cluster's
/// `[[correlations]]` names it, where it names one.
///
/// # Errors
/// Its `xmip:correlate` names no set.
pub fn set(element: &Element) -> Result<Option<String>, Refusal> {
    let Some(correlate) = extension(element, "correlate") else {
        return Ok(None);
    };
    correlate
        .attribute("set")
        .filter(|set| !set.trim().is_empty())
        .map(|set| Some(set.to_string()))
        .ok_or_else(|| Refusal::new(element.label(), "its xmip:correlate names no set"))
}

/// The unit `called` names: `xmip:call` or `xmip:debatch`.
///
/// # Errors
/// It names no module or no unit.
pub fn call(at: &Element, called: &Element) -> Result<Call, Refusal> {
    let named = |what: &str| {
        called
            .attribute(what)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                Refusal::new(
                    at.label(),
                    format!("its xmip:{} names no {what}", called.name),
                )
            })
    };
    let settings = called
        .children_named(XMIP, "setting")
        .map(|setting| {
            let name = setting.attribute("name").unwrap_or_default().to_string();
            let value = literal(setting.attribute("value").unwrap_or_default());
            (name, value)
        })
        .collect();
    Ok(Call {
        module: named("module")?,
        unit: named("unit")?,
        settings,
    })
}

/// A setting's value as its spelling says: a whole number, a truth, or
/// text.
fn literal(text: &str) -> Value {
    if let Ok(number) = text.parse::<i64>() {
        return Value::Integer(number);
    }
    match text {
        "true" => Value::Boolean(true),
        "false" => Value::Boolean(false),
        _ => Value::Text(text.to_string()),
    }
}

/// A receive task's batch: how many, and within how long.
///
/// # Errors
/// `most` is not a whole number above none, or `within` is no duration.
pub fn batch(at: &Element, batch: &Element) -> Result<(u32, Option<Due>), Refusal> {
    let most = batch
        .attribute("most")
        .and_then(|most| most.parse::<u32>().ok())
        .filter(|most| *most > 0)
        .ok_or_else(|| Refusal::new(at.label(), "its xmip:batch gives no 'most' above 0"))?;
    let within = batch
        .attribute("within")
        .map(|within| timer::duration(at, within))
        .transpose()?;
    Ok((most, within))
}

/// The assignments of `task`'s data output associations, in order: each
/// `to` a context name, each `from` a literal or a context name.
pub fn assignments(task: &Element) -> Vec<Assignment> {
    task.children_named(MODEL, "dataOutputAssociation")
        .flat_map(|association| association.children_named(MODEL, "assignment"))
        .filter_map(|assignment| {
            let from = assignment.child(MODEL, "from")?.text.trim().to_string();
            let to = assignment.child(MODEL, "to")?.text.trim().to_string();
            Some(Assignment {
                target_key: to,
                value: source(&from),
            })
        })
        .collect()
}

/// What an assignment's `from` is: a literal as the expression language
/// spells one, else the context name it reads.
fn source(from: &str) -> AssignmentValue {
    if let Some(text) = from
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return AssignmentValue::Literal(ScalarValue::Text(text.replace("''", "'")));
    }
    match literal(from) {
        Value::Integer(number) => AssignmentValue::Literal(ScalarValue::Integer(number)),
        Value::Boolean(flag) => AssignmentValue::Literal(ScalarValue::Bool(flag)),
        Value::Text(name) => AssignmentValue::Context(name),
    }
}

/// A sequence flow's condition, in Xmip's expression language, compiled to
/// check it.
///
/// # Errors
/// It names another language, or does not compile.
pub fn condition(flow: &Element) -> Result<Option<String>, Refusal> {
    let Some(expression) = flow.child(MODEL, "conditionExpression") else {
        return Ok(None);
    };
    if let Some(language) = expression.attribute("language")
        && language != EXPRESSION
    {
        return Err(Refusal::new(
            flow.label(),
            format!("its condition is written in '{language}'; Xmip reads '{EXPRESSION}'"),
        ));
    }
    let text = expression.text.trim();
    path::expression::Expression::parse(text)
        .map(|_| Some(text.to_string()))
        .map_err(|why| Refusal::new(flow.label(), format!("its condition {why}")))
}
