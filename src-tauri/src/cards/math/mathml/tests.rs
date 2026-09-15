use super::*;

#[test]
fn document_mathml_preserves_fractions_scripts_roots_and_display() {
    let parsed = parse(r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><mfrac><mrow><msup><mi>x</mi><mn>2</mn></msup><mo>+</mo><mn>1</mn></mrow><msqrt><mi>y</mi></msqrt></mfrac></math>"#).unwrap();
    assert_eq!(parsed.latex, r"\frac{{x}^{2} + 1}{\sqrt{y}}");
    assert_eq!(parsed.delimited(), r"\[\frac{{x}^{2} + 1}{\sqrt{y}}\]");
    assert_eq!(parse("<math><mroot><mi>x</mi><mn>3</mn></mroot></math>").unwrap().latex, r"\sqrt[{3}]{x}");
    assert_eq!(parse("<math><msubsup><mi>x</mi><mi>i</mi><mn>2</mn></msubsup></math>").unwrap().latex, r"{x}_{i}^{2}");
}

#[test]
fn document_mathml_preserves_matrix_cell_order_and_fences() {
    let parsed = parse("<math><mfenced open='[' close=']'><mtable><mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtr><mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable></mfenced></math>").unwrap();
    assert_eq!(parsed.latex, r"\left[ \begin{matrix}a & b \\ c & d\end{matrix} \right]");
    assert!(parse("<math><mtable><mtr><mtd><mi>a</mi></mtd></mtr><mtr><mtd><mi>b</mi></mtd><mtd><mi>c</mi></mtd></mtr></mtable></math>").is_err());
}

#[test]
fn document_mathml_uses_presentation_not_hidden_annotation_or_comments() {
    let parsed = parse("<math><semantics><mi>x<!--not a symbol--></mi><annotation encoding='application/x-tex'>\\input{private}</annotation></semantics></math>").unwrap();
    assert_eq!(parsed.latex, "x");
    assert_eq!(parse("<m:math xmlns:m='http://www.w3.org/1998/Math/MathML'><m:mi>x</m:mi></m:math>").unwrap().latex, "x");
    assert_eq!(parse("<math><mtext>rate &amp; dose</mtext></math>").unwrap().latex, r"\text{rate \& dose}");
    assert_eq!(parse("<math><mo>&#x2264;</mo></math>").unwrap().latex, "\\le ");
}

#[test]
fn document_mathml_rejects_active_unsupported_or_ambiguous_input() {
    for source in [
        "<!DOCTYPE math [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><math><mi>&x;</mi></math>",
        "<?run now?><math><mi>x</mi></math>",
        "<math onclick='x()'><mi>x</mi></math>",
        "<math><mi href='https://example.invalid'>x</mi></math>",
        "<math><mphantom><mi>x</mi></mphantom></math>",
        "<math><mfrac linethickness='0'><mi>x</mi><mi>y</mi></mfrac></math>",
        "<math><mfrac><mi>x</mi></mfrac></math>",
        "<math><mi xmlns='urn:other'>x</mi></math>",
        "<math><mi><script>x</script></mi></math>",
        "<math><semantics><mi>x</mi><annotation-xml><img src='x'/></annotation-xml></semantics></math>",
        "<math><mi>x</mi></math><math><mi>y</mi></math>",
        "<math><math><mi>x</mi></math></math>",
        "<math><mtd><mi>x</mi></mtd></math>",
        "<math>unstructured</math>",
        "<math><mi>&unknown;</mi></math>",
    ] { assert!(parse(source).is_err(), "{source}"); }
    let literal = parse(r"<math><mi>\input{x}</mi></math>").unwrap().latex;
    assert!(!literal.contains(r"\input"));
    assert!(literal.contains(r"\backslash{}"));
}

#[test]
fn document_mathml_bounds_xml_depth_nodes_and_output() {
    assert!(parse(&format!("<math>{}<mi>x</mi>{}</math>", "<mrow>".repeat(40), "</mrow>".repeat(40))).is_err());
    assert!(parse(&format!("<math>{}</math>", "<mi>x</mi>".repeat(2048))).is_err());
    assert!(parse(&format!("<math><mi>{}</mi></math>", "x".repeat(MAX_MATHML_BYTES))).is_err());
    assert!(parse(&format!("<math><mi>{}</mi></math>", "\\".repeat(MAX_OUTPUT))).is_err());
}

#[test]
fn document_mathml_prefix_respects_xml_boundaries() {
    let source = "<math><mi>x</mi><!-- </math> --><mtext><![CDATA[</math>]]></mtext></math> tail";
    let length = prefix_len(source).unwrap();
    assert_eq!(&source[length..], " tail");
    parse(&source[..length]).unwrap();
    assert!(!starts_mathml("<mathematics>"));
    assert!(!starts_mathml("&lt;math&gt;"));
    assert!(prefix_len("<math><mi>x</mi>").is_err());
    assert!(prefix_len("<math a='>bad'>x</math>").is_ok());
}
