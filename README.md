# xmip-core-process

Xmip Work Processes, designed in VS Code, compiled at design time and run
when called upon (ADR-0066, amendment 2026-10-22). The owner, 2026-10-22:
*I would like a Work Process to be designed in VS Code, but at the end of the
day it is a compiled piece executed when called upon.*

- **`design`** — a design format is a technology behind `DesignFormat`: it
  reads a design and compiles it to the one `Program`. BPMN 2.0 is the
  default (`xmip-core-process-bpmn`, mounted at `bpmn/`); a design names its
  format, `bpmn` where it names none. Nothing is fetched to compile a design:
  `remote` refuses a location on a network, and `LocalResources` answers
  files beside the design and nothing else.
- **`program`** — the compiled form: a graph of typed steps — a start, work
  on a Message (a unit's call, an Assignment, a split), a publish, a wait for
  the next Message of a correlation set's key, a Batch, a timer, a decision
  in Xmip's expression language, a fork and a join, an end — and the scopes
  their boundary timers and catches stand in. It never names a design format.
- **`generate`** and **`export`** — the Work Process unit a program is built
  into: a Rust crate carrying the program's bytes and exporting them with
  `export_work_process!` through `xmip_module.h` section 13, built as a
  `cdylib`, and the `[[modules]]` entry declaring it (`capability =
  "work-process"`, `stages = ["process"]`, `offline = true`). A unit written by
  hand exports its own `Flow` the same way.
- **`flow`** — what a unit is to the runtime: a `Flow`, advanced one turn at a
  time over an `Instance` the Ledger keeps. A turn takes one event — a
  Message arrived, the work it asked for done or thrown, its time due — and
  answers the acts the runtime carries out. `Compiled` runs a program; an
  error no scope catches fails the Instance (`no-branch`, `loops` and a unit's
  own codes).
- **`form`** — the binary forms a program, an Instance and a turn cross in,
  each opening with its version.

The runtime's Work Process step (`xmip-core-runtime`'s `work_step`) claims
each Journey bound for a Work Process, finds its Instance by its correlation
key, takes a turn, and writes it all in one hand-on; an Instance waiting
holds no thread (`doc/architecture/runtime-model.md` section 22). How one
Instance handles many Messages over time is `doc/process-instances.md`, and a
first design from start to finish is `doc/getting-started.md`.

Dependencies: Xmip's own crates only — `xmip-core`, `xmip-core-path` (the
expression language), `xmip-core-assign`, `xmip-core-library-codec` and
`xmip-core-abi`. `architecture.toml` carries the maturity.
