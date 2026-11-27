# Where AI agents can reuse a hard-won discovery

A method earns its place in Instinct when a future agent can recognize the situation, take a better first action, and recognize an exception. The best candidates recur often enough that someone has already paid to rediscover the answer.

## When you start using an agent

| What you find yourself saying | What experience changes | What would change the next move |
|---|---|---|
| “I already showed you how I want this.” | Bring the previously accepted way of doing that task into a fresh chat. | A new brief can override a familiar format. |
| “I asked for one edit. Why did you rewrite the rest?” | Reuse the kind of edit the owner accepted on approved material. | An explicit request for a rewrite gives the agent more room. |
| “You said that was fixed twice already.” | Try the check that separated apparent success from the result the person actually saw. | Once the recipient confirms the result, stop retrying. |

[Play the three examples](https://koscak-labs.github.io/instinct/) · [Inspect both harnesses and all three comparison conditions](../bench/first-use/README.md). Earlier everyday examples are available in the [night out, gift page and shop replay](../bench/everyday/README.md).

## Engineering cases you can inspect today

| Decision | Experience to reuse | When the decision changes |
|---|---|---|
| Reconcile overlapping transaction logs | Keep distinct events for the same account and deduplicate identical replayed events. | With current-state snapshots, take the newest revision. |
| Diagnose false positives in an executable grader | Inspect the exit status, capture completeness and complete output. | A documented prefix-stream protocol has a different acceptance condition. |
| Validate a negative control in CI | Look for the specific rejection and run the valid control. | An ordinary readiness check still needs to succeed. |
| Choose an evaluation metric | Join candidate scores to independent outcome labels on the same examples. | If the labels are missing, collect a labeled sample before choosing the metric. |
| Attribute an outcome to a recommendation | Compare delivery and decision times per event. | A randomized assignment analysis includes everyone assigned, even if they never saw it. |

Inspect [the decision backtest](../bench/backtest/README.md) and [the transfer and file-analysis experiments](../bench/README.md) for case-level results. They include decisions where the unaided agent did just as well.

## Further applications worth testing

| Audience and use case | What the agent might learn | Independent check |
|---|---|---|
| Robotics engineers investigating control latency | Check whether command and acknowledgement timestamps share a clock before interpreting delay. | Replay a trace with a known clock offset and one with a real actuator delay. The current demo illustrates the method using synthetic experience. |
| Data engineers debugging a failed migration | When a join multiplies rows, check key uniqueness on each side before editing the transform. | Seed a duplicate key and a valid one-to-many relationship; preserve the intended cardinality. |
| Reliability engineers investigating stale reads | Compare origin and cache revisions for the same object before restarting services. | Replay stale-cache and delayed-origin incidents and compare first diagnostic actions. |
| Researchers evaluating a model | Split by the entity or time boundary that the deployed model will actually encounter. | Compare an intentionally leaky split with a genuinely independent holdout. |
| Product analysts diagnosing a conversion drop | Check whether an event name or denominator changed across app versions. | Replay an instrumentation change and a genuine behavior change. |
| Support agents triaging repeated incidents | Reuse the question that previously distinguished two superficially identical symptoms. | Check against independently resolved tickets and count unnecessary escalations. |
| Engineers benchmarking inference | Separate time to the first usable result from throughput after generation starts. | Compare cold and warm runs on the same hardware with identical workloads. |
| QA engineers diagnosing an intermittent race | Recreate the ordering that caused the failure before changing retry limits. | Force both event orderings and verify the requested state in each. |

These are proposed experiments. A team can bring its own domain knowledge, freeze an answer or externally observable outcome, then compare unaided, static-instruction and Instinct runs.

## How this fits agent architectures

**Procedural memory** describes what an agent has learned about how to act. Instinct adds a situation, a method, evidence and a condition for reconsidering. **Experience retrieval** is how the engine brings that method into a new decision. A host can call the Rust CLI, consume the JSON decision interface, or connect its MCP server. [Integration guide](../README.md)
