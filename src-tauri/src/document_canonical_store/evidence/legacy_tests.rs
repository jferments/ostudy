use super::*;
use serde_json::json;

#[test]
fn retained_pdf_metadata_keeps_original_json_types_without_changing_evidence() {
    let block = CanonicalBlock::new(
        0,
        0,
        1,
        CanonicalBlockRole::Paragraph,
        "Only while moving: v <= 2 m/s.",
        None,
        Vec::new(),
        None,
        None,
        Vec::new(),
        BTreeMap::new(),
    );
    let original = json!({"fontSize":12.0,"bold":true,"family":null,
        "_ostudyNode":"untrusted","nested":{"value":[1,"2"]}});
    let attributes = serde_json::from_value(original.clone()).unwrap();
    let (hash, payload, location) = encode(&block, Some(&attributes)).unwrap();
    let restored: Location = serde_json::from_str(&location).unwrap();
    assert_eq!(serde_json::to_value(restored.attributes).unwrap(), original);
    assert!(restored.source_node.is_none());
    assert_eq!(serde_json::to_value(&attributes).unwrap(), original);
    let (ordinary_hash, ordinary_payload, _) = encode(&block, None).unwrap();
    assert_eq!(hash, ordinary_hash);
    assert_eq!(payload, ordinary_payload);
    assert_eq!(
        serde_json::from_str::<Payload>(&payload)
            .unwrap()
            .text()
            .unwrap(),
        block.text
    );
}
