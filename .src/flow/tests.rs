//! The engine's turns, over programs built by hand: every step, scope and
//! refusal, and the forms they cross in.

#[cfg(test)]
use super::*;
use crate::form::{self, read_acts, read_turn, write_acts, write_turn};
use crate::instance::{Owner, Standing};
use crate::program::{Branch, Call, Catch, Due, Ending, Node, Scope, Timeout};

const SECOND: u64 = 1_000_000_000;

fn node(id: &str, step: Step) -> Node {
    Node {
        id: id.to_string(),
        scope: None,
        set: None,
        step,
    }
}

fn scoped(mut node: Node, scope: u32) -> Node {
    node.scope = Some(scope);
    node
}

fn call(unit: &str) -> Work {
    Work::Unit(Call {
        module: "orders".into(),
        unit: unit.into(),
        settings: Vec::new(),
    })
}

fn end() -> Step {
    Step::End {
        ending: Ending::Flow,
    }
}

fn program(nodes: Vec<Node>, scopes: Vec<Scope>) -> Compiled {
    Compiled::new(Program {
        name: "Orders".into(),
        nodes,
        scopes,
        starts: vec![0],
        sets: vec!["Order".into()],
    })
    .expect("holds together")
}

fn arrival(message: u128, values: &[(&str, &str)]) -> Event {
    Event::Arrived(Arrival {
        message,
        values: values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
        keys: Vec::new(),
    })
}

/// A Message whose key in set 0 is `key`.
fn keyed(message: u128, key: &str) -> Event {
    Event::Arrived(Arrival {
        message,
        values: Vec::new(),
        keys: vec![(0, key.to_string())],
    })
}

fn done(node: u32, made: &[u128]) -> Event {
    Event::Done {
        node,
        made: made
            .iter()
            .map(|message| Arrival {
                message: *message,
                values: Vec::new(),
                keys: Vec::new(),
            })
            .collect(),
    }
}

#[test]
fn a_start_initializes_its_set_and_a_wait_takes_only_a_message_of_that_key() {
    let mut start = node("start", Step::Start { next: 1 });
    start.set = Some(0);
    let mut wait = node("Invoice", Step::Wait { next: 2 });
    wait.set = Some(0);
    let flow = program(vec![start, wait, node("end", end())], Vec::new());
    let mut instance = Instance::new();
    assert!(matches!(
        flow.advance(&mut instance, &arrival(1, &[]), 0),
        Err(Refused::Unfit(_))
    ));
    flow.advance(&mut instance, &keyed(1, "A1"), 0)
        .expect("starts");
    assert_eq!(instance.key(0), Some("A1"));
    assert_eq!(
        flow.advance(&mut instance, &keyed(2, "B7"), 0),
        Err(Refused::NotWaiting)
    );
    let acts = flow
        .advance(&mut instance, &keyed(3, "A1"), 0)
        .expect("its key");
    assert_eq!(acts, vec![Act::Ended(Outcome::Completed)]);
}

#[test]
fn a_call_then_a_publish_completes_in_two_turns() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node(
                "Check",
                Step::Call {
                    work: call("Check"),
                    next: 2,
                },
            ),
            node("Send", Step::Publish { next: 3 }),
            node("end", end()),
        ],
        Vec::new(),
    );
    let mut instance = Instance::new();
    let acts = flow
        .advance(&mut instance, &arrival(7, &[]), 0)
        .expect("starts");
    assert!(
        matches!(
            acts.as_slice(),
            [Act::Work {
                node: 1,
                message: 7,
                ..
            }]
        ),
        "{acts:?}"
    );
    let acts = flow
        .advance(&mut instance, &done(1, &[8]), 0)
        .expect("goes on");
    assert_eq!(
        acts,
        vec![Act::Publish { message: 8 }, Act::Ended(Outcome::Completed)]
    );
    assert_eq!(instance.turns, 2);
    assert_eq!(
        flow.advance(&mut instance, &Event::Due, 0),
        Err(Refused::Ended)
    );
}

#[test]
fn a_wait_takes_the_next_message_and_one_nothing_waits_for_is_refused() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node("Invoice", Step::Wait { next: 2 }),
            node("Send", Step::Publish { next: 3 }),
            node("end", end()),
        ],
        Vec::new(),
    );
    let mut instance = Instance::new();
    assert!(
        flow.advance(&mut instance, &arrival(1, &[]), 0)
            .expect("starts")
            .is_empty()
    );
    assert!(instance.waits_for_a_message());
    let acts = flow
        .advance(&mut instance, &arrival(2, &[]), 0)
        .expect("taken");
    assert_eq!(acts[0], Act::Publish { message: 2 });
    let mut busy = Instance::new();
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node(
                "Later",
                Step::Timer {
                    due: Due::After { nanos: SECOND },
                    next: 2,
                },
            ),
            node("end", end()),
        ],
        Vec::new(),
    );
    flow.advance(&mut busy, &arrival(1, &[]), 0)
        .expect("starts");
    let before = busy.clone();
    assert_eq!(
        flow.advance(&mut busy, &arrival(2, &[]), 0),
        Err(Refused::NotWaiting)
    );
    assert_eq!(busy, before, "a refused turn leaves the Instance as it was");
}

#[test]
fn a_decision_takes_the_first_branch_that_holds_else_its_default_else_throws() {
    let decide = |otherwise| {
        program(
            vec![
                node("start", Step::Start { next: 1 }),
                node(
                    "Big?",
                    Step::Decide {
                        branches: vec![Branch {
                            condition: "Amount > 1000".into(),
                            next: 2,
                        }],
                        otherwise,
                    },
                ),
                node("Approve", Step::Publish { next: 3 }),
                node("end", end()),
            ],
            Vec::new(),
        )
    };
    let mut big = Instance::new();
    let acts = decide(None)
        .advance(&mut big, &arrival(1, &[("Amount", "5000")]), 0)
        .expect("runs");
    assert_eq!(acts[0], Act::Publish { message: 1 });
    let mut small = Instance::new();
    let acts = decide(Some(3))
        .advance(&mut small, &arrival(1, &[("Amount", "5")]), 0)
        .expect("runs");
    assert_eq!(acts, vec![Act::Ended(Outcome::Completed)]);
    let mut none = Instance::new();
    let acts = decide(None)
        .advance(&mut none, &arrival(1, &[]), 0)
        .expect("runs");
    let [Act::Ended(Outcome::Failed { code, .. })] = acts.as_slice() else {
        panic!("{acts:?}");
    };
    assert_eq!(code, turn::NO_BRANCH);
}

#[test]
fn a_fork_runs_both_flows_and_the_join_goes_on_once_both_arrived() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node("fork", Step::Fork { next: vec![2, 3] }),
            node(
                "A",
                Step::Call {
                    work: call("A"),
                    next: 4,
                },
            ),
            node(
                "B",
                Step::Call {
                    work: call("B"),
                    next: 4,
                },
            ),
            node("join", Step::Join { from: 2, next: 5 }),
            node("Send", Step::Publish { next: 6 }),
            node("end", end()),
        ],
        Vec::new(),
    );
    let mut instance = Instance::new();
    let acts = flow
        .advance(&mut instance, &arrival(1, &[]), 0)
        .expect("starts");
    assert_eq!(acts.len(), 2, "both flows ask for their work: {acts:?}");
    let acts = flow.advance(&mut instance, &done(2, &[2]), 0).expect("one");
    assert!(acts.is_empty(), "the join waits: {acts:?}");
    let acts = flow
        .advance(&mut instance, &done(3, &[3]), 0)
        .expect("both");
    assert_eq!(acts[0], Act::Publish { message: 3 });
    assert_eq!(acts[1], Act::Ended(Outcome::Completed));
}

#[test]
fn a_timer_is_due_in_the_ledger_and_wakes_its_flow_once_due() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node(
                "Later",
                Step::Timer {
                    due: Due::After { nanos: SECOND },
                    next: 2,
                },
            ),
            node("Send", Step::Publish { next: 3 }),
            node("end", end()),
        ],
        Vec::new(),
    );
    let mut instance = Instance::new();
    flow.advance(&mut instance, &arrival(1, &[]), 100)
        .expect("starts");
    assert_eq!(instance.next_due(), Some(100 + i128::from(SECOND)));
    assert!(
        flow.advance(&mut instance, &Event::Due, 50)
            .expect("early")
            .is_empty()
    );
    let acts = flow
        .advance(&mut instance, &Event::Due, 100 + i128::from(SECOND))
        .expect("due");
    assert_eq!(acts[0], Act::Publish { message: 1 });
}

#[test]
fn a_boundary_timer_interrupts_its_wait_and_leads_on() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            scoped(node("Invoice", Step::Wait { next: 2 }), 0),
            node("Send", Step::Publish { next: 3 }),
            node("end", end()),
            node(
                "Late",
                Step::End {
                    ending: Ending::Error {
                        code: "late".into(),
                    },
                },
            ),
        ],
        vec![Scope {
            id: "Invoice".into(),
            parent: None,
            catches: Vec::new(),
            timeout: Some(Timeout {
                due: Due::After { nanos: SECOND },
                next: 4,
            }),
        }],
    );
    let mut instance = Instance::new();
    flow.advance(&mut instance, &arrival(1, &[]), 0)
        .expect("starts");
    assert_eq!(instance.timers[0].owner, Owner::Scope(0));
    let acts = flow
        .advance(&mut instance, &Event::Due, i128::from(SECOND))
        .expect("due");
    let [Act::Ended(Outcome::Failed { code, .. })] = acts.as_slice() else {
        panic!("{acts:?}");
    };
    assert_eq!(code, "late");
    let mut answered = Instance::new();
    flow.advance(&mut answered, &arrival(1, &[]), 0)
        .expect("starts");
    flow.advance(&mut answered, &arrival(2, &[]), 0)
        .expect("answered in time");
    assert!(
        answered.timers.is_empty(),
        "leaving the scope stops its timer"
    );
}

#[test]
fn an_error_boundary_catches_a_failed_call_by_its_code_as_try_and_catch() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            scoped(
                node(
                    "Charge",
                    Step::Call {
                        work: call("Charge"),
                        next: 2,
                    },
                ),
                0,
            ),
            node("end", end()),
            node("Refund", Step::Publish { next: 2 }),
        ],
        vec![Scope {
            id: "Charge".into(),
            parent: None,
            catches: vec![Catch {
                code: Some("declined".into()),
                next: 3,
            }],
            timeout: None,
        }],
    );
    let threw = |code: &str| Event::Threw {
        node: 1,
        code: code.into(),
        reason: "the card".into(),
    };
    let mut caught = Instance::new();
    flow.advance(&mut caught, &arrival(1, &[]), 0)
        .expect("starts");
    let acts = flow
        .advance(&mut caught, &threw("declined"), 0)
        .expect("caught");
    assert_eq!(
        acts,
        vec![Act::Publish { message: 1 }, Act::Ended(Outcome::Completed)]
    );
    let mut uncaught = Instance::new();
    flow.advance(&mut uncaught, &arrival(1, &[]), 0)
        .expect("starts");
    let acts = flow
        .advance(&mut uncaught, &threw("offline"), 0)
        .expect("thrown");
    assert!(
        matches!(acts.as_slice(), [Act::Ended(Outcome::Failed { .. })]),
        "{acts:?}"
    );
}

#[test]
fn a_batch_combines_by_count_or_once_its_time_is_due() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node(
                "Collect",
                Step::Batch {
                    most: 2,
                    within: Some(Due::After { nanos: SECOND }),
                    next: 2,
                },
            ),
            node("Send", Step::Publish { next: 3 }),
            node("end", end()),
        ],
        Vec::new(),
    );
    let mut counted = Instance::new();
    flow.advance(&mut counted, &arrival(1, &[]), 0)
        .expect("starts");
    flow.advance(&mut counted, &arrival(2, &[]), 0)
        .expect("one");
    let acts = flow
        .advance(&mut counted, &arrival(3, &[]), 0)
        .expect("two");
    assert_eq!(
        acts,
        vec![Act::Combine {
            node: 1,
            messages: vec![2, 3]
        }]
    );
    let acts = flow
        .advance(&mut counted, &done(1, &[9]), 0)
        .expect("combined");
    assert_eq!(acts[0], Act::Publish { message: 9 });
    let mut timed = Instance::new();
    flow.advance(&mut timed, &arrival(1, &[]), 0)
        .expect("starts");
    flow.advance(&mut timed, &arrival(2, &[]), 0).expect("one");
    let acts = flow
        .advance(&mut timed, &Event::Due, i128::from(SECOND))
        .expect("due");
    assert_eq!(
        acts,
        vec![Act::Combine {
            node: 1,
            messages: vec![2]
        }]
    );
}

#[test]
fn a_debatch_publishes_every_part_and_goes_on() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            node(
                "Split",
                Step::Call {
                    work: Work::Debatch(Call {
                        module: "xmip".into(),
                        unit: "Split".into(),
                        settings: Vec::new(),
                    }),
                    next: 2,
                },
            ),
            node("end", end()),
        ],
        Vec::new(),
    );
    let mut instance = Instance::new();
    flow.advance(&mut instance, &arrival(1, &[]), 0)
        .expect("starts");
    let acts = flow
        .advance(&mut instance, &done(1, &[4, 5]), 0)
        .expect("split");
    assert_eq!(
        acts,
        vec![
            Act::Publish { message: 4 },
            Act::Publish { message: 5 },
            Act::Ended(Outcome::Completed)
        ]
    );
}

#[test]
fn a_program_that_does_not_hold_together_is_refused_naming_its_node() {
    let refused = Compiled::new(Program {
        name: "Broken".into(),
        nodes: vec![node("start", Step::Start { next: 9 })],
        scopes: Vec::new(),
        starts: vec![0],
        sets: Vec::new(),
    })
    .err()
    .expect("refused");
    assert!(
        refused[0].contains("'start' leads to node 9"),
        "{refused:?}"
    );
}

#[test]
fn every_form_reads_back_as_it_was_written() {
    let flow = program(
        vec![
            node("start", Step::Start { next: 1 }),
            scoped(
                node(
                    "Charge",
                    Step::Call {
                        work: call("Charge"),
                        next: 2,
                    },
                ),
                0,
            ),
            node("end", end()),
        ],
        vec![Scope {
            id: "Charge".into(),
            parent: None,
            catches: vec![Catch {
                code: None,
                next: 2,
            }],
            timeout: Some(Timeout {
                due: Due::At { unix_nanos: 5 },
                next: 2,
            }),
        }],
    );
    let bytes = form::program(flow.program());
    assert_eq!(
        &form::read::<Program>(&bytes).expect("reads"),
        flow.program()
    );
    let mut instance = Instance::new();
    let event = arrival(1, &[("OrderNo", "A1")]);
    let acts = flow.advance(&mut instance, &event, 3).expect("starts");
    assert_eq!(instance.tokens[0].standing, Standing::Work);
    assert_eq!(
        form::read::<Instance>(&form::instance(&instance)).expect("reads"),
        instance
    );
    let (read, again, now) = read_turn(&write_turn(&instance, &event, 3)).expect("reads");
    assert_eq!((read, again, now), (instance.clone(), event, 3));
    let answer = Ok((instance, acts));
    assert_eq!(read_acts(&write_acts(&answer)).expect("reads"), answer);
    let refused = Err(Refused::Unfit("why".into()));
    assert_eq!(read_acts(&write_acts(&refused)).expect("reads"), refused);
    let mut later = form::program(flow.program());
    later[0] = 2;
    assert!(
        form::read::<Program>(&later).is_err(),
        "another version is refused"
    );
}
