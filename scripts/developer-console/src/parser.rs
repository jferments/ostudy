use std::borrow::Cow;

pub const EVENT_PREFIX: &str = "@@OSTUDY:TUI@@";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsoleControl {
    TakeoverStart { message: String },
    TakeoverEnd { exit_code: i32, message: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParsedLine {
    Stage { scope: String, name: String },
    Test(TestResult),
    Error(String),
    Output(String),
    Noise,
}

pub fn strip_ansi(input: &str) -> Cow<'_, str> {
    if !input.as_bytes().contains(&0x1b) {
        return Cow::Borrowed(input);
    }

    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            let start = index;
            while index < bytes.len() && bytes[index] != 0x1b {
                index += 1;
            }
            output.push_str(&input[start..index]);
            continue;
        }

        index += 1;
        if index >= bytes.len() {
            break;
        }
        match bytes[index] {
            b'[' => {
                index += 1;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            b']' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == 0x07 {
                        index += 1;
                        break;
                    }
                    if bytes[index] == 0x1b
                        && index + 1 < bytes.len()
                        && bytes[index + 1] == b'\\'
                    {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    Cow::Owned(output)
}

pub fn parse_console_control(raw: &str) -> Option<ConsoleControl> {
    let clean = strip_ansi(raw);
    let line = clean.trim();
    let rest = line.strip_prefix(EVENT_PREFIX)?.trim_start_matches('\t');
    let mut fields = rest.splitn(4, '\t');
    if fields.next()? != "takeover" {
        return None;
    }
    match fields.next()? {
        "start" => Some(ConsoleControl::TakeoverStart {
            message: fields.next()?.to_owned(),
        }),
        "end" => Some(ConsoleControl::TakeoverEnd {
            exit_code: fields.next()?.parse().ok()?,
            message: fields.next()?.to_owned(),
        }),
        _ => None,
    }
}

pub fn parse_line(raw: &str) -> ParsedLine {
    let clean = strip_ansi(raw);
    let line = clean.trim();
    if line.is_empty() {
        return ParsedLine::Noise;
    }

    if let Some(stage) = parse_stage(line) {
        return stage;
    }
    if let Some(test) = parse_rust_test(line) {
        return ParsedLine::Test(test);
    }
    if let Some(test) = parse_vitest_test(line) {
        return ParsedLine::Test(test);
    }
    if let Some(test) = parse_python_test(line) {
        return ParsedLine::Test(test);
    }
    if looks_like_error(line) {
        return ParsedLine::Error(line.to_owned());
    }
    if is_low_value_noise(line) {
        return ParsedLine::Noise;
    }
    ParsedLine::Output(line.to_owned())
}

fn parse_stage(line: &str) -> Option<ParsedLine> {
    let rest = line.strip_prefix(EVENT_PREFIX)?.trim_start_matches('\t');
    let mut fields = rest.splitn(3, '\t');
    if fields.next()? != "stage" {
        return None;
    }
    Some(ParsedLine::Stage {
        scope: fields.next()?.to_owned(),
        name: fields.next()?.to_owned(),
    })
}

fn parse_rust_test(line: &str) -> Option<TestResult> {
    let rest = line.strip_prefix("test ")?;
    let (name, result) = rest.rsplit_once(" ... ")?;
    let status = match result.trim() {
        "ok" => TestStatus::Passed,
        "FAILED" | "failed" => TestStatus::Failed,
        "ignored" => TestStatus::Skipped,
        _ => return None,
    };
    Some(TestResult {
        name: name.to_owned(),
        status,
        count: 1,
    })
}

fn parse_vitest_test(line: &str) -> Option<TestResult> {
    let passed = line.starts_with('✓') || line.starts_with("PASS ");
    let failed = line.starts_with('×')
        || line.starts_with('✗')
        || line.starts_with('❯')
        || line.starts_with("FAIL ");
    if !passed && !failed {
        return None;
    }

    let body = if let Some(body) = line.strip_prefix("PASS ") {
        body
    } else if let Some(body) = line.strip_prefix("FAIL ") {
        body
    } else {
        let marker = line.chars().next().map(char::len_utf8).unwrap_or(0);
        line.get(marker..)?.trim()
    };
    if body.is_empty() {
        return None;
    }

    let total_count = first_test_count(body).unwrap_or(1);
    let count = if failed {
        failed_test_count(body).unwrap_or(total_count.max(1))
    } else {
        total_count.max(1)
    };
    let name = body
        .split(" (")
        .next()
        .unwrap_or(body)
        .trim()
        .to_owned();
    Some(TestResult {
        name,
        status: if passed {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        },
        count,
    })
}

fn first_test_count(body: &str) -> Option<usize> {
    let open = body.find('(')?;
    let rest = body.get(open + 1..)?;
    let token = rest.split_whitespace().next()?;
    let count = token.parse::<usize>().ok()?;
    let label = rest.split_whitespace().nth(1)?;
    if label.starts_with("test") {
        Some(count)
    } else {
        None
    }
}

fn failed_test_count(body: &str) -> Option<usize> {
    let (_, summary) = body.rsplit_once('|')?;
    let mut words = summary.split_whitespace();
    while let Some(word) = words.next() {
        if let Ok(count) = word.parse::<usize>()
            && words.next().is_some_and(|label| label.starts_with("failed"))
        {
            return Some(count);
        }
    }
    None
}

fn parse_python_test(line: &str) -> Option<TestResult> {
    let (name, suffix) = line.rsplit_once(" ... ")?;
    if !name.starts_with("test_") {
        return None;
    }
    let status = match suffix.trim() {
        "ok" => TestStatus::Passed,
        "FAIL" | "ERROR" => TestStatus::Failed,
        "skipped" => TestStatus::Skipped,
        _ => return None,
    };
    Some(TestResult {
        name: name.to_owned(),
        status,
        count: 1,
    })
}

fn looks_like_error(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("error:")
        || lower.starts_with("error[")
        || lower.contains(" check failed")
        || lower.contains("build failed")
        || lower.contains("panicked at")
        || lower.starts_with("fatal:")
        || line == "FAILED"
        || line.starts_with("FAIL ")
        || line.starts_with('✗')
        || line.starts_with('×')
        || line.starts_with('❯')
}

fn is_low_value_noise(line: &str) -> bool {
    line.starts_with("Compiling ")
        || line.starts_with("Checking ")
        || line.starts_with("Downloading ")
        || line.starts_with("Downloaded ")
        || line.starts_with("Fresh ")
        || line.starts_with("Progress:")
        || line == "running 0 tests"
        || line
            .chars()
            .all(|character| matches!(character, '.' | 'F' | 'E' | 's'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_ansi_sequences() {
        assert_eq!(strip_ansi("\u{1b}[1;31mFAILED\u{1b}[0m"), "FAILED");
    }

    #[test]
    fn parses_structured_stage_event() {
        assert_eq!(
            parse_line("@@OSTUDY:TUI@@\tstage\tfast\tReact tests"),
            ParsedLine::Stage {
                scope: "fast".into(),
                name: "React tests".into(),
            }
        );
    }

    #[test]
    fn parses_terminal_takeover_events() {
        assert_eq!(
            parse_console_control("@@OSTUDY:TUI@@\ttakeover\tstart\tsudo authentication"),
            Some(ConsoleControl::TakeoverStart {
                message: "sudo authentication".into(),
            })
        );
        assert_eq!(
            parse_console_control("@@OSTUDY:TUI@@\ttakeover\tend\t0\tpackage configuration"),
            Some(ConsoleControl::TakeoverEnd {
                exit_code: 0,
                message: "package configuration".into(),
            })
        );
    }

    #[test]
    fn stage_events_are_not_terminal_control_events() {
        assert_eq!(
            parse_console_control("@@OSTUDY:TUI@@\tstage\tfast\tReact tests"),
            None
        );
    }

    #[test]
    fn parses_rust_results() {
        assert_eq!(
            parse_line("test database::tests::opens ... ok"),
            ParsedLine::Test(TestResult {
                name: "database::tests::opens".into(),
                status: TestStatus::Passed,
                count: 1,
            })
        );
    }

    #[test]
    fn parses_vitest_file_counts() {
        assert_eq!(
            parse_line("✓ src/ui/example.test.tsx (14 tests) 42ms"),
            ParsedLine::Test(TestResult {
                name: "src/ui/example.test.tsx".into(),
                status: TestStatus::Passed,
                count: 14,
            })
        );
        assert_eq!(
            parse_line("❯ src/ui/broken.test.tsx (12 tests | 2 failed) 10ms"),
            ParsedLine::Test(TestResult {
                name: "src/ui/broken.test.tsx".into(),
                status: TestStatus::Failed,
                count: 2,
            })
        );
    }

    #[test]
    fn keeps_copyable_errors() {
        assert_eq!(
            parse_line("error[E0308]: mismatched types"),
            ParsedLine::Error("error[E0308]: mismatched types".into())
        );
    }
}
