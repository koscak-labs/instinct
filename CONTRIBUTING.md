# Build with Helix through Instinct

You can write a Helix DB program, change the database runtime, or review a decision that an agent got wrong. Everything in this checkout builds offline with Rust 1.89 or later. Node is used for the integration and browser tests.

## Run your first Helix DB program

```sh
git clone https://github.com/koscak-labs/instinct.git
cd instinct
cargo run --locked --offline --example helix-db -- examples/first-db.hlx
```

The program banks a lesson, verifies it, and prints `Keep the approved headline | ok=1 corrupt=0 corrupt_keys=[]`. Open [the example program](examples/first-db.hlx). Change the stored phrase and its expected value. Run the same command again. Leave the expected value alone on a second edit to see the check fail.

The public workbench compiles a small, documented subset of actual Helix syntax to Rust and runs it against the **same database runtime used by Instinct**. It accepts a `GGGGT # db` declaration, `AAA # fn main()`, `AAC` statements and `TTT # end`. It reports unsupported instructions with their line number. The workbench compiles local code with your account’s permissions. Use examples you trust. Each run gets its own temporary working directory. The reference compiler produces the same output for the bundled exercise.

## Choose a contribution

- **Build the database.** Edit [the public runtime](runtime/helix-db.rs). A change here runs in both the workbench and the shipping engine. Start with `db_putv`, `db_getv` and `db_verify`. Bring a failing fixture and show the result after your change.
- **Review the language integration.** Trace how [an event is stored](src/helix_db.hlx) or how [an agent finds it](src/recall_index.hlx). Include the smallest Helix example that triggers the behavior. For changes beyond the public workbench subset, submit the Helix source and an expected result. Maintainers re-export those modules with the full compiler.
- **Test a real decision.** Share a synthetic or anonymized cue, a successful method, and a different situation where applying it would be wrong. Compare empty-bank and learned outcomes, include misses, and identify what you actually checked. The [badge handoff](bench/relay/README.md) shows the setup.

[Pick a starter issue](https://github.com/koscak-labs/instinct/labels/good%20first%20issue) · [File a Helix review or reproduction](https://github.com/koscak-labs/instinct/issues/new?template=helix-review.yml)

## Check your change

```sh
cargo test --offline --locked
cargo build --offline --locked --bin instinct --target-dir target
INSTINCT_TEST_BIN="$PWD/target/debug/instinct" node --test tests/helix-db.test.mjs
```

Open a pull request with the behavior you changed, the exact command you ran and the observed result. Runtime changes can be reviewed here independently of the full compiler. Maintainers can carry accepted changes upstream. Instinct, the workbench and the embedded DB runtime in this distribution are Apache-2.0. [License](LICENSE)
