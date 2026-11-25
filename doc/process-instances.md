# Work Process instances

What a Work Process is, that its state belongs to the cluster rather than to
any thread or node, and that a Subscription starts it while a Correlation
resumes it, are the estate's: `doc/architecture/runtime-model.md` section 22.
What a Message is, its Sections and its immutability are `doc/terminology.md`,
*Message and Section*. This document says the one thing left to say beside the
code: how a long-running Process Instance relates to the Messages it handles.

## One Instance, many Messages

A Process that lives for a period of time — procurement, onboarding,
offboarding — is one Process Instance handling Messages that arrive at
different times. Each Message still references one immutable Stream; the
multi-Message behavior belongs to the Instance, not to the Message.

```text
Message A -> Stream A
Message B -> Stream B
Message C -> Stream C

Work Process Instance
    handles Message A
    waits
    handles Message B
    waits
    handles Message C
```

## What the Instance keeps between Messages

The owner, 2026-11-25: *A Work Process is like an Orchestration in BizTalk.
So it is aware via subscriptions, even when running or not.* What an
orchestration keeps in its variables an Instance keeps as **values of its
own**: its design declares them — in BPMN 2.0 its data objects — and a task
sets them, each computed in Xmip's expression language from the current
Message and the values already kept. They outlast every Message the Instance
takes, where what is read of a Message is replaced by the next. An order
delivered in parts counts what was ordered, delivered and accepted, and its
flow loops back to its wait until every line is accepted (ADR-0066,
amendment 2026-11-25; `bpmn/tests/design/order-in-parts.bpmn`):

```text
Order        Ordered = Lines, Delivered = 0, Accepted = 0     waits
Delivery 2   Delivered = Delivered + Lines        -> 2         waits
Accept 2     Accepted = Accepted + Lines          -> 2         waits
Delivery 1   Delivered                            -> 3         waits
Accept 1     Accepted                             -> 3         confirmed, ends
```

It keeps its history of Messages too, each with the Journey that carried it,
and every operator surface lists it: running, waiting for a Message of its
key until its deadline, failed with its code and why, dismissed or
completed. A turn that fails it is written as the Instance stood before
that turn, so an operator's Retry of that Message takes it again from there
and a Dismiss ends it; meanwhile every other Message of its key waits.

## Rule

```text
Message = one immutable Stream reference
Work Process Instance = may handle many Messages over time,
                        keeping its own values between them
```
