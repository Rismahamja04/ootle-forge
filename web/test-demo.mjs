// Headless end-to-end test for the Ootle Forge demo.
//
// Drives the real page in headless Chrome: gathers, forges, melts and
// ascends through the UI where practical, and asserts the simulation
// engine's rules directly where a UI path would need dozens of clicks.
// Fails on any console error or page error.
//
// Run:  node web/test-demo.mjs
// Uses the Puppeteer install and chrome-headless-shell kept in
// ~/workspace/makefun/shottools (shared tooling on this machine).

import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";

const require = createRequire("/home/hatch/workspace/makefun/shottools/package.json");
const puppeteer = require("puppeteer");

const CHROME =
  "/home/hatch/workspace/makefun/shottools/browser/chrome-headless-shell-linux64/chrome-headless-shell";
const here = path.dirname(fileURLToPath(import.meta.url));
const PAGE_URL = "file://" + path.join(here, "index.html") + "?still=1";
const SHOT = process.env.SHOT || "/tmp/ootle-forge-demo.png";

let failures = 0;
function check(name, cond, detail = "") {
  if (cond) {
    console.log(`  ok  ${name}`);
  } else {
    failures += 1;
    console.log(`FAIL  ${name} ${detail}`);
  }
}

const browser = await puppeteer.launch({
  executablePath: CHROME,
  args: ["--no-sandbox", "--disable-dev-shm-usage"],
});
const page = await browser.newPage();
await page.setViewport({ width: 1100, height: 1400 });

const errors = [];
page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
page.on("console", (m) => {
  if (m.type() === "error") errors.push(`console: ${m.text()}`);
});

await page.goto(PAGE_URL, { waitUntil: "networkidle0" });
await page.evaluate(() => localStorage.clear());
await page.reload({ waitUntil: "networkidle0" });
await page.waitForSelector("#btn-gather");

console.log("page loads");
check("title mentions Ootle Forge", (await page.title()).includes("Ootle Forge"));
check("simulation badge visible", await page.$eval(".sim-badge", (el) => el.textContent.includes("SIMULATION")));
check("engine exposed", await page.evaluate(() => Boolean(window.__forge?.sim)));
check(
  "ascend bar hidden until a Relic is selected",
  await page.$eval("#ascend-bar", (el) => getComputedStyle(el).display === "none")
);

console.log("gather via UI");
await page.click("#btn-gather");
check("EMBER balance is 5", (await page.$eval("#bal-ember", (el) => el.textContent)) === "5");

console.log("forge via UI");
await page.click("#btn-forge");
check("one Relic owned", (await page.$eval("#bal-relics", (el) => el.textContent)) === "1");
check("EMBER spent back to 0", (await page.$eval("#bal-ember", (el) => el.textContent)) === "0");
check("TARI fee paid (10 -> 9)", (await page.$eval("#bal-tari", (el) => el.textContent)) === "9");
check("forge reveal shown", await page.$eval("#forge-reveal", (el) => !el.hidden && el.textContent.includes("Relic #0")));
check("fee banked in stats", (await page.$eval("#stat-fees", (el) => el.textContent)) === "1 TARI");

console.log("melt via UI");
const meltValue = await page.evaluate(() => {
  const { sim, SPEC } = window.__forge;
  const relic = [...sim.player("You").relics.values()][0];
  return SPEC.MELT_VALUES[relic.tier];
});
await page.click("[data-melt]");
check("no Relics left", (await page.$eval("#bal-relics", (el) => el.textContent)) === "0");
check(
  `melt paid ${meltValue} EMBER`,
  (await page.$eval("#bal-ember", (el) => el.textContent)) === String(meltValue)
);
check("melt counted in stats", (await page.$eval("#stat-melted", (el) => el.textContent)) === "1");

console.log("engine rule checks");
const ruleResults = await page.evaluate(() => {
  const { sim, ForgeError } = window.__forge;
  const out = {};
  const you = () => sim.player("You");
  // Clear to a known state.
  for (const s of [...you().relics.keys()]) sim.melt("You", s);
  you().embers = 0;
  try {
    sim.forge("You");
    out.forgeWithoutEmbers = "no-throw";
  } catch (e) {
    out.forgeWithoutEmbers = e instanceof ForgeError ? "threw" : "wrong-error";
  }
  you().embers = 5;
  you().tari = 0;
  try {
    sim.forge("You");
    out.forgeWithoutFee = "no-throw";
  } catch (e) {
    out.forgeWithoutFee = e instanceof ForgeError ? "threw" : "wrong-error";
  }
  you().tari = 50;
  // Forge until three of a kind (cap 200 — commons alone make this quick).
  let trio = null;
  for (let i = 0; i < 200 && !trio; i++) {
    sim.gather("You");
    sim.forge("You");
    const byTier = new Map();
    for (const r of you().relics.values()) {
      if (!byTier.has(r.tier)) byTier.set(r.tier, []);
      byTier.get(r.tier).push(r.serial);
    }
    for (const [tier, serials] of byTier) {
      if (serials.length >= 3 && tier < 3) trio = { tier, serials: serials.slice(0, 3) };
    }
  }
  out.trioFound = Boolean(trio);
  if (trio) {
    const before = you().relics.size;
    const ascended = sim.ascend("You", trio.serials);
    out.ascendTierUp = ascended.tier === trio.tier + 1;
    out.ascendShrinksBy2 = you().relics.size === before - 2;
    // Mixed tiers must throw.
    const tiers = [...you().relics.values()];
    const a = tiers[0];
    const b = tiers.find((r) => r.tier !== a.tier);
    const c = tiers.find((r) => r !== a && r !== b);
    if (a && b && c) {
      try {
        sim.ascend("You", [a.serial, b.serial, c.serial]);
        out.mixedAscend = "no-throw";
      } catch (e) {
        out.mixedAscend = e instanceof ForgeError ? "threw" : "wrong-error";
      }
    } else {
      out.mixedAscend = "skipped-not-enough-variety";
    }
  }
  window.__forge.render();
  return out;
});
check("forge without EMBER throws", ruleResults.forgeWithoutEmbers === "threw", JSON.stringify(ruleResults));
check("forge without fee throws", ruleResults.forgeWithoutFee === "threw");
check("three-of-a-kind reached", ruleResults.trioFound);
check("ascend mints exactly one tier up", ruleResults.ascendTierUp === true);
check("ascend shrinks collection by 2", ruleResults.ascendShrinksBy2 === true);
check(
  "mixed-tier ascend throws",
  ruleResults.mixedAscend === "threw" || ruleResults.mixedAscend === "skipped-not-enough-variety",
  ruleResults.mixedAscend
);

console.log("ascend via UI selection");
const uiAscend = await page.evaluate(() => {
  // Ensure three same-tier relics exist (engine), then let the test click.
  const { sim } = window.__forge;
  const you = sim.player("You");
  you.tari = Math.max(you.tari, 300);
  let trio = null;
  for (let i = 0; i < 300 && !trio; i++) {
    if (you.embers < 5) sim.gather("You");
    sim.forge("You");
    const byTier = new Map();
    for (const r of you.relics.values()) {
      if (!byTier.has(r.tier)) byTier.set(r.tier, []);
      byTier.get(r.tier).push(r.serial);
    }
    for (const [tier, serials] of byTier) {
      if (serials.length >= 3 && tier < 3) trio = serials.slice(0, 3);
    }
  }
  window.__forge.render();
  return { trio, size: you.relics.size, ascendedBefore: sim.stats.ascended };
});
check("UI ascend setup found a trio", Boolean(uiAscend.trio));
if (uiAscend.trio) {
  for (const serial of uiAscend.trio) {
    await page.click(`.relic-card[data-serial="${serial}"]`);
  }
  check("ascend button enabled", await page.$eval("#btn-ascend", (el) => !el.disabled));
  await page.click("#btn-ascend");
  const after = await page.evaluate(() => ({
    size: window.__forge.sim.player("You").relics.size,
    ascended: window.__forge.sim.stats.ascended,
  }));
  check("UI ascend shrinks collection by 2", after.size === uiAscend.size - 2, `${uiAscend.size} -> ${after.size}`);
  check("UI ascend counted", after.ascended === uiAscend.ascendedBefore + 1);
}

console.log("persistence");
await page.reload({ waitUntil: "networkidle0" });
await page.waitForSelector("#btn-gather");
const persisted = await page.evaluate(() => window.__forge.sim.stats.forged);
check("state survives reload (localStorage)", persisted > 0, `forged=${persisted}`);

await page.screenshot({ path: SHOT, fullPage: true });
console.log(`screenshot: ${SHOT}`);

check("zero console/page errors", errors.length === 0, errors.join(" | "));

await browser.close();
console.log(failures === 0 ? "\nALL CHECKS PASSED" : `\n${failures} CHECK(S) FAILED`);
process.exit(failures === 0 ? 0 : 1);
