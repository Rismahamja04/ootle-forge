# Tari November Build Contest — "MVP" — Design Brief

**Project name:** Ootle Forge — *gather embers, forge relics, climb the tiers*
**Repo layout:** this directory (`~/workspace/tari-mvp/`)
**Contest:** Tari November Build Contest, theme "MVP" (community.tari.com thread t/520)
**Deadline:** submissions close 00:00 UTC, 1 December 2026
**Licence:** MIT (OSI-approved, as the rules require)

---

## 1. Verified contest facts (checked 2026-11-17 against the official pages)

From the rules post (t/323) and the November contest thread (t/520):

- Monthly Council contests; November's theme is **MVP** — "something you could go to
  market with today and start finding users for."
- Entries must be **new code** as of the theme announcement. November explicitly
  *allows* building on code from previous rounds, but borrowed work must be
  **innovated on**, not reskinned. (This entry borrows nothing; it is written from
  scratch on the official template scaffold.)
- All code must be the entrant's own, under an **OSI-approved open-source licence**
  (MIT recommended).
- A submission is **one post** in the forum thread containing:
  1. a publicly viewable link to the code,
  2. a link to the MVP app,
  3. a description of the project,
  4. **a Tari payment address** for winnings,
  5. a link to a public announcement post — for November this must be on
     **Bluesky, Reddit, Medium, or YouTube. X does not count this month.**
- Prizes: **1st 1,000,000 XTM · 2nd 500,000 XTM · 3rd 250,000 XTM**, voted by the
  Tari Council, winners announced the following month.
- A Council member (Fox) confirmed in the rules thread: **"Development using
  agents is fine"** — but judges may weigh **code quality**.
- Council members and Tari Labs employees are ineligible.

### What the existing November entries establish as the bar

Read in the thread on 2026-11-17 (QuietTicket, Ootle Pass, Ootip, Signal Desk,
Ootle Lockbox, OotleInvoice, Allowance, Skydrop, Specter):

- Every serious entry pairs a **real Rust Ootle `#[template]`** (compiled to WASM)
  with **engine tests** run through `tari_template_test_tooling` — the harness
  runs the template in the actual Ootle engine in-process, so tests need **no
  node, no faucet, no fees**.
- The judge-facing demo is normally a **walletless browser simulation** that
  mirrors the template's rules and is *labelled honestly* as a simulation
  (QuietTicket: "The default judge path is the simulation — no faucet, Sapient,
  or testnet fee required"; OotleInvoice ships a clearly labelled DEMO MODE).
  A live Esmeralda/mainnet path is treated as optional bonus evidence.
- Entries state their limits plainly ("Honest notes" sections) and differentiate
  themselves from earlier rounds' entries.
- Toolchain versions in use: `tari_template_lib` 0.37–0.38,
  `tari_template_test_tooling` 0.49–0.50 (check crates.io for the newest minor
  at build time and keep lib/tooling matched — the harness compiles the template
  against its own copy of the library).

**Conclusion:** to be competitive, this entry must ship (a) a compiling Rust
template with green engine tests, and (b) a polished, zero-install browser demo
that runs the full game loop in simulation. That is exactly the plan below.

---

## 2. The concept

### Ootle Forge — a collectible crafting game on Tari digital assets

Digital-asset games are the most natural consumer MVP for a chain whose pitch is
fast, cheap, user-owned assets — and nobody in the September–November rounds has
entered a game. The loop is deliberately tiny and complete:

1. **Gather** — anyone calls `gather()` and receives EMBER, a fungible resource
   minted by the Forge component.
2. **Forge** — burn 5 EMBER plus a small TARI fee and the Forge mints you a
   **Relic**: a non-fungible token whose tier (Common → Rare → Epic → Legendary)
   is rolled from the transaction's own entropy. Odds: 62% / 26% / 9% / 3%.
3. **Ascend** — burn 3 Relics of the same tier to forge one Relic of the next
   tier. Legendaries can't ascend further.
4. **Melt** — burn a Relic and get EMBER back (tier-priced: 2 / 6 / 18 / 54), so
   duplicates are never dead weight — the economy loops.
5. **Collect** — your Relics are ordinary Ootle NFTs in your own account vault:
   transferable, tradable, and checkable by any app with one read-only call.

Why this is an MVP and not a toy: the day mainnet wallets sign, the Forge is a
live game — the template *is* the product (minting rules locked to the component
mean not even the operator can mint a Relic outside `forge`/`ascend`), and the
browser demo is the storefront. It shows off exactly what Tari wants shown:
fungible + non-fungible resources, component-locked mint authority, vaults,
burns, fees, and events — in a loop a judge understands in 30 seconds.

### How it differs from existing entries (no borrowed concepts)

- Not payments (OotleInvoice), tipping (Ootip), memberships (Ootle Pass),
  voting (QuietTicket), escrowed signals (Signal Desk), time-locks (Lockbox),
  or agent allowances (Allowance). It is the round's **first game**, built around
  *crafting rarity* — mint → burn → reroll — rather than moving money from A to B.

---

## 3. The Rust template (`template/`)

One template, `Forge`, using `tari_template_lib` (latest 0.3x) scaffolded from
the official `tari-project/wasm-template` "empty" template shape
(Cargo.toml + `src/lib.rs` + `tests/test.rs`).

**State**

- `ember_resource`, `relic_resource` — created in the constructor; both
  resources' mint rules are `component(<forge>)` and then locked, so only the
  Forge's own methods can ever mint.
- `fee_vault` — collects the TARI forge fee.
- Counters: `embers_gathered`, `relics_forged`, per-tier forge counts,
  `relics_melted` — public via `stats()` for the demo/indexers.
- `admin_badge` — owner badge resource; `withdraw_fees` requires it.

**Methods**

| Method | Access | Behaviour |
|---|---|---|
| `new()` (constructor) | anyone (deployer keeps admin badge) | creates resources, returns component + admin badge bucket |
| `gather()` | public | mints 5 EMBER to the caller |
| `forge(embers: Bucket, fee: Bucket) -> Bucket` | public | requires exactly 5 EMBER + ≥ 1 TARI fee; burns embers, banks fee, rolls tier, mints 1 Relic NFT with tier metadata |
| `ascend(a: Bucket, b: Bucket, c: Bucket) -> Bucket` | public | 3 Relics of one tier (< Legendary); burns them, mints 1 of the next tier |
| `melt(relic: Bucket) -> Bucket` | public | burns 1 Relic, returns its tier's EMBER value |
| `stats()` | read-only | counters + fee balance |
| `withdraw_fees() -> Bucket` | admin badge | empties the fee vault |

Tier roll: hash of (transaction seed material available to the template —
caller-supplied nonce mixed with the forge counter) mapped onto the odds table.
Deterministic per call inputs, unpredictable enough for a game, and identical
logic in the demo's JS mirror.

**Engine tests** (`tari_template_test_tooling`, target ≥ 7 tests):
gather mints; forge with wrong ember count panics and changes nothing; forge
with short fee panics; forge mints exactly one NFT of a valid tier and banks the
fee; ascend rejects mixed tiers; ascend of 3 Legendaries panics; melt pays the
tier price and burns the NFT; fee withdrawal without the badge is rejected,
with the badge it pays out.

## 4. The browser demo (`web/`)

Static, zero-build, no dependencies — open `index.html` and play. It is a
**simulation that mirrors the template rule-for-rule** (same constants, same
odds, same prices — one shared spec table in the brief and in both codebases),
labelled "SIMULATION" in the UI, exactly as the accepted entries do.

- **Players:** "You" plus two simulated rivals with their own collections, so
  the Forge's public stats and a live activity feed feel alive.
- **Screens:** Gather / Forge (with the roll revealed), Collection grid
  (tier-coloured Relic cards), Ascend picker, Melt, and a **"How this maps to
  Ootle"** panel that shows each click as the template method call it mirrors
  (`forge(embers=5, fee=1 TARI) → mint Relic #n (Rare)`) plus the engine-test
  evidence from the Rust side.
- **Persistence:** localStorage, so a judge's collection survives a reload.
- **Core loop test:** automated headless run — load, gather, forge, ascend,
  melt — asserting balances and zero console errors.

## 5. Submission checklist (for later, with Risma — NOT done in this build)

- [ ] Push code to a public GitHub repo under MIT; host `web/` on GitHub Pages.
- [ ] **Risma provides/creates a Tari payment address** (required field — the
      build must not create wallets or accounts).
- [ ] One post in forum thread t/520 with code link, app link, description,
      payment address, announcement link.
- [ ] Announcement on Bluesky / Reddit / Medium / YouTube (X doesn't count).

## 6. Risks & honest limits

- The template is **not published** to Esmeralda/mainnet in this build (that
  needs a wallet + fees — out of scope); the demo is a simulation, and the
  submission post must say so plainly, as the accepted entries do.
- Tier randomness is game-grade, not cryptographic-grade; fine for collectibles,
  not for money gambling — and the post should not oversell it.
- Judging is subjective and the field is strong (several entries are already
  live on mainnet). This entry's edge is a genuinely different category (a game)
  plus a demo a judge can *play* in under a minute.
