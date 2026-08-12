use serde_json::json;

/// Test truncate helper function
#[test]
fn test_truncate() {
    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            format!("{}...", &s[..max - 3])
        }
    }

    assert_eq!(truncate("hello", 10), "hello");
    assert_eq!(truncate("hello world this is long", 10), "hello w...");
    assert_eq!(truncate("abc", 3), "abc");
    assert_eq!(truncate("abcd", 3), "...");
}

/// Test JSON pretty printing
#[test]
fn test_json_pretty_print() {
    let value = json!({"key": "value", "number": 42});
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    assert!(pretty.contains("\"key\""));
    assert!(pretty.contains("\"value\""));
    assert!(pretty.contains("42"));
}

/// Test extracting data from API response with "data" wrapper
#[test]
fn test_api_response_data_extraction() {
    let response = json!({
        "data": [
            {"id": "item-1", "type": "patent"},
            {"id": "item-2", "type": "academic"}
        ]
    });

    let data = response.get("data").unwrap();
    let arr = data.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], "item-1");
}

/// Test extracting results from patent search response
#[test]
fn test_patent_search_response_extraction() {
    let response = json!({
        "results": [
            {
                "publicationNumber": "EP1234567",
                "title": "Test Patent",
                "applicant": "Test Corp",
                "publicationDate": "2024-01-01"
            }
        ],
        "total": 1
    });

    let results = response.get("results").unwrap();
    let arr = results.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["publicationNumber"], "EP1234567");
    assert_eq!(arr[0]["title"], "Test Patent");
}

/// Test academic search response parsing
#[test]
fn test_academic_search_response() {
    let response = json!({
        "results": [
            {
                "title": "Machine Learning in Patent Analysis",
                "authors": "Smith, J.; Doe, A.",
                "year": 2024,
                "source": "Nature AI"
            }
        ]
    });

    let results = response.get("results").unwrap().as_array().unwrap();
    assert_eq!(results[0]["title"], "Machine Learning in Patent Analysis");
    assert_eq!(results[0]["year"], 2024);
}
