use log::Level::Trace;
use std::{
    borrow::Cow,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

// from tokei deps:
use grep_searcher::LineIter;

use tokei::{Config, LanguageSummary, LanguageType};

#[derive(Clone, Debug, PartialEq)]
pub struct CodeLines {
    pub name: PathBuf,
    pub lines: Vec<Vec<u8>>
}

impl LanguageSummary for CodeLines {
    fn new(name: PathBuf) -> Self {
        CodeLines { name, lines: vec![] }
    }
    fn unprocessed_lines(&mut self, lines:LineIter) {
        self.lines.extend(lines.map(|line| line.to_vec()));
    }
    fn code_line(&mut self, line:&[u8]) {
        self.lines.push(line.to_vec());
    }
    fn comment_line(&mut self, _line:&[u8]) {
    }
    fn blank_line(&mut self, _line:&[u8]) {
    }
    fn postprocess(&mut self) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn can_strip_comments() {
        let code = r#"function foo() {

    blah;
    // comment
}
/* longer comment
with blanks

yow
*/
foo();"#;
        let result:CodeLines = LanguageType::JavaScript.parse_from_str(
            PathBuf::from("the_path"),
            code,
            &Config::default(),
        );

        let expected: Vec<Vec<u8>> = vec![
            "function foo() {\n".as_bytes().to_vec(),
            "    blah;\n".as_bytes().to_vec(),
            "}\n".as_bytes().to_vec(),
            "foo();".as_bytes().to_vec(),
        ];

        assert_eq!(result.lines, expected);
    }
}
