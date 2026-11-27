# Does an agent choose a better first move?

The short commit IDs in the benchmark reports identify pre-release experiments. This public snapshot includes their inputs, recorded outputs and graders. The earlier Git history is private.

## Discovery

The original Instinct bank contains a discovery about word overlap as a proxy for whether a retrieved lesson helped: joining independent outcome labels to the same examples exposed the misleading signal. A later exposure audit supplied a related method: distinguish selected, delivered and timely events before interpreting outcomes. The public examples reconstruct these ideas on invented data. No historical sessions or bank exports are bundled.

A fresh Claude Code session received `discovery.txt` and returned `discovery.json`. Its recorded entry is used unchanged across arms. For example, the model inferred self-selection; the input does not establish the selection mechanism. The skill instructs the next agent to check the evidence and conditions before using the method.

## Single-decision transfer

Build from the repository root with Cargo. To reproduce the live benchmark, install jq, GNU timeout, mktemp and the selected authenticated model CLI. Each run launches six fresh decision sessions; framed mode also uses two fresh query-formulation calls. Model calls use your account.

```sh
cargo build --release --locked --offline --bin instinct --example transfer
cargo run --release --locked --offline --example transfer -- claude haiku framed
cargo run --release --locked --offline --example transfer -- codex gpt-6.2-sol prepared
```

Use `raw`, `framed` or `prepared` as the last argument. Raw uses the question verbatim. Framed asks the same model for a situation description before retrieval. Prepared banks the portable trigger phrases in `transfer-cues.txt` alongside the discovery, then looks up the fresh questions directly. Prompts request an extract letter and count. The verifier checks the first choice and the complete answer separately. Errors remain visible. The live arm includes retrieval and any query-formulation time. Static receives the discovery's method on every task, and base receives empty context. Raw and framed use the ferry pair. Prepared uses a pharmacy and a coupon trial. One case in each pair asks about exposure, the other asks about assignment regardless of exposure.

### Observed pilot

| Comparison | Unaided exact | Static exact | Live exact | Interpretation |
|---|---:|---:|---:|---|
| Raw, Claude | 1/2 | 1/2 | 1/2 | Live missed the relevant ferry case; topical vocabulary alone was insufficient. |
| Framed, Claude + Codex | 4/4 | 4/4 | 4/4 | All four live calls retrieved the method. Framing raised end-to-end latency to 9.7–13.9 s; unaided took 3.0–5.2 s. |
| Prepared, Claude + Codex | 4/4 | 4/4 | 4/4 | Local lookup took 1–2 ms but returned no method on these four live calls. Correct answers here earn no credit for Instinct. |

TSV files contain individual answers, context sizes, choice accuracy, latency and exact checks. The framed run revisited the development cases after diagnosing raw recall. One trial per condition measures feasibility. Token counts, repeated-attempt savings and calibrated confidence cannot be inferred from a one-shot forced choice. The example runner uses separate stateless calls for framing and deciding; a normal agent can formulate the query during its existing turn. Timings include process startup. These runs used lexical retrieval, Claude Code 2.1.332 and Codex CLI 0.172.0.

## Artifact task

The `field/` case transfers the original proxy-evaluation method to a print shop. Give each fresh read-only session only the two CSV files in its working directory. Use `task.txt` as the user prompt and append `Prior experience:` with either an empty string, the HOW from `lesson.json`, or the output of live top-one recall for that prompt from an isolated bank containing the lesson. For the clean Claude Code run, allow only the Read tool, disable hooks and MCP, use an empty settings source, and capture each actual read, elapsed time and final output. Independently join receipts to scores by job ID and enumerate every rejected/accepted pair for each candidate column. The correct output is `retries 9 9`. Punctuation between the name and counts is immaterial. All three Claude sessions read both files and calculated the correct answer. All added prose, so all three failed the exact-output requirement. Unaided took 4.99 s, the static method 6.41 s, and live recall 5.81 s. The task demonstrated successful use of a transferable method with no observed gain over the unaided baseline. Individual outputs are in `field/claude-results.json`. An earlier Codex artifact run read host startup guidance; it was excluded from the comparison. The initial Claude pilot omitted filenames and tried nonexistent files; filenames were then supplied identically to every arm.

The single-entry store isolates transfer of one experience; ranking quality among competing experiences remains unmeasured by this pilot. A speed or quality claim requires more independent tasks, signals and repetitions.
