# Xmip Process instances

A Message references one immutable stream.

A Message is not multi-stream. There is no Message Section model in Xmip.

When content changes, an assignment or transformation creates a new Message and, when needed, a new stream. Existing Messages and their referenced streams remain immutable.

## Long-running Xmip Processes

An Xmip Process may live for a period of time and handle multiple Messages over time.

Examples:

```text
procurement process
onboarding process
offboarding process
```

Such a Process is represented by an Xmip Process instance.

A Process instance can correlate Messages that arrive at different times. Each Message still references one immutable stream.

```text
Message A -> Stream A
Message B -> Stream B
Message C -> Stream C

Xmip Process instance
    handles Message A
    waits
    handles Message B
    waits
    handles Message C
```

The multi-message behavior belongs to the Xmip Process instance, not to the Message.

## Rule

```text
Message = one immutable stream reference
Xmip Process instance = may handle many Messages over time
```
