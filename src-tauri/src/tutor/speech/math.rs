//! A reply's LaTeX math as a teacher says it.
//!
//! A voice handed `\frac{[\text{HCO}_3^-]}{0.03 \times P_{CO_2}}` reads
//! commands and braces as letters, stumbles or skips the passage. Each math
//! span is said in words instead: a fraction is "over", a square is
//! "squared", and a charged formula keeps its flat spelling (HCO3-) so the
//! lesson chemistry rules can name it.

/// `line` with each `$…$`, `$$…$$`, `\(…\)` and `\[…\]` span said in words.
/// An unclosed span, an escaped `\$` and a price such as `$5` stay as written.
pub(super) fn with_spoken_math(line: &str) -> String {
    let characters: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < characters.len() {
        if let Some((math, after)) = math_span(&characters, index) {
            out.push(' ');
            out.push_str(&say(&math));
            out.push(' ');
            index = after;
            continue;
        }
        if characters[index] == '\\' && characters.get(index + 1) == Some(&'$') {
            out.push('$');
            index += 2;
            continue;
        }
        out.push(characters[index]);
        index += 1;
    }
    out
}

/// The math opening at `start`, if it closes on this line, and the index
/// after its closing mark.
fn math_span(characters: &[char], start: usize) -> Option<(String, usize)> {
    let rest = &characters[start..];
    let pair = |open: &str, close: &str| -> Option<(String, usize)> {
        let open: Vec<char> = open.chars().collect();
        let close: Vec<char> = close.chars().collect();
        if !rest.starts_with(&open) {
            return None;
        }
        let body = &rest[open.len()..];
        let end = (0..body.len()).find(|&at| body[at..].starts_with(&close))?;
        Some((
            body[..end].iter().collect(),
            start + open.len() + end + close.len(),
        ))
    };
    if let Some(found) = pair("$$", "$$")
        .or_else(|| pair("\\[", "\\]"))
        .or_else(|| pair("\\(", "\\)"))
    {
        return Some(found);
    }
    // Inline `$x$`: the opening mark is followed by a character and the
    // closing one follows a character and comes before no digit, as in TeX.
    if rest.first() != Some(&'$') || rest.get(1).is_none_or(|next| next.is_whitespace()) {
        return None;
    }
    let end = (1..rest.len()).find(|&at| {
        rest[at] == '$'
            && rest[at - 1] != '\\'
            && !rest[at - 1].is_whitespace()
            && !rest.get(at + 1).is_some_and(char::is_ascii_digit)
    })?;
    Some((rest[1..end].iter().collect(), start + end + 1))
}

/// LaTeX math in words.
pub(super) fn say(latex: &str) -> String {
    let characters: Vec<char> = latex.chars().collect();
    let mut parser = Parser {
        characters,
        index: 0,
    };
    let words = parser.until(None);
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Parser {
    characters: Vec<char>,
    index: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.characters.get(self.index).copied()
    }

    /// Words up to `close` (consumed) or the end.
    fn until(&mut self, close: Option<char>) -> String {
        let mut out = String::new();
        while let Some(character) = self.peek() {
            if Some(character) == close {
                self.index += 1;
                break;
            }
            self.index += 1;
            match character {
                '\\' => out.push_str(&self.command()),
                '{' => out.push_str(&self.until(Some('}'))),
                '}' => {}
                '^' => {
                    let power = self.argument();
                    out.push_str(&power_words(&power));
                }
                // Subscripts are said as part of their symbol: CO_2 is CO2.
                '_' => out.push_str(self.argument().trim()),
                '=' => out.push_str(" equals "),
                '+' => out.push_str(" plus "),
                '-' | '−' => out.push_str(" minus "),
                '<' => out.push_str(" is less than "),
                '>' => out.push_str(" is greater than "),
                '/' => out.push_str(" over "),
                '%' => out.push_str(" percent "),
                '&' | '~' | '(' | ')' | '[' | ']' | '|' => out.push(' '),
                other => out.push(other),
            }
        }
        out
    }

    /// The argument after `^`, `_` or a command: a braced group or one
    /// character or command, in words.
    fn argument(&mut self) -> String {
        while self.peek().is_some_and(char::is_whitespace) {
            self.index += 1;
        }
        match self.peek() {
            Some('{') => {
                self.index += 1;
                self.until(Some('}'))
            }
            Some('\\') => {
                self.index += 1;
                self.command()
            }
            Some(character) => {
                self.index += 1;
                match character {
                    '+' => "+".to_string(),
                    '-' | '−' => "-".to_string(),
                    other => other.to_string(),
                }
            }
            None => String::new(),
        }
    }

    /// A braced group's text as written, for `\text` and `\ce`.
    fn raw_group(&mut self) -> String {
        while self.peek().is_some_and(char::is_whitespace) {
            self.index += 1;
        }
        if self.peek() != Some('{') {
            return String::new();
        }
        self.index += 1;
        let mut depth = 1;
        let mut out = String::new();
        while let Some(character) = self.peek() {
            self.index += 1;
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => out.push(character),
            }
        }
        out
    }

    /// The command after a backslash, in words.
    fn command(&mut self) -> String {
        let start = self.index;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            self.index += 1;
        }
        if self.index == start {
            // A one-character command such as \, or \%.
            let Some(character) = self.peek() else {
                return String::new();
            };
            self.index += 1;
            return match character {
                '%' => " percent ".to_string(),
                '\\' => ", ".to_string(),
                '$' => "$".to_string(),
                '#' | '&' | '_' => character.to_string(),
                _ => " ".to_string(),
            };
        }
        let name: String = self.characters[start..self.index].iter().collect();
        if let Some(letter) = greek_name(&name) {
            return format!(" {letter} ");
        }
        let words = match name.as_str() {
            "frac" | "dfrac" | "tfrac" => {
                let top = self.argument();
                let bottom = self.argument();
                return format!(" {top} over {bottom} ");
            }
            "sqrt" => return format!(" the square root of {} ", self.argument()),
            "text" | "textrm" | "textbf" | "textit" | "mbox" => return self.raw_group(),
            "ce" | "pu" => {
                return format!(" {} ", chemistry_words(&self.raw_group()));
            }
            "times" | "cdot" => "times",
            "div" => "divided by",
            "pm" => "plus or minus",
            "mp" => "minus or plus",
            "approx" | "sim" | "simeq" => "is about",
            "neq" | "ne" => "is not equal to",
            "le" | "leq" => "is at most",
            "ge" | "geq" => "is at least",
            "ll" => "is much less than",
            "gg" => "is much greater than",
            "propto" => "is proportional to",
            "equiv" => "is equivalent to",
            "to" => "to",
            "rightarrow" | "longrightarrow" | "Rightarrow" | "Longrightarrow" | "implies" => {
                "gives"
            }
            "leftarrow" | "longleftarrow" => "comes from",
            "rightleftharpoons" | "leftrightarrow" | "rightleftarrows" | "Leftrightarrow"
            | "iff" => "is in balance with",
            "uparrow" => "rises",
            "downarrow" => "falls",
            "infty" => "infinity",
            "circ" | "degree" => "degrees",
            "log" => "log",
            "ln" => "natural log",
            "exp" => "e to the",
            "sin" | "cos" | "tan" | "lim" | "max" | "min" => name.as_str(),
            "sum" => "the sum of",
            "prod" => "the product of",
            "int" => "the integral of",
            "partial" => "partial",
            "nabla" => "del",
            "ldots" | "dots" | "cdots" => "and so on",
            "quad" | "qquad" => ",",
            // Sizing and layout say nothing; their argument, if any, follows.
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl"
            | "Bigr" | "displaystyle" | "textstyle" | "limits" | "nolimits" => "",
            "begin" | "end" => {
                self.raw_group();
                ""
            }
            // Styles and accents say their argument: \mathrm{Na} is Na.
            _ => return self.argument_if_braced(),
        };
        format!(" {words} ")
    }

    /// A braced argument in words, or nothing when none follows.
    fn argument_if_braced(&mut self) -> String {
        let mut look = self.index;
        while self
            .characters
            .get(look)
            .is_some_and(|character| character.is_whitespace())
        {
            look += 1;
        }
        if self.characters.get(look) == Some(&'{') {
            self.index = look + 1;
            self.until(Some('}'))
        } else {
            String::new()
        }
    }
}

/// A superscript in words. A charge (+, 2-, 3+) stays attached to its
/// symbol, so Ca^{2+} is Ca2+ for the chemistry rules to name.
fn power_words(power: &str) -> String {
    let compact: String = power.split_whitespace().collect();
    let compact = compact.replace("plus", "+").replace("minus", "-");
    let charge = compact
        .strip_suffix(['+', '-'])
        .is_some_and(|count| count.chars().all(|character| character.is_ascii_digit()));
    if charge {
        return compact;
    }
    match compact.as_str() {
        "degrees" => " degrees ".to_string(),
        "2" => " squared ".to_string(),
        "3" => " cubed ".to_string(),
        _ => format!(" to the power of {} ", power.trim()),
    }
}

/// mhchem's arrows in words; its formulas are left for the chemistry rules.
fn chemistry_words(formula: &str) -> String {
    formula
        .replace("<=>", " is in balance with ")
        .replace("<->", " is in balance with ")
        .replace("->", " gives ")
        .replace("<-", " comes from ")
}

fn greek_name(name: &str) -> Option<&'static str> {
    const LETTERS: &[&str] = &[
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
        "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi",
        "omega",
    ];
    let lower = name
        .strip_prefix("var")
        .unwrap_or(name)
        .to_ascii_lowercase();
    LETTERS.iter().copied().find(|letter| *letter == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_are_said_in_words() {
        assert_eq!(say(r"\frac{a}{b}"), "a over b");
        assert_eq!(
            say(r"x^2 + y^{3} = z^{n}"),
            "x squared plus y cubed equals z to the power of n"
        );
        assert_eq!(
            say(r"\sqrt{2} \times 10^{-3}"),
            "the square root of 2 times 10 to the power of minus 3"
        );
        assert_eq!(say(r"37^\circ\text{C}"), "37 degrees C");
        assert_eq!(say(r"\Delta G \approx 0"), "delta G is about 0");
    }

    #[test]
    fn a_charged_formula_keeps_its_flat_spelling() {
        assert_eq!(say(r"\text{HCO}_3^-"), "HCO3-");
        assert_eq!(say(r"\mathrm{Ca^{2+}}"), "Ca2+");
        assert_eq!(
            say(r"\text{CO}_2 + \text{H}_2\text{O} \rightleftharpoons \text{H}_2\text{CO}_3"),
            "CO2 plus H2O is in balance with H2CO3"
        );
        assert_eq!(say(r"\ce{CO2 + H2O -> H2CO3}"), "CO2 + H2O gives H2CO3");
        assert_eq!(
            say(
                r"\mathrm{pH} = 6.1 + \log\left(\frac{[\text{HCO}_3^-]}{0.03 \times P_{CO_2}}\right)"
            ),
            "pH equals 6.1 plus log HCO3- over 0.03 times PCO2"
        );
    }

    #[test]
    fn math_spans_are_found_and_prices_left_alone() {
        assert_eq!(
            with_spoken_math(r"Water is $\text{H}_2\text{O}$ and \(x^2\) costs $5 or \$6."),
            "Water is  H2O  and  x squared  costs $5 or $6."
        );
        assert_eq!(with_spoken_math("$$a = b$$"), " a equals b ");
        assert_eq!(with_spoken_math("from $5 to $10"), "from $5 to $10");
        assert_eq!(with_spoken_math("an unclosed $x"), "an unclosed $x");
    }
}
