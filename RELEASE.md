# Instinct 0.1.0

Download **instinct-skill.zip** for the portable skill. Extract its `instinct/` folder into your project’s `.agents/skills/` for Codex or `.claude/skills/` for Claude Code, then start a fresh chat. The ZIP includes the skill, the architecture guide and the Apache license. Add the Linux executable for automatic retrieval across trusted tools. [Read how the bank and Helix work](skills/instinct/references/architecture.md)

Download **try-instinct.html** and open it in your browser. Pick the frustration you recognize: “I already explained this,” “It changed too much,” or “It keeps trying the same fix.” Switch between the actual Claude Code and Codex replies. Change the brief and see when the agent should set the earlier lesson aside. The situations are fictional and the runs are recorded.

Choose **Get the prompt for my AI** to begin with your very first task. If you already have a chat where a correction worked, you can also extract one lesson to take into another tool. Connected agents can bank and retrieve experiences directly. [Cross-harness replay](bench/first-use/README.md)

Choose **See the print run** to open two printable sheets from a live Claude Code to Codex handoff. Change the desk rules to see what happened when yesterday’s lesson stopped fitting. Both PDFs are inside the HTML download. [Inspect the inputs, outputs and grader](bench/relay/README.md).

**Try your own correction.** In the offline demo choose **Test a lesson on my own task**. Decide what success looks like, run the same request in two fresh chats, then compare the answers with their labels hidden. You can share just your verdict. Your criterion and pasted answers stay in the tab.

The connected engine uses native Helix DB (`.hxdb`). Every persisted outcome has a verified record. A long-running MCP session reuses an in-memory, scope-specific retrieval index. Legacy JSONL banks can be migrated and exported. [Helix DB measurements](docs/helix-showcase.md) · [Apache-2.0 license](LICENSE)

Run your first Helix DB program, change its source, or bring a failing example to the review queue. [Contributor guide](CONTRIBUTING.md)

## Try the Linux download

Download the Linux x86-64 archive and its SHA256SUMS file into an empty directory. Check the archive, extract it, then run the executable from inside the extracted folder:

```sh
sha256sum --ignore-missing --check SHA256SUMS
tar xzf instinct-linux-x86_64.tar.gz
cd instinct
./instinct demo
```

The demo needs no API key, model download, network connection, or existing Instinct setup. Its temporary store is removed when it finishes. To connect a real agent, use the included skill and [integration guide](README.md). The Linux archive includes the Cargo sources, tests and benchmark fixtures.

## What the first release demonstrates

Five experiences compete for the next decision. In a reconstructed record-merge task, fresh Codex sessions used a prior method to answer without looking up the reference. In the snapshot variant, they applied the exception and returned the newest balance. The unrelated calculation surfaced no experience.

Across 36 decisions, live retrieval and the full static bank both saved four lookups compared with the unaided arm. All answers were correct. On the longer historical replay, lexical search recovered 123 of 264 previously useful or protective recalls. Recognition across changes in wording is the outstanding research problem. [See the answers, timings and limits](bench/backtest/README.md).

A good external replication starts with a real discovery from one project. Freeze a fresh task, an independent success check and a case where following the hunch would be wrong. Share what the agent tried first, whether it worked, and which lookup or action it saved. Keep private bank entries out of published results.
