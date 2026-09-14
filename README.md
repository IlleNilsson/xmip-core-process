# xmip-core-process

Xmip Process execution: an `XmipProcess` runs a step and answers with a
`ProcessOutcome` — a Message, no Message, or waiting for something named —
and a `ProcessRegistry` finds the Xmip Process a Subscription starts. Each
scripting host is a technology under this repository.

An Xmip Process is a Definition started by a Subscription, not an operating
system process (`doc/terminology.md`). It does not receive external Streams
and does not deliver to external targets; its state belongs to the cluster,
never to a thread or a node.

`doc/architecture/runtime-model.md` section 22 governs it, and how one
Instance handles many Messages over time is `doc/process-instances.md` beside
this file; `architecture.toml` carries the maturity.
