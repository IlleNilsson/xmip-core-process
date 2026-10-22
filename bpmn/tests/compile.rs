//! A BPMN 2.0 design compiled, run and refused: the fixture design
//! `design/order-approval.bpmn` and designs written here for each refusal.

use std::cell::RefCell;

use process::program::{Due, Ending, Step, Work};
use process::{Act, Arrival, Compiled, DesignFormat, Event, Flow, Instance, LocalResources};
use process::{NoResources, Outcome, Refusal};
use xmip_core_process_bpmn::Bpmn;

const ORDER_APPROVAL: &str = include_str!("design/order-approval.bpmn");

/// What a design asked of the files beside it, and what it was given.
struct Beside {
    files: Vec<(&'static str, &'static str)>,
    asked: RefCell<Vec<String>>,
}

impl LocalResources for Beside {
    fn read(&self, location: &str) -> Result<String, String> {
        self.asked.borrow_mut().push(location.to_string());
        self.files
            .iter()
            .find(|(name, _)| *name == location)
            .map(|(_, text)| (*text).to_string())
            .ok_or_else(|| "it is not beside the design".to_string())
    }
}

fn refusals(design: &str) -> Vec<Refusal> {
    Bpmn.compile(design, &NoResources).expect_err("refused")
}

fn wrapped(body: &str) -> String {
    format!(
        "<bpmn:definitions xmlns:bpmn=\"http://www.omg.org/spec/BPMN/20100524/MODEL\" \
         xmlns:xmip=\"urn:xmip:process:1\" id=\"D\" targetNamespace=\"urn:example\">\
         <bpmn:process id=\"P\" isExecutable=\"true\">{body}</bpmn:process>\
         </bpmn:definitions>"
    )
}

#[test]
fn the_order_approval_design_compiles_to_its_steps() {
    let program = Bpmn
        .compile(ORDER_APPROVAL, &NoResources)
        .expect("compiles");
    assert_eq!(program.name, "OrderApproval");
    assert_eq!(program.sets, ["Order"]);
    let step = |id: &str| {
        &program
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("{id}"))
            .step
    };
    let Step::Call {
        work: Work::Unit(call),
        ..
    } = step("Check")
    else {
        panic!("{:?}", step("Check"));
    };
    assert_eq!(
        (call.module.as_str(), call.unit.as_str()),
        ("orders", "Check")
    );
    let Step::Decide {
        branches,
        otherwise,
    } = step("Big")
    else {
        panic!("{:?}", step("Big"));
    };
    assert_eq!(branches[0].condition, "Amount > 1000");
    assert!(otherwise.is_some(), "the default flow is the otherwise");
    assert!(matches!(
        step("Approve"),
        Step::Call {
            work: Work::Assign(_),
            ..
        }
    ));
    assert!(matches!(step("Approval"), Step::Wait { .. }));
    assert!(
        matches!(step("Escalated"), Step::End { ending: Ending::Error { code } } if code == "late")
    );
    let late = &program
        .scopes
        .iter()
        .find(|scope| scope.id == "Approval")
        .expect("scope");
    assert_eq!(
        late.timeout.map(|t| t.due),
        Some(Due::After {
            nanos: 3_600_000_000_000
        })
    );
    let declined = &program
        .scopes
        .iter()
        .find(|scope| scope.id == "Check")
        .expect("scope");
    assert_eq!(declined.catches[0].code.as_deref(), Some("declined"));
}

#[test]
fn the_compiled_design_runs_a_small_order_and_catches_a_declined_one() {
    let flow = Compiled::new(
        Bpmn.compile(ORDER_APPROVAL, &NoResources)
            .expect("compiles"),
    )
    .expect("opens");
    let start = |amount: &str| {
        Event::Arrived(Arrival {
            message: 1,
            values: vec![("Amount".into(), amount.into())],
            keys: vec![(0, "A1".into())],
        })
    };
    let mut small = Instance::new();
    let acts = flow.advance(&mut small, &start("10"), 0).expect("starts");
    let [Act::Work { node, .. }] = acts.as_slice() else {
        panic!("{acts:?}");
    };
    let checked = Event::Done {
        node: *node,
        made: vec![Arrival {
            message: 2,
            values: vec![("Amount".into(), "10".into())],
            keys: Vec::new(),
        }],
    };
    let acts = flow.advance(&mut small, &checked, 0).expect("checked");
    assert!(
        matches!(
            acts[0],
            Act::Work {
                work: Work::Assign(_),
                ..
            }
        ),
        "{acts:?}"
    );
    let mut declined = Instance::new();
    flow.advance(&mut declined, &start("10"), 0)
        .expect("starts");
    let threw = Event::Threw {
        node: *node,
        code: "declined".into(),
        reason: "out of stock".into(),
    };
    let acts = flow.advance(&mut declined, &threw, 0).expect("caught");
    assert_eq!(
        acts,
        vec![Act::Publish { message: 1 }, Act::Ended(Outcome::Completed)]
    );
}

#[test]
fn a_remote_import_is_refused_in_words_and_nothing_is_asked_for_it() {
    let design = wrapped("<bpmn:startEvent id=\"S\"/>").replace(
        "<bpmn:process",
        "<bpmn:import importType=\"http://www.omg.org/spec/BPMN/20100524/MODEL\" \
         location=\"https://partner-x.example/orders.bpmn\" namespace=\"urn:x\"/><bpmn:process",
    );
    let beside = Beside {
        files: Vec::new(),
        asked: RefCell::new(Vec::new()),
    };
    let refused = Bpmn.compile(&design, &beside).expect_err("refused");
    assert!(
        refused
            .iter()
            .any(|r| r.why.contains("on a network") && r.why.contains("never fetches")),
        "{refused:?}"
    );
    assert!(
        beside.asked.borrow().is_empty(),
        "a remote location is never asked for"
    );
}

#[test]
fn a_schema_location_resolves_to_the_shipped_schema_or_beside_the_design() {
    let located = |pairs: &str| {
        wrapped(
            "<bpmn:startEvent id=\"S\"/><bpmn:endEvent id=\"E\"/>\
             <bpmn:sequenceFlow id=\"F\" sourceRef=\"S\" targetRef=\"E\"/>",
        )
        .replace(
            " id=\"D\"",
            &format!(
                " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
                 xsi:schemaLocation=\"{pairs}\" id=\"D\""
            ),
        )
    };
    let shipped = located(
        "http://www.omg.org/spec/BPMN/20100524/MODEL https://www.omg.org/spec/BPMN/20100501/BPMN20.xsd",
    );
    let beside = Beside {
        files: vec![("orders.xsd", "<xsd:schema/>")],
        asked: RefCell::new(Vec::new()),
    };
    Bpmn.compile(&shipped, &beside)
        .expect("the shipped schema answers it");
    assert!(
        beside.asked.borrow().is_empty(),
        "a shipped schema's location is never read"
    );
    Bpmn.compile(&located("urn:example:orders orders.xsd"), &beside)
        .expect("beside it");
    let refused = Bpmn
        .compile(
            &located("urn:example:orders http://partner-x.example/orders.xsd"),
            &beside,
        )
        .expect_err("refused");
    assert!(refused[0].why.contains("on a network"), "{refused:?}");
    assert_eq!(beside.asked.borrow().as_slice(), ["orders.xsd"]);
}

#[test]
fn what_this_version_does_not_compile_is_refused_naming_the_element_and_why() {
    for (element, words) in [
        ("<bpmn:userTask id=\"Ask\"/>", "a task for a person waits"),
        ("<bpmn:manualTask id=\"Do\"/>", "a task for a person waits"),
        ("<bpmn:businessRuleTask id=\"Rule\"/>", "a rule is code"),
        (
            "<bpmn:scriptTask id=\"Script\"/>",
            "never interprets a script",
        ),
        ("<bpmn:transaction id=\"T\"/>", "as try and catch"),
        ("<bpmn:inclusiveGateway id=\"G\"/>", "inclusive gateway"),
        (
            "<bpmn:widget id=\"W\"/>",
            "is not part of what Xmip compiles",
        ),
    ] {
        let refused = refusals(&wrapped(element));
        assert!(
            refused
                .iter()
                .any(|r| r.why.contains(words) && element.contains(&r.at)),
            "{element}: {refused:?}"
        );
    }
}

#[test]
fn a_design_that_does_not_hold_together_is_refused_in_words() {
    let refused = refusals(&wrapped("<bpmn:startEvent id=\"S\"/>"));
    assert!(
        refused
            .iter()
            .any(|r| r.at == "S" && r.why == "leads nowhere"),
        "{refused:?}"
    );
    let refused = refusals("<x/>");
    assert!(refused[0].why.contains("bpmn:definitions"), "{refused:?}");
    let refused = refusals("<bpmn:definitions xmlns:bpmn=\"urn:other\"");
    assert_eq!(refused[0].at, "the design");
    let refused = refusals(&wrapped(
        "<bpmn:startEvent id=\"S\"/><bpmn:exclusiveGateway id=\"G\"/>\
         <bpmn:endEvent id=\"A\"/><bpmn:endEvent id=\"B\"/>\
         <bpmn:sequenceFlow id=\"F\" sourceRef=\"S\" targetRef=\"G\"/>\
         <bpmn:sequenceFlow id=\"FA\" sourceRef=\"G\" targetRef=\"A\">\
         <bpmn:conditionExpression>Amount &gt;</bpmn:conditionExpression></bpmn:sequenceFlow>\
         <bpmn:sequenceFlow id=\"FB\" sourceRef=\"G\" targetRef=\"B\"/>",
    ));
    assert!(refused.iter().any(|r| r.at == "FA"), "{refused:?}");
    assert!(
        refused
            .iter()
            .any(|r| r.at == "FB" && r.why.contains("needs a condition")),
        "{refused:?}"
    );
}

#[test]
fn a_parallel_split_and_join_and_a_subprocess_compile_and_run() {
    let design = wrapped(
        "<bpmn:startEvent id=\"S\"/>\
         <bpmn:parallelGateway id=\"Split\"/>\
         <bpmn:sendTask id=\"A\"/>\
         <bpmn:subProcess id=\"Sub\">\
           <bpmn:startEvent id=\"SubStart\"/>\
           <bpmn:intermediateCatchEvent id=\"Pause\"><bpmn:timerEventDefinition>\
             <bpmn:timeDuration>PT1S</bpmn:timeDuration></bpmn:timerEventDefinition>\
           </bpmn:intermediateCatchEvent>\
           <bpmn:endEvent id=\"SubEnd\"/>\
           <bpmn:sequenceFlow id=\"I1\" sourceRef=\"SubStart\" targetRef=\"Pause\"/>\
           <bpmn:sequenceFlow id=\"I2\" sourceRef=\"Pause\" targetRef=\"SubEnd\"/>\
         </bpmn:subProcess>\
         <bpmn:parallelGateway id=\"Join\"/>\
         <bpmn:endEvent id=\"E\"/>\
         <bpmn:sequenceFlow id=\"F1\" sourceRef=\"S\" targetRef=\"Split\"/>\
         <bpmn:sequenceFlow id=\"F2\" sourceRef=\"Split\" targetRef=\"A\"/>\
         <bpmn:sequenceFlow id=\"F3\" sourceRef=\"Split\" targetRef=\"Sub\"/>\
         <bpmn:sequenceFlow id=\"F4\" sourceRef=\"A\" targetRef=\"Join\"/>\
         <bpmn:sequenceFlow id=\"F5\" sourceRef=\"Sub\" targetRef=\"Join\"/>\
         <bpmn:sequenceFlow id=\"F6\" sourceRef=\"Join\" targetRef=\"E\"/>",
    );
    let flow =
        Compiled::new(Bpmn.compile(&design, &NoResources).expect("compiles")).expect("opens");
    let mut instance = Instance::new();
    let start = Event::Arrived(Arrival {
        message: 1,
        values: Vec::new(),
        keys: Vec::new(),
    });
    let acts = flow.advance(&mut instance, &start, 0).expect("starts");
    assert_eq!(
        acts,
        vec![Act::Publish { message: 1 }],
        "one branch publishes, one waits"
    );
    let acts = flow
        .advance(&mut instance, &Event::Due, 1_000_000_000)
        .expect("due");
    assert_eq!(
        acts,
        vec![Act::Ended(Outcome::Completed)],
        "the join goes on and ends"
    );
}
