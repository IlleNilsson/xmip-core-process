# xmip-core-process

Work Process execution: a `WorkProcess` runs a step and answers with a
`ProcessOutcome` — a Message, no Message, or waiting for something named —
or, where it could not run, with `xcore::Failure`, the estate's one retryable
failure; and a `ProcessRegistry` finds the Work Process a Subscription starts.
These are the traits; nothing implements them outside this crate's tests,
nothing compiles a design into one, and no node runs one
([decided, not built](../../../../doc/architecture/estate-map.md#process-execution)): a Journey to a Work
Process ends saying no runtime runs it yet. What a design compiles into is
ADR-0066's: a native Module the node loads.

A Work Process is a Definition started by a Subscription, and runs
in-process of an Xmip Host Service or an Xmip Host Subprocess
(`doc/terminology.md`). It does not receive external Streams
and does not deliver to external targets; its state belongs to the cluster,
never to a thread or a node.

`doc/architecture/runtime-model.md` section 22 governs it, and how one
Instance handles many Messages over time is `doc/process-instances.md` beside
this file; `architecture.toml` carries the maturity.
