# Does an earlier outing help an agent plan the next one?

We tested six choices in three fictional situations. The situations, available notes and correct choices were frozen at commit `38714f8`. Each choice went to a fresh Codex session with the same options and access to the same extra note on request. For each condition, the six decisions ran once. The runner checked the exact answer and rejected unexpected tool actions.

| Experience supplied | Correct decisions | Requests for an extra fact |
|---|---:|---:|
| None | 6/6 | 2 |
| All three lessons every time | 6/6 | 1 |
| Live Instinct retrieval | 6/6 | 1 |

- **A night out.** The unaided agent asked for the time from the seats to the street. Both forms of experience supplied the earlier twelve-minute observation for this fictional venue. Codex chose to leave after the main set. All three chose the encore when a friend could wait outside.
- **A surprise gift.** All three inspected the two actual link previews before choosing the spoiler-free link. The earlier incident identified what mattered. It could not establish what a new link displayed. After the reveal, all three chose the descriptive album title.
- **A first shop.** All three chose the photo that showed scale. All three chose an exact measurement for the shelf. Live retrieval returned no lesson for the shelf case.

One fewer lookup in a small, deliberately constructed example supports a narrow claim. These runs do not measure adoption, general speed, or the benefit of the product for real groups of friends. The option IDs below are the actual model outputs. The browser preview at commit `0be6669` showed these options with plain-language labels. The current [first-use replay](../first-use/README.md) uses a different set of tasks.

[Tasks, options and reference notes](fixtures.json) · [Unaided answers](base.tsv) · [Static answers](static.tsv) · [Live answers](live.tsv)

## Reproduce

Use the same environment requirements as the [engineering replay](../backtest/README.md). The first command after the build runs only the local retrieval engine. The other three each start six fresh authenticated Codex decisions, with an additional call when the model asks to see a note.

```sh
cargo build --target-dir target --release --locked --offline --bin instinct --example backtest
target/release/examples/backtest demo gpt-6.2-astra everyday
target/release/examples/backtest base gpt-6.2-astra everyday
target/release/examples/backtest static gpt-6.2-astra everyday
target/release/examples/backtest live gpt-6.2-astra everyday
```
