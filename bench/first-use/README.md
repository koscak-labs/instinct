# Does a correction help the next assistant?

Tess had already worked out how to write an update that got her client to act. We tested whether a fresh assistant could use the method with new facts, even after switching tools.

## What happened

| Harness | No lesson | All three lessons | Instinct retrieval |
|---|---:|---:|---:|
| Codex, exact results | 5/6 | 6/6 | 6/6 |
| Codex, extra notes requested | 2 | 1 | 1 |
| Claude Code, exact results | 5/6 | 5/6 | 5/6 |
| Claude Code, extra notes requested | 1 | 1 | 1 |

On the client update, unaided Codex asked for the convention and returned the right words with an extra final period. Given the retrieved lesson, it returned the accepted format immediately. Counting the period as acceptable would move unaided Codex to 6/6. Unaided Claude wrote a full email. With Instinct it used the short action format and added a recap Tess had left out. With all three notes it matched Tess’s update exactly.

For the typo and the explicit rewrite, both agents already produced the requested copy unaided. On the broken link, every condition requested the reader’s observation. Every answer selected the right repair. Claude with all three notes added an explanation where only the action ID was requested. When the reader confirmed the new edition, all conditions selected “done.”

These six deliberately constructed tasks were run once per condition and harness. Exact grading rewards literal format compliance. A reference read models an extra information request. The file task measures a repair decision from supplied evidence. These runs did not connect to a real reader or repair a file.

## What was learned, and how it was tested

1. We froze three fictional prior interactions and six new tasks with their checks at commit `9d561d6`. The cases include an explicit override for each earlier method.
2. Claude Code (`claude-haiku-5-5`) saw only the prior interactions and extracted three discoveries. We parsed its JSON, retaining its wording. The lessons and runner were recorded at `0285232` before the comparisons. [Discovery request](discovery.txt) · [Claude’s extracted methods](discoveries.json)
3. Each new request went to a fresh session with no lessons, all three lessons, or the packet returned by the public `decide` interface from an isolated bank. Each condition got the same user request, facts and optional reference note. The answer key stayed with the grader. The fresh sessions used Codex CLI with `gpt-6.2-astra` and Claude Code with `claude-haiku-5-5`. [Requests, observations and exact checks](cases.json)
4. Each answer was compared before display formatting. Saved TSV records flatten line breaks and tabs in answers for display. An empty live packet on the explicit rewrite left the agent to follow the current brief. Across the six tasks the live packets totaled 4,365 bytes per harness, compared with 13,776 for always including the full bank.

[Codex unaided](codex-base.tsv) · [Codex with all notes](codex-static.tsv) · [Codex with Instinct](codex-live.tsv) · [Claude unaided](claude-base.tsv) · [Claude with all notes](claude-static.tsv) · [Claude with Instinct](claude-live.tsv)

## Why these problems

In Stack Overflow’s 2025 survey, 66% of developers answering its question about frustrations selected almost-right solutions. A CHI study of people new to prompt design documented difficulties diagnosing and systematically testing prompt behavior. Multi-turn experiments also found that models can persist with a wrong early assumption. These observations motivated our examples. They cover different populations and do not establish a universal ranking of beginner complaints. [Stack Overflow survey](https://survey.stackoverflow.co/2025/ai) · [CHI user study](https://people.eecs.berkeley.edu/~bjoern/papers/zamfirescu-johnny-chi2023.pdf) · [Multi-turn research](https://www.microsoft.com/en-us/research/publication/llms-get-lost-in-multi-turn-conversation/)

## Reproduce

Build with Rust and the dependencies listed in the [backtest guide](../backtest/README.md). The demo command exercises the saved discovery bank offline. The loop requires authenticated Codex and Claude CLIs, jq and GNU timeout. It runs 36 fresh decisions, with an additional model call whenever a decision requests the note.

```sh
cargo build --target-dir target --release --locked --offline --bin instinct --example backtest
target/release/examples/backtest demo unused first-use
for arm in base static live; do
  target/release/examples/backtest "$arm" gpt-6.2-astra first-use
  target/release/examples/backtest "$arm" claude-haiku-5-5 first-use claude
done
```
