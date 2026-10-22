# xmip-core-process-bpmn

BPMN 2.0, the default Work Process design format, a technology of
[xmip-core-process](https://github.com/IlleNilsson/xmip-core-process)
(ADR-0066, amendment 2026-10-22). The owner, 2026-10-22: *there are BPEL
solutions out there, no need to invent the wheel unless we have to*, *go with
the best functionality agreeing with Xmip licensing*, and of BPMN 2.0, *It can
not be the only choice, just the default one.*

`Bpmn` reads a `.bpmn` design — OMG's BPMN 2.0 XML with its BPMN DI, as
every BPMN tool writes it — and compiles its one executable process to the
Work Process program. What Xmip adds is in BPMN's own extension elements, in
the namespace `urn:xmip:process:1`, so other tools open the design and keep
them: `xmip:call module unit` and `xmip:setting` on a service task, the unit
it calls; `xmip:debatch` on a service task, the splitting unit; `xmip:batch
most within` on a receive task; and `xmip:correlate set` on a start or a
receive, the cluster's correlation set it takes Messages by. A condition is
written in Xmip's expression language, `language="urn:xmip:expression"`.

**What it compiles**: start, end (none, terminate, error, message), task with
assignments, service, send and receive tasks, message and timer catch
events, message throw events, exclusive and parallel gateways, embedded
subprocesses, and interrupting timer and error boundary events — try and
catch. **What it refuses, in words naming the element**: compensation,
transactions and cancel events; user and manual tasks; business-rule and
script tasks; call activities; inclusive, complex and event-based gateways;
signal, escalation, conditional and link events; event subprocesses; loops
and multi-instance; ad hoc subprocesses; choreographies; and any element it
does not read (`refused.rs`). The owner, 2026-10-22: *Compensation can mostly
be handled with try/catch. Rules can be handled in code. Human tasks, waiting
for an email, text or a phone call can wait.*

**Offline** (the owner, 2026-10-22: *We can't be dependent on resources
online*): the XML Schemas of OMG's BPMN 2.0.2 publication — `BPMN20.xsd`,
`Semantic.xsd`, `BPMNDI.xsd`, `DC.xsd` and `DI.xsd` — ship in `schema/`,
unmodified, as OMG publishes them under its specification license, which
grants a fully paid-up, non-exclusive, perpetual, worldwide license to build
software on the specification. A namespace is a name, never fetched; a
`schemaLocation` for a namespace shipped here is answered by the shipped
schema; any other location naming a network resource, and any `bpmn:import`
of one, is refused before anything is asked; a document type declaration is
refused. Nothing here opens a connection.

**The XML reader is quick-xml** 0.42 (MIT), with no default features; its one
dependency is memchr (MIT or Unlicense). Neither holds network code.

`diagram` reads the design as the VS Code designer draws it and `edit` makes
its edits as text splices, so a design's foreign extensions and comments are
kept as written. Its tests (`tests/`) compile the fixture design, run it, and
refuse each element and every remote reference.
