# Ootle Forge

A collectible crafting game for **Tari Ootle**: gather **EMBER**, forge **Relic**
NFTs with rolled rarity tiers, **ascend** three of a kind into the next tier,
and **melt** duplicates back into EMBER.

Built for the Tari November Build Contest ("MVP").

## What's here

| Path | What it is |
|---|---|
| `template/` | The real thing: a Rust `#[template]` for the Tari Ootle engine, compiled to WASM, with engine tests (`tari_template_test_tooling`). |
| `web/` | A walletless browser **simulation** of the template — the demo judges can play with no wallet, account or network. The JavaScript engine mirrors the template rule for rule (same constants, costs, odds and failure conditions). |
| `BRIEF.md` | The design brief: concept, contest rules recap, and why the entry is shaped the way it is. |

## Play the demo

Open `web/index.html` in a browser, or serve it:

```sh
python3 -m http.server --directory web 8000
# then visit http://localhost:8000
```

Gather → Forge → collect Relics → select three of the same tier to Ascend,
or Melt any Relic back into EMBER. Two simulated rival forgesmiths play
alongside you so the Forge statistics move. Your game is saved in the
browser (localStorage).

Run the demo's headless end-to-end test (drives the real page in Chrome —
gather, forge, melt, ascend, persistence, zero console errors):

```sh
node web/test-demo.mjs
```

## Build and test the template

Requires a Rust toolchain with the `wasm32-unknown-unknown` target.

```sh
cd template
cargo test                                   # engine tests (first build is slow)
cargo build --target wasm32-unknown-unknown --release   # the deployable WASM
```

The engine tests run the template in the actual Ootle engine in-process and
cover the full ruleset: gather amounts, exact forge cost/fee enforcement
(wrong EMBER amount and a fee one microTari short both reject), tier-rolled
minting, melt values per tier, three-of-a-kind ascension, mixed-tier
rejection, and badge-gated fee withdrawal.

## The game rules (identical in template and demo)

- **Gather** — anyone, any time: mints 5 EMBER. EMBER can only ever be minted
  by the Forge component (locked mint rule).
- **Forge** — burn exactly 5 EMBER and pay exactly 1 TARI fee: receive one
  Relic NFT with a rolled tier: Common 62% · Rare 26% · Epic 9% ·
  Legendary 3%. The tier and a sequential serial are stored in the NFT's
  immutable data.
- **Ascend** — burn three Relics of the same tier: receive one Relic of the
  next tier. Legendaries cannot ascend.
- **Melt** — burn one Relic: receive 2 / 6 / 18 / 54 EMBER by tier.
- **Fees** — forge fees accumulate in the Forge's vault; only the holder of
  the Forge admin badge NFT (minted once at instantiation) can withdraw them.
- The Forge component has **no owner** and its resources' rules are locked:
  nobody can change the game after deployment, and Relics can only be minted
  by `forge` and `ascend`.

## Honest status

- The template is **not yet published** to Esmeralda or mainnet — the browser
  demo is a simulation and says so on the page. The path from here to live is
  mechanical (publish the WASM with the Tari CLI, instantiate, point the same
  UI at a wallet connector); the game logic would not change.
- The tier roll uses the engine's transaction entropy (`random_bytes`):
  adequate for a collectible game, not a gambling-grade randomness source.
- Relics are ordinary Ootle NFTs — once in your account they are yours:
  transferable, and readable by any other app.

## License

MIT — see `LICENSE`.
