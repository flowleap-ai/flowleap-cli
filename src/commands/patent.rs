use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::{examiner_baseline, tools};
use crate::output;

#[derive(Parser)]
pub struct PatentArgs {
    #[command(subcommand)]
    command: PatentCommand,
}

/// Where `patent search` searches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum SearchProvider {
    /// EPO OPS, live, with a CQL query
    #[value(alias = "epo_ops", alias = "epo-ops")]
    Epo,
    /// FlowLeap's publication index, with plain words and filters
    Index,
}

/// The sort of an index search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum IndexSort {
    Relevance,
    Date,
}

/// The flags that only the publication index takes.
#[derive(Default)]
pub(crate) struct IndexFilters {
    pub cpc: Option<String>,
    pub applicant: Option<String>,
    pub inventor: Option<String>,
    pub office: Option<String>,
    pub kind: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub sort: Option<&'static str>,
    pub offset: Option<u32>,
}

impl IndexFilters {
    /// The names of the index-only flags that are set, for the usage error.
    fn set_flags(&self) -> Vec<&'static str> {
        [
            ("--cpc", self.cpc.is_some()),
            ("--applicant", self.applicant.is_some()),
            ("--inventor", self.inventor.is_some()),
            ("--office", self.office.is_some()),
            ("--kind", self.kind.is_some()),
            ("--from", self.from.is_some()),
            ("--to", self.to.is_some()),
            ("--sort", self.sort.is_some()),
            ("--offset", self.offset.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, set)| set.then_some(name))
        .collect()
    }
}

// Parsed once per run; the size of the `Search` flags is not a cost.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum PatentCommand {
    /// Search patents in FlowLeap's publication index (the default; every
    /// office, worldwide bibliographic data, plain words), or live via EPO OPS
    /// with CQL (--provider epo)
    #[command(
        long_about = "Search patents.\n\n\
--provider index (the default) searches FlowLeap's publication index: every \
publication of every office with its English title and abstract, one quarterly \
edition. Write plain words (\"quoted phrase\", OR, -exclusion; no CQL), or \"*\" \
with a filter. The filters --cpc, --applicant, --inventor, --office, --kind, \
--from and --to, and --sort and --offset, work with the index only. The index \
uses no office quota and no Patent-Data Key. The answer gives the total, the \
edition date (asOf) and the CC BY attribution: publications newer than asOf \
are not in the index, so add --provider epo for them.\n\n\
--provider epo searches EPO OPS live with a CQL query that you write \
(worldwide bibliographic coverage, the newest publications, field \
precision).\n\n\
Without --provider: a CQL query (for example ti=battery) or --countries \
selects the epo search, and one line on stderr says so. A CQL query together \
with an index filter is refused: add --provider to say which search you want. \
When the server has no publication index, a plain-words search without \
filters runs on EPO OPS as title/abstract CQL, and one line on stderr says so; \
an explicit --provider index is refused with one clear line.",
        after_help = "Examples:\n  flowleap patent search --query '\"solid state\" battery -sulfide' --cpc H01M --applicant toyota --kind B --sort date\n  \
flowleap patent search --query 'lidar point cloud segmentation' --office EP,US --to 2018-02-28\n  \
flowleap patent search --provider epo --query 'ti=\"battery separator\" and pa=lg'"
    )]
    Search {
        /// The query. index: plain words ("quoted phrase", OR, -exclusion), or
        /// "*" with --cpc, --applicant or --inventor. epo: EPO CQL (e.g.
        /// 'ti="battery separator" and pa=lg'), which you write yourself (the
        /// flowleap-patent skill has the method)
        #[arg(long, short)]
        query: String,

        /// Where to search: index (the publication index, one dated edition;
        /// the default) or epo (EPO OPS, live, CQL). Without this flag, a CQL
        /// query or --countries selects epo
        #[arg(long, value_enum)]
        provider: Option<SearchProvider>,

        /// Maximum results to return (1-100)
        #[arg(long, default_value = "10")]
        limit: u32,

        /// epo only: country filter, comma-separated (e.g. "EP,WO"); "all" disables
        #[arg(long)]
        countries: Option<String>,

        /// Only report the result total (cheap count probe: range 1-1,
        /// no per-document bibliography fan-out)
        #[arg(long)]
        count_only: bool,

        /// index only: CPC subclass (H01M), main group (H01M10) or full symbol
        /// (H01M10/0525); comma-separated for a list (any of them)
        #[arg(long)]
        cpc: Option<String>,

        /// index only: words of the applicant name, any case (e.g. "toyota motor")
        #[arg(long)]
        applicant: Option<String>,

        /// index only: words of an inventor name, any case
        #[arg(long)]
        inventor: Option<String>,

        /// index only: publication office (country code), comma-separated
        /// for a list (e.g. "EP,US,CN")
        #[arg(long)]
        office: Option<String>,

        /// index only: kind-code prefix, comma-separated for a list: B grants,
        /// A applications, or an exact kind such as A1
        #[arg(long)]
        kind: Option<String>,

        /// index only: publication date from, inclusive (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// index only: publication date to, inclusive (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// index only: relevance (the default) or date (newest first)
        #[arg(long, value_enum)]
        sort: Option<IndexSort>,

        /// index only: number of results to skip (the next page starts at
        /// offset + limit)
        #[arg(long)]
        offset: Option<u32>,
    },
    /// The Examiner Baseline: every document cited across a publication's
    /// family, by office and by who cited it, from the offices' own records
    #[command(
        long_about = "The Examiner Baseline: every document cited across a publication's \
INPADOC family, as one matrix of cited document x office.\n\n\
It calls the backend examiner_baseline tool, so the CLI and the chat clients \
print one identical Baseline. Against an older backend without that tool it \
runs the same walk locally (--verbose names the path): it walks the family \
(get_family), reads the references-cited block of every \
publication of every member (get_bibliography: cited by examiner or applicant, \
category, cited claims, phase, passages), and for a US grant adds the USPTO \
enriched office-action citations of its application. A cell shows the category \
and cited claims, each category with its own claims where the search report \
pairs them (X cl. 13, A cl. 1,5,9), or `applicant` when only the applicant \
cited the document. A \
member whose office returned no citation block is printed as a gap, never as \
\"nothing cited\". No model is called; every count is the office's count.",
        after_help = "Examples:\n  flowleap patent examiner-baseline EP2110298B1\n  flowleap --json patent examiner-baseline US7722129B2"
    )]
    ExaminerBaseline {
        /// Publication number with kind, e.g. EP2110298B1
        publication: String,
    },
}

/// The search that one `patent search` call runs.
#[derive(Debug, PartialEq, Eq)]
enum SearchRoute {
    /// The publication index. `fallback` is true on the default path only: a
    /// server without the index then gets the EPO search.
    Index { fallback: bool },
    /// EPO OPS. `inferred` is the reason when the caller did not write
    /// `--provider epo`.
    Epo { inferred: Option<&'static str> },
}

/// The CQL index names of EPO OPS. A plain-words query can contain one of
/// them as a word (`in`, `pa`), so a name counts only before a relation.
const CQL_FIELDS: &[&str] = &[
    "ti", "ab", "ta", "txt", "pa", "in", "ia", "pn", "ap", "pr", "num", "pd", "ct", "ex", "op",
    "cpc", "cpci", "cpca", "cpcc", "ipc", "ic", "cl", "fa", "famn",
];

/// Does the query look like EPO CQL and not like plain words?
///
/// Two structural signs only: a relation sign (`ti=battery`, `pd>=2020`), or
/// a CQL index name, then `all`, `any` or `within`, then a quoted value
/// (`ta all "a b"`). Plain words have neither ("radar in all weather" is
/// words); the text inside a quoted phrase is not read.
pub(crate) fn looks_like_cql(query: &str) -> bool {
    let mut outside = String::new();
    let mut in_quotes = false;
    for ch in query.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            // One placeholder word for the phrase.
            outside.push_str(if in_quotes { " \" " } else { " " });
        } else if !in_quotes {
            outside.push(ch);
        }
    }
    if outside.contains(['=', '<', '>']) {
        return true;
    }
    let words: Vec<String> = outside
        .split_whitespace()
        .map(|word| word.trim_start_matches('(').to_ascii_lowercase())
        .collect();
    words.windows(3).any(|triple| {
        CQL_FIELDS.contains(&triple[0].as_str())
            && matches!(triple[1].as_str(), "all" | "any" | "within")
            && triple[2] == "\""
    })
}

/// Decide the search from the flags (backend ADR 0026: the index is where
/// discovery starts).
///
/// An explicit `--provider` is obeyed, and a flag of the other search is a
/// usage error. Without `--provider` the default is the index, with two
/// exceptions that keep the commands of before this default working: a CQL
/// query and `--countries` are EPO-only, so they select the EPO search. Plain
/// words sent to OPS fail loudly, but CQL sent to the index is read as words
/// and gives a wrong answer silently; that is why the CLI infers and does not
/// send CQL to the index. A CQL query together with an index filter has no
/// safe reading, so it is refused.
fn route_search(
    provider: Option<SearchProvider>,
    query: &str,
    countries: bool,
    filters: &IndexFilters,
) -> Result<SearchRoute> {
    let index_flags = filters.set_flags();
    match provider {
        Some(SearchProvider::Index) => {
            if countries {
                bail!("--countries works with --provider epo only. For the index, use --office (e.g. --office EP,US).");
            }
            Ok(SearchRoute::Index { fallback: false })
        }
        Some(SearchProvider::Epo) => {
            if !index_flags.is_empty() {
                bail!(
                    "{} work(s) with --provider index only. Remove --provider epo to search the index, or put the filter in the CQL query.",
                    index_flags.join(", ")
                );
            }
            Ok(SearchRoute::Epo { inferred: None })
        }
        None => {
            let cql = looks_like_cql(query);
            if index_flags.is_empty() {
                return Ok(if cql {
                    SearchRoute::Epo {
                        inferred: Some("the query is CQL"),
                    }
                } else if countries {
                    SearchRoute::Epo {
                        inferred: Some("--countries works with the EPO search only"),
                    }
                } else {
                    SearchRoute::Index { fallback: true }
                });
            }
            if cql {
                bail!(
                    "The query looks like CQL, and {} work(s) with the publication index only. \
                     Add --provider index to search the query as plain words, or add --provider epo \
                     and put the filter in the CQL query.",
                    index_flags.join(", ")
                );
            }
            if countries {
                bail!(
                    "--countries works with --provider epo only, and {} work(s) with the publication \
                     index only. For the index, use --office (e.g. --office EP,US).",
                    index_flags.join(", ")
                );
            }
            // An index filter cannot go to OPS, so there is no fallback.
            Ok(SearchRoute::Index { fallback: false })
        }
    }
}

/// Plain words of an index search as title/abstract CQL, for the fallback to
/// EPO OPS on a server without the publication index. Words and quoted
/// phrases are AND, `OR` joins its two neighbours, `-term` excludes. `None`
/// when there is no positive term (for example `*`).
pub(crate) fn plain_words_to_cql(query: &str) -> Option<String> {
    // (negative, text, is_phrase)
    let mut terms: Vec<(bool, String, bool)> = Vec::new();
    let mut or_before: Vec<bool> = Vec::new();
    let mut pending_or = false;
    let mut chars = query.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        let negative = ch == '-';
        if negative {
            chars.next();
        }
        let (text, phrase) = if chars.peek() == Some(&'"') {
            chars.next();
            let text: String = chars.by_ref().take_while(|c| *c != '"').collect();
            (text, true)
        } else {
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '"' {
                    break;
                }
                text.push(c);
                chars.next();
            }
            (text, false)
        };
        let text: String = text
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '*' | '?'))
            .collect();
        let text = text.trim().to_string();
        if !phrase && !negative && text == "OR" {
            pending_or = !terms.is_empty();
            continue;
        }
        if text.is_empty() || text == "*" {
            continue;
        }
        or_before.push(pending_or && !negative);
        pending_or = false;
        terms.push((negative, text, phrase));
    }
    let clause = |text: &str, phrase: bool| {
        if phrase || text.contains(' ') {
            format!("ta=\"{text}\"")
        } else {
            format!("ta={text}")
        }
    };
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut negatives: Vec<String> = Vec::new();
    for (index, (negative, text, phrase)) in terms.iter().enumerate() {
        let part = clause(text, *phrase);
        if *negative {
            negatives.push(part);
        } else if or_before[index] && !groups.is_empty() {
            groups.last_mut().expect("a group exists").push(part);
        } else {
            groups.push(vec![part]);
        }
    }
    if groups.is_empty() {
        return None;
    }
    let mut cql = groups
        .into_iter()
        .map(|group| {
            if group.len() == 1 {
                group.into_iter().next().expect("one part")
            } else {
                format!("({})", group.join(" or "))
            }
        })
        .collect::<Vec<_>>()
        .join(" and ");
    for negative in negatives {
        cql.push_str(" not ");
        cql.push_str(&negative);
    }
    Some(cql)
}

pub async fn run(ctx: &Context, args: PatentArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        PatentCommand::Search {
            query,
            provider,
            limit,
            countries,
            count_only,
            cpc,
            applicant,
            inventor,
            office,
            kind,
            from,
            to,
            sort,
            offset,
        } => {
            let filters = IndexFilters {
                cpc,
                applicant,
                inventor,
                office,
                kind,
                from,
                to,
                sort: sort.map(|sort| match sort {
                    IndexSort::Relevance => "relevance",
                    IndexSort::Date => "date",
                }),
                offset,
            };
            match route_search(provider, &query, countries.is_some(), &filters)? {
                SearchRoute::Index { fallback } => {
                    index_search(ctx, &query, limit, &filters, count_only, fallback).await
                }
                SearchRoute::Epo { inferred } => {
                    if let Some(reason) = inferred {
                        eprintln!(
                            "Note: {reason}, so this search ran on EPO OPS (--provider epo). \
                             The default search is the publication index, with plain words."
                        );
                    }
                    epo_search(ctx, &query, limit, countries.as_deref(), count_only).await
                }
            }
        }
        PatentCommand::ExaminerBaseline { publication } => {
            examiner_baseline::run(ctx, &publication).await
        }
    }
}

/// Column set for a hydrated EPO result list — the shape `search_patents`
/// returns for provider=epo_ops.
pub(crate) const SEARCH_COLUMNS: &[(&str, &str)] = &[
    ("docId", "Patent ID"),
    ("title", "Title"),
    ("applicants", "Applicants"),
    ("publicationDate", "Date"),
];

/// Build the `search_patents` input for an EPO CQL search over `start`-`end`.
/// `countries` is the CLI's comma-separated filter; "all" means unfiltered, and
/// the tool takes a list of two-letter codes.
pub(crate) fn epo_search_input(query: &str, range: String, countries: Option<&str>) -> Value {
    let mut input = json!({
        "query": query,
        "provider": "epo_ops",
        "range": range,
    });
    let codes: Vec<String> = countries
        .filter(|value| !value.eq_ignore_ascii_case("all"))
        .map(|value| {
            value
                .split(',')
                .map(|code| code.trim().to_uppercase())
                .filter(|code| !code.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if !codes.is_empty() {
        input["countries"] = json!(codes);
    }
    input
}

/// Print an EPO search result. JSON keeps the tool's `data` payload verbatim
/// (`total` and `docs` included) so agents see the documented envelope;
/// table/human render the hydrated `docs` list when present, else whatever
/// the tool returned, so an unexpected shape is never swallowed.
/// The one-line warning for a page the backend could not fully hydrate.
///
/// `search_patents` returns `detailsUnavailable` — the count of results whose
/// bibliography could not be read — and OMITS it when the page is whole. Those
/// results still carry their `docId`; only the detail fields came back null.
///
/// The table prints `docs` alone, so without this line a half-read page looks
/// exactly like a page of patents that have no title and no applicant. That is
/// the wrong conclusion to hand an analyst, so say which it is. `None` when the
/// page is whole, so a complete search stays as quiet as it was before.
fn partial_page_note(result: &Value) -> Option<String> {
    let unavailable = result.get("detailsUnavailable")?.as_u64()?;
    if unavailable == 0 {
        return None;
    }
    Some(format!(
        "Note: {unavailable} result(s) could not be read — their title, applicants and date are \
         blank because the detail fetch failed, not because the patent has none. Re-read those by \
         number with `flowleap ops biblio <docId>`."
    ))
}

pub(crate) fn print_search_result(ctx: &Context, result: &Value) {
    if ctx.output_format == "json" {
        output::print_json(result);
        return;
    }
    match result.get("docs") {
        Some(docs) => output::print_value(&ctx.output_format, docs, SEARCH_COLUMNS),
        None => output::print_value(&ctx.output_format, result, SEARCH_COLUMNS),
    }
    // After the rows, so it is the last thing read and cannot be mistaken for a
    // column header. JSON output already carries the field verbatim.
    if let Some(note) = partial_page_note(result) {
        eprintln!("{note}");
    }
}

/// The EPO search of `patent search`: the page, or the count probe.
async fn epo_search(
    ctx: &Context,
    query: &str,
    limit: u32,
    countries: Option<&str>,
    count_only: bool,
) -> Result<()> {
    if count_only {
        count_probe(ctx, query, countries).await
    } else {
        search(ctx, query, limit, countries).await
    }
}

async fn search(ctx: &Context, query: &str, limit: u32, countries: Option<&str>) -> Result<()> {
    let input = epo_search_input(query, format!("1-{}", limit.clamp(1, 100)), countries);
    if let Some(result) = tools::call_tool_data(ctx, "search_patents", &input).await? {
        print_search_result(ctx, &result);
    }
    Ok(())
}

/// `--count-only`: probe the total for a CQL query without hydrating any
/// documents (range 1-1, `details: false`). The backend computes `total`
/// before the country post-filter, so it always describes the full CQL
/// result set — the probe reports it as such.
async fn count_probe(ctx: &Context, query: &str, countries: Option<&str>) -> Result<()> {
    let mut input = epo_search_input(query, "1-1".into(), countries);
    input["details"] = json!(false);
    let Some(result) = tools::call_tool_data(ctx, "search_patents", &input).await? else {
        return Ok(());
    };
    let total = result.get("total").cloned().unwrap_or(Value::Null);
    if ctx.output_format == "json" {
        let mut payload = json!({ "query": query, "total": total });
        if let Some(countries) = input.get("countries") {
            payload["countries"] = countries.clone();
            payload["note"] = json!("total counts the full CQL result set; the country filter applies to returned documents only");
        }
        output::print_json(&payload);
    } else {
        match &total {
            Value::Null => println!("Total: unknown (tool returned no `total` field)"),
            value => println!("Total: {}", value),
        }
    }
    Ok(())
}

/// A comma-separated flag value as the tool input: one value as a string, more
/// as a list. `upper` is for codes (CPC, office, kind).
fn one_or_many(value: &str, upper: bool) -> Option<Value> {
    let mut items: Vec<String> = value
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| {
            if upper {
                item.to_uppercase()
            } else {
                item.to_string()
            }
        })
        .collect();
    match items.len() {
        0 => None,
        1 => Some(Value::String(items.remove(0))),
        _ => Some(json!(items)),
    }
}

/// Build the `search_patents` input for a publication-index search (backend
/// ADR 0026): plain words, the page as `range`, and only the filters that are
/// set.
pub(crate) fn index_search_input(query: &str, limit: u32, filters: &IndexFilters) -> Value {
    let limit = limit.clamp(1, 100);
    let start = filters.offset.unwrap_or(0) + 1;
    let mut input = json!({
        "query": query,
        "provider": "index",
        "range": format!("{start}-{}", start + limit - 1),
    });
    let lists = [
        ("cpc", &filters.cpc),
        ("office", &filters.office),
        ("kind", &filters.kind),
    ];
    for (name, value) in lists {
        if let Some(value) = value.as_deref().and_then(|value| one_or_many(value, true)) {
            input[name] = value;
        }
    }
    let texts = [
        ("applicant", &filters.applicant),
        ("inventor", &filters.inventor),
        ("from", &filters.from),
        ("to", &filters.to),
    ];
    for (name, value) in texts {
        if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            input[name] = json!(value);
        }
    }
    if let Some(sort) = filters.sort {
        input["sort"] = json!(sort);
    }
    input
}

/// Does this failed envelope say that the server has no publication index?
///
/// Two served shapes, both read from structured fields only:
/// - keyless access is off (or an older backend): the `search_patents` schema
///   has no provider `index`, so the input validation fails and one of its
///   `issues` has the path `provider`;
/// - keyless access is on but the index database is not configured:
///   `503 publication_index_not_configured`.
pub(crate) fn index_not_enabled(envelope: &Value) -> bool {
    let body = &envelope["body"];
    let code = body
        .pointer("/error/code")
        .or_else(|| body.get("code"))
        .and_then(Value::as_str);
    if code == Some("publication_index_not_configured") {
        return true;
    }
    let status = envelope.get("status").and_then(Value::as_u64);
    if !matches!(status, Some(400 | 422)) {
        return false;
    }
    body.pointer("/error/issues")
        .or_else(|| body.pointer("/error/details/issues"))
        .and_then(Value::as_array)
        .is_some_and(|issues| {
            issues.iter().any(|issue| match issue.get("path") {
                Some(Value::String(path)) => path == "provider",
                Some(Value::Array(path)) => path.first() == Some(&json!("provider")),
                _ => false,
            })
        })
}

/// The one line a user reads when the server has no publication index.
pub(crate) const INDEX_NOT_ENABLED: &str =
    "The publication index is not enabled on this server. Use the EPO search: \
     flowleap patent search --provider epo --query '<CQL>'.";

/// The lines after the rows of an index search: the total, the edition date
/// and the CC BY attribution (the license asks for the credit wherever the
/// results are shown).
pub(crate) fn index_footer(result: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(total) = result.get("total").and_then(Value::as_u64) {
        let lower_bound = result.get("totalIsLowerBound").and_then(Value::as_bool) == Some(true);
        lines.push(if lower_bound {
            format!("Total: at least {total} (the count stopped at its cap)")
        } else {
            format!("Total: {total}")
        });
    }
    if let Some(as_of) = result.get("asOf").and_then(Value::as_str) {
        lines.push(format!(
            "As of: {as_of} (publication index edition; publications newer than this date are not in the index: add --provider epo for them)"
        ));
    }
    if let Some(text) = result.pointer("/attribution/text").and_then(Value::as_str) {
        lines.push(format!("Source: {text}"));
    }
    lines
}

/// Run a publication-index search. The call reads the raw envelope so that a
/// server without the index gets one clear line, not an error dump; every
/// other failure renders as on every other data command.
async fn index_search(
    ctx: &Context,
    query: &str,
    limit: u32,
    filters: &IndexFilters,
    count_only: bool,
    fallback: bool,
) -> Result<()> {
    let page = if count_only { 1 } else { limit };
    let input = index_search_input(query, page, filters);
    let mut envelope = tools::call_tool_envelope(ctx, "search_patents", &input).await?;
    let dry_run = envelope.get("dryRun").and_then(Value::as_bool) == Some(true);
    if !dry_run && envelope.get("ok").and_then(Value::as_bool) != Some(true) {
        if !index_not_enabled(&envelope) {
            return Err(ctx.fail_with_envelope(&envelope));
        }
        // The default path only: the caller did not ask for the index by
        // name, so a server without it gets the search it has.
        if let Some(cql) = fallback.then(|| plain_words_to_cql(query)).flatten() {
            eprintln!(
                "Note: the publication index is not enabled on this server, so this search ran \
                 on EPO OPS (--provider epo) with the CQL: {cql}"
            );
            return epo_search(ctx, &cql, limit, None, count_only).await;
        }
        let error = crate::client::printed_error_for(&envelope);
        if ctx.output_format == "json" {
            // JSON keeps the backend envelope; the hint is an additive field.
            envelope["indexUnavailableHint"] = json!({
                "code": "publication_index_not_enabled",
                "requiresHumanIntervention": false,
                "message": INDEX_NOT_ENABLED,
                "fallback": "flowleap patent search --provider epo --query '<CQL>'",
            });
            output::print_json(&envelope);
        } else {
            eprintln!("{INDEX_NOT_ENABLED}");
        }
        return Err(error.into());
    }
    let result = if dry_run {
        envelope
    } else {
        ctx.print_trial_budget_low_warning(&envelope);
        envelope.get("body").cloned().unwrap_or(Value::Null)
    };
    let Some(result) = tools::unwrap_tool_result(ctx, result) else {
        return Ok(());
    };
    if ctx.output_format == "json" {
        output::print_json(&result);
        return Ok(());
    }
    if !count_only {
        match result.get("docs") {
            Some(docs) => output::print_value(&ctx.output_format, docs, SEARCH_COLUMNS),
            None => output::print_value(&ctx.output_format, &result, SEARCH_COLUMNS),
        }
    }
    for line in index_footer(&result) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        epo_search_input, index_footer, index_not_enabled, index_search_input, looks_like_cql,
        partial_page_note, plain_words_to_cql, route_search, IndexFilters, SearchProvider,
        SearchRoute,
    };
    use serde_json::json;

    #[test]
    fn the_index_input_carries_the_page_and_only_the_set_filters() {
        let plain = index_search_input("lidar", 10, &IndexFilters::default());
        assert_eq!(
            plain,
            json!({ "query": "lidar", "provider": "index", "range": "1-10" })
        );

        let filters = IndexFilters {
            cpc: Some("h01m, G06N3".into()),
            office: Some("ep".into()),
            kind: Some("B".into()),
            applicant: Some("toyota motor".into()),
            sort: Some("date"),
            offset: Some(25),
            ..IndexFilters::default()
        };
        let input = index_search_input("*", 25, &filters);
        assert_eq!(input["range"], "26-50");
        assert_eq!(input["cpc"], json!(["H01M", "G06N3"]));
        assert_eq!(input["office"], "EP");
        assert_eq!(input["kind"], "B");
        assert_eq!(input["applicant"], "toyota motor");
        assert_eq!(input["sort"], "date");
        assert!(input.get("inventor").is_none());
        assert!(input.get("countries").is_none());
    }

    #[test]
    fn a_relation_or_an_index_name_before_all_any_within_is_cql() {
        for cql in [
            "ti=battery",
            "ti=\"battery separator\" and pa=lg",
            "pd>=2020",
            "ta all \"solid state battery\"",
            "(cpc any \"H01M10 H01M4\") and pa = toyota",
            "pd within \"2020 2021\"",
        ] {
            assert!(looks_like_cql(cql), "{cql}");
        }
        for words in [
            "solid state battery",
            "\"solid state\" battery -sulfide",
            "lidar OR radar in all weather",
            "\"a=b\" notation",
            "*",
        ] {
            assert!(!looks_like_cql(words), "{words}");
        }
    }

    #[test]
    fn the_default_is_the_index_and_an_epo_only_sign_selects_epo() {
        let none = IndexFilters::default();
        let cpc = IndexFilters {
            cpc: Some("H01M".into()),
            ..IndexFilters::default()
        };
        let route =
            |provider, query, countries, filters| route_search(provider, query, countries, filters);
        assert_eq!(
            route(None, "solid state battery", false, &none).unwrap(),
            SearchRoute::Index { fallback: true }
        );
        // A filter cannot go to OPS: the index, without the fallback.
        assert_eq!(
            route(None, "battery", false, &cpc).unwrap(),
            SearchRoute::Index { fallback: false }
        );
        assert_eq!(
            route(Some(SearchProvider::Index), "battery", false, &none).unwrap(),
            SearchRoute::Index { fallback: false }
        );
        assert!(matches!(
            route(None, "ti=battery", false, &none).unwrap(),
            SearchRoute::Epo { inferred: Some(_) }
        ));
        assert!(matches!(
            route(None, "battery", true, &none).unwrap(),
            SearchRoute::Epo { inferred: Some(_) }
        ));
        assert_eq!(
            route(Some(SearchProvider::Epo), "ti=battery", true, &none).unwrap(),
            SearchRoute::Epo { inferred: None }
        );
        // No safe reading: refused.
        assert!(route(None, "ti=battery", false, &cpc).is_err());
        assert!(route(None, "battery", true, &cpc).is_err());
        assert!(route(Some(SearchProvider::Epo), "ti=battery", false, &cpc).is_err());
        assert!(route(Some(SearchProvider::Index), "battery", true, &none).is_err());
    }

    #[test]
    fn plain_words_become_title_abstract_cql() {
        assert_eq!(
            plain_words_to_cql("solid state").as_deref(),
            Some("ta=solid and ta=state")
        );
        assert_eq!(
            plain_words_to_cql("\"solid state\" battery -sulfide").as_deref(),
            Some("ta=\"solid state\" and ta=battery not ta=sulfide")
        );
        assert_eq!(
            plain_words_to_cql("lidar OR radar segmentation").as_deref(),
            Some("(ta=lidar or ta=radar) and ta=segmentation")
        );
        assert_eq!(plain_words_to_cql("*"), None);
        assert_eq!(plain_words_to_cql("-sulfide"), None);
    }

    #[test]
    fn only_a_provider_issue_or_the_not_configured_code_means_no_index() {
        let off = json!({ "status": 422, "body": { "success": false, "error": {
            "code": "INVALID_INPUT", "message": "Invalid input for tool search_patents",
            "issues": [{ "path": "provider", "message": "Invalid option" }] } } });
        assert!(index_not_enabled(&off));
        let not_configured = json!({ "status": 503, "body": { "error": {
            "code": "publication_index_not_configured", "message": "x" } } });
        assert!(index_not_enabled(&not_configured));
        // A bad date is the caller's error, not a missing index.
        let bad_date = json!({ "status": 422, "body": { "error": {
            "code": "INVALID_INPUT", "issues": [{ "path": "from", "message": "a date" }] } } });
        assert!(!index_not_enabled(&bad_date));
        // The message is never read.
        let wording = json!({ "status": 500, "body": { "error": {
            "code": "INTERNAL_ERROR", "message": "provider index failed" } } });
        assert!(!index_not_enabled(&wording));
    }

    #[test]
    fn the_index_footer_names_the_total_the_edition_and_the_credit() {
        let lines = index_footer(&json!({
            "total": 1000, "totalIsLowerBound": true, "asOf": "2026-09-23",
            "attribution": { "text": "Contains data from Google Patents Public Datasets, licensed under CC BY 4.0." }
        }));
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("at least 1000"));
        assert!(lines[1].contains("2026-09-23"));
        assert!(lines[2].contains("CC BY 4.0"));
        assert!(index_footer(&json!({ "docs": [] })).is_empty());
    }

    #[test]
    fn a_partial_page_says_how_many_rows_were_not_read() {
        // Without this the table shows blank Title/Applicants cells and nothing
        // says whether the patent HAS no title or was simply never read.
        let note = partial_page_note(&json!({ "returned": 10, "detailsUnavailable": 3 }));
        let note = note.expect("a partial page must announce itself");
        assert!(note.contains('3'), "note must carry the count: {note}");
        assert!(
            note.contains("not read") || note.contains("could not be read"),
            "note must say the rows were unread, not empty: {note}"
        );
    }

    #[test]
    fn a_whole_page_says_nothing() {
        // The backend omits the field when every result hydrated, so silence is
        // the correct rendering — a "0 unavailable" line would be noise.
        assert_eq!(partial_page_note(&json!({ "returned": 10 })), None);
        assert_eq!(partial_page_note(&json!({ "detailsUnavailable": 0 })), None);
        assert_eq!(partial_page_note(&json!({ "docs": [] })), None);
    }

    #[test]
    fn a_non_numeric_count_is_ignored_rather_than_printed_raw() {
        assert_eq!(
            partial_page_note(&json!({ "detailsUnavailable": "three" })),
            None
        );
        assert_eq!(
            partial_page_note(&json!({ "detailsUnavailable": null })),
            None
        );
    }

    #[test]
    fn country_filter_becomes_a_code_list_and_all_means_unfiltered() {
        let filtered = epo_search_input("ti=battery", "1-10".into(), Some("ep, wo"));
        assert_eq!(filtered["provider"], "epo_ops");
        assert_eq!(filtered["range"], "1-10");
        assert_eq!(filtered["countries"], json!(["EP", "WO"]));

        // "all" and an absent filter both send no countries key at all.
        for countries in [Some("all"), Some("ALL"), None] {
            let unfiltered = epo_search_input("ti=battery", "1-10".into(), countries);
            assert!(
                unfiltered.get("countries").is_none(),
                "countries must be absent for {countries:?}"
            );
        }
    }
}
