"use strict";

/* ============================================================================
 * Ootle Forge — simulation engine
 *
 * This class mirrors the Rust template in ../template/src/lib.rs rule for
 * rule: same constants, same costs, same tier odds, same melt values, same
 * failure conditions. The only substitution is the source of randomness:
 * on-chain the template draws engine entropy (random_bytes); here we draw
 * from Math.random with identical odds.
 * ==========================================================================*/

const SPEC = {
  GATHER_AMOUNT: 5,
  FORGE_COST: 5, // EMBER, burned
  FORGE_FEE: 1, // TARI, banked by the Forge
  MELT_VALUES: [2, 6, 18, 54], // EMBER per tier
  MAX_TIER: 3,
  TIERS: [
    { name: "Common", icon: "🪨", odds: 62 },
    { name: "Rare", icon: "🔷", odds: 26 },
    { name: "Epic", icon: "🔮", odds: 9 },
    { name: "Legendary", icon: "🌟", odds: 3 },
  ],
};

class ForgeError extends Error {}

function rollTier() {
  // Same distribution as the template: a byte mod 100 against cumulative
  // thresholds 62 / 88 / 97 / 100.
  const roll = Math.floor(Math.random() * 100);
  if (roll < 62) return 0;
  if (roll < 88) return 1;
  if (roll < 97) return 2;
  return 3;
}

class ForgeSim {
  constructor() {
    this.nextSerial = 0;
    this.stats = { gathered: 0, forged: 0, ascended: 0, melted: 0, fees: 0 };
    this.players = new Map();
  }

  addPlayer(name, tari) {
    const player = { name, tari, embers: 0, relics: new Map() };
    this.players.set(name, player);
    return player;
  }

  player(name) {
    const p = this.players.get(name);
    if (!p) throw new ForgeError(`unknown player ${name}`);
    return p;
  }

  /** Mirrors Forge::gather — mints GATHER_AMOUNT EMBER. */
  gather(name) {
    const p = this.player(name);
    p.embers += SPEC.GATHER_AMOUNT;
    this.stats.gathered += SPEC.GATHER_AMOUNT;
    return SPEC.GATHER_AMOUNT;
  }

  /** Mirrors Forge::forge — burns 5 EMBER + 1 TARI fee, mints one Relic. */
  forge(name) {
    const p = this.player(name);
    if (p.embers < SPEC.FORGE_COST) {
      throw new ForgeError(
        `forging needs exactly ${SPEC.FORGE_COST} EMBER — you have ${p.embers}`
      );
    }
    if (p.tari < SPEC.FORGE_FEE) {
      throw new ForgeError(
        `forging needs a ${SPEC.FORGE_FEE} TARI fee — you have ${p.tari} TARI (try the simulated faucet)`
      );
    }
    p.embers -= SPEC.FORGE_COST;
    p.tari -= SPEC.FORGE_FEE;
    this.stats.fees += SPEC.FORGE_FEE;
    this.stats.forged += 1;
    const relic = { serial: this.nextSerial, tier: rollTier() };
    this.nextSerial += 1;
    p.relics.set(relic.serial, relic);
    return relic;
  }

  /** Mirrors Forge::ascend — 3 same-tier Relics burn, 1 next-tier mints. */
  ascend(name, serials) {
    const p = this.player(name);
    if (serials.length !== 3) throw new ForgeError("ascension takes exactly three Relics");
    const relics = serials.map((s) => {
      const r = p.relics.get(s);
      if (!r) throw new ForgeError(`Relic #${s} is not in your collection`);
      return r;
    });
    const tier = relics[0].tier;
    if (relics.some((r) => r.tier !== tier)) {
      throw new ForgeError("ascension requires three Relics of the same tier");
    }
    if (tier >= SPEC.MAX_TIER) {
      throw new ForgeError("Legendary Relics cannot ascend further");
    }
    for (const r of relics) p.relics.delete(r.serial);
    this.stats.ascended += 1;
    const relic = { serial: this.nextSerial, tier: tier + 1 };
    this.nextSerial += 1;
    p.relics.set(relic.serial, relic);
    return relic;
  }

  /** Mirrors Forge::melt — burns one Relic, mints its EMBER value back. */
  melt(name, serial) {
    const p = this.player(name);
    const relic = p.relics.get(serial);
    if (!relic) throw new ForgeError(`Relic #${serial} is not in your collection`);
    p.relics.delete(serial);
    this.stats.melted += 1;
    const value = SPEC.MELT_VALUES[relic.tier];
    p.embers += value;
    return { relic, value };
  }

  faucet(name, amount) {
    this.player(name).tari += amount;
    return amount;
  }

  toJSON() {
    return {
      nextSerial: this.nextSerial,
      stats: this.stats,
      players: [...this.players.values()].map((p) => ({
        name: p.name,
        tari: p.tari,
        embers: p.embers,
        relics: [...p.relics.values()],
      })),
    };
  }

  static fromJSON(data) {
    const sim = new ForgeSim();
    sim.nextSerial = data.nextSerial;
    sim.stats = data.stats;
    for (const p of data.players) {
      const player = sim.addPlayer(p.name, p.tari);
      player.embers = p.embers;
      for (const r of p.relics) player.relics.set(r.serial, r);
    }
    return sim;
  }
}

/* ============================================================================
 * UI
 * ==========================================================================*/

const YOU = "You";
const RIVAL_NAMES = ["Mira", "Tomas"];
const SAVE_KEY = "ootle-forge-save-v1";

let sim;
let selected = new Set(); // serials picked for ascension
const logEntries = [];

function tierOf(relic) {
  return SPEC.TIERS[relic.tier];
}

function $(id) {
  return document.getElementById(id);
}

function save() {
  try {
    localStorage.setItem(SAVE_KEY, JSON.stringify(sim.toJSON()));
  } catch {
    /* storage unavailable (e.g. some file:// setups) — the demo still runs */
  }
}

function load() {
  try {
    const raw = localStorage.getItem(SAVE_KEY);
    if (raw) return ForgeSim.fromJSON(JSON.parse(raw));
  } catch {
    /* fall through to a fresh game */
  }
  return null;
}

function freshSim() {
  const s = new ForgeSim();
  s.addPlayer(YOU, 10);
  for (const name of RIVAL_NAMES) s.addPlayer(name, 25);
  return s;
}

function log(who, text, call) {
  logEntries.unshift({ who, text, call });
  if (logEntries.length > 60) logEntries.pop();
  renderLog();
}

function renderLog() {
  $("log").innerHTML = logEntries
    .map(
      (e) =>
        `<li><span class="who">${e.who}</span> ${e.text}` +
        (e.call ? `<span class="call"><code>${e.call}</code></span>` : "") +
        `</li>`
    )
    .join("");
}

function setHint(message, isError) {
  const hint = $("action-hint");
  hint.textContent = message || "";
  hint.classList.toggle("error", Boolean(isError));
}

function render() {
  const you = sim.player(YOU);
  $("bal-ember").textContent = you.embers;
  $("bal-tari").textContent = you.tari;
  $("bal-relics").textContent = you.relics.size;

  $("btn-forge").disabled = you.embers < SPEC.FORGE_COST || you.tari < SPEC.FORGE_FEE;

  // Collection
  const relics = [...you.relics.values()].sort((a, b) => a.serial - b.serial);
  $("collection-empty").style.display = relics.length ? "none" : "";
  $("collection").innerHTML = relics
    .map((r) => {
      const t = tierOf(r);
      return `<article class="relic-card tier-${r.tier} ${selected.has(r.serial) ? "selected" : ""}" data-serial="${r.serial}" tabindex="0" role="button" aria-pressed="${selected.has(r.serial)}" title="Click to select for ascension">
        <span class="relic-icon">${t.icon}</span>
        <span class="relic-name">Relic #${r.serial}</span>
        <span class="relic-tier">${t.name}</span>
        <div class="relic-actions">
          <button data-melt="${r.serial}">Melt (+${SPEC.MELT_VALUES[r.tier]} EMBER)</button>
        </div>
      </article>`;
    })
    .join("");

  // Ascension bar
  const picked = [...selected]
    .map((s) => you.relics.get(s))
    .filter(Boolean);
  if (picked.length) {
    const sameTier = picked.every((r) => r.tier === picked[0].tier);
    const canAscend = picked.length === 3 && sameTier && picked[0].tier < SPEC.MAX_TIER;
    $("ascend-bar").hidden = false;
    $("ascend-text").textContent =
      picked.length < 3
        ? `Selected ${picked.length} of 3 Relics for ascension — pick ${3 - picked.length} more.`
        : sameTier
          ? picked[0].tier >= SPEC.MAX_TIER
            ? "Legendary Relics cannot ascend further."
            : `Three ${tierOf(picked[0]).name} Relics → one ${SPEC.TIERS[picked[0].tier + 1].name}.`
          : "Ascension needs three Relics of the same tier.";
    $("btn-ascend").disabled = !canAscend;
  } else {
    $("ascend-bar").hidden = true;
  }

  // Stats
  $("stat-gathered").textContent = sim.stats.gathered;
  $("stat-forged").textContent = sim.stats.forged;
  $("stat-ascended").textContent = sim.stats.ascended;
  $("stat-melted").textContent = sim.stats.melted;
  $("stat-fees").textContent = `${sim.stats.fees} TARI`;

  // Rivals
  $("rivals").innerHTML = RIVAL_NAMES.map((name) => {
    const p = sim.player(name);
    const best = [...p.relics.values()].reduce((m, r) => Math.max(m, r.tier), -1);
    return `<div class="rival"><span><strong>${name}</strong></span><span>${p.embers} EMBER · ${p.relics.size} Relics${best >= 0 ? ` · best: ${SPEC.TIERS[best].name}` : ""}</span></div>`;
  }).join("");

  save();
}

/* ---------- player actions ---------- */

function doGather() {
  sim.gather(YOU);
  setHint("");
  log(YOU, `gathered ${SPEC.GATHER_AMOUNT} EMBER.`, "forge.gather() → 5 EMBER minted");
  render();
}

function doForge() {
  try {
    const relic = sim.forge(YOU);
    const t = tierOf(relic);
    setHint("");
    const reveal = $("forge-reveal");
    reveal.hidden = false;
    reveal.innerHTML = `The forge roars… you forged <span class="tier-name" style="color: var(--${t.name.toLowerCase() === "legendary" ? "legendary" : t.name.toLowerCase()})">${t.icon} Relic #${relic.serial} — ${t.name}</span>!`;
    log(
      YOU,
      `forged Relic #${relic.serial} (${t.name}).`,
      `forge.forge(embers=5 EMBER, fee=1 TARI) → Relic NFT #${relic.serial} minted`
    );
  } catch (e) {
    setHint(e.message, true);
  }
  render();
}

function doMelt(serial) {
  try {
    const { relic, value } = sim.melt(YOU, serial);
    selected.delete(serial);
    setHint("");
    log(
      YOU,
      `melted Relic #${relic.serial} (${tierOf(relic).name}) for ${value} EMBER.`,
      `forge.melt(relic=#${relic.serial}) → ${value} EMBER minted`
    );
  } catch (e) {
    setHint(e.message, true);
  }
  render();
}

function doAscend() {
  try {
    const serials = [...selected];
    const fromTier = sim.player(YOU).relics.get(serials[0]).tier;
    const relic = sim.ascend(YOU, serials);
    selected.clear();
    setHint("");
    log(
      YOU,
      `ascended three ${SPEC.TIERS[fromTier].name} Relics into Relic #${relic.serial} (${tierOf(relic).name}).`,
      `forge.ascend(#${serials.join(", #")}) → Relic NFT #${relic.serial} minted`
    );
  } catch (e) {
    setHint(e.message, true);
  }
  render();
}

/* ---------- rivals (ambient activity so the world feels alive) ---------- */

function rivalAct() {
  const name = RIVAL_NAMES[Math.floor(Math.random() * RIVAL_NAMES.length)];
  const p = sim.player(name);
  try {
    if (p.embers >= SPEC.FORGE_COST && p.tari >= SPEC.FORGE_FEE && Math.random() < 0.55) {
      const relic = sim.forge(name);
      log(name, `forged Relic #${relic.serial} (${tierOf(relic).name}).`, `forge.forge(...) → Relic #${relic.serial}`);
    } else if (p.relics.size >= 4 && Math.random() < 0.3) {
      const byTier = new Map();
      for (const r of p.relics.values()) {
        if (!byTier.has(r.tier)) byTier.set(r.tier, []);
        byTier.get(r.tier).push(r.serial);
      }
      const trio = [...byTier.values()].find((v) => v.length >= 3);
      const ascendedTier = trio ? sim.player(name).relics.get(trio[0]).tier : null;
      if (trio && ascendedTier < SPEC.MAX_TIER) {
        const relic = sim.ascend(name, trio.slice(0, 3));
        log(name, `ascended into Relic #${relic.serial} (${tierOf(relic).name}).`, `forge.ascend(...) → Relic #${relic.serial}`);
      } else {
        sim.gather(name);
        log(name, `gathered ${SPEC.GATHER_AMOUNT} EMBER.`, "forge.gather()");
      }
    } else {
      sim.gather(name);
      log(name, `gathered ${SPEC.GATHER_AMOUNT} EMBER.`, "forge.gather()");
    }
  } catch {
    /* a rival fumble is not worth surfacing */
  }
  render();
}

/* ---------- wiring ---------- */

function init() {
  sim = load() || freshSim();

  $("btn-gather").addEventListener("click", doGather);
  $("btn-forge").addEventListener("click", doForge);
  $("btn-faucet").addEventListener("click", () => {
    sim.faucet(YOU, 10);
    setHint("Demo faucet: +10 simulated TARI. On a live network this fee would come from your own wallet.");
    log(YOU, "took 10 TARI from the simulated faucet (demo only).", "— no on-chain equivalent —");
    render();
  });
  $("btn-ascend").addEventListener("click", doAscend);
  $("btn-ascend-clear").addEventListener("click", () => {
    selected.clear();
    render();
  });
  $("btn-reset").addEventListener("click", () => {
    localStorage.removeItem(SAVE_KEY);
    sim = freshSim();
    selected.clear();
    logEntries.length = 0;
    $("forge-reveal").hidden = true;
    log("Forge", "A fresh Forge has been lit. Gather your first embers.", "forge = Forge::instantiate()");
    render();
  });

  $("collection").addEventListener("click", (ev) => {
    const meltBtn = ev.target.closest("[data-melt]");
    if (meltBtn) {
      doMelt(Number(meltBtn.dataset.melt));
      return;
    }
    const card = ev.target.closest(".relic-card");
    if (card) {
      const serial = Number(card.dataset.serial);
      if (selected.has(serial)) selected.delete(serial);
      else if (selected.size < 3) selected.add(serial);
      else setHint("Three Relics are already selected for ascension.", true);
      render();
    }
  });

  if (!logEntries.length) {
    log("Forge", "The Forge is lit. Gather embers to begin.", "forge = Forge::instantiate()");
  }
  render();

  // Rivals act every few seconds; disabled with ?still=1 (used by tests).
  if (!new URLSearchParams(location.search).has("still")) {
    setInterval(rivalAct, 5000);
  }
}

document.addEventListener("DOMContentLoaded", init);

// Exposed for the headless test harness (and curious judges in devtools).
window.__forge = {
  SPEC,
  get sim() {
    return sim;
  },
  ForgeSim,
  ForgeError,
  render,
};
