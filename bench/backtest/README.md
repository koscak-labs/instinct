# Does experience help Codex make its next decision?

On six reconstructed decisions, Codex got every answer right with or without Instinct. Retrieved experience let it skip four reference lookups across two runs. Supplying every method achieved the same result and used 4.1 times as many context bytes.

## Fresh Codex sessions

We fixed the cases and expected outputs at commit `3be0747` before running the model. Three methods came from earlier engineering work: executable grading, reconciling event log replicas, and checking an expected rejection. Two other methods competed in the same bank. The six decisions included a changed streaming contract, a current-state snapshot and an unrelated calculation. Each run saw invented public records. Some cue words match the bank closely; this is a reconstruction of historical decision types, not a blind replay of original user tasks.

| Experience supplied | Exact answers | Reference reads | Median time to checked answer | Experience bytes across 12 decisions |
|---|---:|---:|---:|---:|
| None | 12/12 | 10 | 12.79 s | 0 |
| Five static methods | 12/12 | 6 | 8.30 s | 23,568 |
| Live Instinct | 12/12 | 6 | 9.89 s | 5,744 |

Codex skipped the reference on both repeats of the replica and snapshot decisions when either form of experience was available. On the rejection task it still asked for the contract: the bank supplied the method, while the contract specified the expected exit code. Instinct returned an empty experience list for the unrelated calculation. No answer required an error-repair turn.

We used Codex CLI 0.172.0 with gpt-6.2-astra in ephemeral sessions and an empty working directory. Every arm could request the same authoritative reference using READ. A request cost another model call, including process startup. The CLI used the read-only sandbox, ignored user config, and the runner rejected traces containing external tool actions. Each arm ran the six tasks once forward and once in reverse. Arms ran concurrently under shared load; one static decision took 52.41 s. The reference choices are more informative than the small timing sample. This is evidence that experience can save a lookup in this harness. The design cannot estimate a general speedup or an accuracy lift at this ceiling.

## Backwards replay against historical use

We reconstructed the discovery bank as it would have been available before each recorded prompt, taking only surviving ledger events stamped strictly earlier than the prompt. We excluded embedding updates and vectors, set an isolated store, passed the original recorded query text, and used the public `decide` interface with its normal two-method limit. Inputs and outputs stayed on the originating machine. Historic scope and environment metadata could not all be reconstructed. Labels indicate what the earlier judge called helpful or protective; alternative results usually have no label.

- 536 distinct nonblank session/time/prompt combinations, 2026-10-15 through 2026-11-25. Thirteen of 919 deduplicated label records lacked prompt text.
- The lexical engine returned something on 479/536 prompts. Of 264 previously helpful or protective labeled recalls, **123 were recovered** (88 helpful and 35 protective). This is 46.6% recovery of those specific lessons.
- Of 937 returned IDs across contexts, 186 overlapped a judged ID for the same prompt. Among the overlapping IDs, 123 were positive, 57 irrelevant and six unused. The remaining 751 were unjudged for that prompt. A precision estimate for all returned methods would require labeling those alternatives.
- Replaying the quieting rule with only feedback known *before* each injection would retain 262 of the 264 positives while suppressing 33 of 919 deliveries. An offline similarity cutoff at 0.75 retained 146 positives while suppressing 625 deliveries. These policy simulations use logged historical scores; they do not evaluate a newly trained ranker or unseen user population.

The full-sized banks also exposed unnecessary lexical scoring cost. We changed BM25 to count matching query terms in one pass while retaining document lengths, Unicode normalization behavior and term ordering. On every sixteenth eligible context in chronological order (34 queries), old and new executable outputs were byte-identical. Median cold-process lookup went from 129.0 to 47.7 ms; sampled p95 from 267.5 to 92.1 ms. Binary order alternated. Timings used local files with optional vectors omitted and include process startup; they do not predict performance with large embedded banks.

## Applying an instinct during development

We asked the running engine how to investigate a suspicious executable grader. It returned the full-output and exit-status method. We used it to run the archived and repaired graders against real compiled programs offline. The historical graders gave credit to all 11 incorrect executions; the repaired graders rejected those and preserved all seven valid executions. The replay has seven grouped test cases. This confirms that the method points to an actionable distinction. The repair already existed before this release; the replay cannot attribute its discovery to Instinct.

## Reproduce the public comparison

From the source tree, with Rust, jq, GNU timeout, mktemp, and an authenticated Codex CLI:

```sh
cargo build --release --locked --offline --bin instinct --example backtest
cargo run --release --locked --offline --example backtest -- live gpt-6.2-astra
```

Use `base` and `static` for the comparison arms; append `reverse` to repeat in reverse order. Each model run consumes account usage. `instinct demo` uses no model account. The fixtures, runner and six TSV files in this directory contain the public cases, decision conditions and raw answers. An infrastructure failure is explicitly marked in the grade column.
