#!/usr/bin/env node
/*=============================================================================
 * CI smoke test for the four dashboard templates.
 *
 * Runs each template against its recorded fixtures with the FlowLeap CLI
 * STUBBED (no network), then asserts the product invariants:
 *   - the template exits 0 and writes dashboard.html,
 *   - the HTML has the provenance-footer markers and >= 1 <svg>,
 *   - NO external URL appears in any src=/href= attribute (offline-safe),
 *   - known fixture values reach the rendered HTML,
 *   - a raw-JSON sidecar per backend call is written under data/.
 * Plus the guardrail path: an ambiguous applicant exits non-zero, lists the
 * candidates, and writes NO dashboard (never auto-picks a 422 candidate).
 *
 * Zero dependencies; run with `node smoke.mjs`. Exits non-zero on any failure.
 *===========================================================================*/

import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, cpSync, readFileSync, existsSync, writeFileSync, chmodSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(HERE, 'fixtures');
const STUB = join(HERE, 'stub-flowleap.mjs');

const failures = [];
function check(name, cond, detail = '') {
	if (cond) { console.log(`  ok   ${name}`); }
	else { console.log(`  FAIL ${name}${detail ? ` — ${detail}` : ''}`); failures.push(`${name}${detail ? `: ${detail}` : ''}`); }
}

/** Build a tiny always-executable shim so templates can execFile the stub regardless of file mode. */
function makeShim(dir) {
	const shim = join(dir, 'flowleap');
	writeFileSync(shim, `#!/bin/sh\nexec node "${STUB}" "$@"\n`);
	chmodSync(shim, 0o755);
	return shim;
}

/** Copy a template into a fresh bundle dir, run it stubbed, capture result. */
function runTemplate(templateFile, { applicantSwap } = {}) {
	const work = mkdtempSync(join(tmpdir(), 'dash-smoke-'));
	const bundle = join(work, 'bundle');
	mkdirSync(bundle, { recursive: true });
	let src = readFileSync(join(HERE, templateFile), 'utf8');
	if (applicantSwap) { src = src.replace(applicantSwap[0], applicantSwap[1]); }
	writeFileSync(join(bundle, 'generate.mjs'), src);
	const env = { ...process.env, FLOWLEAP_CLI_BIN: makeShim(work), FLOWLEAP_STUB_DIR: FIXTURES };
	let status = 0, stdout = '', stderr = '';
	try {
		stdout = execFileSync('node', [join(bundle, 'generate.mjs')], { encoding: 'utf8', env });
	} catch (e) {
		status = e.status ?? 1; stdout = e.stdout || ''; stderr = e.stderr || '';
	}
	const htmlPath = join(bundle, 'dashboard.html');
	const html = existsSync(htmlPath) ? readFileSync(htmlPath, 'utf8') : null;
	const dataFiles = existsSync(join(bundle, 'data')) ? readdirSync(join(bundle, 'data')) : [];
	return { status, stdout, stderr, html, dataFiles, cleanup: () => rmSync(work, { recursive: true, force: true }) };
}

/** External URLs are only forbidden inside src=/href= attributes (offline-safe rendering). */
function externalAttrUrls(html) {
	return [...html.matchAll(/(?:src|href)\s*=\s*"([^"]*)"/gi)].map((m) => m[1]).filter((v) => /^https?:/i.test(v));
}

function assertRendered(label, r, expectValues) {
	console.log(`\n[${label}]`);
	check(`${label}: exits 0`, r.status === 0, `status=${r.status} ${r.stderr}`);
	check(`${label}: dashboard.html written`, !!r.html);
	if (!r.html) return;
	check(`${label}: has provenance footer id`, r.html.includes('id="provenance"'));
	for (const marker of ['Provenance', 'Reproduce', 'Generated:', 'Dataset identity:', 'Retrieved:']) {
		check(`${label}: footer marker "${marker}"`, r.html.includes(marker));
	}
	check(`${label}: has >= 1 <svg>`, (r.html.match(/<svg/g) || []).length >= 1);
	const ext = externalAttrUrls(r.html);
	check(`${label}: no external src/href URLs`, ext.length === 0, ext.join(', '));
	check(`${label}: writes data/ sidecars`, r.dataFiles.length >= 1, `data files: ${r.dataFiles.length}`);
	for (const v of expectValues) {
		check(`${label}: renders fixture value "${v}"`, r.html.includes(v));
	}
}

console.log('Dashboard template smoke test\n=============================');

// 1-4: the four templates render green against fixtures.
let r;
r = runTemplate('template-portfolio.mjs');
assertRendered('portfolio', r, ['15202 patent applications', 'PATSTAT Global 2025 Spring', 'SIEMENS']);
r.cleanup();

r = runTemplate('template-filing-trends.mjs');
assertRendered('filing-trends', r, ['LG ELECTRONICS', 'ROBERT BOSCH', 'SIEMENS', 'PATSTAT Global 2025 Spring']);
r.cleanup();

r = runTemplate('template-landscape-whitespace.mjs');
assertRendered('landscape-whitespace', r, ['machine learning', 'G16H', '1,520']);
r.cleanup();

r = runTemplate('template-citation-impact.mjs');
assertRendered('citation-impact', r, ['US 20150284881 A1', 'USPTO Enriched Citations', '64']);
r.cleanup();

// 5: guardrail — an ambiguous (un-fixtured) applicant must exit non-zero, list
// candidates, and write NO dashboard (rule 6: never auto-pick a 422 candidate).
// Exercises the tool error shape { success:false, error:{ code, message, details:{ candidates } }, status }.
console.log('\n[guardrail: ambiguous applicant]');
const amb = runTemplate('template-portfolio.mjs', { applicantSwap: ["'Siemens AG'", "'Zzz Ambiguous Co'"] });
check('ambiguous: exits non-zero', amb.status !== 0, `status=${amb.status}`);
check('ambiguous: writes no dashboard', amb.html === null);
check('ambiguous: lists candidates', /Zzz Ambiguous Co (HOLDING|TECH)/.test(amb.stderr));
amb.cleanup();

// 6: degrade — patstat_unavailable must exit non-zero, name the usable
// dashboards, and write NO dashboard. Exercises the patstat_unavailable tool error shape.
console.log('\n[degrade: patstat unavailable]');
const un = runTemplate('template-portfolio.mjs', { applicantSwap: ["'Siemens AG'", "'patstat unavailable'"] });
check('unavailable: exits non-zero', un.status !== 0, `status=${un.status}`);
check('unavailable: writes no dashboard', un.html === null);
check('unavailable: names usable dashboards', /landscape and citation-impact/.test(un.stderr));
un.cleanup();

console.log('\n=============================');
if (failures.length) {
	console.log(`FAILED (${failures.length}):`);
	for (const f of failures) console.log(`  - ${f}`);
	process.exit(1);
}
console.log('All dashboard template smoke checks passed.');
