use regex::bytes::Regex;
use std::path::PathBuf;
#[macro_use]
use lazy_static;
use grep_searcher::LineIter;

use tokei::LanguageSummary;

#[derive(Clone, Debug, PartialEq)]
pub struct CodeLineData {
    pub spaces: i32,
    pub tabs: i32,
    pub text: i32,
}

impl CodeLineData {
    fn new(line: &[u8]) -> Self {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^(\s*)([^\n]*)\n?$").unwrap();
        }
        println!("scanning '{}'", String::from_utf8_lossy(line));
        let caps = RE.captures(line).unwrap();
        let whitespace_chars: &[u8] = &caps[1];
        let text_chars: &[u8] = &caps[2];
        let mut spaces = 0;
        let mut tabs = 0;
        for ch in whitespace_chars {
            if *ch == b' ' {
                spaces += 1;
            } else if *ch == b'\t' {
                tabs += 1;
            } else {
                panic!("Invalid whitespace char: {}", *ch);
            }
        }
        let text = String::from_utf8_lossy(text_chars).chars().count();
        CodeLineData {
            spaces,
            tabs,
            text: text as i32,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeLines {
    pub name: PathBuf,
    pub lines: Vec<CodeLineData>,
}

impl LanguageSummary for CodeLines {
    fn new(name: PathBuf) -> Self {
        CodeLines {
            name,
            lines: vec![],
        }
    }
    fn unprocessed_lines(&mut self, lines: LineIter) {
        self.lines.extend(lines.map(|line| CodeLineData::new(line)));
    }
    fn code_line(&mut self, line: &[u8]) {
        self.lines.push(CodeLineData::new(line));
    }
    fn comment_line(&mut self, _line: &[u8]) {}
    fn blank_line(&mut self, _line: &[u8]) {}
    fn postprocess(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokei::{Config, LanguageType};

    #[test]
    pub fn can_process_tabs_and_spaces() {
        let data = CodeLineData::new(" \t \t foo".as_bytes());
        assert_eq!(
            data,
            CodeLineData {
                spaces: 3,
                tabs: 2,
                text: 3
            }
        );
    }

    #[test]
    pub fn can_process_unicode() {
        let data = CodeLineData::new("①②③④⑤⑥⑦⑧⑨⑩".as_bytes());
        assert_eq!(
            data,
            CodeLineData {
                spaces: 0,
                tabs: 0,
                text: 10
            }
        );
    }

    #[test]
    pub fn can_parse_source_code() {
        let code = r#"function foo☃() {

    blah;

    // comment
}
/* longer comment
with blanks

yow
*/
foo();"#;
        let result: CodeLines = LanguageType::JavaScript.parse_from_str(
            PathBuf::from("the_path"),
            code,
            &Config::default(),
        );

        let expected = vec![
            CodeLineData {
                spaces: 0,
                tabs: 0,
                text: 17,
            },
            CodeLineData {
                spaces: 4,
                tabs: 0,
                text: 5,
            },
            CodeLineData {
                spaces: 0,
                tabs: 0,
                text: 1,
            },
            CodeLineData {
                spaces: 0,
                tabs: 0,
                text: 6,
            },
        ];

        assert_eq!(result.lines, expected);
    }
}
