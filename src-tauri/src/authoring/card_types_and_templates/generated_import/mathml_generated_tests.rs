use super::super::markdown_document;

#[test]
fn document_generated_mathml_normalizes_before_markdown_and_rejects_loss() {
    let document = markdown_document("**before <math><msub><mi>x</mi><mi>i</mi></msub></math> after**", "front").unwrap();
    crate::cards::validate_document(&document).unwrap();
    assert_eq!(document["attrs"]["schemaVersion"], 3);
    assert_eq!(document["content"][0]["content"][1]["attrs"]["latex"], r"{x}_{i}");
    assert!(markdown_document("<math><mfrac><mi>x</mi></mfrac></math>", "front").is_err());
    assert!(markdown_document("<script>alert(1)</script>", "front").is_err());
    let code = markdown_document("`<math><mi>x</mi></math>`", "front").unwrap();
    assert!(code.get("attrs").is_none());
    assert_eq!(code["content"][0]["content"][0]["text"], "<math><mi>x</mi></math>");
}
