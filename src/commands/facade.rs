//! Ergonomic verbs over the `/v1/tools` facade: `compare`, `figures`,
//! `summary`, `timeline` and `convert-number`.
//!
//! Each verb is a thin wrapper: it maps CLI flags onto the corresponding
//! backend tool's input schema and delegates through
//! [`tools::call_tool`] — the exact execution path `flowleap tools run`
//! uses. Human mode renders readable sections/columns; `--json` emits the
//! backend's standard tool envelope (`{ success, tool, data,
//! executionTimeMs }`).

use anyhow::{bail, Context as AnyhowContext, Result};
use base64::Engine;
use clap::{Parser, ValueEnum};
use comfy_table::{Cell, ContentArrangement, Table};
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

/// Compare 2-10 patents side by side (bibliographic comparison).
///
/// Examples:
///   flowleap compare EP1000000 US5443036
///   flowleap compare EP1000000 EP1000001 EP1000002 --json
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap compare EP1000000 US5443036
  flowleap compare EP1000000 EP1000001 EP1000002 --json")]
pub struct CompareArgs {
    /// Patent/publication numbers to compare (2-10)
    #[arg(value_name = "DOC", num_args = 2..=10)]
    documents: Vec<String>,
}

/// List a patent's drawings/figures; save image data with --out.
///
/// Examples:
///   flowleap figures EP1000000
///   flowleap figures EP1000000 --out fig1.png
///   flowleap figures EP1000000 --out drawings.pdf --page 3
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap figures EP1000000
  flowleap figures EP1000000 --out fig1.png
  flowleap figures EP1000000 --out drawings.pdf --page 3

--out names a FILE, not a directory, and infers the format from its extension:
.png (default, rasterized from the PDF source), .pdf, or .tiff.
Without --page it saves the page the drawings start on, not the cover sheet.")]
pub struct FiguresArgs {
    /// Patent/publication number, e.g. EP1000000
    document: String,

    /// Save figure image data to this file (.png/.pdf/.tiff, binary-safe)
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,

    /// Page to save with --out (1-based; default: the drawings-start page)
    #[arg(long, requires = "out")]
    page: Option<u32>,
}

/// One-call patent snapshot: bibliography, legal status, family and term.
///
/// Examples:
///   flowleap summary EP1000000
///   flowleap summary US5443036 --json
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap summary EP1000000
  flowleap summary US5443036 --json")]
pub struct SummaryArgs {
    /// Patent/publication number, e.g. EP1000000
    document: String,
}

/// Chronological prosecution timeline (EP register + INPADOC legal events).
///
/// Examples:
///   flowleap timeline EP1000000
///   flowleap timeline EP1000000 --json
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap timeline EP1000000
  flowleap timeline EP1000000 --json")]
pub struct TimelineArgs {
    /// Publication/application number, e.g. EP1000000
    application: String,
}

/// Convert a patent number between epodoc, docdb and original formats.
///
/// Examples:
///   flowleap convert-number EP1000000 --to docdb
///   flowleap convert-number US5443036.A --to epodoc --json
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap convert-number EP1000000 --to docdb
  flowleap convert-number US5443036.A --to epodoc --json")]
pub struct ConvertNumberArgs {
    /// Patent number to convert, e.g. EP1000000
    number: String,

    /// Target format
    #[arg(long, value_enum)]
    to: TargetFormat,
}

/// Number formats supported by the backend's convert_patent_number tool.
#[derive(Clone, Copy, ValueEnum)]
enum TargetFormat {
    Epodoc,
    Docdb,
    Original,
}

impl TargetFormat {
    fn as_backend_value(&self) -> &'static str {
        match self {
            TargetFormat::Epodoc => "epodoc",
            TargetFormat::Docdb => "docdb",
            TargetFormat::Original => "original",
        }
    }
}

/// `flowleap compare` → compare_patents.
pub async fn compare(ctx: &Context, args: CompareArgs) -> Result<()> {
    ctx.require_auth()?;
    let input = json!({ "patent_numbers": args.documents });
    if let Some(data) = run_facade_tool(ctx, "compare_patents", &input).await? {
        println!("{}", render_compare(&data));
    }
    Ok(())
}

/// `flowleap figures` → get_patent_image; `--out` saves one page's image
/// payload (the same tool with include_images, base64-decoded to bytes).
pub async fn figures(ctx: &Context, args: FiguresArgs) -> Result<()> {
    ctx.require_auth()?;
    if let Some(out) = args.out {
        return save_figure(ctx, &args.document, args.page, &out).await;
    }
    let input = json!({ "patent_number": args.document });
    if let Some(data) = run_facade_tool(ctx, "get_patent_image", &input).await? {
        println!("{}", render_figures(&data));
    }
    Ok(())
}

/// `flowleap summary` → get_patent_summary.
pub async fn summary(ctx: &Context, args: SummaryArgs) -> Result<()> {
    ctx.require_auth()?;
    let input = json!({ "patent_number": args.document });
    if let Some(data) = run_facade_tool(ctx, "get_patent_summary", &input).await? {
        println!("{}", render_summary(&data));
    }
    Ok(())
}

/// `flowleap timeline` → get_prosecution_timeline.
pub async fn timeline(ctx: &Context, args: TimelineArgs) -> Result<()> {
    ctx.require_auth()?;
    let input = json!({ "patent_number": args.application });
    if let Some(data) = run_facade_tool(ctx, "get_prosecution_timeline", &input).await? {
        println!("{}", render_timeline(&data));
    }
    Ok(())
}

/// `flowleap convert-number` → convert_patent_number.
pub async fn convert_number(ctx: &Context, args: ConvertNumberArgs) -> Result<()> {
    ctx.require_auth()?;
    let input = json!({
        "patent_number": args.number,
        "to_format": args.to.as_backend_value(),
    });
    if let Some(data) = run_facade_tool(ctx, "convert_patent_number", &input).await? {
        println!("{}", render_convert(&data));
    }
    Ok(())
}

/// Call a facade tool and return its `data` payload for human rendering.
///
/// Returns `None` when the response was already fully handled: dry-run
/// descriptions and `--json` mode (which emits the standard tool envelope)
/// print directly.
async fn run_facade_tool(ctx: &Context, name: &str, input: &Value) -> Result<Option<Value>> {
    let result = tools::call_tool(ctx, name, input).await?;
    if result.get("dryRun").and_then(|v| v.as_bool()) == Some(true) {
        output::print_json(&result);
        return Ok(None);
    }
    if ctx.verbose {
        if let Some(ms) = result.get("executionTimeMs").and_then(|v| v.as_u64()) {
            eprintln!("  executionTimeMs: {}", ms);
        }
    }
    if ctx.output_format == "json" {
        output::print_json(&result);
        return Ok(None);
    }
    let data = result.get("data").cloned().unwrap_or(result);
    Ok(Some(data))
}

/// Build the `get_patent_image` input that fetches one page's image payload.
/// `.png` outputs are rasterized from the PDF source (most patents are
/// PDF-only); `.pdf`/`.tiff` fetch that format directly.
fn figure_page_input(doc: &str, page: u32, format: &str) -> Value {
    let mut input = json!({
        "patent_number": doc,
        "include_images": true,
        "pages": [page],
        "format": if format == "png" { "pdf" } else { format },
    });
    if format == "png" {
        input["render"] = json!("png");
    }
    input
}

/// The image format to write, inferred from the output file's extension.
fn figure_format_for(out: &Path) -> &'static str {
    match out
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => "pdf",
        Some("tif") | Some("tiff") => "tiff",
        _ => "png",
    }
}

/// Reject an `--out` value that cannot name an image file before anything is
/// fetched. Both cases used to fail late, at the write, as a bare
/// "write <path>" with an OS error that never said what was expected.
fn check_out_path(out: &Path) -> Result<()> {
    if out.is_dir() {
        bail!(
            "--out must name a file, not a directory ({}). Expected a path ending in \
             .png, .pdf or .tiff — for example --out {}",
            out.display(),
            out.join("figure.png").display()
        );
    }
    if out.extension().is_none() {
        bail!(
            "--out needs a file extension so the image format is unambiguous ({}). \
             Expected .png (the default, rasterized from the PDF source), .pdf or .tiff — \
             for example --out {}.png",
            out.display(),
            out.display()
        );
    }
    Ok(())
}

/// The page the drawings begin on for `source_format`, as the figure metadata
/// reports it. None when the backend named no start page — patents whose
/// drawings genuinely start at page 1 included, which costs nothing.
fn drawings_start_page(data: &Value, source_format: &str) -> Option<u32> {
    let formats = data.get("formats")?.as_array()?;
    formats
        .iter()
        .find(|entry| {
            entry
                .get("format")
                .and_then(Value::as_str)
                .is_some_and(|f| f.eq_ignore_ascii_case(source_format))
        })
        .or_else(|| formats.first())?
        .get("drawingStartPage")?
        .as_u64()
        .filter(|page| *page >= 1)
        .map(|page| page as u32)
}

/// The page `--out` saves when `--page` was omitted. Page 1 of a patent PDF is
/// the cover sheet, so defaulting to it hands back a page with no drawing on it
/// whenever the metadata already knows where the drawings start. Any failure
/// falls back to page 1 — a missing default is never worth failing the save
/// for. Uses the non-printing envelope seam so a metadata probe can never
/// write to stdout.
async fn default_figure_page(ctx: &Context, doc: &str, source_format: &str) -> u32 {
    if ctx.dry_run {
        return 1;
    }
    let input = json!({ "patent_number": doc });
    let Ok(envelope) = tools::call_tool_envelope(ctx, "get_patent_image", &input).await else {
        return 1;
    };
    if envelope["ok"].as_bool() != Some(true) {
        return 1;
    }
    drawings_start_page(&envelope["body"]["data"], source_format).unwrap_or(1)
}

/// Fetch one figure page as binary image data and write it to `out`.
///
/// Metadata and image bytes both come from `get_patent_image`: with
/// `include_images` the tool returns the page as base64 inside the JSON result,
/// so there is no second byte-fetching surface to straddle.
async fn save_figure(ctx: &Context, doc: &str, page: Option<u32>, out: &Path) -> Result<()> {
    check_out_path(out)?;
    let format = figure_format_for(out);
    // .png is rasterized from the PDF source, so the metadata to consult is
    // the PDF entry's.
    let source_format = if format == "png" { "pdf" } else { format };
    let page = match page {
        Some(page) => page,
        None => {
            let page = default_figure_page(ctx, doc, source_format).await;
            if page > 1 {
                eprintln!(
                    "note: --page not given; saving page {page}, where the drawings start \
                     (page 1 is the cover sheet)."
                );
            }
            page
        }
    };
    let input = figure_page_input(doc, page, format);
    let Some(data) = tools::call_tool_data(ctx, "get_patent_image", &input).await? else {
        return Ok(());
    };

    let figures = data.get("figures").and_then(|f| f.as_array());
    let figure = figures
        .and_then(|figs| {
            figs.iter()
                .find(|f| f.get("page").and_then(|p| p.as_u64()) == Some(u64::from(page)))
        })
        .or_else(|| figures.and_then(|figs| figs.first()));
    let Some(figure) = figure else {
        bail!("No figure data returned for page {} of {}", page, doc);
    };
    let Some(encoded) = figure.get("base64").and_then(|v| v.as_str()) else {
        bail!(
            "Backend returned no image payload for page {} of {}",
            page,
            doc
        );
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("decode base64 image data")?;
    std::fs::write(out, &bytes).with_context(|| format!("write {}", out.display()))?;

    if ctx.output_format == "json" {
        output::print_json(&json!({
            "ok": true,
            "docId": data.get("docId").cloned().unwrap_or(json!(doc)),
            "page": page,
            "format": figure.get("format").cloned().unwrap_or(json!(format)),
            "bytes": bytes.len(),
            "savedTo": out.display().to_string(),
        }));
    } else {
        println!(
            "Saved page {} of {} ({} bytes, {}) to {}",
            page,
            doc,
            bytes.len(),
            figure
                .get("format")
                .and_then(|f| f.as_str())
                .unwrap_or(format),
            out.display()
        );
    }
    Ok(())
}

/// Plain-text value for human output; "-" when missing or empty.
fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "-".to_string(),
    }
}

/// Comma-joined string array for human output; "-" when missing or empty.
fn list(value: Option<&Value>) -> String {
    let items: Vec<&str> = value
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(", ")
    }
}

/// Extracts one comparison-table attribute from a patent object.
type AttributeExtractor = fn(&Value) -> String;

/// Side-by-side attribute table for compare_patents data
/// (`{ count, patents: [{ patentNumber, title, dates, … , error? }] }`).
fn render_compare(data: &Value) -> String {
    let empty = Vec::new();
    let patents = data
        .get("patents")
        .and_then(|p| p.as_array())
        .unwrap_or(&empty);
    if patents.is_empty() {
        return "No patents to compare.".to_string();
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    let mut header = vec![Cell::new("")];
    header.extend(
        patents
            .iter()
            .map(|p| Cell::new(text(p.get("patentNumber")))),
    );
    table.set_header(header);

    let attribute_rows: [(&str, AttributeExtractor); 7] = [
        ("Title", |p| text(p.get("title"))),
        ("Filed", |p| text(p.pointer("/dates/filing"))),
        ("Published", |p| text(p.pointer("/dates/publication"))),
        ("Applicants", |p| list(p.get("applicants"))),
        ("Inventors", |p| list(p.get("inventors"))),
        ("IPC", |p| list(p.get("ipc"))),
        ("CPC", |p| list(p.get("cpc"))),
    ];
    for (label, extract) in attribute_rows {
        let mut row = vec![Cell::new(label)];
        row.extend(patents.iter().map(|p| Cell::new(extract(p))));
        table.add_row(row);
    }
    if patents
        .iter()
        .any(|p| p.get("error").and_then(|e| e.as_str()).is_some())
    {
        let mut row = vec![Cell::new("Error")];
        row.extend(patents.iter().map(|p| Cell::new(text(p.get("error")))));
        table.add_row(row);
    }

    format!("{}\nCompared {} patents", table, patents.len())
}

/// Figure metadata table for get_patent_image data
/// (`{ docId, formats: [{ format, pages, availableFormats, drawingStartPage }] }`).
fn render_figures(data: &Value) -> String {
    let doc = text(data.get("docId"));
    let empty = Vec::new();
    let formats = data
        .get("formats")
        .and_then(|f| f.as_array())
        .unwrap_or(&empty);
    if formats.is_empty() {
        return format!("No figure data available for {}.", doc);
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Format", "Pages", "Drawings start", "Also available"]);
    for entry in formats {
        table.add_row(vec![
            text(entry.get("format")),
            text(entry.get("pages")),
            text(entry.get("drawingStartPage")),
            list(entry.get("availableFormats")),
        ]);
    }

    format!(
        "Figures for {}\n{}\nSave a page: flowleap figures {} --out figure.png [--page N]",
        doc, table, doc
    )
}

/// Sectioned snapshot for get_patent_summary data
/// (`{ patentNumber, bibliography, legalStatus, family, term, errors? }`).
fn render_summary(data: &Value) -> String {
    let biblio = data.get("bibliography").cloned().unwrap_or(Value::Null);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} — {}",
        text(data.get("patentNumber")),
        text(biblio.get("title"))
    );
    let _ = writeln!(
        out,
        "  Filed:      {}    Published: {}",
        text(biblio.pointer("/dates/filing")),
        text(biblio.pointer("/dates/publication"))
    );
    let _ = writeln!(out, "  Applicants: {}", list(biblio.get("applicants")));
    let _ = writeln!(out, "  Inventors:  {}", list(biblio.get("inventors")));
    let _ = writeln!(out, "  IPC:        {}", list(biblio.get("ipc")));
    let _ = writeln!(out, "  CPC:        {}", list(biblio.get("cpc")));
    if let Some(abstract_text) = biblio.get("abstract").and_then(|a| a.as_str()) {
        let _ = writeln!(out, "\n  {}", output::truncate(abstract_text, 300));
    }

    if let Some(term) = data.get("term").filter(|t| !t.is_null()) {
        let _ = writeln!(
            out,
            "\nTerm:   {} → {} ({})",
            text(term.get("filingDate")),
            text(term.get("baseExpiryDate")),
            text(term.get("basis"))
        );
    }

    match data.get("legalStatus").filter(|l| !l.is_null()) {
        Some(legal) => {
            let empty = Vec::new();
            let events = legal
                .get("events")
                .and_then(|e| e.as_array())
                .unwrap_or(&empty);
            let latest = events
                .iter()
                .max_by_key(|e| e.get("date").and_then(|d| d.as_str()).unwrap_or(""));
            match latest {
                Some(event) => {
                    let _ = writeln!(
                        out,
                        "Legal:  {} events (latest: {} {} {})",
                        events.len(),
                        text(event.get("date")),
                        text(event.get("code")),
                        output::truncate(&text(event.get("text")), 60)
                    );
                }
                None => {
                    let _ = writeln!(out, "Legal:  no events");
                }
            }
        }
        None => {
            let _ = writeln!(out, "Legal:  unavailable");
        }
    }

    match data.get("family").filter(|f| !f.is_null()) {
        Some(family) => {
            let empty = Vec::new();
            let members = family
                .get("familyMembers")
                .and_then(|m| m.as_array())
                .unwrap_or(&empty);
            let mut ids: Vec<String> = members
                .iter()
                .take(8)
                .map(|m| text(m.get("docId")))
                .collect();
            if members.len() > 8 {
                ids.push("…".to_string());
            }
            let _ = writeln!(out, "Family: {} members: {}", members.len(), ids.join(", "));
        }
        None => {
            let _ = writeln!(out, "Family: unavailable");
        }
    }

    if let Some(errors) = data.get("errors").and_then(|e| e.as_object()) {
        for (source, message) in errors {
            let _ = writeln!(
                out,
                "Partial: {} unavailable — {}",
                source,
                text(Some(message))
            );
        }
    }

    out.trim_end().to_string()
}

/// Chronological event list for get_prosecution_timeline data
/// (`{ patentNumber, totalEvents, events: [{ source, date, code, description }] }`).
fn render_timeline(data: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Prosecution timeline for {} — {} events",
        text(data.get("patentNumber")),
        text(data.get("totalEvents"))
    );
    let empty = Vec::new();
    let events = data
        .get("events")
        .and_then(|e| e.as_array())
        .unwrap_or(&empty);
    for event in events {
        let _ = writeln!(
            out,
            "  {:<12}{:<10}{:<10}{}",
            text(event.get("date")),
            text(event.get("source")),
            text(event.get("code")),
            text(event.get("description"))
        );
    }
    if let Some(errors) = data.get("sourceErrors").and_then(|e| e.as_object()) {
        for (source, message) in errors {
            let _ = writeln!(
                out,
                "note: {} source unavailable — {}",
                source,
                text(Some(message))
            );
        }
    }
    out.trim_end().to_string()
}

/// One-line result for convert_patent_number data
/// (`{ input, inputFormat, outputFormat, converted }`).
fn render_convert(data: &Value) -> String {
    format!(
        "{} ({}) → {} ({})",
        text(data.get("input")),
        text(data.get("inputFormat")),
        text(data.get("converted")),
        text(data.get("outputFormat"))
    )
}

#[cfg(test)]
mod tests {
    use super::{
        check_out_path, drawings_start_page, figure_page_input, render_compare, render_convert,
        render_figures, render_summary, render_timeline,
    };
    use serde_json::json;
    use std::path::Path;

    /// An `--out` that cannot name an image file is rejected up front, and the
    /// message states the form that would work.
    #[test]
    fn out_path_must_name_a_file_with_an_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = check_out_path(dir.path()).expect_err("a directory is not an output file");
        let message = err.to_string();
        assert!(message.contains("not a directory"), "{message}");
        assert!(
            message.contains(".png"),
            "names the expected form: {message}"
        );

        let err = check_out_path(Path::new("figure")).expect_err("no extension");
        assert!(err.to_string().contains("file extension"), "{err}");

        for good in ["figure.png", "drawings.pdf", "scan.tiff"] {
            check_out_path(Path::new(good)).unwrap_or_else(|e| panic!("{good}: {e}"));
        }
    }

    /// The drawings-start page comes from the entry matching the format being
    /// fetched; a patent whose metadata names none leaves the caller on page 1.
    #[test]
    fn drawings_start_reads_the_matching_format_entry() {
        let data = json!({ "formats": [
            { "format": "pdf", "pages": 12, "drawingStartPage": 5 },
            { "format": "tiff", "pages": 12, "drawingStartPage": 4 },
        ]});
        assert_eq!(drawings_start_page(&data, "pdf"), Some(5));
        assert_eq!(drawings_start_page(&data, "tiff"), Some(4));
        // Unknown format falls back to the first entry rather than guessing.
        assert_eq!(drawings_start_page(&data, "jp2"), Some(5));

        let no_start = json!({ "formats": [{ "format": "pdf", "pages": 3 }] });
        assert_eq!(drawings_start_page(&no_start, "pdf"), None);
        assert_eq!(drawings_start_page(&json!({}), "pdf"), None);
    }

    /// A `.png` output rasterizes from the PDF source (EPO serves most patents
    /// PDF-only); the other formats are fetched as-is with no render step.
    #[test]
    fn figure_page_input_rasterizes_only_for_png() {
        let png = figure_page_input("EP1000000", 2, super::figure_format_for(Path::new("f.png")));
        assert_eq!(png["patent_number"], "EP1000000");
        assert_eq!(png["include_images"], true);
        assert_eq!(png["pages"], json!([2]));
        assert_eq!(png["format"], "pdf");
        assert_eq!(png["render"], "png");

        for (file, expected) in [("drawings.pdf", "pdf"), ("scan.tiff", "tiff")] {
            let input = figure_page_input("EP1", 1, super::figure_format_for(Path::new(file)));
            assert_eq!(input["format"], expected);
            assert!(input.get("render").is_none(), "{file} must not rasterize");
        }
    }

    #[test]
    fn renders_convert_as_one_line() {
        let data = json!({
            "input": "EP1000000",
            "inputFormat": "epodoc",
            "outputFormat": "docdb",
            "converted": "EP.1000000.A1",
        });
        assert_eq!(
            render_convert(&data),
            "EP1000000 (epodoc) → EP.1000000.A1 (docdb)"
        );
    }

    #[test]
    fn renders_timeline_chronologically_with_source_notes() {
        let data = json!({
            "patentNumber": "EP1000000",
            "totalEvents": 2,
            "events": [
                { "source": "register", "date": "2000-01-05", "code": "EXAM", "description": "Examination requested" },
                { "source": "legal", "date": "2005-06-01", "code": "PGFP", "description": null },
            ],
            "sourceErrors": { "legal": "rate limited" },
        });
        assert_eq!(
            render_timeline(&data),
            "Prosecution timeline for EP1000000 — 2 events\n\
             \x20 2000-01-05  register  EXAM      Examination requested\n\
             \x20 2005-06-01  legal     PGFP      -\n\
             note: legal source unavailable — rate limited"
        );
    }

    #[test]
    fn renders_summary_sections() {
        let data = json!({
            "patentNumber": "EP1000000",
            "bibliography": {
                "title": "Apparatus for manufacturing green bricks",
                "abstract": "A brick press.",
                "applicants": ["BOER MASCH BEHEER"],
                "inventors": ["DOE J"],
                "ipc": ["B28B5/02"],
                "cpc": ["B28B5/026"],
                "dates": { "filing": "1999-12-22", "publication": "2000-06-28", "priority": [] },
            },
            "legalStatus": {
                "docId": "EP1000000",
                "events": [
                    { "code": "PGFP", "date": "2010-01-03", "text": "Annual fee paid" },
                    { "code": "AK", "date": "2000-06-28", "text": "Designated states" },
                ],
            },
            "family": {
                "docId": "EP1000000",
                "familyMembers": [{ "docId": "EP1000000" }, { "docId": "NL1010536" }],
                "totalCount": 2,
            },
            "term": {
                "filingDate": "1999-12-22",
                "baseExpiryDate": "2019-12-22",
                "basis": "20 years from filing date",
            },
            "errors": { "legalStatus": "partial" },
        });
        assert_eq!(
            render_summary(&data),
            "EP1000000 — Apparatus for manufacturing green bricks\n\
             \x20 Filed:      1999-12-22    Published: 2000-06-28\n\
             \x20 Applicants: BOER MASCH BEHEER\n\
             \x20 Inventors:  DOE J\n\
             \x20 IPC:        B28B5/02\n\
             \x20 CPC:        B28B5/026\n\
             \n\
             \x20 A brick press.\n\
             \n\
             Term:   1999-12-22 → 2019-12-22 (20 years from filing date)\n\
             Legal:  2 events (latest: 2010-01-03 PGFP Annual fee paid)\n\
             Family: 2 members: EP1000000, NL1010536\n\
             Partial: legalStatus unavailable — partial"
        );
    }

    #[test]
    fn renders_compare_as_side_by_side_table() {
        let data = json!({
            "count": 2,
            "patents": [
                {
                    "patentNumber": "EP1000000",
                    "title": "Brick press",
                    "applicants": ["ACME"],
                    "inventors": ["DOE J"],
                    "ipc": ["B28B5/02"],
                    "cpc": [],
                    "dates": { "filing": "1999-12-22", "publication": "2000-06-28" },
                },
                { "patentNumber": "US5443036", "error": "not found" },
            ],
        });
        let rendered = render_compare(&data);
        // Table borders vary with the library preset — assert the content.
        for expected in [
            "EP1000000",
            "US5443036",
            "Brick press",
            "1999-12-22",
            "2000-06-28",
            "ACME",
            "DOE J",
            "B28B5/02",
            "Error",
            "not found",
            "Compared 2 patents",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}: {rendered}"
            );
        }
    }

    #[test]
    fn renders_figures_metadata_with_save_hint() {
        let data = json!({
            "docId": "EP1000000",
            "formats": [
                {
                    "format": "pdf",
                    "pages": 12,
                    "availableFormats": ["pdf", "tiff"],
                    "drawingStartPage": 5,
                },
            ],
        });
        let rendered = render_figures(&data);
        for expected in [
            "Figures for EP1000000",
            "pdf",
            "12",
            "5",
            "pdf, tiff",
            "flowleap figures EP1000000 --out figure.png",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}: {rendered}"
            );
        }
        assert_eq!(
            render_figures(&json!({ "docId": "EP1", "formats": [] })),
            "No figure data available for EP1."
        );
    }
}
