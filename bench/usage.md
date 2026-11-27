# When does experience improve an agent’s decision?

Koščák Labs audited the available historical Instinct installation through 2026-11-27. These aggregate measurements cover the original system across versions. The public release has a separate [clean-install transfer benchmark](README.md).

## Everyday use

An automated judge assigned 925 labels. Keeping the latest label per session, injection timestamp and discovery ID gave 919 distinct recall events across 139 sessions and 355 discoveries. It classified 187 as helpful, 77 as guarding against a mistake, 619 as irrelevant, 32 as unused and four as misleading. The positive judgments covered 101 discoveries. A guarded judgment is the judge's interpretation of the interaction; the counterfactual failure was unobserved.

That yields **264/919 (28.7%) judged helpful or protective**. These were selected, judgeable cases. The proportion is unsuitable as an estimate of all sessions, all users or the public edition. October contributed 253 positive judgments among 855 cases; November contributed 11 among 64. Versions, coverage and workloads changed. These counts give little basis for estimating a trend.

More selective matching looks promising. Of 116 judged cases with logged similarity at least 0.80, 73 were positive (62.9%). Applying that cutoff retrospectively would retain just 73 of the 264 positive labels. Porting the cutoff to the public engine would need prospective evaluation.

## Could the agent apply what it recalled?

The historical application benchmark used 20 scenarios and a deterministic expected-marker grader. For each model we matched base and live-retrieval answers on probe ID, kept the corrected grader (expect-v3), and counted a score of at least one on a 0–2 scale. One scenario had a malformed marker list; Claude also lacked valid paired scores on two others.

| Model | Unaided | With live retrieval | Full-credit results, unaided → live |
|---|---:|---:|---:|
| Gemma 4 31B cloud | 8/19 | 14/19 | 8 → 13 |
| Claude Haiku | 13/17 | 17/17 | 13 → 16 |
| Codex | 16/19 | 17/19 | 16 → 16 |

The test checked whether an answer contained expected procedure markers. Its best-supported use case is bringing a previously learned procedure to a model that would otherwise miss it. The test did not verify execution of those procedures. The historical Codex environment already exposed related reflex skills.

## What we can claim about completed work

The operational scoreboard associated useful labels with fewer subsequent tool errors (0.59 versus 0.98 on its 510-label comparison). Task difficulty, repeated sessions and the judge's access to downstream behavior limit causal interpretation. The historical controlled correction-rate audit was inconclusive. At inspection the outcome ledger contained 3,778 distinct event IDs with model consumption marked unproven and task success unset throughout.

In the public artifact pilot every arm found the correct score. Claude took 4.99 seconds unaided, 5.81 seconds with live recall and 6.41 seconds with a static method. All three missed the exact-output requirement. A bank can demonstrably surface useful experience; a general productivity gain still requires independent replications on harder tasks.

**Try it first where experience is scarce:** a recurring diagnostic choice, a project-specific measurement, or a previously successful investigation whose method a fresh agent would struggle to rediscover. Record the first action, the independently checked outcome, the cost of lookup and any misleading recalls. A user can share those measurements using invented or public data while keeping their actual bank private.
