# Where Helix runs inside Instinct

A method that solved yesterday’s problem may belong to another project or depend on a condition that has since changed. Part of Instinct’s decision policy is authored in Helix and compiled to Rust. For example, these three lines withdraw a time-limited experience after it expires:

```text
CAA # if m.valid_until != 0 && now > m.valid_until
AAC # return Err("expired")
TTT # end
```

The generated Rust preserves the condition:

```rust
if m.valid_until != 0 && now > m.valid_until {
    return Err("expired");
}
```

In the same function, the engine checks project, persona, harness and version when a discovery carries those restrictions. That lets a general method travel while a project-specific rule stays in scope. Helix also authors the durable journal operations, the payoff calculation and promotion threshold. The Rust engine folds historical rows, ranks candidate lessons and serves the CLI and MCP. The demo and backtest runner are authored in Helix. [Helix policy](../src/eligibility.hlx) · [Generated Rust](../src/eligibility.rs) · [Rust engine](../src/main.rs) · [Helix journal](../src/journal.hlx) · [Helix demo](../src/demo.hlx)

## Try a decision

```sh
cargo run --release --locked --offline --bin instinct -- demo
```

The demo retrieves a method Claude learned from a successful client update. The native engine supplies that same experience to a fresh Codex or Claude session. In the browser, switch tools and see how each used it. [Try the demo](https://koscak-labs.github.io/instinct/) · [Recorded replies](../bench/first-use/README.md)

The backtest measures agent behavior with different experience supplied. A language comparison would need a separately authored Rust implementation of the same task, identical acceptance tests and measurements of authoring effort.


## Reproduce the release exports

The public DB runtime is in [runtime/helix-db.rs](../runtime/helix-db.rs). The shipping adapter and public workbench compile that same file. Runtime contributions can be built and tested from this checkout. The reference compiler is private. Its revision for this release is `362aee7385f2f61a603d4ba6b413a70788602324` (`hxc 2.201.1-alphaprime`). Maintainers can reproduce the Instinct exports with that compiler and `rustfmt 1.9.0-stable (59807616e 2026-04-14)`:

```sh
for unit in src/journal src/eligibility src/helix_db src/recall_index src/demo bench/storage tools/db-workbench; do
  hxc export-rust "$unit.hlx" || exit 1
  rustfmt --edition 2021 "$unit.rs" || exit 1
done
node tools/split-db-export.mjs src/helix_db.rs
sha256sum --check docs/helix-exports.sha256
cargo test --locked --offline
```

The split step connects the regenerated adapter to the public runtime and leaves public runtime edits intact. CI checks the source/export fingerprints, the contributor exercise and native behavior. [Contributor guide](../CONTRIBUTING.md)

## Cold and resident lookups

These measurements use one Linux machine, short synthetic lessons in one project and optional embeddings disabled. Cold CLI calls start a process and find the same exact cue. Resident MCP calls use one live process per format and retrieve a different expected lesson on each request. Both arms check every result. Each cell is the median / 95th percentile in milliseconds for 31 calls after two warmups. Index construction, session startup and rebuilds are excluded from the resident figures.

| Lessons | Cold CLI, JSONL | Cold CLI, Helix DB | Resident MCP, JSONL | Resident MCP, Helix DB |
| ---: | ---: | ---: | ---: | ---: |
| 100 | 3.82 / 4.58 | 4.80 / 5.39 | 1.30 / 1.55 | 0.10 / 0.22 |
| 1,000 | 17.02 / 20.33 | 19.06 / 25.01 | 9.96 / 14.05 | 0.09 / 0.30 |
| 10,000 | 146.00 / 189.94 | 191.45 / 240.04 | 100.92 / 140.39 | 0.22 / 0.34 |

The native cold path checks keys, record boundaries and checksums. The Unix MCP path reuses verified lessons and word postings for the current scope. Changes from a second writer, bank replacement, expiry or a scope switch trigger rebuilding. A busy write-heavy bank will pay that cost more often. These timings measure lookup, excluding model generation. The larger historical replay remains the separate test of whether a lesson is useful.

```sh
CARGO_TARGET_DIR=target cargo build --release --locked --offline --bin instinct --example storage
target/release/examples/storage target/release/instinct
```

[Benchmark source](../bench/storage.hlx) · [Historical replay](../bench/backtest/README.md) · [Native storage and backup](../skills/instinct/references/architecture.md)
