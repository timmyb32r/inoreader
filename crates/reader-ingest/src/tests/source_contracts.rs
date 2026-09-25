use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
struct Index {
    source_count: usize,
    contracts: Vec<String>,
}

#[derive(Deserialize)]
struct Contract {
    source_id: String,
    provenance: String,
    configuration: Value,
    historical_expectations: Vec<Value>,
    limitations: Vec<String>,
}

/// This corpus is deliberately an evidence-preservation regression, not a
/// claim that historical observations are current live extraction results.
#[test]
fn every_known_source_has_an_explicit_provenance_contract() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let index: Index = serde_json::from_str(include_str!(
        "../../../../source-inventory/fixtures/contract-index.json"
    ))
    .unwrap();
    assert_eq!(index.source_count, 42);
    assert_eq!(index.contracts.len(), 42);
    for relative in index.contracts {
        let contract: Contract =
            serde_json::from_slice(&fs::read(root.join(relative)).unwrap()).unwrap();
        assert!(!contract.source_id.is_empty());
        assert!(!contract.provenance.is_empty());
        assert!(contract.configuration.is_object());
        assert!(contract
            .limitations
            .iter()
            .any(|v| v == "not_a_raw_http_response"));
        if contract.provenance == "configuration_only_no_observed_result" {
            assert!(contract.historical_expectations.is_empty());
        }
    }
}
