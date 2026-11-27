---
name: instinct
description: Learn a reusable method from an observed success, correction, or failed attempt. Use it to choose a first move on a similar task, especially when switching agents, repeating a fix, or deciding whether yesterday’s approach still fits.
license: Apache-2.0
metadata:
  author: Filip Koščák
  version: "0.1.0"
---

# Instinct

Use what experience has taught you to take the next worthwhile action. Sometimes that means committing to a familiar move. Sometimes the experienced move is to explore.

## At the moment of choice

1. Identify the live decision. What are you trying to change, what have you observed, and which action are you hesitating between? Describe the situation in the language of the decision. A file name or task label alone is a poor cue.
2. Bring the most relevant previous methods to mind. Use experience already supplied by the harness, or ask its Instinct adapter once. A custom host can call `instinct decide --q '...'`. The response includes the method, what happened before, evidence, reuse history and any condition for reconsidering. The MCP equivalent for retrieving methods is `instinct_recall`. Use the current project scope for project-specific work. Empty recall is a normal starting point for discovery.
3. Decide. When the cues fit, the evidence supports the method, and the action is within your authority, take it. Skip redundant scouting. If two experiences point in different directions, identify the concrete condition that distinguishes them. Pay for a small probe only when its result could change your move. With no relevant precedent, make the best first-principles move available.
4. Observe what happens while pursuing the task. Change course when the predicted signal fails to appear. Keep the user's objective in view; narrate the reasoning only when it helps them understand a consequential choice.

**On a first task**, start from the request and available evidence. One accepted correction can explain what to try next time. When the user has already given you enough to make a reversible change, make it. If a missing fact would lead to a different action, check it with an available tool or ask the one question that resolves it.

**Example.** A reader still sees the old file after two exports. Last time this happened, opening the exact link as the reader exposed a stale target. Do that check if available, or ask for what the reader sees. Relink only if the check identifies the stale target. Once the reader confirms the new contents, the task is complete.

## When something pays off

Bank a discovery when the agent itself finds a reusable method: a discriminating test, a productive shortcut, a surprising strategy, or a failed approach whose boundary it can now explain. The user does not have to issue a correction. Use `instinct_bank` or `instinct bank` with:

- **when** — recognizable cues and the condition that changes the decision;
- **found** — the observed result;
- **how** — the method that uncovered it, including what to check first;
- **evidence** — an actual measurement, test result, or reference;
- **reconsider** — an observable exception that would make a different first move better, if known;
- **title** — a short name for the discovery.

When an observed convention is the default, bank what to do on an ordinary repeat and the specific change that would make it wrong. Keep speculative exceptions out of the lesson. When the user asks for the usual approach, apply the established default unless the current evidence triggers its exception.

When a user accepts a specific format after correcting a draft, preserve the smallest approved example and the change that made it work. Keep the convention scoped to that user and task.

If you can name another situation where the same method applies, add a few short semicolon-separated trigger phrases in tags. Write these while banking so later sessions can match them locally.

Put private or implementation-specific details in their project scope. A transferable method belongs in a portable entry only when its wording and evidence are appropriate across those projects. Use a separate store for each person or trust boundary. When teaching from a sensitive example, replace identifying values with placeholders before showing the example to the learning agent. Inspect the saved evidence before sharing a lesson across tools. Never save credentials or a raw transcript.

## After reuse

Compare the new outcome with the prediction. Use `instinct_feedback` or `instinct hit ID --note '...'` for an independently useful application and `instinct miss ID --note '...'` for a misleading one. Record what you observed. Record each outcome once. When a call may be retried, pass the same `outcome_id` through MCP or `--outcome-id` on the CLI for that observed reuse. Seeing the entry or repeating its words earns no hit. If the result is inconclusive, leave the count alone.

When experience narrows the boundary, quarantine the obsolete version with `instinct review ID --state quarantined --reason WHY --source-ref REF`, then bank the narrower discovery with a distinct title. Inspect promoted methods with `instinct instincts`. Its strength score summarizes past feedback and does not estimate the probability that today's answer is right.

## Carry it across harnesses

A harness can place this skill in its system instructions and supply relevant experiences before a model call. The agent can also invoke the CLI or MCP tools on demand. If the host has its own persistence, use the same situation, method, evidence, boundary and feedback fields there. Without persistence, use lessons in the current conversation. When a user wants to switch tools, hand them a short note with the cue, first move, observed outcome and exception. Ask the receiving agent to check the new situation against it.

For connected setup, storage choices, backups or how the Helix policy changes confidence, see [the bank and Helix guide](references/architecture.md).
