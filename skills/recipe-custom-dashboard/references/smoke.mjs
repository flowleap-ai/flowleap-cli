#!/usr/bin/env node
/*=============================================================================
 * Smoke test for the four dashboard templates.
 *
 * Each case copies a template into a fresh bundle as generate.mjs, puts
 * recorded tool answers from ./fixtures/ into its data/ folder under the file
 * names the template reads, runs it with plain `node`, and asserts:
 *   - `--plan` names the tool calls and the data/ file of each answer,
 *   - the template exits 0 and writes dashboard.html,
 *   - the HTML has the provenance-footer markers and >= 1 <svg>,
 *   - NO external URL appears in any src=/href= attribute (offline-safe),
 *   - known fixture values reach the rendered HTML.
 * Plus the stop paths, each with exit code and NO dashboard: an ambiguous
 * applicant (never auto-picks a candidate), a name that matches only other
 * entities, a saved-how.json with an unknown value, PATSTAT unavailable, a saved answer
 * whose rows do not add up to its totals, a saved answer for another input,
 * and a missing citation page.
 *
 * Zero dependencies; run with `node smoke.mjs`. Exits non-zero on any failure.
 *===========================================================================*/

import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, readFileSync, existsSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(HERE, 'fixtures');
const fixture = (name) => readFileSync(join(FIXTURES, name), 'utf8');

const failures = [];
function check(name, cond, detail = '') {
	if (cond) { console.log(`  ok   ${name}`); }
	else { console.log(`  FAIL ${name}${detail ? ` (${detail})` : ''}`); failures.push(`${name}${detail ? `: ${detail}` : ''}`); }
}

/**
 * Copy a template into a fresh bundle, write `data` ({ file: text }) into its
 * data/ folder, run it with `args`, and capture the result.
 */
function runTemplate(templateFile, data, { swap, args = [] } = {}) {
	const work = mkdtempSync(join(tmpdir(), 'dash-smoke-'));
	const bundle = join(work, 'bundle');
	mkdirSync(join(bundle, 'data'), { recursive: true });
	let src = readFileSync(join(HERE, templateFile), 'utf8');
	if (swap) { src = src.replace(swap[0], swap[1]); }
	writeFileSync(join(bundle, 'generate.mjs'), src);
	for (const [file, text] of Object.entries(data)) writeFileSync(join(bundle, 'data', file), text);
	let status = 0, stdout = '', stderr = '';
	try {
		stdout = execFileSync('node', [join(bundle, 'generate.mjs'), ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
	} catch (e) {
		status = e.status ?? 1; stdout = e.stdout || ''; stderr = e.stderr || '';
	}
	const htmlPath = join(bundle, 'dashboard.html');
	const html = existsSync(htmlPath) ? readFileSync(htmlPath, 'utf8') : null;
	rmSync(work, { recursive: true, force: true });
	return { status, stdout, stderr, html };
}

/** External URLs are only forbidden inside src=/href= attributes (offline-safe rendering). */
function externalAttrUrls(html) {
	return [...html.matchAll(/(?:src|href)\s*=\s*"([^"]*)"/gi)].map((m) => m[1]).filter((v) => /^https?:/i.test(v));
}

function assertPlan(label, templateFile, expected) {
	const r = runTemplate(templateFile, {}, { args: ['--plan'] });
	let plan = [];
	try { plan = JSON.parse(r.stdout); } catch { /* checked below */ }
	check(`${label}: --plan exits 0`, r.status === 0, r.stderr);
	for (const [tool, saveTo] of expected) {
		check(`${label}: --plan names ${tool} -> ${saveTo}`, plan.some((p) => p.tool === tool && p.saveTo === saveTo));
	}
}

function assertRendered(label, r, expectValues) {
	check(`${label}: exits 0`, r.status === 0, `status=${r.status} ${r.stderr}`);
	check(`${label}: dashboard.html written`, !!r.html);
	if (!r.html) return;
	check(`${label}: has provenance footer id`, r.html.includes('id="provenance"'));
	for (const marker of ['Provenance', 'Reproduce', 'Generated:', 'Dataset identity:', 'Retrieved:', 'Saved:']) {
		check(`${label}: footer marker "${marker}"`, r.html.includes(marker));
	}
	check(`${label}: has >= 1 <svg>`, (r.html.match(/<svg/g) || []).length >= 1);
	const ext = externalAttrUrls(r.html);
	check(`${label}: no external src/href URLs`, ext.length === 0, ext.join(', '));
	for (const v of expectValues) {
		check(`${label}: renders fixture value "${v}"`, r.html.includes(v));
	}
}

function assertStopped(label, r, status, stderrPattern) {
	check(`${label}: exits ${status}`, r.status === status, `status=${r.status} ${r.stderr}`);
	check(`${label}: writes no dashboard`, r.html === null);
	check(`${label}: says ${stderrPattern}`, stderrPattern.test(r.stderr), r.stderr);
}

const PORTFOLIO = { 'portfolio.json': fixture('portfolio-siemens.json') };
const TRENDS = {
	'portfolio-siemens.json': fixture('portfolio-siemens.json'),
	'portfolio-robert-bosch.json': fixture('portfolio-robert-bosch.json'),
	'portfolio-lg-electronics.json': fixture('portfolio-lg-electronics.json'),
};
const LANDSCAPE = Object.fromEntries(['analytics-overview.json', ...['g06n', 'h04l', 'g06f', 'a61b', 'g16h'].map((c) => `analytics-cpc-${c}.json`)]
	.map((f) => [f, fixture(f)]));
const CITATIONS = Object.fromEntries(['citations-0.json', 'citations-25.json', 'citations-50.json'].map((f) => [f, fixture(f)]));

console.log('Dashboard template smoke test\n=============================');

console.log('\n[portfolio]');
assertPlan('portfolio', 'template-portfolio.mjs', [['patstat_portfolio', 'data/portfolio.json']]);
assertRendered('portfolio', runTemplate('template-portfolio.mjs', PORTFOLIO), ['38,658 patent applications', 'PATSTAT 2026 Spring', 'SIEMENS', 'save method not recorded']);

console.log('\n[filing-trends]');
assertPlan('filing-trends', 'template-filing-trends.mjs', [['patstat_portfolio', 'data/portfolio-robert-bosch.json']]);
assertRendered('filing-trends', runTemplate('template-filing-trends.mjs', TRENDS), ['LG ELECTRONICS', 'ROBERT BOSCH', 'SIEMENS', 'PATSTAT 2026 Spring']);

console.log('\n[landscape-whitespace]');
assertPlan('landscape', 'template-landscape-whitespace.mjs', [['patent_analytics', 'data/analytics-overview.json'], ['patent_analytics', 'data/analytics-cpc-g16h.json']]);
assertRendered('landscape-whitespace', runTemplate('template-landscape-whitespace.mjs', LANDSCAPE), ['machine learning', 'G16H', '1,520', 'data_slice: BigQuery']);

console.log('\n[citation-impact]');
assertPlan('citation-impact', 'template-citation-impact.mjs', [['search_enriched_citations', 'data/citations-0.json']]);
assertRendered('citation-impact', runTemplate('template-citation-impact.mjs', CITATIONS), ['US 20150284881 A1', 'USPTO enriched citation data', '64']);

console.log('\n[saved as text blocks]');
assertRendered('text blocks', runTemplate('template-portfolio.mjs', { 'portfolio.json': JSON.stringify([{ type: 'text', text: PORTFOLIO['portfolio.json'] }]) }), ['38,658 patent applications']);

console.log('\n[footer: how each file was saved]');
assertRendered('saved-how', runTemplate('template-portfolio.mjs', { ...PORTFOLIO, 'saved-how.json': JSON.stringify({ 'portfolio.json': 'written' }) }), ['recorded by the agent as written out']);
assertStopped('saved-how bad value', runTemplate('template-portfolio.mjs', { ...PORTFOLIO, 'saved-how.json': JSON.stringify({ 'portfolio.json': 'typed' }) }), 4, /use "copied" or "written"/);

console.log('\n[guardrail: ambiguous applicant]');
assertStopped('ambiguous', runTemplate('template-portfolio.mjs', { 'portfolio.json': fixture('error-applicant-ambiguous.json') }), 3, /ZZZ AMBIGUOUS CO (HOLDING|TECH)/);

console.log('\n[guardrail: ambiguous applicant among several]');
const skip = runTemplate('template-filing-trends.mjs', { ...TRENDS, 'portfolio-lg-electronics.json': fixture('error-applicant-ambiguous.json') });
assertRendered('skip ambiguous', skip, ['Skipped as ambiguous', 'ROBERT BOSCH']);

console.log('\n[guardrail: name matches only other entities]');
assertStopped('only other matches', runTemplate('template-portfolio.mjs', { 'portfolio.json': fixture('error-applicant-only-other-matches.json') }, { swap: ["'Siemens'", "'Siemens AG'"] }), 3, /matched no applications of its own.*\n.*SIEMENS AG DE/);
const onlyOthers = runTemplate('template-filing-trends.mjs', { ...TRENDS, 'portfolio-siemens-ag.json': fixture('error-applicant-only-other-matches.json') }, { swap: ["'Siemens'", "'Siemens AG'"] });
assertRendered('skip only other matches', onlyOthers, ['matching only other entities', 'Siemens AG', 'ROBERT BOSCH']);

console.log('\n[degrade: patstat unavailable]');
assertStopped('unavailable', runTemplate('template-portfolio.mjs', { 'portfolio.json': fixture('error-patstat-unavailable.json') }), 2, /landscape and citation-impact/);

console.log('\n[check: rows do not add up]');
const tampered = JSON.parse(PORTFOLIO['portfolio.json']);
tampered.totals.applications += 1;
assertStopped('tampered', runTemplate('template-portfolio.mjs', { 'portfolio.json': JSON.stringify(tampered) }), 4, /data\/portfolio\.json is not the answer/);

console.log('\n[check: answer for another input]');
assertStopped('other input', runTemplate('template-portfolio.mjs', PORTFOLIO, { swap: ["'Siemens'", "'Robert Bosch'"] }), 4, /applicant\.query/);

console.log('\n[check: missing citation page]');
const { 'citations-50.json': _omitted, ...twoPages } = CITATIONS;
assertStopped('missing page', runTemplate('template-citation-impact.mjs', twoPages), 4, /Missing data\/citations-50\.json.*"offset":50/);

console.log('\n=============================');
if (failures.length) {
	console.log(`FAILED (${failures.length}):`);
	for (const f of failures) console.log(`  - ${f}`);
	process.exit(1);
}
console.log('All dashboard template smoke checks passed.');
