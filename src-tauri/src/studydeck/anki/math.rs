//! Bounded Anki/MathJax math parsing into canonical LaTeX source.

use std::collections::BTreeMap;

const MAX_MATH_BYTES: usize = 256 * 1024;
const MAX_XML_NODES: usize = 4096;
const MAX_XML_DEPTH: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MathSpan {
    Text(String),
    Inline(String),
    Block(String),
}

pub(super) fn split_math_text(input: &str) -> Result<Vec<MathSpan>, String> {
    if input.len() > MAX_MATH_BYTES {
        return Err("Math-bearing text exceeds the supported size.".into());
    }
    let delimiters = [
        ("\\(", "\\)", false),
        ("\\[", "\\]", true),
        ("[$$]", "[/$$]", true),
        ("[$]", "[/$]", false),
        ("[latex]", "[/latex]", true),
    ];
    let mut spans = Vec::new();
    let mut remaining = input;
    while !remaining.is_empty() {
        let next = delimiters
            .iter()
            .filter_map(|delimiter| remaining.find(delimiter.0).map(|index| (index, *delimiter)))
            .min_by_key(|(index, _)| *index);
        let Some((index, (open, close, block))) = next else {
            push_text(&mut spans, remaining);
            break;
        };
        push_text(&mut spans, &remaining[..index]);
        let after_open = &remaining[index + open.len()..];
        let end = after_open
            .find(close)
            .ok_or_else(|| "An Anki math delimiter is not closed.".to_string())?;
        let latex = after_open[..end].trim();
        if latex.is_empty() {
            return Err("An Anki math expression is empty.".into());
        }
        spans.push(if block {
            MathSpan::Block(latex.to_string())
        } else {
            MathSpan::Inline(latex.to_string())
        });
        remaining = &after_open[end + close.len()..];
    }
    Ok(spans)
}

fn push_text(spans: &mut Vec<MathSpan>, text: &str) {
    if !text.is_empty() {
        spans.push(MathSpan::Text(text.to_string()));
    }
}

pub(super) fn mathml_to_latex(input: &str) -> Result<(String, bool), String> {
    if input.len() > MAX_MATH_BYTES {
        return Err("MathML exceeds the supported size.".into());
    }
    let root = XmlParser::new(input).parse()?;
    if local_name(&root.name) != "math" {
        return Err("MathML must have a math root element.".into());
    }
    let display = root
        .attrs
        .get("display")
        .is_some_and(|value| value.eq_ignore_ascii_case("block"));
    if let Some(annotation) = find_tex_annotation(&root) {
        let latex = annotation.trim();
        if latex.is_empty() {
            return Err("MathML contains an empty TeX annotation.".into());
        }
        return Ok((latex.to_string(), display));
    }
    let latex = render_node(&root)?.trim().to_string();
    if latex.is_empty() {
        return Err("MathML does not contain a supported expression.".into());
    }
    Ok((latex, display))
}

#[derive(Clone, Debug)]
struct XmlNode {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<XmlNode>,
    text: String,
}

struct XmlParser<'a> {
    input: &'a str,
    position: usize,
    nodes: usize,
}

impl<'a> XmlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            nodes: 0,
        }
    }

    fn parse(mut self) -> Result<XmlNode, String> {
        self.skip_ws();
        let node = self.element(0)?;
        self.skip_ws();
        if self.position != self.input.len() {
            return Err("Unexpected data after MathML expression.".into());
        }
        Ok(node)
    }

    fn element(&mut self, depth: usize) -> Result<XmlNode, String> {
        if depth > MAX_XML_DEPTH || self.nodes >= MAX_XML_NODES {
            return Err("MathML is nested too deeply or contains too many elements.".into());
        }
        self.nodes += 1;
        if !self.rest().starts_with('<') || self.rest().starts_with("</") {
            return Err("Malformed MathML element.".into());
        }
        if self.rest().starts_with("<!--")
            || self.rest().starts_with("<!")
            || self.rest().starts_with("<?")
        {
            return Err(
                "MathML declarations, comments, and external markup are unsupported.".into(),
            );
        }
        let end = find_tag_end(self.rest())?;
        let raw = self.rest()[1..end].trim();
        let self_closing = raw.ends_with('/');
        let raw = raw.trim_end_matches('/').trim();
        let name_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
        let name = raw[..name_end].to_ascii_lowercase();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-'))
        {
            return Err("Malformed MathML tag name.".into());
        }
        let attrs = parse_attrs(&raw[name_end..])?;
        self.position += end + 1;
        let mut node = XmlNode {
            name,
            attrs,
            children: Vec::new(),
            text: String::new(),
        };
        if self_closing {
            return Ok(node);
        }
        loop {
            if self.position >= self.input.len() {
                return Err("Unclosed MathML element.".into());
            }
            if self.rest().starts_with("</") {
                let close_end = find_tag_end(self.rest())?;
                let close = self.rest()[2..close_end].trim().to_ascii_lowercase();
                if local_name(&close) != local_name(&node.name) {
                    return Err("Mismatched MathML closing element.".into());
                }
                self.position += close_end + 1;
                return Ok(node);
            }
            if self.rest().starts_with('<') {
                node.children.push(self.element(depth + 1)?);
            } else {
                let length = self.rest().find('<').unwrap_or(self.rest().len());
                let decoded = decode_entities(&self.rest()[..length])?;
                node.text.push_str(&decoded);
                self.position += length;
            }
        }
    }

    fn rest(&self) -> &'a str {
        &self.input[self.position..]
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start();
        self.position += self.rest().len() - trimmed.len();
    }
}

fn find_tag_end(input: &str) -> Result<usize, String> {
    let mut quote = None;
    for (index, byte) in input.bytes().enumerate().skip(1) {
        if index > 8192 {
            return Err("MathML tag exceeds the supported size.".into());
        }
        match (quote, byte) {
            (Some(current), value) if current == value => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Ok(index),
            _ => {}
        }
    }
    Err("Unclosed MathML tag.".into())
}

fn parse_attrs(mut input: &str) -> Result<BTreeMap<String, String>, String> {
    let mut attrs = BTreeMap::new();
    while !input.trim().is_empty() {
        input = input.trim_start();
        let end = input
            .find(|character: char| character.is_whitespace() || character == '=')
            .unwrap_or(input.len());
        if end == 0 || attrs.len() >= 64 {
            return Err("Malformed or excessive MathML attributes.".into());
        }
        let name = local_name(&input[..end].to_ascii_lowercase()).to_string();
        input = input[end..].trim_start();
        let Some(rest) = input.strip_prefix('=') else {
            return Err("MathML attributes require quoted values.".into());
        };
        input = rest.trim_start();
        let quote = input
            .chars()
            .next()
            .filter(|value| matches!(value, '\'' | '"'))
            .ok_or("MathML attributes require quoted values.")?;
        let end = input[1..].find(quote).ok_or("Unclosed MathML attribute.")? + 1;
        let value = decode_entities(&input[1..end])?;
        input = &input[end + 1..];
        if attrs.insert(name, value).is_some() {
            return Err("Duplicate MathML attributes are ambiguous.".into());
        }
    }
    Ok(attrs)
}

fn find_tex_annotation(node: &XmlNode) -> Option<&str> {
    if local_name(&node.name) == "annotation"
        && node.attrs.get("encoding").is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "application/x-tex" | "application/x-latex" | "text/latex"
            )
        })
    {
        return Some(node.text.as_str());
    }
    node.children.iter().find_map(find_tex_annotation)
}

fn render_node(node: &XmlNode) -> Result<String, String> {
    let name = local_name(&node.name);
    let children =
        || -> Result<Vec<String>, String> { node.children.iter().map(render_node).collect() };
    match name {
        "math" | "mrow" | "mstyle" | "mpadded" | "semantics" => Ok(children()?.join("")),
        "annotation" | "annotation-xml" | "mphantom" => Ok(String::new()),
        "mi" | "mn" => render_identifier(node.text.trim()),
        "mo" => Ok(render_operator(node.text.trim())),
        "mtext" | "ms" => Ok(format!("\\text{{{}}}", escape_text(node.text.trim()))),
        "mspace" => Ok("\\,".into()),
        "mfrac" => {
            let values = exact_children(node, 2)?;
            Ok(format!("\\frac{{{}}}{{{}}}", values[0], values[1]))
        }
        "msqrt" => Ok(format!("\\sqrt{{{}}}", children()?.join(""))),
        "mroot" => {
            let values = exact_children(node, 2)?;
            Ok(format!("\\sqrt[{}]{{{}}}", values[1], values[0]))
        }
        "msup" => {
            let values = exact_children(node, 2)?;
            Ok(format!("{{{}}}^{{{}}}", values[0], values[1]))
        }
        "msub" => {
            let values = exact_children(node, 2)?;
            Ok(format!("{{{}}}_{{{}}}", values[0], values[1]))
        }
        "msubsup" => {
            let values = exact_children(node, 3)?;
            Ok(format!(
                "{{{}}}_{{{}}}^{{{}}}",
                values[0], values[1], values[2]
            ))
        }
        "mover" => {
            let values = exact_children(node, 2)?;
            Ok(format!("\\overset{{{}}}{{{}}}", values[1], values[0]))
        }
        "munder" => {
            let values = exact_children(node, 2)?;
            Ok(format!("\\underset{{{}}}{{{}}}", values[1], values[0]))
        }
        "munderover" => {
            let values = exact_children(node, 3)?;
            Ok(format!(
                "\\overset{{{}}}{{\\underset{{{}}}{{{}}}}}",
                values[2], values[1], values[0]
            ))
        }
        "mfenced" => {
            let open = node.attrs.get("open").map(String::as_str).unwrap_or("(");
            let close = node.attrs.get("close").map(String::as_str).unwrap_or(")");
            let separator = node
                .attrs
                .get("separators")
                .and_then(|value| value.chars().next())
                .unwrap_or(',');
            let values = children()?;
            Ok(format!(
                "\\left{}{}\\right{}",
                fence(open),
                values.join(&format!("{}", separator)),
                fence(close)
            ))
        }
        "mtable" => {
            let rows = node
                .children
                .iter()
                .map(render_matrix_row)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!(
                "\\begin{{matrix}}{}\\end{{matrix}}",
                rows.join("\\\\")
            ))
        }
        "mtr" | "mlabeledtr" => render_matrix_row(node),
        "mtd" => Ok(children()?.join("")),
        "menclose" => {
            let body = children()?.join("");
            if node
                .attrs
                .get("notation")
                .is_some_and(|value| value.split_whitespace().any(|item| item == "box"))
            {
                Ok(format!("\\boxed{{{body}}}"))
            } else {
                Ok(body)
            }
        }
        "none" => Ok(String::new()),
        _ => Err(format!("Unsupported MathML element: {name}.")),
    }
}

fn exact_children(node: &XmlNode, count: usize) -> Result<Vec<String>, String> {
    if node.children.len() != count {
        return Err(format!(
            "MathML {} requires exactly {count} child expressions.",
            local_name(&node.name)
        ));
    }
    node.children.iter().map(render_node).collect()
}

fn render_matrix_row(node: &XmlNode) -> Result<String, String> {
    if !matches!(local_name(&node.name), "mtr" | "mlabeledtr") {
        return Err("MathML matrix contains a non-row element.".into());
    }
    node.children
        .iter()
        .map(render_node)
        .collect::<Result<Vec<_>, _>>()
        .map(|cells| cells.join("&"))
}

fn render_identifier(text: &str) -> Result<String, String> {
    if text.is_empty() {
        return Err("MathML identifier is empty.".into());
    }
    if text.chars().count() == 1 {
        if let Some(mapped) = greek(text) {
            return Ok(mapped.to_string());
        }
        return Ok(text.to_string());
    }
    if text
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        return Ok(format!("\\mathrm{{{text}}}"));
    }
    Ok(format!("\\mathrm{{{}}}", escape_text(text)))
}

fn render_operator(text: &str) -> String {
    match text {
        "≤" => "\\le".into(),
        "≥" => "\\ge".into(),
        "≠" => "\\ne".into(),
        "≈" => "\\approx".into(),
        "±" => "\\pm".into(),
        "∓" => "\\mp".into(),
        "×" => "\\times".into(),
        "÷" => "\\div".into(),
        "·" | "⋅" => "\\cdot".into(),
        "∞" => "\\infty".into(),
        "∑" => "\\sum".into(),
        "∏" => "\\prod".into(),
        "∫" => "\\int".into(),
        "→" => "\\to".into(),
        "←" => "\\leftarrow".into(),
        "↔" => "\\leftrightarrow".into(),
        "∈" => "\\in".into(),
        "∉" => "\\notin".into(),
        "⊂" => "\\subset".into(),
        "⊆" => "\\subseteq".into(),
        "∪" => "\\cup".into(),
        "∩" => "\\cap".into(),
        "∂" => "\\partial".into(),
        "∇" => "\\nabla".into(),
        "∝" => "\\propto".into(),
        "≡" => "\\equiv".into(),
        "" => String::new(),
        other => other.to_string(),
    }
}

fn greek(text: &str) -> Option<&'static str> {
    Some(match text {
        "α" => "\\alpha",
        "β" => "\\beta",
        "γ" => "\\gamma",
        "δ" => "\\delta",
        "ε" => "\\epsilon",
        "ζ" => "\\zeta",
        "η" => "\\eta",
        "θ" => "\\theta",
        "ι" => "\\iota",
        "κ" => "\\kappa",
        "λ" => "\\lambda",
        "μ" => "\\mu",
        "ν" => "\\nu",
        "ξ" => "\\xi",
        "π" => "\\pi",
        "ρ" => "\\rho",
        "σ" => "\\sigma",
        "τ" => "\\tau",
        "φ" => "\\phi",
        "χ" => "\\chi",
        "ψ" => "\\psi",
        "ω" => "\\omega",
        "Γ" => "\\Gamma",
        "Δ" => "\\Delta",
        "Θ" => "\\Theta",
        "Λ" => "\\Lambda",
        "Ξ" => "\\Xi",
        "Π" => "\\Pi",
        "Σ" => "\\Sigma",
        "Φ" => "\\Phi",
        "Ψ" => "\\Psi",
        "Ω" => "\\Omega",
        _ => return None,
    })
}

fn fence(value: &str) -> String {
    match value {
        "{" => "\\{".into(),
        "}" => "\\}".into(),
        other => other.to_string(),
    }
}

fn escape_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\\' => "\\textbackslash{}".chars().collect::<Vec<_>>(),
            '{' => "\\{".chars().collect(),
            '}' => "\\}".chars().collect(),
            '%' => "\\%".chars().collect(),
            '$' => "\\$".chars().collect(),
            '#' => "\\#".chars().collect(),
            '&' => "\\&".chars().collect(),
            '_' => "\\_".chars().collect(),
            '^' => "\\^{}".chars().collect(),
            '~' => "\\~{}".chars().collect(),
            other => vec![other],
        })
        .collect()
}

fn local_name(value: &str) -> &str {
    value.rsplit(':').next().unwrap_or(value)
}

fn decode_entities(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut remaining = input;
    while let Some(start) = remaining.find('&') {
        output.push_str(&remaining[..start]);
        let entity = &remaining[start + 1..];
        let end = entity
            .find(';')
            .filter(|index| *index <= 32)
            .ok_or("Malformed MathML entity.")?;
        let name = &entity[..end];
        let character =
            if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(decimal) = name.strip_prefix('#') {
                decimal.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                match name {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                }
            };
        let character = character.ok_or("Unsupported MathML entity.")?;
        if character.is_control() && !matches!(character, '\n' | '\t') {
            return Err("MathML contains an invalid control character.".into());
        }
        output.push(character);
        remaining = &entity[end + 1..];
    }
    output.push_str(remaining);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_mathjax_and_legacy_anki_math_without_treating_currency_as_math() {
        let spans = split_math_text("Dose \\(x^2\\), cost $5, then \\[\\frac{a}{b}\\]").unwrap();
        assert_eq!(
            spans,
            vec![
                MathSpan::Text("Dose ".into()),
                MathSpan::Inline("x^2".into()),
                MathSpan::Text(", cost $5, then ".into()),
                MathSpan::Block("\\frac{a}{b}".into()),
            ]
        );
        assert!(split_math_text("\\(unclosed").is_err());
    }

    #[test]
    fn converts_common_presentation_mathml_and_prefers_tex_annotation() {
        let (latex, block) = mathml_to_latex(
            r#"<math display="block"><mfrac><mi>α</mi><msup><mi>x</mi><mn>2</mn></msup></mfrac></math>"#,
        )
        .unwrap();
        assert!(block);
        assert_eq!(latex, "\\frac{\\alpha}{{x}^{2}}");

        let (latex, _) = mathml_to_latex(
            r#"<math><semantics><mrow><mi>x</mi></mrow><annotation encoding="application/x-tex">\\sqrt{x}</annotation></semantics></math>"#,
        )
        .unwrap();
        assert_eq!(latex, "\\sqrt{x}");
    }

    #[test]
    fn rejects_unknown_or_executable_xml_constructs() {
        assert!(mathml_to_latex("<math><maction><mi>x</mi></maction></math>").is_err());
        assert!(mathml_to_latex("<?xml version='1.0'?><math><mi>x</mi></math>").is_err());
    }
}
