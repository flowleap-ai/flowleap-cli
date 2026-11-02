#!/usr/bin/env node
/*=============================================================================
 * Dashboard template: LANDSCAPE WHITE-SPACE (Topic Analytics)
 *
 * QUESTION  Where is the white space in a technology area? A CPC x year heatmap
 *           of publication counts: sparse or absent cells are candidate white space.
 * BASIS     byYear counts publications by publication year (each office's
 *           publication is one row; the data_slice names publications_slim).
 *           topCPC counts classification entries, not documents, so it only
 *           picks the codes: each heatmap row is the byYear of its own scoped call.
 * DATA      Answers of the FlowLeap Data tool `patent_analytics`, each saved
 *           unchanged to data/:
 *             { searchDescription, request, analytics:{ byYear:[{year,count}],
 *               byCountry, topAssignees, topCPC }, data_slice }
 *           One BROAD call (data/analytics-overview.json) shows the active
 *           classes (topCPC); then ONE scoped call per CPC
 *           (data/analytics-cpc-<code>.json) gives that row's byYear. The
 *           heatmap is a real cross-tab of scoped calls, never invented.
 *
 * HOW TO ADAPT
 *   1. Set KEYWORDS and/or PHRASES + DATE_FROM/DATE_TO to scope the area
 *      (keywords are OR-combined substrings; a phrase is one exact substring).
 *   2. Pin CPC_CODES to the subclasses to compare (read them from the broad
 *      call's topCPC and pin the ones you care about).
 *   3. `node generate.mjs --plan` prints the tool calls and their files.
 *      Make the calls, save the answers, then run `node generate.mjs`.
 *
 * CHECKS    Each saved answer must echo its pinned request (exit 4 otherwise).
 *           data_slice is the dataset identity; a missing one renders the
 *           explicit "dataset identity unavailable" (rule 5). This template does
 *           NOT need PATSTAT.
 *===========================================================================*/

// ---- PINNED CONSTANTS (edit these) -----------------------------------------
const KEYWORDS = [];
const PHRASES = ['machine learning'];
const DATE_FROM = '2015-01-01';
const DATE_TO = '2024-12-31';
const CPC_CODES = ['G06N', 'H04L', 'G06F', 'A61B', 'G16H'];
// ----------------------------------------------------------------------------

import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';

/** The patent_analytics input for one call: only the criteria that are set. */
function analyticsInput(cpc) {
	return {
		...(KEYWORDS.length ? { keywords: KEYWORDS } : {}),
		...(PHRASES.length ? { phrases: PHRASES } : {}),
		...(cpc ? { cpc: [cpc] } : {}),
		dateFrom: DATE_FROM, dateTo: DATE_TO,
	};
}
const OVERVIEW = { tool: 'patent_analytics', input: analyticsInput(null), file: 'analytics-overview.json' };
const SCOPED = CPC_CODES.map((cpc) => ({ cpc, tool: 'patent_analytics', input: analyticsInput(cpc), file: `analytics-cpc-${cpc.toLowerCase()}.json` }));

/*======================= COMMON BLOCK (copy-in) ==============================
 * Identical across the four dashboard templates: the reader of the saved tool
 * answers, inline SVG chart helpers, HTML shell, and the provenance/reproduce
 * footer.
 *===========================================================================*/

const OUT_DIR = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(OUT_DIR, 'data');

/** A saved tool answer that is missing, is not the answer to the pinned input, or is an error answer. */
class ToolAnswerError extends Error {
	constructor(message, { code = null, candidates = null, exitCode = 4 } = {}) {
		super(message);
		this.code = code;
		this.candidates = candidates;
		this.exitCode = exitCode;
	}
}

/** The error of an error answer ({ error:{ code, message, details } }, also inside { body }), or null. */
function answerError(value) {
	if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
	const body = value.body && typeof value.body === 'object' ? value.body : value;
	const err = body.error && typeof body.error === 'object' ? body.error : null;
	if (!err && body.success !== false) return null;
	return { code: err?.code ?? null, message: err?.message ?? '', candidates: err?.details?.candidates ?? null };
}

/** Parse saved answer text: the answer JSON itself, or a list of text blocks that holds it. */
function parseAnswerText(text, file) {
	let value;
	try { value = JSON.parse(text); } catch {
		throw new ToolAnswerError(`${file} is not JSON. Save the tool answer exactly as the tool returned it.`);
	}
	if (Array.isArray(value) && value.length > 0 && value.every((b) => b && b.type === 'text' && typeof b.text === 'string')) {
		return parseAnswerText(value.map((b) => b.text).join(''), file);
	}
	return value;
}

/**
 * Read the saved answer of one tool call. source = { tool, input, file }.
 * Returns { value, savedAt } (savedAt = the time the answer file was written).
 * Throws ToolAnswerError for a missing file, a file that is not JSON, or an error answer.
 */
function readToolAnswer(source) {
	const rel = `data/${source.file}`;
	const path = join(DATA_DIR, source.file);
	if (!existsSync(path)) {
		throw new ToolAnswerError(`Missing ${rel}. Call ${source.tool} with ${JSON.stringify(source.input)} and save its answer there.`);
	}
	const value = parseAnswerText(readFileSync(path, 'utf8'), rel);
	const err = answerError(value);
	if (err) {
		throw new ToolAnswerError(`${rel}: ${source.tool} answered an error${err.code ? ` (${err.code})` : ''}: ${err.message}`,
			{ code: err.code, candidates: err.candidates });
	}
	return { value, savedAt: statSync(path).mtime.toISOString() };
}

/** The two ways an answer file can be saved (recipe rule 2), as data/saved-how.json names them. */
const SAVE_METHODS = { copied: 'copied from the stored result', written: 'written out by the agent' };

/**
 * How the agent saved one answer file, from data/saved-how.json
 * ({ "<file>": "copied" | "written" }), which the agent updates after each save.
 * Returns the footer text; a file without an entry is "save method not recorded".
 * Throws ToolAnswerError when saved-how.json is not JSON or holds another value.
 */
function savedHow(sidecar) {
	const path = join(DATA_DIR, 'saved-how.json');
	if (!existsSync(path)) return 'save method not recorded';
	let map;
	try { map = JSON.parse(readFileSync(path, 'utf8')); } catch {
		throw new ToolAnswerError('data/saved-how.json is not JSON. Write it as { "<file>": "copied" | "written" }.');
	}
	const file = sidecar.replace(/^data\//, '');
	if (!Object.hasOwn(map, file)) return 'save method not recorded';
	if (!Object.hasOwn(SAVE_METHODS, map[file])) {
		throw new ToolAnswerError(`data/saved-how.json: ${file} is ${JSON.stringify(map[file])}; use "copied" or "written".`);
	}
	return SAVE_METHODS[map[file]];
}

/** Stop when a saved answer does not match its pinned input or its own totals. */
function expectAnswer(cond, source, what) {
	if (!cond) {
		throw new ToolAnswerError(`data/${source.file} is not the answer of ${source.tool} with ${JSON.stringify(source.input)}: ${what}. Call the tool again and save its answer without changes.`);
	}
}

const sum = (rows, key) => rows.reduce((a, r) => a + (typeof r[key] === 'number' ? r[key] : 0), 0);
const sameList = (a, b) => JSON.stringify([...(a ?? [])].sort()) === JSON.stringify([...(b ?? [])].sort());

/**
 * Entry point: `node generate.mjs --plan` prints the tool calls to make and the
 * file of each answer as JSON; `node generate.mjs` renders the dashboard.
 */
function run(sources, main) {
	if (process.argv.includes('--plan')) {
		console.log(JSON.stringify(sources.map((s) => ({ tool: s.tool, input: s.input, saveTo: `data/${s.file}` })), null, 2));
		return;
	}
	try {
		main();
	} catch (e) {
		if (!(e instanceof ToolAnswerError)) throw e;
		console.error(e.message);
		process.exit(e.exitCode);
	}
}

const fmtInt = (n) => Number(n ?? 0).toLocaleString('en-US');
const svgEsc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const htmlEsc = svgEsc;

// Okabe-Ito colorblind-safe categorical palette; single-hue ramp for heatmaps.
const OKABE_ITO = ['#0072B2', '#E69F00', '#009E73', '#D55E00', '#CC79A7', '#56B4E9', '#F0E442', '#000000'];
const BLUES = ['#deebf7', '#9ecae1', '#6baed6', '#3182bd', '#08519c'];
const AXIS = '#333', GRID = '#e2e2e2', INK = '#1a1a1a', MUTE = '#5a5a5a';

/** Round an axis maximum up to a readable value. */
function niceMax(v) {
	if (v <= 0) return 1;
	const p = Math.pow(10, Math.floor(Math.log10(v)));
	const f = v / p;
	const n = f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10;
	return n * p;
}

function chartFrame(title, source, w, h, inner) {
	return `<svg viewBox="0 0 ${w} ${h}" width="100%" role="img" aria-label="${svgEsc(title)}" style="max-width:${w}px;background:#fff;font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif">
<title>${svgEsc(title)}</title>
<text x="16" y="24" fill="${INK}" font-size="16" font-weight="600">${svgEsc(title)}</text>
<text x="16" y="42" fill="${MUTE}" font-size="11">Source: ${svgEsc(source)}</text>
${inner}
</svg>`;
}

function legend(x, y, series) {
	return series.map((s, i) => {
		const cy = y + i * 18;
		return `<rect x="${x}" y="${cy - 9}" width="11" height="11" fill="${OKABE_ITO[i % OKABE_ITO.length]}"/>` +
			`<text x="${x + 16}" y="${cy}" fill="${INK}" font-size="11">${svgEsc(s.name)}</text>`;
	}).join('');
}

/** Grouped or stacked bar chart. series = [{name, values:[]}] aligned to categories. */
function barChart({ title, source, categories, series, yLabel = 'Count', stacked = false }) {
	const W = 760, H = 420, m = { t: 64, r: 150, b: 70, l: 64 };
	const pw = W - m.l - m.r, ph = H - m.t - m.b;
	const max = niceMax(stacked
		? Math.max(1, ...categories.map((_, ci) => series.reduce((a, s) => a + (s.values[ci] || 0), 0)))
		: Math.max(1, ...series.flatMap((s) => s.values)));
	const y = (v) => m.t + ph - (v / max) * ph;
	const bandW = pw / Math.max(categories.length, 1);
	const ticks = 4;
	let g = '';
	for (let i = 0; i <= ticks; i++) {
		const v = (max / ticks) * i, yy = y(v);
		g += `<line x1="${m.l}" y1="${yy}" x2="${m.l + pw}" y2="${yy}" stroke="${GRID}"/>` +
			`<text x="${m.l - 8}" y="${yy + 4}" fill="${MUTE}" font-size="10" text-anchor="end">${fmtInt(v)}</text>`;
	}
	let bars = '';
	categories.forEach((cat, ci) => {
		const x0 = m.l + ci * bandW;
		if (stacked) {
			let acc = 0;
			series.forEach((s, si) => {
				const v = s.values[ci] || 0; if (!v) return;
				const yTop = y(acc + v), hgt = y(acc) - y(acc + v);
				bars += `<rect x="${x0 + bandW * 0.18}" y="${yTop}" width="${bandW * 0.64}" height="${Math.max(hgt, 0)}" fill="${OKABE_ITO[si % OKABE_ITO.length]}"><title>${svgEsc(cat)} - ${svgEsc(s.name)}: ${fmtInt(v)}</title></rect>`;
				acc += v;
			});
		} else {
			const gw = (bandW * 0.72) / Math.max(series.length, 1);
			series.forEach((s, si) => {
				const v = s.values[ci] || 0;
				const bx = x0 + bandW * 0.14 + si * gw;
				bars += `<rect x="${bx}" y="${y(v)}" width="${Math.max(gw - 2, 1)}" height="${m.t + ph - y(v)}" fill="${OKABE_ITO[si % OKABE_ITO.length]}"><title>${svgEsc(cat)} - ${svgEsc(s.name)}: ${fmtInt(v)}</title></rect>`;
			});
		}
		bars += `<text x="${x0 + bandW / 2}" y="${m.t + ph + 16}" fill="${MUTE}" font-size="10" text-anchor="middle">${svgEsc(cat)}</text>`;
	});
	const axes = `<line x1="${m.l}" y1="${m.t}" x2="${m.l}" y2="${m.t + ph}" stroke="${AXIS}"/>` +
		`<line x1="${m.l}" y1="${m.t + ph}" x2="${m.l + pw}" y2="${m.t + ph}" stroke="${AXIS}"/>` +
		`<text transform="translate(16,${m.t + ph / 2}) rotate(-90)" fill="${MUTE}" font-size="11" text-anchor="middle">${svgEsc(yLabel)}</text>`;
	return chartFrame(title, source, W, H, g + axes + bars + legend(m.l + pw + 20, m.t + 6, series));
}

/** Multi-series line chart over an ordered category axis (e.g. years). */
function lineChart({ title, source, categories, series, yLabel = 'Count' }) {
	const W = 760, H = 420, m = { t: 64, r: 150, b: 70, l: 64 };
	const pw = W - m.l - m.r, ph = H - m.t - m.b;
	const max = niceMax(Math.max(1, ...series.flatMap((s) => s.values)));
	const y = (v) => m.t + ph - (v / max) * ph;
	const x = (i) => m.l + (categories.length <= 1 ? pw / 2 : (i / (categories.length - 1)) * pw);
	const ticks = 4;
	let g = '';
	for (let i = 0; i <= ticks; i++) {
		const v = (max / ticks) * i, yy = y(v);
		g += `<line x1="${m.l}" y1="${yy}" x2="${m.l + pw}" y2="${yy}" stroke="${GRID}"/>` +
			`<text x="${m.l - 8}" y="${yy + 4}" fill="${MUTE}" font-size="10" text-anchor="end">${fmtInt(v)}</text>`;
	}
	categories.forEach((cat, i) => {
		g += `<text x="${x(i)}" y="${m.t + ph + 16}" fill="${MUTE}" font-size="10" text-anchor="middle">${svgEsc(cat)}</text>`;
	});
	let lines = '';
	series.forEach((s, si) => {
		const col = OKABE_ITO[si % OKABE_ITO.length];
		const pts = s.values.map((v, i) => `${x(i)},${y(v)}`).join(' ');
		lines += `<polyline points="${pts}" fill="none" stroke="${col}" stroke-width="2.5"/>`;
		lines += s.values.map((v, i) => `<circle cx="${x(i)}" cy="${y(v)}" r="3.2" fill="${col}"><title>${svgEsc(s.name)} ${svgEsc(categories[i])}: ${fmtInt(v)}</title></circle>`).join('');
	});
	const axes = `<line x1="${m.l}" y1="${m.t}" x2="${m.l}" y2="${m.t + ph}" stroke="${AXIS}"/>` +
		`<line x1="${m.l}" y1="${m.t + ph}" x2="${m.l + pw}" y2="${m.t + ph}" stroke="${AXIS}"/>` +
		`<text transform="translate(16,${m.t + ph / 2}) rotate(-90)" fill="${MUTE}" font-size="11" text-anchor="middle">${svgEsc(yLabel)}</text>`;
	return chartFrame(title, source, W, H, g + axes + lines + legend(m.l + pw + 20, m.t + 6, series));
}

/** Heatmap. rows/cols are labels; matrix[r][c] is the value. Empty cells (0) flag white space. */
function heatmap({ title, source, rows, cols, matrix, valueLabel = 'Filings' }) {
	const cell = 46, labelW = 96, top = 64, left = labelW + 8;
	const W = Math.max(760, left + cols.length * cell + 24);
	const H = top + rows.length * cell + 96;
	const max = Math.max(1, ...matrix.flat());
	const colorFor = (v) => (v <= 0 ? '#f5f5f5' : BLUES[Math.min(BLUES.length - 1, Math.floor((v / max) * BLUES.length - 1e-9))]);
	let cells = '';
	cols.forEach((c, ci) => {
		cells += `<text x="${left + ci * cell + cell / 2}" y="${top - 8}" fill="${MUTE}" font-size="10" text-anchor="middle">${svgEsc(c)}</text>`;
	});
	rows.forEach((r, ri) => {
		cells += `<text x="${labelW}" y="${top + ri * cell + cell / 2 + 4}" fill="${INK}" font-size="11" text-anchor="end">${svgEsc(r)}</text>`;
		cols.forEach((c, ci) => {
			const v = matrix[ri][ci] || 0;
			const x = left + ci * cell, yv = top + ri * cell;
			cells += `<rect x="${x}" y="${yv}" width="${cell - 2}" height="${cell - 2}" fill="${colorFor(v)}" stroke="#fff"><title>${svgEsc(r)} / ${svgEsc(c)}: ${fmtInt(v)} ${svgEsc(valueLabel)}</title></rect>`;
			cells += `<text x="${x + cell / 2 - 1}" y="${yv + cell / 2 + 3}" fill="${v / max > 0.6 ? '#fff' : INK}" font-size="10" text-anchor="middle">${v === 0 ? '·' : fmtInt(v)}</text>`;
		});
	});
	// Discrete swatch legend (no gradient — chartjunk-free).
	const ly = top + rows.length * cell + 28;
	let leg = `<text x="${left}" y="${ly}" fill="${MUTE}" font-size="10">Low</text>`;
	['#f5f5f5', ...BLUES].forEach((col, i) => {
		leg += `<rect x="${left + 30 + i * 22}" y="${ly - 10}" width="20" height="12" fill="${col}" stroke="#ccc"/>`;
	});
	leg += `<text x="${left + 30 + 6 * 22 + 4}" y="${ly}" fill="${MUTE}" font-size="10">High &nbsp; (leftmost = white space)</text>`;
	return chartFrame(title, source, W, H, cells + leg);
}

/**
 * Provenance + reproduce footer. `sources` is one descriptor per tool call:
 * { tool, input, datasetIdentity, savedAt, sidecar }. A null datasetIdentity
 * renders the explicit "dataset identity unavailable" row (never omitted).
 * Each row also says how its file was saved (savedHow).
 */
function renderFooter({ generatedAt, sources }) {
	const rows = sources.map((s) => `<li>
		<code>${htmlEsc(s.tool)}</code> <span class="params">${htmlEsc(JSON.stringify(s.input))}</span><br>
		<span class="lbl">Dataset identity:</span> ${s.datasetIdentity ? htmlEsc(s.datasetIdentity) : '<em>dataset identity unavailable</em>'}<br>
		<span class="lbl">Retrieved:</span> ${htmlEsc(s.savedAt)} (answer saved)<br>
		<span class="lbl">Saved:</span> ${htmlEsc(savedHow(s.sidecar))}
		${s.sidecar ? ` &middot; <a href="${htmlEsc(s.sidecar)}">${htmlEsc(s.sidecar)}</a>` : ''}
	</li>`).join('\n');
	const reproduce = [
		'# Call each tool with this input and save its answer, unchanged, to the named file:',
		...sources.map((s) => `${s.tool} ${JSON.stringify(s.input)}  ->  ${s.sidecar}`),
		'node generate.mjs',
	].join('\n');
	return `<footer id="provenance" data-provenance="footer">
	<h2>Provenance &amp; Reproduce</h2>
	<p class="meta"><span class="lbl">Generated:</span> ${htmlEsc(generatedAt)}</p>
	<h3>Data sources</h3>
	<ul class="sources">${rows}</ul>
	<h3>Reproduce</h3>
	<pre class="reproduce">${htmlEsc(reproduce)}</pre>
	<p class="note">Every figure above is computed in <code>generate.mjs</code> from the linked
	<code>data/</code> files, each the saved answer of one tool call, copied or written out as its Saved line says.
	The program checks that each file echoes its pinned input and, where the answer has both, that its rows add up to its totals.
	These checks find a file for another input or an inconsistent file; they cannot prove that a written-out file is the unchanged answer. Regenerate by re-running the script.</p>
</footer>`;
}

/** Assemble the self-contained HTML document (inline CSS, no external assets). */
function htmlDoc({ title, subtitle, summary, bodyHtml, footerHtml }) {
	return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${htmlEsc(title)}</title>
<style>
:root { color-scheme: light; }
* { box-sizing: border-box; }
body { margin: 0; background: #f4f5f7; color: ${INK};
	font-family: system-ui,-apple-system,Segoe UI,Roboto,sans-serif; line-height: 1.5; }
main { max-width: 900px; margin: 0 auto; padding: 32px 20px 64px; }
header.page h1 { margin: 0 0 4px; font-size: 24px; }
header.page .sub { color: ${MUTE}; margin: 0 0 8px; }
.summary { background: #fff; border-left: 4px solid ${OKABE_ITO[0]}; padding: 12px 16px;
	border-radius: 6px; margin: 16px 0 24px; }
section.card { background: #fff; border: 1px solid #e6e6e6; border-radius: 10px;
	padding: 18px 20px; margin: 0 0 20px; overflow-x: auto; }
section.card h2 { margin: 0 0 4px; font-size: 17px; }
section.card p.lead { color: ${MUTE}; margin: 0 0 12px; font-size: 14px; }
table { border-collapse: collapse; width: 100%; font-size: 14px; }
th, td { text-align: right; padding: 6px 10px; border-bottom: 1px solid #eee; }
th:first-child, td:first-child { text-align: left; }
thead th { color: ${MUTE}; font-weight: 600; border-bottom: 2px solid #ddd; }
footer#provenance { background: #fff; border: 1px solid #e6e6e6; border-radius: 10px;
	padding: 18px 20px; font-size: 13px; color: #333; }
footer#provenance h2 { font-size: 16px; margin: 0 0 8px; }
footer#provenance h3 { font-size: 13px; text-transform: uppercase; letter-spacing: .04em;
	color: ${MUTE}; margin: 16px 0 6px; }
footer .lbl { color: ${MUTE}; font-weight: 600; }
footer ul.sources { list-style: none; padding: 0; margin: 0; }
footer ul.sources li { padding: 8px 0; border-bottom: 1px dashed #eee; }
footer code { background: #f0f2f4; padding: 1px 5px; border-radius: 4px; }
footer .params { color: #444; }
pre.reproduce { background: #1a1a1a; color: #eee; padding: 12px 14px; border-radius: 8px;
	overflow-x: auto; font-size: 12.5px; }
.caveat { color: #8a5a00; font-size: 13px; margin: 8px 0 0; }
</style>
</head>
<body>
<main>
<header class="page">
	<h1>${htmlEsc(title)}</h1>
	<p class="sub">${htmlEsc(subtitle)}</p>
</header>
${summary ? `<div class="summary">${htmlEsc(summary)}</div>` : ''}
${bodyHtml}
${footerHtml}
</main>
</body>
</html>`;
}

/** Print the chat summary block: computed numbers + file paths, for the agent to paste. */
function printSummaryBlock(lines, htmlPath) {
	const out = ['```', ...lines, `Dashboard: ${htmlPath}`, '```'];
	console.log(out.join('\n'));
}


/*======================= END COMMON BLOCK ===================================*/

/** Read one saved analytics answer and check that it answers the pinned request. */
function readAnalytics(source) {
	const answer = readToolAnswer(source);
	const req = answer.value.request ?? {};
	expectAnswer(sameList(req.keywords, source.input.keywords), source, 'request.keywords differ');
	expectAnswer(sameList(req.phrases, source.input.phrases), source, 'request.phrases differ');
	expectAnswer(sameList(req.cpc, source.input.cpc), source, 'request.cpc differs');
	expectAnswer(req.dateFrom === DATE_FROM && req.dateTo === DATE_TO, source, 'request dates differ');
	expectAnswer(Array.isArray(answer.value.analytics?.byYear), source, 'analytics.byYear is missing');
	return answer;
}

/** Narrative white-space read: numbers interpolated from computed `c` (rule 1). */
function renderNarrative(c) {
	const parts = [
		`Over publication years ${c.years[0]}–${c.years[c.years.length - 1]}, ${c.rows.length} CPC subclasses span ${fmtInt(c.min)} to ${fmtInt(c.max)} publications per cell.`,
		c.whiteSpace.length
			? `White space (cells at or below ${fmtInt(c.threshold)} publications, 10% of the busiest cell): ${c.whiteSpace.map((w) => `${htmlEsc(w.cpc)} in ${w.years.join(', ')}`).join('; ')}.`
			: 'No clearly sparse cells were found in this window.',
		`The most active subclass is ${htmlEsc(c.busiest.cpc)} (${fmtInt(c.busiest.total)} publications over the window).`,
	];
	return parts.filter(Boolean).join(' ');
}

function main() {
	const generatedAt = new Date().toISOString();
	const sources = [];
	const identity = (v) => (v.data_slice && v.data_slice !== 'unknown' ? `data_slice: ${v.data_slice}` : null);

	// Broad call: the active classes and the corpus context.
	const overview = readAnalytics(OVERVIEW);
	sources.push({ tool: OVERVIEW.tool, input: OVERVIEW.input, datasetIdentity: identity(overview.value), savedAt: overview.savedAt, sidecar: `data/${OVERVIEW.file}` });

	// One scoped call per CPC -> that row's byYear. Cross-tab into the matrix.
	const years = [];
	for (let y = Number(DATE_FROM.slice(0, 4)); y <= Number(DATE_TO.slice(0, 4)); y++) years.push(String(y));
	const rows = [];
	const matrix = [];
	for (const source of SCOPED) {
		const answer = readAnalytics(source);
		const byYear = answer.value.analytics.byYear;
		rows.push(source.cpc);
		matrix.push(years.map((yr) => { const r = byYear.find((x) => String(x.year) === yr); return r ? r.count : 0; }));
		sources.push({ tool: source.tool, input: source.input, datasetIdentity: identity(answer.value), savedAt: answer.savedAt, sidecar: `data/${source.file}` });
	}

	// White space: cells at or below 10% of the busiest cell.
	const flat = matrix.flat();
	const max = Math.max(1, ...flat), min = Math.min(...flat);
	const threshold = Math.max(1, Math.round(max * 0.1));
	const whiteSpace = rows.map((cpc, ri) => ({
		cpc, years: years.filter((_, ci) => matrix[ri][ci] <= threshold),
	})).filter((w) => w.years.length > 0);
	const totals = rows.map((cpc, ri) => ({ cpc, total: matrix[ri].reduce((a, b) => a + b, 0) }));
	const busiest = totals.reduce((a, b) => (b.total > a.total ? b : a), totals[0]);
	const c = { years, rows, matrix, min, max, threshold, whiteSpace, busiest };

	const heat = heatmap({
		title: 'Publications: CPC × Publication Year',
		source: 'patent_analytics (Topic Analytics; publications by publication year; scoped per CPC)',
		rows, cols: years, matrix, valueLabel: 'publications',
	});
	const totalsChart = barChart({
		title: 'Total Publications by CPC (window)',
		source: 'patent_analytics (Topic Analytics; publications)',
		categories: totals.map((t) => t.cpc),
		series: [{ name: 'Publications', values: totals.map((t) => t.total) }], yLabel: 'Publications',
	});

	const bodyHtml = `
<section class="card"><h2>White-Space Read</h2><p class="lead">${renderNarrative(c)}</p>
<p class="caveat">Basis: publications, by publication year; one publication per office, so one family can count more than once. Low or empty cells (leftmost legend swatch) mark candidate white space. Validate before acting: low counts can also mean a mis-scoped class, and the latest years are incomplete because applications publish about 18 months after filing.</p></section>
<section class="card"><h2>CPC × Year Heatmap</h2>${heat}</section>
<section class="card"><h2>Total Publications by CPC</h2>${totalsChart}</section>`;

	const footerHtml = renderFooter({ generatedAt, sources });
	const scope = [...KEYWORDS, ...PHRASES].join(', ');
	const html = htmlDoc({
		title: 'Technology Landscape: White Space',
		subtitle: `${scope} · ${years[0]}–${years[years.length - 1]} · Topic Analytics`,
		summary: overview.value.searchDescription ? `Search: ${overview.value.searchDescription}` : '', bodyHtml, footerHtml,
	});
	const htmlPath = join(OUT_DIR, 'dashboard.html');
	writeFileSync(htmlPath, html);
	printSummaryBlock([
		`Landscape white-space: ${scope} ${years[0]}-${years[years.length - 1]}`,
		`CPC subclasses: ${rows.join(', ')}`,
		`Busiest: ${busiest.cpc} (${fmtInt(busiest.total)} publications). White-space cells flagged: ${whiteSpace.reduce((a, w) => a + w.years.length, 0)}.`,
	], htmlPath);
}

run([OVERVIEW, ...SCOPED], main);
