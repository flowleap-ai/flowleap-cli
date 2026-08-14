use std::path::Path;

use anyhow::{bail, Result};
use base64::Engine as _;
use clap::Parser;
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

/// File extensions the backend's `ocr` tool accepts (mirrors its
/// server-side validation so unsupported files fail fast, locally).
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "pdf", "png", "jpg", "jpeg", "gif", "webp", "avif", "docx", "pptx",
];

/// Largest local file we upload, in bytes. The backend accepts JSON bodies up
/// to 50 MB and base64 inflates content by 4/3, so ~37 MB of raw bytes is the
/// hard ceiling — stay slightly under it for JSON framing overhead.
const MAX_FILE_BYTES: u64 = 36 * 1024 * 1024;

/// Extract text from a PDF, image, or document via OCR (Mistral Document AI)
///
/// Accepts a local file path (read and base64-encoded before upload) or an
/// http(s) URL the backend fetches directly. Extracted markdown lands on
/// stdout; page/model diagnostics go to stderr, so output pipes cleanly.
///
/// Supported formats: pdf, png, jpg, jpeg, gif, webp, avif, docx, pptx.
///
/// Examples:
///   flowleap ocr ./scanned-patent.pdf
///   flowleap ocr https://example.com/spec.pdf
///   flowleap ocr figure.png --json
///   flowleap ocr ./office-action.pdf > office-action.md
#[derive(Parser)]
#[command(after_help = "\
Extracted markdown lands on stdout; diagnostics go to stderr, so output pipes cleanly.
Supported formats: pdf, png, jpg, jpeg, gif, webp, avif, docx, pptx.

Examples:
  flowleap ocr ./scanned-patent.pdf
  flowleap ocr https://example.com/spec.pdf
  flowleap ocr figure.png --json
  flowleap ocr ./office-action.pdf > office-action.md")]
pub struct OcrArgs {
    /// Local file path or http(s) URL to OCR
    input: String,
}

pub async fn run(ctx: &Context, args: OcrArgs) -> Result<()> {
    ctx.require_auth()?;

    let input = build_request_body(&args.input)?;
    let Some(result) = tools::call_tool_data(ctx, "ocr", &input).await? else {
        return Ok(());
    };

    if ctx.output_format == "json" {
        output::print_json(&result);
        return Ok(());
    }

    // Extracted text on stdout (pipe-friendly), diagnostics on stderr.
    match result.get("markdown").and_then(Value::as_str) {
        Some(markdown) => println!("{}", markdown),
        None => output::print_value(&ctx.output_format, &result, &[]),
    }
    eprintln!("{}", diagnostics_line(&result));

    Ok(())
}

/// Build the `ocr` tool input: `{ url }` for http(s) inputs,
/// `{ file: <base64>, filename }` for local paths. Local paths are validated
/// against the backend's format and size limits before any bytes are read.
fn build_request_body(input: &str) -> Result<Value> {
    if input.starts_with("http://") || input.starts_with("https://") {
        return Ok(json!({ "url": input }));
    }

    let path = Path::new(input);
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => bail!("Not a file: {}", input),
        Err(err) => bail!("Cannot read {}: {}", input, err),
    };

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        bail!(
            "Unsupported file type '{}'. Supported: {}",
            if extension.is_empty() {
                "(none)"
            } else {
                &extension
            },
            SUPPORTED_EXTENSIONS.join(", ")
        );
    }

    if metadata.len() > MAX_FILE_BYTES {
        bail!(
            "File is {:.1} MB — exceeds the {} MB OCR upload limit (the backend caps request bodies at 50 MB and base64 encoding adds ~33%)",
            metadata.len() as f64 / (1024.0 * 1024.0),
            MAX_FILE_BYTES / (1024 * 1024)
        );
    }

    let bytes = std::fs::read(path)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| input.to_string());

    Ok(json!({ "file": encoded, "filename": filename }))
}

/// One-line stderr summary of an OCR response (never mixed into stdout).
fn diagnostics_line(result: &Value) -> String {
    let pages = result
        .get("pageCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let model = result
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let cached = result
        .get("cached")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    format!(
        "OCR: {} page(s), model {}{}",
        pages,
        model,
        if cached { ", cached" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::{build_request_body, diagnostics_line, MAX_FILE_BYTES};
    use base64::Engine as _;
    use serde_json::json;

    #[test]
    fn url_input_passes_through() {
        let body = build_request_body("https://example.com/spec.pdf").unwrap();
        assert_eq!(body, json!({ "url": "https://example.com/spec.pdf" }));
    }

    #[test]
    fn local_file_is_encoded_with_filename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.pdf");
        std::fs::write(&path, b"%PDF-1.4 fake").unwrap();

        let body = build_request_body(path.to_str().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({
                "file": base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4 fake"),
                "filename": "sample.pdf",
            })
        );
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, b"plain text").unwrap();

        let err = build_request_body(path.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("Unsupported file type 'txt'"));
        assert!(err.to_string().contains("pdf"));
    }

    #[test]
    fn missing_file_is_rejected() {
        let err = build_request_body("/nonexistent/never.pdf").unwrap_err();
        assert!(err.to_string().contains("Cannot read"));
    }

    #[test]
    fn oversized_file_is_rejected_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.pdf");
        // Sparse file: sized over the limit without writing the bytes.
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();

        let err = build_request_body(path.to_str().unwrap()).unwrap_err();
        assert!(err
            .to_string()
            .contains("exceeds the 36 MB OCR upload limit"));
    }

    #[test]
    fn diagnostics_line_summarizes_response() {
        let line = diagnostics_line(&json!({
            "pageCount": 3,
            "model": "mistral-ocr-latest",
            "cached": true,
        }));
        assert_eq!(line, "OCR: 3 page(s), model mistral-ocr-latest, cached");
    }
}
