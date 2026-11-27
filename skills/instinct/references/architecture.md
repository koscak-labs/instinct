# How Instinct carries a lesson between agents

A designer corrects an AI that rewrote an approved headline while fixing a link. The next time the brief asks for a small repair, a different agent can recognize the cue, preserve approved copy, fix the link and check it. An explicit request to rewrite the headline changes that decision. The useful thing to carry forward is the judgment and its boundary.

## Choose how far the lesson travels

| Where you work | What to set up | What the next agent receives |
| --- | --- | --- |
| One conversation | Give the agent the skill | The experience available in that conversation |
| Two separate chats | Carry a short note with the cue, move, observed outcome and exception | The experience you chose to share |
| Trusted agents on one machine | Point both engines at the same local file and use the same project for project-specific lessons | Relevant, eligible experiences at the moment of choice |

For a lasting bank, choose a private directory on a local disk. In a Unix shell:

```sh
umask 077
mkdir -p "$HOME/.local/share/instinct"
export INSTINCT_STORE="$HOME/.local/share/instinct/bank.hxdb"
export INSTINCT_PROJECT="my-project"
```

Pass the same absolute store path and project to each trusted host. The project is an exact string. A friendly slug works with the CLI and MCP. The optional Claude prompt hook supplies the working directory, so use the exact project directory as the project value when using that hook. A tool-enabled host can launch `instinct mcp` over standard input/output. A host that runs subprocesses can call `instinct decide --q "..."` and read the `experiences` array. Starting with an empty bank is supported.

## What the database keeps

**Use a native Helix DB file ending in `.hxdb` for a new bank.** The Helix compiler’s `GGGGT` database primitive writes each observation or correction as a verified record. Its key contains the byte offset in the file. The value contains the original event. Instinct checks the keys, record boundaries and native FNV checksums in order before rebuilding a lesson and its confidence. A damaged withdrawal could otherwise resurrect an obsolete method.

| File | Role |
| --- | --- |
| `bank.hxdb` | Authoritative ordered history of discoveries, evidence, outcomes, reviews and retractions. |
| `.lock` and `.io.lock` alongside the bank, with the bank name as prefix | Coordinate identity checks and native reads/writes between local processes. |
| `bank.hxdb.when.bin` | Optional trigger-vector cache, rebuildable with the same embedding model. Full-lesson vectors, when present, travel in the event stream. |

Use a local disk and a private directory. New Unix files request owner-only permissions. Project, persona, harness and version filter applicability inside one trust boundary. Anyone with file write access can change the evidence. The checksum detects accidental record damage. It provides neither authentication nor a proof that a complete tail has never been removed.

A long-running Unix MCP process holds an in-memory term-to-lesson index of one eligible scope. When the bank file changes, the requested scope changes or an eligibility window expires, it verifies and rebuilds before serving that scope. Each ranked decision still checks current exclusions. Cold CLI requests replay the bank. A detected bad record stops native recall. Restore a known-good copy before resuming.

### Bring an existing bank forward

Pause writers for the cutover. Choose an unused destination on a local filesystem supporting hard links.

```sh
instinct migrate --store old.jsonl --to bank.hxdb
instinct export --store bank.hxdb > recovered-events.jsonl
instinct stats --store bank.hxdb
```

Migration takes a snapshot, writes a private staging file, rereads and compares every event, then publishes it without replacing an existing destination. IDs, order, evidence, feedback keys, reviews and retractions travel together. Blank separator lines and final newline spelling are normalized. Set both hosts to the new absolute path. Existing JSONL installations continue to use their own files until switched.

For a backup, pause writers and copy the native bank. Preserve any separately configured labels, embedding settings, and the trigger cache when exact vector rankings matter. Migration moves the event stream. Copy the optional source `.when.bin` under the new bank name or rebuild it with the original model. Test the restored copy in a new directory with a known decision and a formerly withdrawn lesson.

## What Helix decides

Helix supplies the embedded database, the resident text index and the policy that determines when experience can guide action. Its compiler exports Rust that ships in the executable. End users can use the skill or run the binary without installing the compiler.

1. **Find candidates.** The Rust engine filters by applicability and ranks eligible material with BM25 text search. A directly banked candidate can match two meaningful words in its cue or a trigger phrase. If ordinary recall comes up empty, a lesson with a previous hit can also match a lightly misspelled cue through Helix DB’s character trigrams. An ambiguous near-tie yields no suggestion. This fallback recognizes spelling similarity. Optional Ollama embeddings can add semantic matches. Enabling them sends the query or lesson text to the configured endpoint. With embeddings disabled, the engine searches offline.
2. **Apply learned boundaries.** Helix checks project, persona, harness, version, expiry, quarantine and contradicted evidence. A repeatedly misleading lesson can go quiet. The agent still checks today’s request against the human-readable exception. Changed instructions take precedence.
3. **Update confidence after the outcome.** A hit or miss changes the smoothed payoff, computed as `(hits + 1) / (hits + misses + 2)`. Two hits and payoff of at least 0.6 meet the default promotion threshold. The score describes observed reuse. A high score cannot grant authority or independently verify the source claim.

Stored vectors have no model fingerprint. Keep the embedding model fixed for a vector-enabled bank. A model change needs a separately rebuilt bank/cache and a recall comparison.

When reporting the same outcome again after a retry, reuse its key, for example `instinct hit ID --outcome-id issue-42-check-1 --note "Reader confirmed the link opens"`. The MCP field is `outcome_id`. A matching retry has no additional effect. A conflicting verdict for that key is refused. Old clients can omit the key, in which case each call counts. A retraction corrects a count and retains the key so a delayed retry cannot restore it.

## Obsidian and sharing

A Markdown note can carry a method into another chat or an Obsidian vault. Trusted agents on one machine can share the native bank directly. Automatic vault synchronization and cross-device replication would need their own access, conflict and withdrawal rules.
