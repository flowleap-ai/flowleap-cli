#!/usr/bin/env node
/*=============================================================================
 * Offline `flowleap` stand-in used ONLY by the CI smoke test (smoke.mjs).
 *
 * It never touches the network. It inspects the argv the templates pass and
 * echoes a recorded fixture from ./fixtures/, matching the real CLI's --json
 * contract (verified against src/commands/patstat.rs, cli #95):
 *   - success: prints the backend body verbatim, exit 0.
 *   - the two TYPED patstat codes (patstat_applicant_ambiguous /
 *     patstat_unavailable): the tool error envelope verbatim, { success:false,
 *     error:{ code, message, details }, status } — candidates live at
 *     error.details.candidates — exit 1.
 *   - any other failure: the generic CLI envelope { ok:false, status, body:{
 *     success:false, error:{ code, message } } } — exit 1.
 * Set FLOWLEAP_STUB_DIR to the fixtures directory.
 *
 * Dispatch is by subcommand + key flags so one stub serves every template.
 *===========================================================================*/

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

const FIX = process.env.FLOWLEAP_STUB_DIR || join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
const argv = process.argv.slice(2).filter((a) => a !== '--json');

function fixture(name) {
	return JSON.parse(readFileSync(join(FIX, name), 'utf8'));
}
function emit(value) {
	process.stdout.write(JSON.stringify(value, null, 2) + '\n');
	process.exit(0);
}
/** Generic CLI error envelope (auth/rate-limit/5xx and unhandled cases), then exit non-zero. */
function emitError(status, code, message, extra = {}) {
	const body = { success: false, error: { code, message, ...extra }, status };
	process.stdout.write(JSON.stringify({ ok: false, status, contentType: 'application/json', body }, null, 2) + '\n');
	process.exit(1);
}
/** Flat typed-PATSTAT error shape the real CLI emits for the two typed codes, then exit non-zero. */
function emitPatstatError(code, message, status, details = {}) {
	process.stdout.write(JSON.stringify({ success: false, error: { code, message, details }, status }, null, 2) + '\n');
	process.exit(1);
}
function flagValue(name) {
	const i = argv.indexOf(name);
	return i >= 0 && i + 1 < argv.length ? argv[i + 1] : null;
}
function positionalsAfter(word) {
	const i = argv.indexOf(word);
	if (i < 0) return [];
	return argv.slice(i + 1).filter((a) => !a.startsWith('--'));
}
const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

if (argv[0] === '--version' || argv.includes('--version')) {
	process.stdout.write('flowleap 0.3.5\n');
	process.exit(0);
}
if (argv.includes('doctor')) {
	emit(fixture('doctor.json'));
}
if (argv.includes('patstat') && argv.includes('portfolio')) {
	const applicant = positionalsAfter('portfolio')[0] || '';
	// Test hook: this reserved name models a deployment with no PATSTAT dataset.
	if (slug(applicant) === 'patstat-unavailable') {
		emitPatstatError('patstat_unavailable', 'The PATSTAT analytics layer is not configured on this deployment (PATSTAT_DATABASE_URL is unset).', 503);
	}
	const file = `portfolio-${slug(applicant)}.json`;
	try {
		emit(fixture(file));
	} catch {
		// No fixture for this applicant -> model the 422 ambiguous case.
		emitPatstatError('patstat_applicant_ambiguous',
			`"${applicant}" matches 2 distinct applicant entities: ${applicant} HOLDING (900 applications), ${applicant} TECH (410 applications). These may be separate companies, so they are not merged automatically.`,
			422, { candidates: [{ name: `${applicant} HOLDING`, applications: 900 }, { name: `${applicant} TECH`, applications: 410 }] });
	}
}
if (argv.includes('analytics')) {
	const cpc = flagValue('--cpc');
	emit(fixture(cpc ? `analytics-cpc-${slug(cpc)}.json` : 'analytics-overview.json'));
}
if (argv.includes('citation') && argv.includes('forward')) {
	emit(fixture('citation-forward.json'));
}
emitError(400, 'stub_unhandled', `stub-flowleap: no fixture rule for args: ${argv.join(' ')}`);
