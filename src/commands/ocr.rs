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

    /// Include extracted images as base64 in the response (default: text
    /// only; the diagnostics line reports how many images exist)
    #[arg(long)]
    include_images: bool,

    /// Save extracted images to this directory (implies --include-images);
    /// stdout stays text-only
    #[arg(long, value_name = "DIR")]
    images_out: Option<std::path::PathBuf>,
}

pub async fn run(ctx: &Context, args: OcrArgs) -> Result<()> {
    ctx.require_auth()?;

    let include_images = args.include_images || args.images_out.is_some();
    let input = build_request_body(&args.input, include_images)?;
    let Some(mut result) = tools::call_tool_data(ctx, "ocr", &input).await? else {
        return Ok(());
    };

    let saved = match &args.images_out {
        Some(dir) => Some(save_images(&result, dir)?),
        None => None,
    };
    if args.images_out.is_some() {
        // The images landed on disk — drop the base64 payload so stdout
        // (either format) stays text-sized.
        strip_images(&mut result);
    }

    if ctx.output_format == "json" {
        output::print_json(&result);
    } else {
        // Extracted text on stdout (pipe-friendly), diagnostics on stderr.
        match result.get("markdown").and_then(Value::as_str) {
            Some(markdown) => println!("{}", markdown),
            None => output::print_value(&ctx.output_format, &result, &[]),
        }
        eprintln!("{}", diagnostics_line(&result));
    }
    if let (Some(saved), Some(dir)) = (saved, &args.images_out) {
        eprintln!("Saved {} image(s) to {}", saved, dir.display());
    }

    Ok(())
}

/// Decode every `images[]` entry to `<dir>/<id>.<ext>`; returns how many
/// files were written. A response without images writes nothing.
fn save_images(result: &Value, dir: &Path) -> Result<usize> {
    let Some(images) = result.get("images").and_then(Value::as_array) else {
        return Ok(0);
    };
    if images.is_empty() {
        return Ok(0);
    }
    std::fs::create_dir_all(dir)?;
    let mut saved = 0;
    for (index, image) in images.iter().enumerate() {
        let Some(base64_data) = image.get("base64").and_then(Value::as_str) else {
            continue;
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64_data)
            .map_err(|err| anyhow::anyhow!("image {} is not valid base64: {}", index, err))?;
        let id = image
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("img-{}", index));
        let extension = match image.get("mimeType").and_then(Value::as_str) {
            Some("image/jpeg") => "jpg",
            Some("image/png") => "png",
            Some("image/gif") => "gif",
            Some("image/webp") => "webp",
            Some("image/avif") => "avif",
            _ => "bin",
        };
        // Ids come from the backend response — keep only a safe basename.
        let safe_id: String = id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file_name = if safe_id.to_lowercase().ends_with(&format!(".{extension}")) {
            safe_id
        } else {
            format!("{safe_id}.{extension}")
        };
        std::fs::write(dir.join(file_name), bytes)?;
        saved += 1;
    }
    Ok(saved)
}

/// Remove base64 image payloads from an OCR result (top-level `images` and
/// per-page `images`), leaving `imageCount` as the record of what existed.
fn strip_images(result: &mut Value) {
    if let Some(obj) = result.as_object_mut() {
        obj.remove("images");
    }
    if let Some(pages) = result.get_mut("pages").and_then(Value::as_array_mut) {
        for page in pages {
            if let Some(page) = page.as_object_mut() {
                page.remove("images");
            }
        }
    }
}

/// Build the `ocr` tool input: `{ url }` for http(s) inputs,
/// `{ file: <base64>, filename }` for local paths, plus `include_images`
/// when images were requested (the backend defaults to text-only). Local
/// paths are validated against the backend's format and size limits before
/// any bytes are read.
fn build_request_body(input: &str, include_images: bool) -> Result<Value> {
    let mut body = build_source_body(input)?;
    if include_images {
        body["include_images"] = json!(true);
    }
    Ok(body)
}

fn build_source_body(input: &str) -> Result<Value> {
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
    let images = match result.get("imageCount").and_then(Value::as_u64) {
        Some(count) if count > 0 => format!(", {} image(s)", count),
        _ => String::new(),
    };
    format!(
        "OCR: {} page(s){}, model {}{}",
        pages,
        images,
        model,
        if cached { ", cached" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::{build_request_body, diagnostics_line, save_images, strip_images, MAX_FILE_BYTES};
    use base64::Engine as _;
    use serde_json::json;

    #[test]
    fn url_input_passes_through() {
        let body = build_request_body("https://example.com/spec.pdf", false).unwrap();
        assert_eq!(body, json!({ "url": "https://example.com/spec.pdf" }));
    }

    #[test]
    fn include_images_lands_in_the_request_body() {
        let body = build_request_body("https://example.com/spec.pdf", true).unwrap();
        assert_eq!(
            body,
            json!({ "url": "https://example.com/spec.pdf", "include_images": true })
        );
    }

    #[test]
    fn save_images_decodes_to_files_and_strip_images_removes_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let png = base64::engine::general_purpose::STANDARD.encode(b"fake-png-bytes");
        let mut result = json!({
            "markdown": "text",
            "imageCount": 1,
            "images": [{ "id": "img-0", "base64": png, "mimeType": "image/png" }],
            "pages": [{ "pageNumber": 1, "markdown": "text",
                        "images": [{ "id": "img-0", "base64": png, "mimeType": "image/png" }] }],
        });

        let saved = save_images(&result, dir.path()).unwrap();
        assert_eq!(saved, 1);
        assert_eq!(
            std::fs::read(dir.path().join("img-0.png")).unwrap(),
            b"fake-png-bytes"
        );

        strip_images(&mut result);
        assert!(result.get("images").is_none());
        assert!(result["pages"][0].get("images").is_none());
        assert_eq!(result["imageCount"], 1);
    }

    #[test]
    fn unsafe_image_ids_are_sanitized_to_a_basename() {
        let dir = tempfile::tempdir().unwrap();
        let data = base64::engine::general_purpose::STANDARD.encode(b"x");
        let result = json!({
            "images": [{ "id": "../evil/name", "base64": data, "mimeType": "image/jpeg" }],
        });
        save_images(&result, dir.path()).unwrap();
        assert!(dir.path().join(".._evil_name.jpg").is_file());
    }

    #[test]
    fn local_file_is_encoded_with_filename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.pdf");
        std::fs::write(&path, b"%PDF-1.4 fake").unwrap();

        let body = build_request_body(path.to_str().unwrap(), false).unwrap();
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

        let err = build_request_body(path.to_str().unwrap(), false).unwrap_err();
        assert!(err.to_string().contains("Unsupported file type 'txt'"));
        assert!(err.to_string().contains("pdf"));
    }

    #[test]
    fn missing_file_is_rejected() {
        let err = build_request_body("/nonexistent/never.pdf", false).unwrap_err();
        assert!(err.to_string().contains("Cannot read"));
    }

    #[test]
    fn oversized_file_is_rejected_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.pdf");
        // Sparse file: sized over the limit without writing the bytes.
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();

        let err = build_request_body(path.to_str().unwrap(), false).unwrap_err();
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
