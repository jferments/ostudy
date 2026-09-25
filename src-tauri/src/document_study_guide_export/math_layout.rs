//! Bounded native mathematical layout for study-guide PDF export.
//!
//! Stored LaTeX is data. This parser recognizes a deliberately small
//! presentation subset. It never expands user macros, reads files, executes
//! TeX, follows URLs, or loads packages. Unsupported notation fails closed so
//! the PDF exporter can preserve the original source visibly.

use crate::cards::math::validate_latex;

const MAX_NODES: usize = 768;
const MAX_DEPTH: usize = 24;
const SCRIPT_SCALE: f64 = 0.72;
const FRACTION_SCALE: f64 = 0.86;

#[derive(Clone, Debug)]
pub(super) struct MathLayout {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub ops: Vec<DrawOp>,
}

#[derive(Clone, Debug)]
pub(super) enum DrawOp {
    Text {
        x: f64,
        y: f64,
        text: String,
        size: f64,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        width: f64,
    },
}

#[derive(Clone, Debug)]
enum Node {
    Row(Vec<Node>),
    Text(String),
    Space(f64),
    Fraction(Box<Node>, Box<Node>),
    Sqrt(Box<Node>),
    Scripts {
        base: Box<Node>,
        superscript: Option<Box<Node>>,
        subscript: Option<Box<Node>>,
    },
    Matrix {
        rows: Vec<Vec<Node>>,
        kind: MatrixKind,
    },
}

#[derive(Clone, Copy, Debug)]
enum MatrixKind {
    Plain,
    Parentheses,
    Brackets,
}

pub(super) fn layout(source: &str, size: f64) -> Result<MathLayout, ()> {
    validate_latex(source).map_err(|_| ())?;
    if !(6.0..=72.0).contains(&size) {
        return Err(());
    }
    let mut parser = Parser::new(source);
    let node = parser.parse()?;
    layout_node(&node, size, 0)
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
    nodes: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            nodes: 0,
        }
    }

    fn parse(&mut self) -> Result<Node, ()> {
        let node = self.parse_row(None, 0)?;
        self.skip_whitespace();
        if self.position != self.source.len() {
            return Err(());
        }
        Ok(node)
    }

    fn bump_node(&mut self) -> Result<(), ()> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_NODES {
            Err(())
        } else {
            Ok(())
        }
    }

    fn parse_row(&mut self, stop: Option<char>, depth: usize) -> Result<Node, ()> {
        if depth > MAX_DEPTH {
            return Err(());
        }
        let mut nodes = Vec::new();
        while let Some(character) = self.peek() {
            if Some(character) == stop {
                break;
            }
            if character == '}' {
                return Err(());
            }
            if character.is_whitespace() {
                self.skip_whitespace();
                if !nodes.is_empty() {
                    nodes.push(Node::Space(0.24));
                }
                continue;
            }
            let mut atom = self.parse_atom(depth + 1)?;
            let mut superscript = None;
            let mut subscript = None;
            loop {
                match self.peek() {
                    Some('^') if superscript.is_none() => {
                        self.next();
                        superscript = Some(Box::new(self.parse_script(depth + 1)?));
                    }
                    Some('_') if subscript.is_none() => {
                        self.next();
                        subscript = Some(Box::new(self.parse_script(depth + 1)?));
                    }
                    Some('^') | Some('_') => return Err(()),
                    _ => break,
                }
            }
            if superscript.is_some() || subscript.is_some() {
                self.bump_node()?;
                atom = Node::Scripts {
                    base: Box::new(atom),
                    superscript,
                    subscript,
                };
            }
            nodes.push(atom);
        }
        if nodes.is_empty() {
            return Err(());
        }
        self.bump_node()?;
        Ok(Node::Row(nodes))
    }

    fn parse_script(&mut self, depth: usize) -> Result<Node, ()> {
        self.skip_whitespace();
        if self.peek() == Some('{') {
            self.next();
            let node = self.parse_row(Some('}'), depth + 1)?;
            self.expect('}')?;
            Ok(node)
        } else {
            self.parse_atom(depth + 1)
        }
    }

    fn parse_atom(&mut self, depth: usize) -> Result<Node, ()> {
        if depth > MAX_DEPTH {
            return Err(());
        }
        self.bump_node()?;
        match self.peek().ok_or(())? {
            '{' => {
                self.next();
                let node = self.parse_row(Some('}'), depth + 1)?;
                self.expect('}')?;
                Ok(node)
            }
            '\\' => {
                self.next();
                self.parse_command(depth + 1)
            }
            '^' | '_' | '}' => Err(()),
            _ => {
                let start = self.position;
                while let Some(character) = self.peek() {
                    if character.is_whitespace()
                        || matches!(character, '\\' | '{' | '}' | '^' | '_')
                    {
                        break;
                    }
                    self.next();
                }
                if start == self.position {
                    return Err(());
                }
                Ok(Node::Text(self.source[start..self.position].to_string()))
            }
        }
    }

    fn parse_command(&mut self, depth: usize) -> Result<Node, ()> {
        let command = self.command_name()?;
        match command.as_str() {
            "frac" => {
                let numerator = self.required_group(depth + 1)?;
                let denominator = self.required_group(depth + 1)?;
                Ok(Node::Fraction(Box::new(numerator), Box::new(denominator)))
            }
            "sqrt" => {
                self.skip_whitespace();
                if self.peek() == Some('[') {
                    return Err(());
                }
                Ok(Node::Sqrt(Box::new(self.required_group(depth + 1)?)))
            }
            "text" | "operatorname" => Ok(Node::Text(self.raw_group()?)),
            "mathrm" | "mathbf" | "mathit" | "mathsf" | "mathtt" | "mathcal" | "mathbb"
            | "mathfrak" => self.required_group(depth + 1),
            "left" | "right" => Ok(Node::Text(self.delimiter()?)),
            "begin" => self.matrix(depth + 1),
            "displaystyle" | "textstyle" => Ok(Node::Space(0.0)),
            "," => Ok(Node::Space(0.18)),
            ";" => Ok(Node::Space(0.36)),
            ":" => Ok(Node::Space(0.28)),
            "!" => Ok(Node::Space(-0.08)),
            "quad" => Ok(Node::Space(1.0)),
            "qquad" => Ok(Node::Space(2.0)),
            "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "lim" | "max" | "min" => {
                Ok(Node::Text(command))
            }
            _ => symbol(&command)
                .map(|value| Node::Text(value.to_string()))
                .ok_or(()),
        }
    }

    fn matrix(&mut self, depth: usize) -> Result<Node, ()> {
        let environment = self.raw_group()?;
        let kind = match environment.as_str() {
            "matrix" => MatrixKind::Plain,
            "pmatrix" => MatrixKind::Parentheses,
            "bmatrix" => MatrixKind::Brackets,
            _ => return Err(()),
        };
        let closing = format!("\\end{{{environment}}}");
        let rest = &self.source[self.position..];
        let offset = rest.find(&closing).ok_or(())?;
        let body = &rest[..offset];
        self.position += offset + closing.len();
        let raw_rows = split_matrix(body)?;
        if raw_rows.is_empty() || raw_rows.len() > 16 {
            return Err(());
        }
        let width = raw_rows[0].len();
        if width == 0 || width > 12 || raw_rows.iter().any(|row| row.len() != width) {
            return Err(());
        }
        let mut rows = Vec::with_capacity(raw_rows.len());
        for row in raw_rows {
            let mut parsed = Vec::with_capacity(row.len());
            for cell in row {
                let mut parser = Parser::new(cell.trim());
                let node = parser.parse()?;
                self.nodes = self.nodes.saturating_add(parser.nodes);
                if self.nodes > MAX_NODES {
                    return Err(());
                }
                parsed.push(node);
            }
            rows.push(parsed);
        }
        if depth > MAX_DEPTH {
            return Err(());
        }
        Ok(Node::Matrix { rows, kind })
    }

    fn required_group(&mut self, depth: usize) -> Result<Node, ()> {
        self.skip_whitespace();
        self.expect('{')?;
        let node = self.parse_row(Some('}'), depth + 1)?;
        self.expect('}')?;
        Ok(node)
    }

    fn raw_group(&mut self) -> Result<String, ()> {
        self.skip_whitespace();
        self.expect('{')?;
        let start = self.position;
        let mut nesting = 1_usize;
        while let Some(character) = self.next() {
            match character {
                '{' => nesting = nesting.saturating_add(1),
                '}' => {
                    nesting = nesting.saturating_sub(1);
                    if nesting == 0 {
                        return Ok(self.source[start..self.position - 1].to_string());
                    }
                }
                _ => {}
            }
            if nesting > MAX_DEPTH {
                return Err(());
            }
        }
        Err(())
    }

    fn delimiter(&mut self) -> Result<String, ()> {
        self.skip_whitespace();
        if self.peek() == Some('\\') {
            self.next();
            let name = self.command_name()?;
            return match name.as_str() {
                "{" => Ok("{".to_string()),
                "}" => Ok("}".to_string()),
                "langle" => Ok("⟨".to_string()),
                "rangle" => Ok("⟩".to_string()),
                "vert" => Ok("|".to_string()),
                "Vert" => Ok("‖".to_string()),
                _ => Err(()),
            };
        }
        self.next().map(|value| value.to_string()).ok_or(())
    }

    fn command_name(&mut self) -> Result<String, ()> {
        let first = self.peek().ok_or(())?;
        if first.is_alphabetic() {
            let start = self.position;
            while self.peek().is_some_and(char::is_alphabetic) {
                self.next();
            }
            Ok(self.source[start..self.position].to_string())
        } else {
            self.next().map(|value| value.to_string()).ok_or(())
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.next();
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), ()> {
        if self.next() == Some(expected) {
            Ok(())
        } else {
            Err(())
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn next(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += character.len_utf8();
        Some(character)
    }
}

fn symbol(command: &str) -> Option<&'static str> {
    Some(match command {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" | "vartheta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" | "varpi" => "π",
        "rho" | "varrho" => "ρ",
        "sigma" | "varsigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" | "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "times" => "×",
        "cdot" => "·",
        "div" => "÷",
        "pm" => "±",
        "mp" => "∓",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "infty" => "∞",
        "sum" => "∑",
        "prod" => "∏",
        "int" => "∫",
        "partial" => "∂",
        "nabla" => "∇",
        "in" => "∈",
        "notin" => "∉",
        "subset" => "⊂",
        "supset" => "⊃",
        "subseteq" => "⊆",
        "supseteq" => "⊇",
        "cup" => "∪",
        "cap" => "∩",
        "rightarrow" | "to" => "→",
        "leftarrow" => "←",
        "leftrightarrow" => "↔",
        "Rightarrow" => "⇒",
        "Leftarrow" => "⇐",
        "Leftrightarrow" => "⇔",
        "degree" => "°",
        "%" => "%",
        "#" => "#",
        "$" => "$",
        "&" => "&",
        "_" => "_",
        "{" => "{",
        "}" => "}",
        "|" => "|",
        "\\" => "\\",
        _ => return None,
    })
}

fn split_matrix(body: &str) -> Result<Vec<Vec<&str>>, ()> {
    let mut rows = vec![Vec::new()];
    let mut start = 0_usize;
    let mut nesting = 0_usize;
    let bytes = body.as_bytes();
    let mut index = 0_usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => nesting = nesting.saturating_add(1),
            b'}' => nesting = nesting.checked_sub(1).ok_or(())?,
            b'&' if nesting == 0 => {
                rows.last_mut().ok_or(())?.push(&body[start..index]);
                start = index + 1;
            }
            b'\\' if nesting == 0 && bytes.get(index + 1) == Some(&b'\\') => {
                rows.last_mut().ok_or(())?.push(&body[start..index]);
                rows.push(Vec::new());
                index += 1;
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if nesting != 0 {
        return Err(());
    }
    rows.last_mut().ok_or(())?.push(&body[start..]);
    if rows
        .last()
        .is_some_and(|row| row.len() == 1 && row[0].trim().is_empty())
    {
        rows.pop();
    }
    Ok(rows)
}

fn layout_node(node: &Node, size: f64, depth: usize) -> Result<MathLayout, ()> {
    if depth > MAX_DEPTH {
        return Err(());
    }
    match node {
        Node::Text(text) => Ok(text_layout(text, size)),
        Node::Space(em) => Ok(MathLayout {
            width: size * *em,
            height: 0.0,
            depth: 0.0,
            ops: Vec::new(),
        }),
        Node::Row(nodes) => {
            let mut result = MathLayout {
                width: 0.0,
                height: 0.0,
                depth: 0.0,
                ops: Vec::new(),
            };
            for child in nodes {
                let child = layout_node(child, size, depth + 1)?;
                append_layout(&mut result.ops, &child, result.width, 0.0);
                result.width += child.width;
                result.height = result.height.max(child.height);
                result.depth = result.depth.max(child.depth);
            }
            Ok(result)
        }
        Node::Fraction(numerator, denominator) => {
            let numerator = layout_node(numerator, size * FRACTION_SCALE, depth + 1)?;
            let denominator = layout_node(denominator, size * FRACTION_SCALE, depth + 1)?;
            let padding = size * 0.28;
            let width = numerator.width.max(denominator.width) + (2.0 * padding);
            let gap = size * 0.18;
            let numerator_y = gap + numerator.depth + 1.0;
            let denominator_y = -(gap + denominator.height + 1.0);
            let mut ops = Vec::new();
            append_layout(
                &mut ops,
                &numerator,
                (width - numerator.width) / 2.0,
                numerator_y,
            );
            append_layout(
                &mut ops,
                &denominator,
                (width - denominator.width) / 2.0,
                denominator_y,
            );
            ops.push(DrawOp::Line {
                x1: padding * 0.35,
                y1: 0.0,
                x2: width - (padding * 0.35),
                y2: 0.0,
                width: (size * 0.055).max(0.55),
            });
            Ok(MathLayout {
                width,
                height: numerator_y + numerator.height,
                depth: -denominator_y + denominator.depth,
                ops,
            })
        }
        Node::Sqrt(value) => {
            let value = layout_node(value, size, depth + 1)?;
            let radical = text_layout("√", size * 1.08);
            let gap = size * 0.08;
            let x = radical.width + gap;
            let mut ops = radical.ops;
            append_layout(&mut ops, &value, x, 0.0);
            ops.push(DrawOp::Line {
                x1: x,
                y1: value.height + 1.2,
                x2: x + value.width,
                y2: value.height + 1.2,
                width: (size * 0.045).max(0.5),
            });
            Ok(MathLayout {
                width: x + value.width,
                height: radical.height.max(value.height + 2.0),
                depth: radical.depth.max(value.depth),
                ops,
            })
        }
        Node::Scripts {
            base,
            superscript,
            subscript,
        } => {
            let base = layout_node(base, size, depth + 1)?;
            let superscript = superscript
                .as_deref()
                .map(|value| layout_node(value, size * SCRIPT_SCALE, depth + 1))
                .transpose()?;
            let subscript = subscript
                .as_deref()
                .map(|value| layout_node(value, size * SCRIPT_SCALE, depth + 1))
                .transpose()?;
            let script_width = superscript
                .as_ref()
                .map_or(0.0, |value| value.width)
                .max(subscript.as_ref().map_or(0.0, |value| value.width));
            let mut ops = base.ops;
            let mut height = base.height;
            let mut below = base.depth;
            if let Some(superscript) = superscript {
                let y = (base.height * 0.68).max(size * 0.45) + superscript.depth;
                append_layout(&mut ops, &superscript, base.width + (size * 0.04), y);
                height = height.max(y + superscript.height);
            }
            if let Some(subscript) = subscript {
                let y = -((base.depth + (size * 0.30)).max(size * 0.32) + subscript.height);
                append_layout(&mut ops, &subscript, base.width + (size * 0.04), y);
                below = below.max(-y + subscript.depth);
            }
            Ok(MathLayout {
                width: base.width + (size * 0.04) + script_width,
                height,
                depth: below,
                ops,
            })
        }
        Node::Matrix { rows, kind } => layout_matrix(rows, *kind, size, depth + 1),
    }
}

fn layout_matrix(
    rows: &[Vec<Node>],
    kind: MatrixKind,
    size: f64,
    depth: usize,
) -> Result<MathLayout, ()> {
    let cell_size = size * 0.82;
    let columns = rows.first().map_or(0, Vec::len);
    let mut cells = Vec::with_capacity(rows.len());
    let mut column_widths = vec![0.0_f64; columns];
    let mut row_heights = vec![0.0_f64; rows.len()];
    let mut row_depths = vec![0.0_f64; rows.len()];
    for (row_index, row) in rows.iter().enumerate() {
        let mut layouts = Vec::with_capacity(row.len());
        for (column_index, cell) in row.iter().enumerate() {
            let layout = layout_node(cell, cell_size, depth + 1)?;
            column_widths[column_index] = column_widths[column_index].max(layout.width);
            row_heights[row_index] = row_heights[row_index].max(layout.height);
            row_depths[row_index] = row_depths[row_index].max(layout.depth);
            layouts.push(layout);
        }
        cells.push(layouts);
    }
    let column_gap = size * 0.7;
    let row_gap = size * 0.35;
    let content_width =
        column_widths.iter().sum::<f64>() + column_gap * columns.saturating_sub(1) as f64;
    let content_height = row_heights
        .iter()
        .zip(&row_depths)
        .map(|(height, below)| height + below)
        .sum::<f64>()
        + row_gap * rows.len().saturating_sub(1) as f64;
    let delimiter_width = match kind {
        MatrixKind::Plain => 0.0,
        MatrixKind::Parentheses | MatrixKind::Brackets => size * 0.55,
    };
    let mut ops = Vec::new();
    let mut top = content_height / 2.0;
    for (row_index, row) in cells.iter().enumerate() {
        let baseline = top - row_heights[row_index];
        let mut x = delimiter_width;
        for (column_index, cell) in row.iter().enumerate() {
            append_layout(
                &mut ops,
                cell,
                x + ((column_widths[column_index] - cell.width) / 2.0),
                baseline,
            );
            x += column_widths[column_index] + column_gap;
        }
        top -= row_heights[row_index] + row_depths[row_index] + row_gap;
    }
    let half = (content_height / 2.0) + (size * 0.18);
    match kind {
        MatrixKind::Plain => {}
        MatrixKind::Brackets => {
            let right = content_width + (2.0 * delimiter_width);
            for x in [0.0, right] {
                ops.push(DrawOp::Line {
                    x1: x,
                    y1: -half,
                    x2: x,
                    y2: half,
                    width: (size * 0.05).max(0.55),
                });
            }
            for y in [-half, half] {
                ops.push(DrawOp::Line {
                    x1: 0.0,
                    y1: y,
                    x2: delimiter_width * 0.65,
                    y2: y,
                    width: (size * 0.05).max(0.55),
                });
                ops.push(DrawOp::Line {
                    x1: right - (delimiter_width * 0.65),
                    y1: y,
                    x2: right,
                    y2: y,
                    width: (size * 0.05).max(0.55),
                });
            }
        }
        MatrixKind::Parentheses => {
            let left = text_layout("(", size * 1.65);
            let right = text_layout(")", size * 1.65);
            append_layout(&mut ops, &left, 0.0, 0.0);
            append_layout(&mut ops, &right, delimiter_width + content_width, 0.0);
        }
    }
    Ok(MathLayout {
        width: content_width + (2.0 * delimiter_width),
        height: half,
        depth: half,
        ops,
    })
}

fn text_layout(text: &str, size: f64) -> MathLayout {
    MathLayout {
        width: text_width(text, size),
        height: size * 0.78,
        depth: size * 0.22,
        ops: vec![DrawOp::Text {
            x: 0.0,
            y: 0.0,
            text: text.to_string(),
            size,
        }],
    }
}

fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|character| {
            if character.is_whitespace() {
                size * 0.32
            } else if character.is_ascii_punctuation() {
                size * 0.42
            } else {
                size * 0.56
            }
        })
        .sum()
}

fn append_layout(target: &mut Vec<DrawOp>, layout: &MathLayout, dx: f64, dy: f64) {
    for operation in &layout.ops {
        target.push(match operation {
            DrawOp::Text { x, y, text, size } => DrawOp::Text {
                x: x + dx,
                y: y + dy,
                text: text.clone(),
                size: *size,
            },
            DrawOp::Line {
                x1,
                y1,
                x2,
                y2,
                width,
            } => DrawOp::Line {
                x1: x1 + dx,
                y1: y1 + dy,
                x2: x2 + dx,
                y2: y2 + dy,
                width: *width,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_roots_and_scripts_produce_two_dimensional_layout() {
        let fraction = layout(r"\frac{x_1}{\sqrt{y^2}}", 12.0).unwrap();
        assert!(fraction
            .ops
            .iter()
            .any(|operation| matches!(operation, DrawOp::Line { .. })));
        let debug = format!("{:?}", fraction.ops);
        for expected in ["x", "1", "√", "y", "2"] {
            assert!(debug.contains(expected));
        }
        assert!(fraction.height > 10.0);
        assert!(fraction.depth > 3.0);
    }

    #[test]
    fn matrices_are_bounded_and_laid_out_by_rows_and_columns() {
        let matrix = layout(r"\begin{bmatrix}a&b\\c&d\end{bmatrix}", 12.0).unwrap();
        let debug = format!("{:?}", matrix.ops);
        for expected in ["a", "b", "c", "d"] {
            assert!(debug.contains(expected));
        }
        assert!(
            matrix
                .ops
                .iter()
                .filter(|operation| matches!(operation, DrawOp::Line { .. }))
                .count()
                >= 4
        );
    }

    #[test]
    fn common_symbols_and_functions_are_native_text_not_source_commands() {
        let value = layout(r"\alpha + \beta \le \infty \quad \sin(x)", 12.0).unwrap();
        let debug = format!("{:?}", value.ops);
        assert!(debug.contains("α"));
        assert!(debug.contains("β"));
        assert!(debug.contains("≤"));
        assert!(debug.contains("∞"));
        assert!(debug.contains("sin"));
    }

    #[test]
    fn unsupported_or_active_tex_fails_closed() {
        for source in [
            r"\includegraphics{file}",
            r"\input{file}",
            r"\href{https://example.com}{x}",
            r"\newcommand{\x}{y}",
            r"\sqrt[3]{x}",
            r"\begin{align}x&=y\end{align}",
        ] {
            assert!(layout(source, 12.0).is_err(), "{source}");
        }
    }
}
