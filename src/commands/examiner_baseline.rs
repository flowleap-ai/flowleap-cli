//! `flowleap patent examiner-baseline <publication>`: the Examiner Baseline
//! (PRD 0019 F1, agent-v2 ADR 0010).
//!
//! Walks the INPADOC family of a publication, reads the `references-cited`
//! block of every publication of every member (`get_bibliography`), adds the
//! USPTO enriched office-action citations for granted US members
//! (`get_us_grant` resolves the application number,
//! `search_office_action_citations` reads the rows), and prints one matrix:
//! cited document × office.
//!
//! Guardrail (Verified-Data Contract): this verb never calls a model. Every
//! category, claim list and count comes from a backend response. A member
//! whose office returned no citation block is a gap, printed as one — never as
//! "nothing cited".
//!
//! The work is split so the logic is testable without a backend: [`collect`]
//! does the I/O and keeps every raw answer in a [`Collected`]; [`assemble`]
//! turns that into the [`Baseline`] that `--json` prints; [`render_human`]
//! derives the table from the same struct.

use std::collections::BTreeMap;

use anyhow::Result;
use comfy_table::{Cell as TableCell, ContentArrangement, Table};
use serde::Serialize;
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

/// The largest page `search_office_action_citations` serves.
const ENRICHED_PAGE_SIZE: u64 = 1000;

const SOURCE_OPS: &str = "ops_biblio";
const SOURCE_ENRICHED: &str = "uspto_enriched";

/// The verb's entry point.
pub async fn run(ctx: &Context, publication: &str) -> Result<()> {
    if ctx.dry_run {
        // A dry run can only describe the first request; every later one
        // depends on the family it would have returned.
        let input = json!({ "patent_number": publication });
        if let Some(data) = tools::call_tool_data(ctx, "get_family", &input).await? {
            output::print_json(&data);
        }
        return Ok(());
    }
    let collected = collect(ctx, publication).await?;
    let baseline = assemble(&collected);
    if ctx.output_format == "json" {
        output::print_json(&serde_json::to_value(&baseline)?);
    } else {
        print!("{}", render_human(&baseline));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Collection (I/O)
// ---------------------------------------------------------------------------

/// One tool answer as this verb keeps it: the data, or a failure it records
/// as a gap instead of stopping the walk.
#[derive(Debug, Clone)]
pub(crate) enum Read {
    Ok(Value),
    Failed(ReadError),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadError {
    pub status: Option<u64>,
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Everything the backend answered, before any interpretation.
#[derive(Debug, Clone)]
pub(crate) struct Collected {
    pub publication: String,
    pub family: Value,
    /// Biblio read per publication number.
    pub biblio: BTreeMap<String, Read>,
    /// Per US representative publication: the grant read that resolves the
    /// application number (None when the member has no US grant to resolve).
    pub us_grants: BTreeMap<String, Read>,
    /// Per USPTO application number: the enriched-citation read.
    pub enriched: BTreeMap<String, Read>,
}

async fn collect(ctx: &Context, publication: &str) -> Result<Collected> {
    // The family is the walk itself: without it there is nothing to report,
    // so any failure here stops the verb with the ordinary error envelope.
    let family = tools::call_tool_data(ctx, "get_family", &json!({ "patent_number": publication }))
        .await?
        .unwrap_or(Value::Null);
    let members = family_members(&family, publication);

    let mut collected = Collected {
        publication: publication.to_string(),
        family,
        biblio: BTreeMap::new(),
        us_grants: BTreeMap::new(),
        enriched: BTreeMap::new(),
    };

    for member in &members {
        for publication in &member.publications {
            if collected.biblio.contains_key(publication) {
                continue;
            }
            let input = json!({ "patent_number": publication, "include_designated_states": false });
            let read = soft_call(ctx, "get_bibliography", &input).await?;
            collected.biblio.insert(publication.clone(), read);
        }

        let Some(grant_number) = us_grant_number(&member.representative) else {
            continue;
        };
        let grant = soft_call(
            ctx,
            "get_us_grant",
            &json!({ "patent_number": grant_number }),
        )
        .await?;
        let application = match &grant {
            Read::Ok(data) => us_application_number(data),
            Read::Failed(_) => None,
        };
        collected
            .us_grants
            .insert(member.representative.clone(), grant);
        if let Some(application) = application {
            if collected.enriched.contains_key(&application) {
                continue;
            }
            let input = json!({
                "application_number": application,
                "size": ENRICHED_PAGE_SIZE,
                "offset": 0,
                "examiner_cited_only": false,
            });
            let read = soft_call(ctx, "search_office_action_citations", &input).await?;
            collected.enriched.insert(application, read);
        }
    }
    Ok(collected)
}

/// Call one tool and keep a per-document failure (not found, invalid number,
/// an office-side error) as data, so one unreadable member becomes a gap row
/// instead of ending the walk. A failure a human must act on — a key gate,
/// the subscription gate, a rate limit, a sign-in problem, a retired
/// endpoint — stops the verb with its normal envelope, hints and exit code.
async fn soft_call(ctx: &Context, tool: &str, input: &Value) -> Result<Read> {
    let envelope = tools::call_tool_envelope(ctx, tool, input).await?;
    if envelope.get("ok").and_then(Value::as_bool) == Some(true) {
        let body = envelope.get("body").cloned().unwrap_or(Value::Null);
        return Ok(Read::Ok(body.get("data").cloned().unwrap_or(body)));
    }
    let status = envelope.get("status").and_then(Value::as_u64);
    let needs_human = [
        "providerKeysHint",
        "subscriptionHint",
        "rateLimitHint",
        "endpointGoneHint",
    ]
    .iter()
    .any(|hint| envelope.get(*hint).is_some());
    if needs_human || matches!(status, Some(401) | Some(403)) {
        return Err(ctx.fail_with_envelope(&envelope));
    }
    let error = envelope.pointer("/body/error");
    Ok(Read::Failed(ReadError {
        status,
        code: error
            .and_then(|e| e.get("code"))
            .and_then(Value::as_str)
            .map(str::to_string),
        message: error
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }))
}

// ---------------------------------------------------------------------------
// Family reading
// ---------------------------------------------------------------------------

/// One family member (one application) as the walk sees it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FamilyMember {
    pub office: String,
    /// DOCDB application value from the family read. A label only: it is
    /// never printed as, or parsed into, a filing number.
    pub docdb_application: Option<String>,
    pub representative: String,
    pub publications: Vec<String>,
}

/// The members of a `get_family` answer, from its `representatives` block
/// (one entry per application). An answer without that block falls back to
/// one member per distinct publication row, so nothing is dropped.
pub(crate) fn family_members(family: &Value, requested: &str) -> Vec<FamilyMember> {
    let representatives = family
        .pointer("/representatives/members")
        .and_then(Value::as_array);
    let mut members: Vec<FamilyMember> = match representatives {
        Some(list) => list
            .iter()
            .filter_map(|entry| {
                let representative = entry
                    .get("representativePublication")
                    .and_then(Value::as_str)?
                    .to_string();
                let mut publications: Vec<String> = entry
                    .get("publications")
                    .and_then(Value::as_array)
                    .map(|p| {
                        p.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                if publications.is_empty() {
                    publications.push(representative.clone());
                }
                Some(FamilyMember {
                    office: office_of(&representative),
                    docdb_application: entry
                        .get("application")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    representative,
                    publications,
                })
            })
            .collect(),
        None => {
            let mut seen: Vec<String> = Vec::new();
            for row in family
                .get("members")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(publication) = row.get("publication").and_then(Value::as_str) {
                    if !seen.iter().any(|p| p == publication) {
                        seen.push(publication.to_string());
                    }
                }
            }
            seen.into_iter()
                .map(|publication| FamilyMember {
                    office: office_of(&publication),
                    docdb_application: None,
                    representative: publication.clone(),
                    publications: vec![publication],
                })
                .collect()
        }
    };
    if members.is_empty() {
        // A family read that names no member still has the requested
        // document: read it alone rather than report an empty walk.
        members.push(FamilyMember {
            office: office_of(requested),
            docdb_application: None,
            representative: requested.to_string(),
            publications: vec![requested.to_string()],
        });
    }
    members
}

/// The office of a publication number: its two-letter prefix.
fn office_of(publication: &str) -> String {
    publication
        .chars()
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// The bare patent number of a US grant publication (`US7722129B2` →
/// `7722129`), the input `get_us_grant` takes. None for anything that is not a
/// US utility grant (B1/B2), because only a grant resolves this way.
pub(crate) fn us_grant_number(publication: &str) -> Option<String> {
    let rest = publication.strip_prefix("US")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let kind = &rest[digits.len()..];
    if digits.is_empty() || !matches!(kind, "B1" | "B2") {
        return None;
    }
    Some(digits)
}

/// The USPTO application number a `get_us_grant` answer names.
fn us_application_number(grant: &Value) -> Option<String> {
    grant
        .get("applicationNumberText")
        .or_else(|| grant.pointer("/patentFileWrapperDataBag/0/applicationNumberText"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// The Baseline (the `--json` shape)
// ---------------------------------------------------------------------------

/// The Examiner Baseline. `--json` prints this struct; the human table is
/// derived from it. Field names are a contract for the Find Better skill and
/// the report writer: add fields, never rename.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Baseline {
    /// The publication the walk started from, as given.
    pub publication: String,
    /// Column order of the matrix: the offices of the members walked.
    pub offices: Vec<String>,
    /// Every member walked, with what each read returned.
    pub members_walked: Vec<MemberWalked>,
    /// Matrix rows: one per cited document.
    pub documents: Vec<CitedDocument>,
    /// Members or sources that returned no citation record. A gap is not
    /// "nothing cited".
    pub gaps: Vec<Gap>,
    /// How rows were deduplicated: `docdb-number` (country + number, kind
    /// dropped). Family-level dedupe is not applied: no cited document's
    /// family is read.
    pub dedupe: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemberWalked {
    pub office: String,
    pub representative_publication: String,
    /// The DOCDB application value from the family read. A label, never a
    /// filing number.
    pub docdb_application: Option<String>,
    pub publications: Vec<PublicationRead>,
    /// Only on members with a US grant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uspto_enriched: Option<EnrichedRead>,
}

/// What one publication's biblio read returned.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicationRead {
    pub publication: String,
    /// `read` (a citation block came back), `no_citation_record` (the read
    /// worked and carried no block) or `read_failed`.
    pub status: &'static str,
    /// The office's own count of entries in the block.
    pub cited_count: usize,
    pub examiner_count: usize,
    pub applicant_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ReadError>,
}

/// What the USPTO enriched-citation read returned for a US member.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrichedRead {
    /// The USPTO application number, as `get_us_grant` served it.
    pub application_number: Option<String>,
    /// `read`, `no_citation_record`, `not_resolved` (no application number)
    /// or `read_failed`.
    pub status: &'static str,
    /// Rows returned.
    pub rows: usize,
    /// The `total` the tool reported.
    pub total: Option<u64>,
    /// Rows that name no cited document (kept in the count, not in the
    /// matrix).
    pub unidentified_rows: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ReadError>,
}

/// One matrix row.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CitedDocument {
    /// The dedupe key: country + number without kind (`US4964287`), or
    /// `NPL: <text>` for non-patent literature.
    pub document: String,
    /// Kind codes seen across the citations (OPS serves it apart from the
    /// number).
    pub kinds: Vec<String>,
    /// Always null today: no cited document's family is read.
    pub family_id: Option<String>,
    /// The non-patent-literature citation text, for NPL rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npl: Option<String>,
    /// Per office: the cell, keyed by office code. An office absent here did
    /// not cite the document (or is in `gaps`).
    pub cells: BTreeMap<String, OfficeCell>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfficeCell {
    /// The rendered cell: category and cited claims, or `applicant` when only
    /// the applicant cited it.
    pub text: String,
    /// Every citation behind the cell, verbatim from its source.
    pub citations: Vec<Citation>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Citation {
    /// `ops_biblio` or `uspto_enriched`.
    pub source: &'static str,
    /// The citing publication (OPS) or the USPTO application number
    /// (enriched).
    pub citing: String,
    /// `examiner`, `applicant` or `unknown`, as the source states it.
    pub cited_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// OPS: the claims the search report names. Enriched: the claims the
    /// office action rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevant_claims: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub relevant_passages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub office_action_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub office_action_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Gap {
    pub office: String,
    /// The member's representative publication.
    pub member: String,
    /// `ops_biblio` or `uspto_enriched`.
    pub source: &'static str,
    /// `no_citation_record`, `read_failed` or `not_resolved`.
    pub reason: &'static str,
    /// The line the table prints.
    pub message: String,
}

/// Build the Baseline from the raw answers. Pure: no I/O, no model.
pub(crate) fn assemble(collected: &Collected) -> Baseline {
    let members = family_members(&collected.family, &collected.publication);
    let mut offices: Vec<String> = Vec::new();
    let requested_office = office_of(&collected.publication);
    if members.iter().any(|m| m.office == requested_office) {
        offices.push(requested_office);
    }
    for member in &members {
        if !offices.contains(&member.office) {
            offices.push(member.office.clone());
        }
    }

    let mut rows: BTreeMap<String, CitedDocument> = BTreeMap::new();
    let mut walked = Vec::new();
    let mut gaps = Vec::new();

    for member in &members {
        let mut publications = Vec::new();
        for publication in &member.publications {
            let read = collected.biblio.get(publication);
            publications.push(read_publication(
                publication,
                read,
                &member.office,
                &mut rows,
            ));
        }
        for failed in publications.iter().filter(|p| p.status == "read_failed") {
            gaps.push(Gap {
                office: member.office.clone(),
                member: member.representative.clone(),
                source: SOURCE_OPS,
                reason: "read_failed",
                message: format!(
                    "could not read citations from {} ({}){}",
                    member.office,
                    failed.publication,
                    describe_error(failed.error.as_ref())
                ),
            });
        }
        if publications.iter().all(|p| p.status != "read") {
            gaps.push(Gap {
                office: member.office.clone(),
                member: member.representative.clone(),
                source: SOURCE_OPS,
                reason: "no_citation_record",
                message: format!(
                    "no citation record from {} ({})",
                    member.office, member.representative
                ),
            });
        }

        let uspto_enriched = us_grant_number(&member.representative).map(|_| {
            let enriched =
                read_enriched(collected, &member.representative, &member.office, &mut rows);
            if enriched.status != "read" {
                let detail = match enriched.status {
                    "not_resolved" => format!(
                        "US application number not resolved for {}{}",
                        member.representative,
                        describe_error(enriched.error.as_ref())
                    ),
                    "read_failed" => format!(
                        "could not read USPTO enriched citations for {} (application {}){}",
                        member.representative,
                        enriched.application_number.as_deref().unwrap_or("?"),
                        describe_error(enriched.error.as_ref())
                    ),
                    _ => format!(
                        "no USPTO enriched-citation record for {} (application {})",
                        member.representative,
                        enriched.application_number.as_deref().unwrap_or("?")
                    ),
                };
                gaps.push(Gap {
                    office: member.office.clone(),
                    member: member.representative.clone(),
                    source: SOURCE_ENRICHED,
                    reason: enriched.status,
                    message: detail,
                });
            }
            enriched
        });

        walked.push(MemberWalked {
            office: member.office.clone(),
            representative_publication: member.representative.clone(),
            docdb_application: member.docdb_application.clone(),
            publications,
            uspto_enriched,
        });
    }

    for row in rows.values_mut() {
        for cell in row.cells.values_mut() {
            cell.text = cell_text(&cell.citations);
        }
    }
    let mut documents: Vec<CitedDocument> = rows.into_values().collect();
    // Rows with examiner evidence first, then by document key.
    documents.sort_by(|a, b| {
        examiner_rank(b)
            .cmp(&examiner_rank(a))
            .then_with(|| a.document.cmp(&b.document))
    });

    Baseline {
        publication: collected.publication.clone(),
        offices,
        members_walked: walked,
        documents,
        gaps,
        dedupe: "docdb-number",
    }
}

fn describe_error(error: Option<&ReadError>) -> String {
    let Some(error) = error else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(status) = error.status {
        parts.push(status.to_string());
    }
    if let Some(code) = &error.code {
        parts.push(code.clone());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(": {}", parts.join(" "))
    }
}

fn examiner_rank(document: &CitedDocument) -> u8 {
    let citations = document.cells.values().flat_map(|c| &c.citations);
    let mut rank = 0;
    for citation in citations {
        if citation.cited_by == "examiner" {
            return 2;
        }
        if citation.category.is_some() {
            rank = 1;
        }
    }
    rank
}

fn read_publication(
    publication: &str,
    read: Option<&Read>,
    office: &str,
    rows: &mut BTreeMap<String, CitedDocument>,
) -> PublicationRead {
    let mut result = PublicationRead {
        publication: publication.to_string(),
        status: "no_citation_record",
        cited_count: 0,
        examiner_count: 0,
        applicant_count: 0,
        error: None,
    };
    let data = match read {
        Some(Read::Ok(data)) => data,
        Some(Read::Failed(error)) => {
            result.status = "read_failed";
            result.error = Some(error.clone());
            return result;
        }
        None => {
            result.status = "read_failed";
            return result;
        }
    };
    let Some(references) = data.get("citedReferences").and_then(Value::as_array) else {
        return result;
    };
    if references.is_empty() {
        return result;
    }
    result.status = "read";
    result.cited_count = references.len();
    for reference in references {
        let cited_by = str_field(reference, "citedBy").unwrap_or_else(|| "unknown".into());
        match cited_by.as_str() {
            "examiner" => result.examiner_count += 1,
            "applicant" => result.applicant_count += 1,
            _ => {}
        }
        let npl = str_field(reference, "npl");
        let key = match (str_field(reference, "docId"), &npl) {
            (Some(doc), _) if !doc.is_empty() => doc_key(&doc),
            (_, Some(text)) => format!("NPL: {text}"),
            _ => continue,
        };
        let citation = Citation {
            source: SOURCE_OPS,
            citing: publication.to_string(),
            cited_by,
            category: str_field(reference, "category"),
            relevant_claims: str_field(reference, "relevantClaims"),
            phase: str_field(reference, "phase"),
            relevant_passages: reference
                .get("relevantPassages")
                .and_then(Value::as_array)
                .map(|p| {
                    p.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            office_action_date: None,
            office_action_type: None,
        };
        add_citation(
            rows,
            &key,
            str_field(reference, "kind"),
            npl,
            office,
            citation,
        );
    }
    result
}

fn read_enriched(
    collected: &Collected,
    representative: &str,
    office: &str,
    rows: &mut BTreeMap<String, CitedDocument>,
) -> EnrichedRead {
    let mut result = EnrichedRead {
        application_number: None,
        status: "not_resolved",
        rows: 0,
        total: None,
        unidentified_rows: 0,
        error: None,
    };
    match collected.us_grants.get(representative) {
        Some(Read::Ok(grant)) => result.application_number = us_application_number(grant),
        Some(Read::Failed(error)) => {
            result.error = Some(error.clone());
            return result;
        }
        None => return result,
    }
    let Some(application) = result.application_number.clone() else {
        return result;
    };
    let data = match collected.enriched.get(&application) {
        Some(Read::Ok(data)) => data,
        Some(Read::Failed(error)) => {
            result.status = "read_failed";
            result.error = Some(error.clone());
            return result;
        }
        None => {
            result.status = "read_failed";
            return result;
        }
    };
    result.total = data.get("total").and_then(Value::as_u64);
    let citations = data
        .get("citations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    result.rows = citations.len();
    result.status = if citations.is_empty() {
        "no_citation_record"
    } else {
        "read"
    };
    for row in &citations {
        let is_npl = row.get("isNPL").and_then(Value::as_bool) == Some(true);
        let cited_text = str_field(row, "citedDocument");
        let reference = row
            .pointer("/documentReference/publicationNumber")
            .and_then(Value::as_str)
            .map(str::to_string);
        let (key, npl) = match (reference, &cited_text) {
            (_, Some(text)) if is_npl => (format!("NPL: {text}"), Some(text.clone())),
            (Some(number), _) => (doc_key(&number), None),
            (None, Some(text)) => (doc_key(text), None),
            (None, None) => {
                result.unidentified_rows += 1;
                continue;
            }
        };
        let cited_by = if row.get("examinerCited").and_then(Value::as_bool) == Some(true) {
            "examiner"
        } else if row.get("applicantCited").and_then(Value::as_bool) == Some(true) {
            "applicant"
        } else {
            "unknown"
        };
        let citation = Citation {
            source: SOURCE_ENRICHED,
            citing: application.clone(),
            cited_by: cited_by.to_string(),
            category: str_field(row, "category"),
            relevant_claims: str_field(row, "rejectedClaims"),
            phase: None,
            relevant_passages: Vec::new(),
            office_action_date: str_field(row, "officeActionDate")
                .map(|d| d.chars().take(10).collect()),
            office_action_type: str_field(row, "officeActionType"),
        };
        add_citation(rows, &key, None, npl, office, citation);
    }
    result
}

fn add_citation(
    rows: &mut BTreeMap<String, CitedDocument>,
    key: &str,
    kind: Option<String>,
    npl: Option<String>,
    office: &str,
    citation: Citation,
) {
    let row = rows
        .entry(key.to_string())
        .or_insert_with(|| CitedDocument {
            document: key.to_string(),
            kinds: Vec::new(),
            family_id: None,
            npl: npl.clone(),
            cells: BTreeMap::new(),
        });
    if let Some(kind) = kind {
        if !row.kinds.contains(&kind) {
            row.kinds.push(kind);
        }
    }
    let cell = row
        .cells
        .entry(office.to_string())
        .or_insert_with(|| OfficeCell {
            text: String::new(),
            citations: Vec::new(),
        });
    if !cell.citations.contains(&citation) {
        cell.citations.push(citation);
    }
}

fn str_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// The dedupe key of a cited patent document: country + number, uppercase,
/// punctuation and kind code removed, so OPS's `US2007052285` and USPTO's
/// `US 2007/0052285` (or `US20070052285`) meet on one row.
pub(crate) fn doc_key(raw: &str) -> String {
    let compact: String = raw
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_uppercase();
    if compact.len() < 3 {
        return compact;
    }
    let (country, body) = compact.split_at(2);
    // Drop a trailing kind code (A, A1, B2, …) that follows a digit.
    let bytes = body.as_bytes();
    let mut end = bytes.len();
    if end >= 2 && bytes[end - 1].is_ascii_digit() && bytes[end - 2].is_ascii_alphabetic() {
        end -= 2;
    } else if end >= 1 && bytes[end - 1].is_ascii_alphabetic() {
        end -= 1;
    }
    let body = if end > 0 && bytes[end - 1].is_ascii_digit() {
        &body[..end]
    } else {
        body
    };
    // USPTO writes pre-grant publications as year + 7 digits; DOCDB as year +
    // 6 digits. The extra digit is a leading zero.
    if country == "US"
        && body.len() == 11
        && body.chars().all(|c| c.is_ascii_digit())
        && (body.starts_with("19") || body.starts_with("20"))
        && &body[4..5] == "0"
    {
        return format!("US{}{}", &body[..4], &body[5..]);
    }
    format!("{country}{body}")
}

/// The cell text for one office: each distinct citation as category plus
/// cited claims, the citing side named when it is not the examiner. A bare
/// `applicant` (or `examiner`) appears only when no citation of the document
/// in that office carries a category.
pub(crate) fn cell_text(citations: &[Citation]) -> String {
    let mut labelled: Vec<String> = Vec::new();
    let mut bare: Vec<String> = Vec::new();
    for citation in citations {
        let label = match &citation.category {
            Some(category) => {
                let mut label = category.clone();
                if let Some(claims) = &citation.relevant_claims {
                    label.push_str(&format!(" cl. {claims}"));
                }
                if citation.source == SOURCE_ENRICHED {
                    label.push_str(" (US OA");
                    if let Some(date) = &citation.office_action_date {
                        label.push_str(&format!(" {date}"));
                    }
                    label.push(')');
                }
                if citation.cited_by != "examiner" {
                    label.push_str(&format!(" [{}]", citation.cited_by));
                }
                label
            }
            None => {
                if !bare.contains(&citation.cited_by) {
                    bare.push(citation.cited_by.clone());
                }
                continue;
            }
        };
        if !labelled.contains(&label) {
            labelled.push(label);
        }
    }
    if !labelled.is_empty() {
        return labelled.join("; ");
    }
    if bare.iter().any(|b| b == "examiner") {
        bare.retain(|b| b != "applicant");
    }
    bare.join("; ")
}

// ---------------------------------------------------------------------------
// Human rendering
// ---------------------------------------------------------------------------

/// The human table, derived from the Baseline only.
pub(crate) fn render_human(baseline: &Baseline) -> String {
    let mut out = String::new();
    out.push_str(&format!("Examiner Baseline: {}\n", baseline.publication));
    out.push_str(&format!(
        "Family members walked: {}. Offices: {}.\n\n",
        baseline.members_walked.len(),
        baseline.offices.join(", ")
    ));

    out.push_str("Members walked (counts are the offices' own):\n");
    for member in &baseline.members_walked {
        out.push_str(&format!(
            "  {}  {}\n",
            member.office, member.representative_publication
        ));
        for publication in &member.publications {
            let line = match publication.status {
                "read" => format!(
                    "{} cited ({} examiner, {} applicant)",
                    publication.cited_count,
                    publication.examiner_count,
                    publication.applicant_count
                ),
                "read_failed" => {
                    format!("read failed{}", describe_error(publication.error.as_ref()))
                }
                _ => "no citation block".to_string(),
            };
            out.push_str(&format!(
                "      OPS biblio {}: {}\n",
                publication.publication, line
            ));
        }
        if let Some(enriched) = &member.uspto_enriched {
            let application = enriched
                .application_number
                .as_deref()
                .unwrap_or("not resolved");
            let line = match enriched.status {
                "read" => {
                    let mut line = format!("{} rows", enriched.rows);
                    if let Some(total) = enriched.total {
                        if total as usize != enriched.rows {
                            line.push_str(&format!(" of {total} (truncated)"));
                        }
                    }
                    if enriched.unidentified_rows > 0 {
                        line.push_str(&format!(
                            ", {} naming no document",
                            enriched.unidentified_rows
                        ));
                    }
                    line
                }
                "no_citation_record" => "no rows".to_string(),
                "read_failed" => format!("read failed{}", describe_error(enriched.error.as_ref())),
                _ => format!(
                    "application number not resolved{}",
                    describe_error(enriched.error.as_ref())
                ),
            };
            out.push_str(&format!(
                "      USPTO enriched citations, application {}: {}\n",
                application, line
            ));
        }
    }
    out.push('\n');

    out.push_str(&format!(
        "Cited documents: {} (deduplicated by DOCDB number; family dedupe not applied)\n",
        baseline.documents.len()
    ));
    if !baseline.documents.is_empty() {
        let mut table = Table::new();
        table.set_content_arrangement(ContentArrangement::Dynamic);
        let mut header = vec![TableCell::new("Document")];
        header.extend(baseline.offices.iter().map(TableCell::new));
        table.set_header(header);
        for document in &baseline.documents {
            let mut name = document.document.clone();
            if !document.kinds.is_empty() {
                name.push_str(&format!(" {}", document.kinds.join("/")));
            }
            let mut row = vec![TableCell::new(output::truncate(&name, 80))];
            for office in &baseline.offices {
                let text = document
                    .cells
                    .get(office)
                    .map(|cell| cell.text.clone())
                    .unwrap_or_else(|| "-".to_string());
                row.push(TableCell::new(text));
            }
            table.add_row(row);
        }
        out.push_str(&format!("{table}\n"));
    }

    if !baseline.gaps.is_empty() {
        out.push_str("\nGaps (no record is not \"nothing cited\"):\n");
        for gap in &baseline.gaps {
            out.push_str(&format!("  {}\n", gap.message));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The EP2110298 family as `get_family` serves it (representatives block,
    /// trimmed to what the verb reads), from a live read on 2026-10-05.
    fn family() -> Value {
        json!({
            "docId": "EP2110298",
            "memberCount": 3,
            "representatives": { "members": [
                { "application": "EP09250131 (A)", "isGrant": true,
                  "publications": ["EP2110298A2", "EP2110298A3", "EP2110298B1"],
                  "representativePublication": "EP2110298B1" },
                { "application": "US10374408 (A)", "isGrant": true,
                  "publications": ["US2009261648A1", "US7722129B2"],
                  "representativePublication": "US7722129B2" },
            ]}
        })
    }

    fn ops_ref(doc: &str, kind: &str, by: &str, extra: Value) -> Value {
        let mut value = json!({ "docId": doc, "kind": kind, "citedBy": by });
        if let Value::Object(map) = extra {
            for (k, v) in map {
                value[k] = v;
            }
        }
        value
    }

    fn collected() -> Collected {
        let mut biblio = BTreeMap::new();
        biblio.insert(
            "EP2110298A2".into(),
            Read::Ok(
                json!({ "citedReferences": [ops_ref("US4964287", "A", "applicant", json!({}))] }),
            ),
        );
        biblio.insert(
            "EP2110298A3".into(),
            Read::Ok(json!({ "citedReferences": [
                ops_ref("EP1602570", "A1", "applicant", json!({ "category": "X,A", "relevantClaims": "13" })),
                ops_ref("US4964287", "A", "examiner", json!({ "category": "X,A", "relevantClaims": "1-3,5,7,8", "phase": "national-search-report", "relevantPassages": ["* the whole document *"] })),
                ops_ref("US2007052285", "A1", "examiner", json!({ "category": "X,A", "relevantClaims": "5" })),
            ]})),
        );
        // The B1 carries no citation block at all.
        biblio.insert("EP2110298B1".into(), Read::Ok(json!({ "title": "x" })));
        biblio.insert(
            "US2009261648A1".into(),
            Read::Failed(ReadError {
                status: Some(404),
                code: Some("NOT_FOUND".into()),
                message: None,
            }),
        );
        biblio.insert(
            "US7722129B2".into(),
            Read::Ok(json!({ "citedReferences": [
                ops_ref("US4964287", "A", "applicant", json!({})),
                ops_ref("US5135330", "A", "examiner", json!({ "phase": "national-search-report" })),
            ]})),
        );
        let mut us_grants = BTreeMap::new();
        us_grants.insert(
            "US7722129B2".into(),
            Read::Ok(json!({ "applicationNumberText": "12103744" })),
        );
        let mut enriched = BTreeMap::new();
        enriched.insert(
            "12103744".into(),
            Read::Ok(json!({ "total": 3, "citations": [
                { "citedDocument": "US 5,135,330", "documentReference": { "publicationNumber": "US5135330" },
                  "category": "X", "examinerCited": true, "applicantCited": false,
                  "rejectedClaims": "1-4", "officeActionDate": "2009-08-18T00:00:00", "officeActionType": "CTNF", "isNPL": false },
                { "citedDocument": "US 2007/0052285", "documentReference": { "publicationNumber": "US20070052285" },
                  "category": "Y", "examinerCited": true, "applicantCited": false,
                  "rejectedClaims": "2", "officeActionDate": "2009-08-18T00:00:00", "officeActionType": "CTNF", "isNPL": false },
                { "citedDocument": null, "documentReference": null, "category": "X",
                  "examinerCited": false, "applicantCited": false, "isNPL": false },
            ]})),
        );
        Collected {
            publication: "EP2110298B1".into(),
            family: family(),
            biblio,
            us_grants,
            enriched,
        }
    }

    #[test]
    fn doc_key_meets_ops_and_uspto_spellings_on_one_row() {
        assert_eq!(
            [
                doc_key("US2007052285"),
                doc_key("US 2007/0052285"),
                doc_key("US20070052285"),
                doc_key("US5,135,330"),
                doc_key("EP1602570A1"),
                doc_key("US7722129B2"),
                doc_key("WO2020123456"),
            ],
            [
                "US2007052285",
                "US2007052285",
                "US2007052285",
                "US5135330",
                "EP1602570",
                "US7722129",
                "WO2020123456",
            ]
        );
    }

    #[test]
    fn us_grant_number_takes_only_us_grants() {
        assert_eq!(
            [
                us_grant_number("US7722129B2"),
                us_grant_number("US6241322B1"),
                us_grant_number("US2009261648A1"),
                us_grant_number("EP2110298B1"),
            ],
            [
                Some("7722129".to_string()),
                Some("6241322".to_string()),
                None,
                None
            ]
        );
    }

    #[test]
    fn the_baseline_matrix_is_the_offices_record() {
        let baseline = assemble(&collected());
        let matrix: Vec<(String, Vec<(String, String)>)> = baseline
            .documents
            .iter()
            .map(|d| {
                (
                    d.document.clone(),
                    d.cells
                        .iter()
                        .map(|(office, cell)| (office.clone(), cell.text.clone()))
                        .collect(),
                )
            })
            .collect();
        let pairs = |items: &[(&str, &str)]| -> Vec<(String, String)> {
            items
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        assert_eq!(
            matrix,
            vec![
                (
                    "US2007052285".to_string(),
                    pairs(&[("EP", "X,A cl. 5"), ("US", "Y cl. 2 (US OA 2009-08-18)")])
                ),
                (
                    "US4964287".to_string(),
                    pairs(&[("EP", "X,A cl. 1-3,5,7,8"), ("US", "applicant")])
                ),
                (
                    "US5135330".to_string(),
                    pairs(&[("US", "X cl. 1-4 (US OA 2009-08-18)")])
                ),
                (
                    "EP1602570".to_string(),
                    pairs(&[("EP", "X,A cl. 13 [applicant]")])
                ),
            ]
        );
        assert_eq!(baseline.offices, vec!["EP", "US"]);
    }

    #[test]
    fn a_member_without_a_citation_block_is_a_gap_not_an_empty_column() {
        let mut collected = collected();
        collected
            .biblio
            .insert("US7722129B2".into(), Read::Ok(json!({})));
        collected.enriched.insert(
            "12103744".into(),
            Read::Ok(json!({ "total": 0, "citations": [] })),
        );
        let baseline = assemble(&collected);
        let gaps: Vec<(&str, &str, String)> = baseline
            .gaps
            .iter()
            .map(|g| (g.source, g.reason, g.message.clone()))
            .collect();
        assert_eq!(
            gaps,
            vec![
                (
                    SOURCE_OPS,
                    "read_failed",
                    "could not read citations from US (US2009261648A1): 404 NOT_FOUND".to_string()
                ),
                (
                    SOURCE_OPS,
                    "no_citation_record",
                    "no citation record from US (US7722129B2)".to_string()
                ),
                (
                    SOURCE_ENRICHED,
                    "no_citation_record",
                    "no USPTO enriched-citation record for US7722129B2 (application 12103744)"
                        .to_string()
                ),
            ]
        );
    }

    #[test]
    fn json_shape_names_members_counts_and_gaps() {
        let value = serde_json::to_value(assemble(&collected())).unwrap();
        let ep = &value["membersWalked"][0];
        assert_eq!(
            (
                ep["publications"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| (
                        p["publication"].clone(),
                        p["status"].clone(),
                        p["citedCount"].clone()
                    ))
                    .collect::<Vec<_>>(),
                value["membersWalked"][1]["usptoEnriched"].clone(),
                value["dedupe"].clone(),
                value["documents"][0]["familyId"].clone(),
                value["documents"][0]["cells"]["EP"]["citations"][0]["source"].clone(),
            ),
            (
                vec![
                    (json!("EP2110298A2"), json!("read"), json!(1)),
                    (json!("EP2110298A3"), json!("read"), json!(3)),
                    (json!("EP2110298B1"), json!("no_citation_record"), json!(0)),
                ],
                json!({ "applicationNumber": "12103744", "status": "read", "rows": 3, "total": 3, "unidentifiedRows": 1 }),
                json!("docdb-number"),
                Value::Null,
                json!("ops_biblio"),
            )
        );
    }

    #[test]
    fn the_human_table_is_derived_from_the_same_struct() {
        let text = render_human(&assemble(&collected()));
        for expected in [
            "Examiner Baseline: EP2110298B1",
            "OPS biblio EP2110298A3: 3 cited (2 examiner, 1 applicant)",
            "OPS biblio EP2110298B1: no citation block",
            "USPTO enriched citations, application 12103744: 3 rows, 1 naming no document",
            "X,A cl. 13 [applicant]",
            "could not read citations from US (US2009261648A1): 404 NOT_FOUND",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn a_family_without_representatives_keeps_every_publication() {
        let family = json!({ "members": [
            { "publication": "WO2020123456A1" },
            { "publication": "WO2020123456A1" },
            { "publication": "CN111111111A" },
        ]});
        let members = family_members(&family, "WO2020123456A1");
        assert_eq!(
            members
                .iter()
                .map(|m| (m.office.as_str(), m.representative.as_str()))
                .collect::<Vec<_>>(),
            vec![("WO", "WO2020123456A1"), ("CN", "CN111111111A")]
        );
    }
}
