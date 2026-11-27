# Instinct — help your AI learn from experience

**Teach an agent a method once. See whether another agent uses it when the same decision comes up.** Instinct stores the cue, the move, the evidence and the reason to change course in Helix DB.

[Try the live demo](https://koscak-labs.github.io/instinct/) · [Download the skill](https://github.com/koscak-labs/instinct/releases/latest/download/instinct-skill.zip) · [Write your first Helix DB program](CONTRIBUTING.md)

**Why do I have to explain this again?** Tess spent one chat getting a client update right. A long recap left the client wondering what to do. One line worked: “You: choose a photo / Me: finish the poster.” When she asks a different assistant for her next update, Instinct can give it the method behind that correction. Today’s actions can be entirely different.

**Try it offline.** [Download the standalone demo](https://github.com/koscak-labs/instinct/releases/latest/download/try-instinct.html) and open it on your phone or computer. Choose a frustration: re-explaining a correction, getting an unwanted rewrite, or watching an agent repeat a fix. Change the request and compare actual Claude Code and Codex replies. The page works offline.

**Can another agent actually do the job?** Claude learned how a workshop desk corrected a bad print run. Fresh Codex received a new roster. With an empty bank it asked for the desk note. With Instinct it found Claude’s method and printed all four badges correctly, including two different seats for people named Jules. A pasted copy of the lesson also worked. Open **See the print run** in the demo to download the actual sheet. [Run the independent check](bench/relay/README.md).

**Try your own correction.** In the offline demo choose **Test a lesson on my own task**. Decide what success looks like, run the same request in two fresh chats, then compare the answers with their labels hidden. You can share just your verdict. Your criterion and pasted answers stay in the tab.

## Install in a project

With Node.js 22.20 or later, run this in your project and choose your agent:

```sh
npx skills add koscak-labs/instinct --skill instinct
```

Start a fresh session. Instinct can learn from an outcome in that conversation straight away. To share experience across sessions, connect a bank to your agents. [Set up a shared bank](skills/instinct/references/architecture.md)

For a manual install, extract the `instinct/` folder from the [skill ZIP](https://github.com/koscak-labs/instinct/releases/latest/download/instinct-skill.zip) into your project’s `.agents/skills/` for Codex or `.claude/skills/` for Claude Code.

## Try it in the AI you already use

- **First task.** Choose **Get the prompt for my AI**, paste it before a real request, and let the agent begin. When a correction or a check reveals a better first move, the agent can reuse it within that conversation.
- **Switching tools.** Choose **A chat where we learned something**. Paste the prompt into that conversation and carry its short, evidence-backed note into the next tool. Ordinary chats receive only the experience you supply.
- **Connected agent.** Install the skill and connect the engine below. The host can retrieve relevant experience across sessions that share an authorized bank. The lesson carries the situation, what worked and when to reconsider. The model’s weights stay fixed.

Created by Filip Koščák (@philphauler) at Koščák Labs.

## Does it change what agents do?

Claude extracted three lessons from fictional interactions. On the client update, fresh Codex used one of those lessons to produce Tess’s format without asking for the convention again. Claude reused the same format but added an unwanted recap. In the editing examples, both tools already got the copy right unaided. [Compare all 36 replies](bench/first-use/README.md).

The [engineering backtest](bench/backtest/README.md), [everyday replay](bench/everyday/README.md) and [historical usage audit](bench/usage.md) include irrelevant recalls and tasks with no measured gain. Explore [other use cases](docs/use-cases.md) and [the decision policy written in Helix](docs/helix-showcase.md).

## The bank behind the skill

A new connected installation stores its lessons in **Helix DB**, Koščák’s embedded database. Each success, miss, retraction and quarantine becomes a checksummed event. A running agent can look up a different situation using the same verified index. When another agent changes the bank, its next lookup rebuilds that index. The same file works through the CLI and MCP.

To bring a previous Instinct bank forward, pause its writers, run `instinct migrate --store old.jsonl --to experience.hxdb`, then point your agents at the new file. The command preserves the original and verifies the new event stream before exposing the destination. [Storage, permissions and recovery](skills/instinct/references/architecture.md) · [Cold and warm measurements](docs/helix-showcase.md)

## Run the engine demo

From a source checkout run `cargo run --release --locked --offline --bin instinct -- demo`. With the Linux executable run `./instinct demo`. The demo banks Claude’s three discoveries in a temporary store, retrieves a method for Tess’s new request, and tries an unrelated request. To bring your own situation to this sample bank:

```sh
./instinct demo "Can you clean up this approved signup page for me?"
```

## Use with your own agent

Give the agent [the Instinct skill](skills/instinct/SKILL.md) as instructions. At a consequential choice, pass the situation to the engine. A host with subprocess access can do this before calling its model:

```python
import json
import subprocess

def relevant_experience(situation, project):
    reply = subprocess.run(
        ["instinct", "decide", "--project", project, "--q", situation],
        capture_output=True, text=True, check=True,
    )
    return json.loads(reply.stdout)["experiences"]
```

The host inherits the configured store from its environment. Give the model those experiences as data alongside the current observation. An empty list leaves it free to explore. Each match carries the method, observed result, evidence status, prior hits and misses, and an optional condition for reconsidering. The agent chooses the action. After observing the outcome, it can bank a new discovery or give feedback on an existing one. MCP and the named harness setups below are other ways to connect the same engine.

## Try it locally

From this checkout, with Rust 1.89 or later:

```sh
cargo install --locked --offline --path .
umask 077
export INSTINCT_STORE="$(mktemp -d)/practice.hxdb"
instinct bank --title 'Check what the score actually measures' \
  --when 'A proxy score stands in for an independently measurable outcome' \
  --found 'High text similarity can accompany a failed task' \
  --how 'Compare the proxy against independently checked outcomes on the same examples' \
  --reconsider 'If no independent outcomes exist, collect a labeled sample first' \
  --evidence 'Toy fixture: two high-similarity failures and one low-similarity success'
instinct recall --q 'Does our proxy score reward completion of the new task?' --k 1
instinct recall --q 'Prepare dinner' --k 1
```

The first query returns the method; the unrelated query prints nothing. This toy example adapts an actual banked discovery about a misleading proxy for agent performance. Its evidence values are synthetic. Independent reuse can earn a hit; `instinct instincts` shows methods with at least two hits and a smoothed payoff of at least 0.6. Use the same store path across sessions. For lasting use, choose a file in a private directory such as `~/.local/share/instinct/bank.hxdb`. Point trusted harnesses at the same bank to share experience. Give different people separate private banks.

## Give an agent the skill

Copy `skills/instinct` into a project's `.agents/skills/` for Codex or `.claude/skills/` for Claude Code. Start a fresh session with the installed binary on PATH and the intended store configured. The skill can call the CLI directly. To expose the same operations through MCP instead:

```sh
claude mcp add --transport stdio --scope local instinct --env "INSTINCT_STORE=$INSTINCT_STORE" -- instinct mcp
codex mcp add instinct --env "INSTINCT_STORE=$INSTINCT_STORE" -- instinct mcp
```

Four tools expose banking, recall, feedback and promoted instincts. Set `project` to the current project path when saving or recalling project-specific material. An unscoped method is eligible across projects in that store. Scope filters support relevance inside a trusted user's bank. Give mutually untrusted clients different files and OS permissions.

**Optional automatic retrieval in Claude Code.** Merge this hook into your local settings and launch Claude with the store variable exported. The hook reads the prompt and working directory from stdin, looks up one relevant method, and writes it to stdout for the session. A skill-driven lookup can also describe the situation in more general language when surface vocabulary changes.

```json
{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"instinct prompt"}]}]}}
```

## How confidence changes

Each entry preserves WHEN, FOUND, HOW, evidence and optional scope. Recall ranks lexical matches with BM25 and supports optional Ollama embeddings using `INSTINCT_OLLAMA=host:port` and `INSTINCT_EMBED_MODEL`. The default runs offline. Directly banked candidates can surface on two matching trigger words. This heuristic has false positives and misses paraphrases. Mined candidates retain the original stricter eligibility check.

A retrieval alone changes no score. Independent feedback uses `hit ID --note ...` or `miss ID --note ...`. Strength is (hits + 1) / (hits + misses + 2). Repeated misses can silence an entry. Quarantine withdraws a lesson while keeping its evidence; `retract hit ID` or `retract miss ID` corrects erroneous feedback. New installations use a native Helix DB bank. Existing JSONL banks remain readable. The optional trigger-vector cache is local. To uninstall, remove the skill directory, its MCP or hook registration, and the binary with `cargo uninstall instinct`. Delete a bank directory only if you intend to erase its history.

## Does experience improve the next decision?

[Read the transfer experiment](bench/README.md). It includes the discovery prompt, the resulting bank entry, fresh decision tasks, comparison conditions and timings. A rendered lesson and an independently verified outcome are separate observations.

Run the offline regression suite with `cargo test --locked --offline`. The native Helix DB runtime, its resident retrieval index and the decision policy ship as Rust exports. The engine builds offline with no third-party Rust crates. The optional benchmark calls an authenticated model CLI and requires jq, mktemp and GNU timeout.

Instinct and its embedded Helix DB runtime are licensed under [Apache-2.0](LICENSE). [Attribution](NOTICE)
