//! What this version of the BPMN design format does not compile, each
//! with the reason a developer reads (ADR-0066, amendment 2026-10-22).
//!
//! The owner, 2026-10-22: *Compensation can mostly be handled with
//! try/catch. Rules can be handled in code. Human tasks, waiting for an
//! email, text or a phone call can wait.* An element of the model's
//! namespace this list names is refused; one it does not name and the
//! compiler does not read either is refused too, as unknown, so nothing a
//! designer drew is silently left out of what runs.

/// The elements refused, each with why.
pub const REFUSED: [(&str, &str); 22] = [
    ("userTask", HUMAN),
    ("manualTask", HUMAN),
    (
        "businessRuleTask",
        "a rule is code: call a unit from a service task",
    ),
    (
        "scriptTask",
        "a node never interprets a script; call a compiled unit from a service task",
    ),
    (
        "callActivity",
        "calling another process is not supported in this version",
    ),
    ("transaction", COMPENSATION),
    ("compensateEventDefinition", COMPENSATION),
    (
        "inclusiveGateway",
        "an inclusive gateway is not supported in this version",
    ),
    (
        "complexGateway",
        "a complex gateway is not supported in this version",
    ),
    (
        "eventBasedGateway",
        "an event-based gateway is not supported in this version",
    ),
    (
        "signalEventDefinition",
        "a signal is not supported in this version",
    ),
    (
        "escalationEventDefinition",
        "an escalation is not supported in this version",
    ),
    (
        "conditionalEventDefinition",
        "a conditional event is not supported in this version",
    ),
    (
        "linkEventDefinition",
        "a link event is not supported in this version",
    ),
    ("cancelEventDefinition", COMPENSATION),
    ("multiInstanceLoopCharacteristics", LOOP),
    ("standardLoopCharacteristics", LOOP),
    (
        "adHocSubProcess",
        "an ad hoc subprocess is not supported in this version",
    ),
    ("choreography", CHOREOGRAPHY),
    ("choreographyTask", CHOREOGRAPHY),
    ("subChoreography", CHOREOGRAPHY),
    (
        "globalTask",
        "a global task is not supported in this version",
    ),
];

const HUMAN: &str = "a task for a person waits for a later version: Xmip runs no human \
                     workflow yet";

const COMPENSATION: &str = "compensation and transactions are not supported: catch an \
                            error with an error boundary event, as try and catch";

const LOOP: &str = "a loop on an activity is not supported in this version; split the \
                    Message with a Debatch instead";

const CHOREOGRAPHY: &str = "a choreography is a model of an exchange, not a process that runs";

/// Why `name` is refused, where this list names it.
#[must_use]
pub fn why(name: &str) -> Option<&'static str> {
    REFUSED
        .iter()
        .find(|(refused, _)| *refused == name)
        .map(|(_, why)| *why)
}

/// What a flow element is read as, where the compiler reads it.
pub const READ: [&str; 15] = [
    "startEvent",
    "endEvent",
    "task",
    "serviceTask",
    "sendTask",
    "receiveTask",
    "intermediateCatchEvent",
    "intermediateThrowEvent",
    "boundaryEvent",
    "exclusiveGateway",
    "parallelGateway",
    "subProcess",
    "sequenceFlow",
    "extensionElements",
    "documentation",
];

/// What a process may hold that does not run and is left as drawn: data,
/// notes, lanes and what the diagram groups.
pub const KEPT: [&str; 11] = [
    "dataObject",
    "dataObjectReference",
    "dataStoreReference",
    "textAnnotation",
    "association",
    "group",
    "laneSet",
    "ioSpecification",
    "property",
    "incoming",
    "outgoing",
];
