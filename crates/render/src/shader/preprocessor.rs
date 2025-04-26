use std::{borrow::Cow, collections::VecDeque};

use fnv::FnvHashMap;

pub struct Preprocessor<'a> {
    input: &'a str,
    current_start: usize,
    current: usize,
    line: usize,
    ifdef_state: VecDeque<bool>,
    next_buffered: Option<&'a str>,
    defines: FnvHashMap<&'a str, &'a str>,
}

#[derive(PartialEq, Eq, Copy, Clone)]
enum PreprocessorState {
    Neutral,
    String,
    Char,
    Ident,
    DirectiveStart,
    DirectiveName(usize),
    DirectiveArg(usize, usize),
    Other,
}

enum Token<'a> {
    Ident(&'a str),
    Directive(&'a str, &'a str),
    Other,
}

impl<'a> Preprocessor<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            current_start: 0,
            current: 0,
            line: 0,
            next_buffered: None,
            ifdef_state: VecDeque::new(),
            defines: FnvHashMap::default(),
        }
    }

    pub fn define(&mut self, key: &'a str, value: &'a str) -> &mut Self {
        self.defines.insert(key, value);
        self
    }

    pub fn process(mut self) -> Cow<'a, str> {
        let Some(first) = self.next() else {
            return Cow::Borrowed("");
        };
        let Some(more) = self.next() else {
            return Cow::Borrowed(first);
        };
        let mut owned = first.to_string();
        owned.push_str(more);
        for more in self {
            owned.push_str(more);
        }
        Cow::Owned(owned)
    }

    fn next_token(&mut self) -> Option<Token<'a>> {
        use self::PreprocessorState::*;
        self.current_start = self.current;
        let mut state = Neutral;
        let bytes = self.input.as_bytes();
        while let Some(&c) = bytes.get(self.current) {
            match state {
                Neutral => {
                    self.current += 1;
                    if c == b'#' {
                        state = DirectiveStart;
                    } else if c == b'"' {
                        state = String;
                    } else if c == b'\'' {
                        state = Char;
                    } else if matches!(c, b'A'..=b'Z' | b'a'..=b'z' | b'_') {
                        state = Ident;
                    } else if c == b'\n' {
                        self.line += 1;
                        state = Other;
                    } else {
                        state = Other;
                    }
                }
                String | Char => {
                    self.current += 1;
                    if state == String && c == b'"' || state == Char && c == b'\'' {
                        state = Other;
                    } else if c == b'\\' {
                        // next character escaped
                        self.current += 1;
                    }
                }
                Ident => {
                    if matches!(c, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_') {
                        self.current += 1;
                    } else {
                        break;
                    }
                }
                DirectiveStart => {
                    if matches!(c, b' ' | b'\t' | b'\r' | b'\x0C') {
                        self.current += 1;
                    } else {
                        state = DirectiveName(self.current);
                    }
                }
                DirectiveName(name_start) => {
                    if matches!(c, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_') {
                        self.current += 1;
                    } else {
                        state = DirectiveArg(name_start, self.current);
                    }
                }
                DirectiveArg(_, _) => {
                    if c == b'\n' {
                        break;
                    } else {
                        self.current += 1;
                    }
                }
                Other => {
                    if c == b'\n' {
                        self.line += 1;
                        self.current += 1;
                    } else if matches!(c, b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'#' | b'"' | b'\'') {
                        break;
                    } else {
                        self.current += 1;
                    }
                }
            }
        }
        match state {
            Neutral => None,
            Ident => Some(Token::Ident(&self.input[self.current_start..self.current])),
            DirectiveStart => Some(Token::Directive("", "")),
            DirectiveName(name_start) => Some(Token::Directive(
                self.input[name_start..self.current].trim_start(),
                "",
            )),
            DirectiveArg(name_start, arg_start) => Some(Token::Directive(
                self.input[name_start..arg_start].trim_start(),
                self.input[arg_start..self.current].trim(),
            )),
            String | Char | Other => Some(Token::Other),
        }
    }

    fn evaluate_ifdef(&self, arg: &str) -> bool {
        self.defines.contains_key(arg)
    }

    fn push_ifdef_state<F>(&mut self, eval: F) -> bool
    where
        F: FnOnce(&Self) -> bool,
    {
        let mut state = self.ifdef_state.back().copied().unwrap_or(true);
        if state {
            state = eval(self);
        }
        self.ifdef_state.push_back(state);
        state
    }

    fn pop_ifdef_state(&mut self) -> bool {
        self.ifdef_state.pop_back().unwrap_or(true)
    }

    fn ifdef_state(&self) -> bool {
        self.ifdef_state.back().copied().unwrap_or(true)
    }
}

impl<'a> Iterator for Preprocessor<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        use self::Token::*;
        if let Some(next_buffered) = self.next_buffered.take() {
            return Some(next_buffered);
        }

        let mut start = None;
        let mut end = self.current;
        while let Some(token) = self.next_token() {
            match token {
                Directive("ifdef", arg) => {
                    self.push_ifdef_state(|s| s.evaluate_ifdef(arg));
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Directive("ifndef", arg) => {
                    self.push_ifdef_state(|s| !s.evaluate_ifdef(arg));
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Directive("elifdef", arg) => {
                    let old_state = self.pop_ifdef_state();
                    self.push_ifdef_state(|s| !old_state && s.evaluate_ifdef(arg));
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Directive("elifndef", arg) => {
                    let old_state = self.pop_ifdef_state();
                    self.push_ifdef_state(|s| !old_state && !s.evaluate_ifdef(arg));
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Directive("else", _) => {
                    let old_state = self.pop_ifdef_state();
                    self.push_ifdef_state(|_| !old_state);
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Directive("endif", _) => {
                    self.pop_ifdef_state();
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Directive("define", arg) => {
                    let mut args = arg.splitn(2, |c: char| c.is_ascii_whitespace());
                    if let Some(key) = args.next() {
                        let value = args.next().unwrap_or("");
                        self.defines.insert(key, value);
                    }
                    if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                Ident(ident) => {
                    if self.ifdef_state() {
                        if let Some(value) = self.defines.get(ident) {
                            if let Some(start) = start {
                                self.next_buffered = Some(value);
                                return Some(&self.input[start..end]);
                            } else {
                                return Some(value);
                            }
                        } else {
                            if start.is_none() {
                                start = Some(self.current_start);
                            }
                            end = self.current;
                        }
                    } else if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
                _ => {
                    // pass unknown directives, and other tokens
                    if self.ifdef_state() {
                        if start.is_none() {
                            start = Some(self.current_start);
                        }
                        end = self.current;
                    } else if let Some(start) = start {
                        return Some(&self.input[start..end]);
                    }
                }
            }
        }
        if let Some(start) = start {
            Some(&self.input[start..end])
        } else {
            None
        }
    }
}
