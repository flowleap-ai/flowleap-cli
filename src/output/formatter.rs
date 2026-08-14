use comfy_table::{Cell, Table};
use serde_json::Value;

/// Print raw JSON (for --output json)
pub fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// Resolve a column key against one record. A dotted key walks nested objects
/// (`applicationMetaData.inventionTitle`, the shape USPTO ODP actually
/// returns); when that path is absent, the last segment is tried as a flat key
/// so the same column list still renders a payload the backend has flattened.
fn field<'a>(record: &'a Value, key: &str) -> Option<&'a Value> {
    let walked = key
        .split('.')
        .try_fold(record, |cursor, segment| cursor.get(segment));
    match walked {
        Some(value) => Some(value),
        None => key
            .rsplit('.')
            .next()
            .filter(|last| *last != key)
            .and_then(|last| record.get(last)),
    }
}

/// Print a JSON array as a table (for --output table)
pub fn print_table(rows: &[Value], columns: &[(&str, &str)]) {
    let mut table = Table::new();
    table.set_header(columns.iter().map(|(_, header)| Cell::new(header)));

    for row in rows {
        let cells: Vec<Cell> = columns
            .iter()
            .map(|(key, _)| {
                let val = field(row, key).cloned().unwrap_or(Value::Null);
                match val {
                    Value::String(s) => Cell::new(truncate(&s, 50)),
                    Value::Array(arr) => {
                        let items: String = arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        Cell::new(truncate(&items, 50))
                    }
                    Value::Null => Cell::new("-"),
                    other => Cell::new(other.to_string()),
                }
            })
            .collect();
        table.add_row(cells);
    }

    println!("{table}");
}

/// Print a value in human-readable format
pub fn print_value(format: &str, value: &Value, columns: &[(&str, &str)]) {
    match format {
        "json" => print_json(value),
        "table" => {
            if let Some(arr) = value.as_array() {
                print_table(arr, columns);
            } else {
                print_json(value);
            }
        }
        _ => {
            // Human-readable: pretty print with some structure
            if let Some(arr) = value.as_array() {
                if arr.is_empty() {
                    println!("No results found.");
                    return;
                }
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        println!("---");
                    }
                    print_object_human(item, columns);
                }
                println!("\n{} result(s)", arr.len());
            } else {
                print_object_human(value, columns);
            }
        }
    }
}

fn print_object_human(obj: &Value, columns: &[(&str, &str)]) {
    let mut printed = false;

    for (key, label) in columns {
        if let Some(val) = field(obj, key) {
            match val {
                Value::String(s) => {
                    println!("  {}: {}", label, s);
                    printed = true;
                }
                Value::Array(arr) => {
                    let items: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    if !items.is_empty() {
                        println!("  {}: {}", label, items.join(", "));
                        printed = true;
                    }
                }
                Value::Null => {}
                other => {
                    println!("  {}: {}", label, other);
                    printed = true;
                }
            }
        }
    }

    if !printed {
        print_json(obj);
    }
}

/// Truncate a string to at most `max` characters (not bytes), appending "..."
/// when truncation occurs. Unicode-safe: counts by `char`, never slices on a
/// byte boundary mid-codepoint. For `max <= 3`, returns the first `max` chars
/// without an ellipsis since "..." alone wouldn't fit.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_string();
    }
    if max <= 3 {
        return s.chars().take(max).collect();
    }
    let prefix: String = s.chars().take(max - 3).collect();
    format!("{}...", prefix)
}

#[cfg(test)]
mod tests {
    use super::{field, truncate};
    use serde_json::json;

    #[test]
    fn dotted_column_keys_walk_nested_records() {
        // The USPTO ODP search shape: identity at the top, everything else
        // under applicationMetaData.
        let record = json!({
            "applicationNumberText": "16123456",
            "applicationMetaData": { "inventionTitle": "Battery cooling" },
        });
        assert_eq!(
            field(&record, "applicationMetaData.inventionTitle"),
            Some(&json!("Battery cooling"))
        );
        assert_eq!(
            field(&record, "applicationNumberText"),
            Some(&json!("16123456"))
        );
        assert_eq!(field(&record, "applicationMetaData.grantDate"), None);
    }

    #[test]
    fn a_dotted_key_falls_back_to_the_flat_last_segment() {
        // Same column list against a payload the backend flattened.
        let flat = json!({ "inventionTitle": "Battery cooling" });
        assert_eq!(
            field(&flat, "applicationMetaData.inventionTitle"),
            Some(&json!("Battery cooling"))
        );
        // A plain key never falls back to anything.
        assert_eq!(field(&flat, "title"), None);
    }

    #[test]
    fn truncate_under_max() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_max() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_over_max() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_zero_does_not_panic() {
        assert_eq!(truncate("anything", 0), "");
    }

    #[test]
    fn truncate_small_max_does_not_panic() {
        // max < 3 would have caused &s[..max-3] to underflow.
        assert_eq!(truncate("hello", 1), "h");
        assert_eq!(truncate("hello", 2), "he");
        assert_eq!(truncate("hello", 3), "hel");
    }

    #[test]
    fn truncate_unicode_boundary() {
        // "résumé" is 6 chars but 8 bytes; naive byte slicing would panic.
        assert_eq!(truncate("résumé", 10), "résumé");
        assert_eq!(truncate("résuméé", 5), "ré...");
    }

    #[test]
    fn truncate_multibyte_emoji() {
        // Chinese characters are each 3 bytes. Must not split them.
        assert_eq!(truncate("你好世界你好世界", 5), "你好...");
    }
}
