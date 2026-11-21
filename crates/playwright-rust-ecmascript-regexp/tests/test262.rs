use base64::Engine as _;
use flate2::read::GzDecoder;
use playwright_rust_ecmascript_regexp::EcmaScriptRegex;
use serde::Deserialize;
use std::io::Read as _;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Corpus {
    test262_commit: String,
    node_version: String,
    normalized_unpaired_source_count: usize,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    file: String,
    source: String,
    flags: String,
    expected_compile: bool,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    input_utf16le_base64: String,
    matched: bool,
    last_index: usize,
}

#[test]
fn source_pinned_test262_regexp_test_corpus_matches_node() {
    // Arrange
    let mut json = String::new();
    GzDecoder::new(&include_bytes!("../testdata/test262-regexp-test-corpus.json.gz")[..])
        .read_to_string(&mut json)
        .expect("the checked-in Test262 corpus should be valid gzip UTF-8");
    let corpus: Corpus =
        serde_json::from_str(&json).expect("the checked-in Test262 corpus should be valid JSON");
    assert_eq!(
        corpus.test262_commit,
        "3c6180f581e5f468d6767af2f9b1f9c3ca444e77"
    );
    assert_eq!(corpus.node_version, "v24.12.0");
    assert_eq!(corpus.normalized_unpaired_source_count, 2);
    assert!(!corpus.cases.is_empty());
    let mut failures = Vec::new();

    for case in corpus.cases {
        let compiled = EcmaScriptRegex::compile(case.source.clone(), case.flags.clone());

        // Act
        if compiled.is_ok() != case.expected_compile {
            failures.push(format!(
                "compile {} /{}/{}: Node={} Rust={compiled:?}",
                case.file, case.source, case.flags, case.expected_compile
            ));
        }
        let Ok(pattern) = compiled else {
            continue;
        };
        if !case.expected_compile {
            continue;
        }
        for (step_index, step) in case.steps.into_iter().enumerate() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&step.input_utf16le_base64)
                .expect("the generated UTF-16LE input should be valid base64");
            let (pairs, remainder) = bytes.as_chunks::<2>();
            assert!(remainder.is_empty());
            let input = pairs
                .iter()
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            let matched = pattern.test_utf16(&input);
            if matched != step.matched {
                failures.push(format!(
                    "match {} /{}/{} step {step_index} UTF-16 {input:?}: Node={} Rust={matched}",
                    case.file, case.source, case.flags, step.matched
                ));
            }
            let last_index = pattern.last_index();
            if last_index != step.last_index {
                failures.push(format!(
                    "lastIndex {} /{}/{} step {step_index} UTF-16 {input:?}: Node={} Rust={last_index}",
                    case.file, case.source, case.flags, step.last_index
                ));
            }
        }
    }

    // Assert
    assert!(
        failures.is_empty(),
        "{} Test262/Node discrepancies:\n{}",
        failures.len(),
        failures
            .iter()
            .take(200)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
