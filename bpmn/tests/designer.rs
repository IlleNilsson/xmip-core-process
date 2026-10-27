//! The Work Process designer's view of a design and each edit it makes:
//! every edit applied, the result read again, still a design, and the
//! change there — with what the edit did not touch left byte for byte.

use xmip_core_process_bpmn::diagram::{Diagram, diagram};
use xmip_core_process_bpmn::edit::{Edit, Extended, Value, apply};

const ORDER_APPROVAL: &str = include_str!("design/order-approval.bpmn");

fn edited(text: &str, edit: &str) -> (String, Diagram) {
    let edit: Edit = serde_json::from_str(edit).expect("an edit");
    let text = apply(text, &edit).unwrap_or_else(|why| panic!("{why}"));
    let drawn = diagram(&text).unwrap_or_else(|why| panic!("{why}\n{text}"));
    (text, drawn)
}

#[test]
fn the_view_draws_every_step_and_flow_where_its_di_says() {
    let drawn = diagram(ORDER_APPROVAL).expect("reads");
    assert_eq!(drawn.process, "OrderApproval");
    let check = drawn.nodes.iter().find(|n| n.id == "Check").expect("Check");
    assert_eq!((check.x, check.y, check.width), (120.0, 98.0, 100.0));
    assert_eq!(check.kind, "serviceTask");
    let call = check.call.as_ref().expect("its call");
    assert_eq!(
        (call.module.as_str(), call.unit.as_str()),
        ("orders", "Check")
    );
    let late = drawn
        .nodes
        .iter()
        .find(|n| n.id == "ApprovalLate")
        .expect("boundary");
    assert_eq!(
        (late.kind.as_str(), late.timer.as_deref()),
        ("timerBoundary", Some("PT1H"))
    );
    assert_eq!(late.attached.as_deref(), Some("Approval"));
    let approval = drawn
        .flows
        .iter()
        .find(|f| f.id == "ToApproval")
        .expect("flow");
    assert_eq!(approval.condition.as_deref(), Some("Amount > 1000"));
    assert_eq!(approval.waypoints.len(), 3);
    assert!(drawn.flows.iter().any(|f| f.id == "ToApprove" && f.default));
    assert!(drawn.problems.is_empty(), "{:?}", drawn.problems);
    assert!(drawn.kinds.iter().any(|(kind, _)| kind == "serviceTask"));
}

#[test]
fn a_step_added_connected_moved_and_removed_leaves_the_rest_as_written() {
    let (text, drawn) = edited(
        ORDER_APPROVAL,
        r#"{"edit":"add","kind":"serviceTask","x":900,"y":98}"#,
    );
    let added = drawn
        .nodes
        .iter()
        .find(|n| n.id == "serviceTask_1")
        .expect("added");
    assert_eq!((added.x, added.y), (900.0, 98.0));
    let (text, drawn) = edited(
        &text,
        r#"{"edit":"connect","source":"Done","target":"serviceTask_1"}"#,
    );
    let flow = drawn
        .flows
        .iter()
        .find(|f| f.target == "serviceTask_1")
        .expect("flow");
    assert_eq!(flow.waypoints, vec![(806.0, 138.0), (900.0, 138.0)]);
    let (text, drawn) = edited(
        &text,
        r#"{"edit":"move","id":"serviceTask_1","x":950,"y":200}"#,
    );
    let flow = drawn
        .flows
        .iter()
        .find(|f| f.target == "serviceTask_1")
        .expect("flow");
    assert_eq!(flow.waypoints, vec![(806.0, 138.0), (950.0, 240.0)]);
    let (text, drawn) = edited(&text, r#"{"edit":"remove","id":"serviceTask_1"}"#);
    assert!(drawn.nodes.iter().all(|n| n.id != "serviceTask_1"));
    assert!(drawn.flows.iter().all(|f| f.target != "serviceTask_1"));
    assert_eq!(
        text, ORDER_APPROVAL,
        "everything else was left byte for byte"
    );
}

#[test]
fn a_name_a_condition_a_default_and_a_timer_are_set_in_place() {
    let (text, drawn) = edited(
        ORDER_APPROVAL,
        r#"{"edit":"rename","id":"Check","name":"Vet"}"#,
    );
    assert_eq!(
        drawn
            .nodes
            .iter()
            .find(|n| n.id == "Check")
            .expect("Check")
            .name,
        "Vet"
    );
    let (text, drawn) = edited(
        &text,
        r#"{"edit":"condition","flow":"ToApproval","condition":"Amount > 5000 and Region = 'EU'"}"#,
    );
    let flow = drawn
        .flows
        .iter()
        .find(|f| f.id == "ToApproval")
        .expect("flow");
    assert_eq!(
        flow.condition.as_deref(),
        Some("Amount > 5000 and Region = 'EU'")
    );
    let (text, drawn) = edited(&text, r#"{"edit":"default","gateway":"Big"}"#);
    assert!(drawn.flows.iter().all(|f| !f.default));
    let (_, drawn) = edited(
        &text,
        r#"{"edit":"timer","id":"ApprovalLate","duration":"P1D"}"#,
    );
    let late = drawn
        .nodes
        .iter()
        .find(|n| n.id == "ApprovalLate")
        .expect("boundary");
    assert_eq!(late.timer.as_deref(), Some("P1D"));
}

#[test]
fn an_extension_is_set_replaced_and_removed() {
    let edit = |name, value| Edit::Extension {
        id: "Approval".into(),
        name,
        value,
    };
    let batch = Value {
        most: "10".into(),
        within: Some("PT5M".into()),
        ..value()
    };
    let text = apply(ORDER_APPROVAL, &edit(Extended::Batch, Some(batch))).expect("set");
    let drawn = diagram(&text).expect("reads");
    let approval = drawn
        .nodes
        .iter()
        .find(|n| n.id == "Approval")
        .expect("Approval");
    let batched = approval.batch.as_ref().expect("batch");
    assert_eq!(
        (batched.most.as_str(), batched.within.as_deref()),
        ("10", Some("PT5M"))
    );
    assert_eq!(
        approval.correlation.as_deref(),
        Some("Order"),
        "its correlation kept"
    );
    let set = Value {
        correlation: "Payment".into(),
        ..value()
    };
    let text = apply(&text, &edit(Extended::Correlate, Some(set))).expect("replaced");
    let text = apply(&text, &edit(Extended::Batch, None)).expect("removed");
    let drawn = diagram(&text).expect("reads");
    let approval = drawn
        .nodes
        .iter()
        .find(|n| n.id == "Approval")
        .expect("Approval");
    assert_eq!(
        (approval.batch.as_ref(), approval.correlation.as_deref()),
        (None, Some("Payment"))
    );
    let call = Value {
        module: "orders".into(),
        unit: "Price".into(),
        settings: vec![("currency".into(), "EUR".into())],
        ..value()
    };
    let text = apply(
        &text,
        &Edit::Extension {
            id: "Approve".into(),
            name: Extended::Call,
            value: Some(call),
        },
    )
    .expect("set on a task with none");
    let drawn = diagram(&text).expect("reads");
    let approve = drawn
        .nodes
        .iter()
        .find(|n| n.id == "Approve")
        .expect("Approve");
    let call = approve.call.as_ref().expect("its call");
    assert_eq!(
        call.settings,
        vec![("currency".to_string(), "EUR".to_string())]
    );
}

#[test]
fn a_design_with_nothing_in_it_gets_its_process_and_its_plane() {
    let empty = "<?xml version=\"1.0\"?>\n<definitions \
                 xmlns=\"http://www.omg.org/spec/BPMN/20100524/MODEL\" id=\"D\" \
                 targetNamespace=\"urn:example\"/>\n";
    let (text, drawn) = edited(empty, r#"{"edit":"add","kind":"startEvent","x":40,"y":40}"#);
    assert_eq!(drawn.process, "Process_1");
    assert_eq!(drawn.nodes.len(), 1, "{text}");
    let (text, drawn) = edited(&text, r#"{"edit":"add","kind":"endEvent","x":200,"y":40}"#);
    let (_, drawn_after) = edited(
        &text,
        r#"{"edit":"connect","source":"startEvent_1","target":"endEvent_1"}"#,
    );
    assert_eq!(drawn.nodes.len(), 2);
    assert!(
        drawn_after.problems.is_empty(),
        "{:?}",
        drawn_after.problems
    );
}

#[test]
fn an_edit_naming_what_is_not_there_is_refused_in_words() {
    let refused = apply(
        ORDER_APPROVAL,
        &Edit::Remove {
            id: "Nowhere".into(),
        },
    );
    assert_eq!(
        refused,
        Err("the design has no element 'Nowhere'".to_string())
    );
    let refused = apply(
        ORDER_APPROVAL,
        &Edit::Timer {
            id: "ApprovalLate".into(),
            duration: "soon".into(),
        },
    );
    assert!(refused.is_err());
}

fn value() -> Value {
    Value {
        module: String::new(),
        unit: String::new(),
        settings: Vec::new(),
        most: String::new(),
        within: None,
        correlation: String::new(),
    }
}
