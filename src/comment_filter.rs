use log::Level::Trace;
use std::{
    borrow::Cow,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

// from tokei deps:
use encoding_rs_io::DecodeReaderBytesBuilder;
use grep_searcher::LineIter;

use tokei::{Config, LanguageType, Stats};

// This contains a pile of things lifted from tokei privates
// it really should be done better - this is mostly spike code for now.

pub(crate) trait AsciiExt {
    fn is_whitespace(&self) -> bool;
}

impl AsciiExt for u8 {
    fn is_whitespace(&self) -> bool {
        *self == b' ' || (b'\x09'..=b'\x0d').contains(self)
    }
}

pub(crate) trait SliceExt {
    fn trim(&self) -> &Self;
    fn contains_slice(&self, needle: &Self) -> bool;
}

impl SliceExt for [u8] {
    fn trim(&self) -> &Self {
        let length = self.len();

        if length == 0 {
            return &self;
        }

        let start = match self.iter().position(|c| !c.is_whitespace()) {
            Some(start) => start,
            None => return &[],
        };

        let end = match self.iter().rposition(|c| !c.is_whitespace()) {
            Some(end) => end.max(start),
            _ => length,
        };

        &self[start..=end]
    }

    fn contains_slice(&self, needle: &Self) -> bool {
        let self_length = self.len();
        let needle_length = needle.len();

        if needle_length == 0 || needle_length > self_length {
            return false;
        } else if needle_length == self_length {
            return self == needle;
        }

        for window in self.windows(needle_length) {
            if needle == window {
                return true;
            }
        }

        false
    }
}

pub(crate) struct SyntaxCounter {
    pub(crate) allows_nested: bool,
    pub(crate) doc_quotes: &'static [(&'static str, &'static str)],
    pub(crate) is_fortran: bool,
    pub(crate) line_comments: &'static [&'static str],
    pub(crate) multi_line_comments: &'static [(&'static str, &'static str)],
    pub(crate) nested_comments: &'static [(&'static str, &'static str)],
    pub(crate) quote: Option<&'static str>,
    pub(crate) quote_is_doc_quote: bool,
    pub(crate) quotes: &'static [(&'static str, &'static str)],
    pub(crate) stack: Vec<&'static str>,
}

impl SyntaxCounter {
    pub(crate) fn new(language: LanguageType) -> Self {
        Self {
            allows_nested: language.allows_nested(),
            doc_quotes: language.doc_quotes(),
            is_fortran: language.is_fortran(),
            line_comments: language.line_comments(),
            multi_line_comments: language.multi_line_comments(),
            nested_comments: language.nested_comments(),
            quote_is_doc_quote: false,
            quotes: language.quotes(),
            stack: Vec::with_capacity(1),
            quote: None,
        }
    }

    #[inline]
    pub(crate) fn important_syntax(&self) -> impl Iterator<Item = &str> {
        self.quotes
            .iter()
            .map(|(s, _)| *s)
            .chain(self.doc_quotes.iter().map(|(s, _)| *s))
            .chain(self.multi_line_comments.iter().map(|(s, _)| *s))
            .chain(self.nested_comments.iter().map(|(s, _)| *s))
    }

    #[inline]
    pub(crate) fn start_of_comments(&self) -> impl Iterator<Item = &&str> {
        self.line_comments
            .iter()
            .chain(self.multi_line_comments.iter().map(|(s, _)| s))
            .chain(self.nested_comments.iter().map(|(s, _)| s))
    }

    #[inline]
    pub(crate) fn parse_line_comment(&self, window: &[u8]) -> bool {
        if self.quote.is_some() || !self.stack.is_empty() {
            return false;
        }

        for comment in self.line_comments {
            if window.starts_with(comment.as_bytes()) {
                trace!("Start {:?}", comment);
                return true;
            }
        }

        false
    }

    #[inline]
    pub(crate) fn parse_quote(&mut self, window: &[u8]) -> Option<usize> {
        if !self.stack.is_empty() {
            return None;
        }

        for &(start, end) in self.doc_quotes {
            if window.starts_with(start.as_bytes()) {
                trace!("Start Doc {:?}", start);
                self.quote = Some(end);
                self.quote_is_doc_quote = true;
                return Some(start.len());
            }
        }

        for &(start, end) in self.quotes {
            if window.starts_with(start.as_bytes()) {
                trace!("Start {:?}", start);
                self.quote = Some(end);
                self.quote_is_doc_quote = false;
                return Some(start.len());
            }
        }

        None
    }

    #[inline]
    pub(crate) fn parse_multi_line_comment(&mut self, window: &[u8]) -> Option<usize> {
        if self.quote.is_some() {
            return None;
        }

        let iter = self.multi_line_comments.iter().chain(self.nested_comments);
        for &(start, end) in iter {
            if window.starts_with(start.as_bytes()) {
                if self.stack.is_empty()
                    || self.allows_nested
                    || self.nested_comments.contains(&(start, end))
                {
                    self.stack.push(end);

                    if log_enabled!(Trace) && self.allows_nested {
                        trace!("Start nested {:?}", start);
                    } else {
                        trace!("Start {:?}", start);
                    }
                }

                return Some(start.len());
            }
        }

        None
    }

    #[inline]
    pub(crate) fn parse_end_of_quote(&mut self, window: &[u8]) -> Option<usize> {
        if self
            .quote
            .map_or(false, |q| window.starts_with(q.as_bytes()))
        {
            let quote = self.quote.take().unwrap();
            trace!("End {:?}", quote);
            Some(quote.len())
        } else if window.starts_with(br"\") {
            // Tell the state machine to skip the next character because it has
            // been escaped.
            Some(2)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn parse_end_of_multi_line(&mut self, window: &[u8]) -> Option<usize> {
        if self
            .stack
            .last()
            .map_or(false, |l| window.starts_with(l.as_bytes()))
        {
            let last = self.stack.pop().unwrap();
            if log_enabled!(Trace) && self.stack.is_empty() {
                trace!("End {:?}", last);
            } else {
                trace!("End {:?}. Still in comments.", last);
            }

            Some(last.len())
        } else {
            None
        }
    }
}

fn parse_basic(
    language: LanguageType,
    syntax: &SyntaxCounter,
    line: &[u8],
    raw_line: &str,
    stats: &mut Stats,
    code_lines: &mut Vec<String>,
) -> bool {
    if syntax.quote.is_some()
        || !syntax.stack.is_empty()
        || syntax
            .important_syntax()
            .any(|s| line.contains_slice(s.as_bytes()))
    {
        return false;
    }

    if syntax
        .line_comments
        .iter()
        .any(|s| line.starts_with(s.as_bytes()))
    {
        stats.comments += 1;
        trace!("Comment No.{}", stats.comments);
    } else {
        stats.code += 1;
        code_lines.push(raw_line.to_owned());
        trace!("Code No.{}", stats.code);
    }

    trace!("{}", String::from_utf8_lossy(line));
    trace!("^ Skippable.");

    true
}

pub fn parse_lines<'a>(
    language: LanguageType,
    config: &Config,
    lines: impl IntoIterator<Item = &'a [u8]>,
    mut stats: Stats,
) -> Vec<String> {
    let mut syntax = SyntaxCounter::new(language);
    let mut code_lines: Vec<String> = Vec::new();

    for line in lines {
        let raw_line: String = String::from_utf8_lossy(line).to_string();

        if line.trim().is_empty() {
            stats.blanks += 1;
            trace!("Blank No.{}", stats.blanks);
            continue;
            // TODO: can I add the blank line to code/comments???
        }

        // FORTRAN has a rule where it only counts as a comment if it's the
        // first character in the column, so removing starting whitespace
        // could cause a miscount.
        let line = if syntax.is_fortran { line } else { line.trim() };
        let had_multi_line = !syntax.stack.is_empty();
        let mut ended_with_comments = false;
        let mut skip = 0;
        macro_rules! skip {
            ($skip:expr) => {{
                skip = $skip - 1;
            }};
        }

        if parse_basic(
            language,
            &syntax,
            line,
            &raw_line,
            &mut stats,
            &mut code_lines,
        ) {
            continue;
        }

        'window: for i in 0..line.len() {
            if skip != 0 {
                skip -= 1;
                continue;
            }

            ended_with_comments = false;
            let window = &line[i..];

            let is_end_of_quote_or_multi_line = syntax
                .parse_end_of_quote(window)
                .or_else(|| syntax.parse_end_of_multi_line(window));

            if let Some(skip_amount) = is_end_of_quote_or_multi_line {
                ended_with_comments = true;
                skip!(skip_amount);
                continue;
            } else if syntax.quote.is_some() {
                continue;
            }

            let is_quote_or_multi_line = syntax
                .parse_quote(window)
                .or_else(|| syntax.parse_multi_line_comment(window));

            if let Some(skip_amount) = is_quote_or_multi_line {
                skip!(skip_amount);
                continue;
            }

            if syntax.parse_line_comment(window) {
                ended_with_comments = true;
                break 'window;
            }
        }
        trace!("{}", String::from_utf8_lossy(line));

        let is_comments = ((!syntax.stack.is_empty() || ended_with_comments) && had_multi_line)
            || (
                // If we're currently in a comment or we just ended
                // with one.
                syntax
                    .start_of_comments()
                    .any(|comment| line.starts_with(comment.as_bytes()))
                    && syntax.quote.is_none()
            )
            || ((
                        // If we're currently in a doc string or we just ended
                        // with one.
                        syntax.quote.is_some() ||
                        syntax.doc_quotes.iter().any(|(s, _)| line.starts_with(s.as_bytes()))
                    ) &&
                    // `Some(true)` is import in order to respect the current
                    // configuration.
                    config.treat_doc_strings_as_comments == Some(true) &&
                    syntax.quote_is_doc_quote);

        if is_comments {
            stats.comments += 1;
            trace!("Comment No.{}", stats.comments);
            trace!("Was the Comment stack empty?: {}", !had_multi_line);
        } else {
            code_lines.push(raw_line);
            stats.code += 1;
            trace!("Code No.{}", stats.code);
        }
    }

    stats.lines = stats.blanks + stats.code + stats.comments;
    code_lines
}

/// Parses the text provided. Returns `Stats` on success.
pub fn parse_from_str<A: AsRef<str>>(
    language: LanguageType,
    path: PathBuf,
    text: A,
    config: &Config,
) -> Vec<String> {
    parse_from_slice(language, path, text.as_ref().as_bytes(), config)
}

/// Parses the text provided. Returning `Stats` on success.
pub fn parse_from_slice<A: AsRef<[u8]>>(
    language: LanguageType,
    path: PathBuf,
    text: A,
    config: &Config,
) -> Vec<String> {
    let lines = LineIter::new(b'\n', text.as_ref());
    let mut stats = Stats::new(path);

    if language.is_blank() {
        lines
            .map(|line| String::from_utf8_lossy(line).to_string())
            .collect()
    } else {
        parse_lines(language, config, lines, stats)
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
        let result = parse_from_str(
            LanguageType::JavaScript,
            PathBuf::from("the_path"),
            code,
            &Config::default(),
        );

        let expected: Vec<String> = vec![
            "function foo() {\n".to_owned(),
            "\n".to_owned(),
            "    blah;\n".to_owned(),
            "}\n".to_owned(),
            "foo();".to_owned(),
        ];

        assert_eq!(result, expected);
    }
}
