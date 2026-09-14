# Xmip Process instances

What an Xmip Process is, that its state belongs to the cluster rather than to
any thread or node, and that a Subscription starts it while a Correlation Rule
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

Xmip Process Instance
    handles Message A
    waits
    handles Message B
    waits
    handles Message C
```

## Rule

```text
Message = one immutable Stream reference
Xmip Process Instance = may handle many Messages over time
```
