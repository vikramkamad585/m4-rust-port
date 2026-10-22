// Input engine: a stack of input blocks (files, rescanned strings and
// builtin tokens from defn), line tracking and the tokenizer.

use crate::builtins::Builtin;
use crate::m4::M4;
use std::rc::Rc;

pub const CHAR_EOF: i32 = 256;
pub const CHAR_MACRO: i32 = 257;

pub enum Src {
    Str(Vec<u8>, usize),
    File(Vec<u8>, usize, bool),
    Macro(Builtin),
}

pub struct Block {
    pub src: Src,
    pub file: Rc<[u8]>,
    pub line: i32,
}

#[derive(Clone, Copy)]
pub enum TokKind {
    Eof,
    Str,
    Word,
    Open,
    Comma,
    Close,
    Simple,
    MacDef(Builtin),
}

pub struct Token {
    pub kind: TokKind,
    pub text: Vec<u8>,
}

fn is_word_start(c: i32) -> bool {
    (c >= b'a' as i32 && c <= b'z' as i32) || (c >= b'A' as i32 && c <= b'Z' as i32) || c == b'_' as i32
}

fn is_word_char(c: i32) -> bool {
    is_word_start(c) || (c >= b'0' as i32 && c <= b'9' as i32)
}

impl M4 {
    pub fn push_file(&mut self, data: Vec<u8>, title: Rc<[u8]>) {
        self.next_valid = false;
        self.stack.push(Block {
            src: Src::File(data, 0, self.start_of_input_line),
            file: title,
            line: 1,
        });
        self.input_change = true;
    }

    pub fn push_macro(&mut self, b: Builtin) {
        self.next_valid = false;
        self.stack.push(Block {
            src: Src::Macro(b),
            file: self.current_file.clone(),
            line: self.current_line,
        });
        self.input_change = true;
    }

    pub fn push_string_init(&mut self) {
        while let Some(b) = self.stack.last() {
            match &b.src {
                Src::Str(d, p) if *p >= d.len() => self.pop_input(),
                _ => break,
            }
        }
        self.next_valid = true;
        self.next_file = self.current_file.clone();
        self.next_line = self.current_line;
    }

    pub fn push_string_finish(&mut self, mut text: Vec<u8>) {
        if !self.next_valid {
            return;
        }
        self.next_valid = false;
        if let Some(p) = text.iter().position(|&c| c == 0) {
            text.truncate(p);
        }
        if !text.is_empty() {
            self.stack.push(Block {
                src: Src::Str(text, 0),
                file: self.next_file.clone(),
                line: self.next_line,
            });
            self.input_change = true;
        }
    }

    pub fn push_wrapup(&mut self, text: Vec<u8>) {
        let mut text = text;
        if let Some(p) = text.iter().position(|&c| c == 0) {
            text.truncate(p);
        }
        self.wrapup.push(Block {
            src: Src::Str(text, 0),
            file: self.current_file.clone(),
            line: self.current_line,
        });
    }

    fn pop_input(&mut self) {
        if let Some(b) = self.stack.pop() {
            if let Src::File(_, _, adv) = b.src {
                self.start_of_input_line = adv;
            }
        }
        self.next_valid = false;
        self.input_change = true;
    }

    pub fn pop_wrapup(&mut self) -> bool {
        self.next_valid = false;
        self.stack.clear();
        if self.wrapup.is_empty() {
            return false;
        }
        self.stack = std::mem::take(&mut self.wrapup);
        self.input_change = true;
        true
    }

    pub fn peek_input(&self) -> i32 {
        for b in self.stack.iter().rev() {
            match &b.src {
                Src::Str(d, p) | Src::File(d, p, _) => {
                    if *p < d.len() {
                        return d[*p] as i32;
                    }
                }
                Src::Macro(_) => return CHAR_MACRO,
            }
        }
        CHAR_EOF
    }

    fn peek_macro(&self) -> Option<Builtin> {
        for b in self.stack.iter().rev() {
            match &b.src {
                Src::Str(d, p) | Src::File(d, p, _) => {
                    if *p < d.len() {
                        return None;
                    }
                }
                Src::Macro(m) => return Some(*m),
            }
        }
        None
    }

    pub fn next_char(&mut self) -> i32 {
        loop {
            if self.stack.is_empty() {
                self.current_file = Rc::from(&b""[..]);
                self.current_line = 0;
                return CHAR_EOF;
            }
            if self.input_change {
                let b = self.stack.last().unwrap();
                self.current_file = b.file.clone();
                self.current_line = b.line;
                self.input_change = false;
            }
            let b = self.stack.last_mut().unwrap();
            match &mut b.src {
                Src::Str(d, p) => {
                    if *p < d.len() {
                        let c = d[*p];
                        *p += 1;
                        return c as i32;
                    }
                }
                Src::File(d, p, _) => {
                    if self.start_of_input_line {
                        self.start_of_input_line = false;
                        b.line += 1;
                        self.current_line = b.line;
                    }
                    if *p < d.len() {
                        let c = d[*p];
                        *p += 1;
                        if c == b'\n' {
                            self.start_of_input_line = true;
                        }
                        return c as i32;
                    }
                }
                Src::Macro(_) => {
                    self.pop_input();
                    return CHAR_MACRO;
                }
            }
            self.pop_input();
        }
    }

    /// Used by dnl: discard input through the next newline.
    pub fn skip_line(&mut self) {
        let file = self.current_file.clone();
        let line = self.current_line;
        let mut ch;
        loop {
            ch = self.next_char();
            if ch == CHAR_EOF || ch == b'\n' as i32 {
                break;
            }
        }
        if ch == CHAR_EOF {
            self.warn_at(&file, line, b"Warning: end of file treated as newline");
        }
        if *file != *self.current_file || line != self.current_line {
            self.input_change = true;
        }
    }

    fn match_input(&mut self, s: &[u8], consume: bool) -> bool {
        let ch = self.peek_input();
        if ch != s[0] as i32 {
            return false;
        }
        if s.len() == 1 {
            if consume {
                self.next_char();
            }
            return true;
        }
        self.next_char();
        let mut n = 1;
        let mut i = 1;
        let mut result = false;
        while self.peek_input() == s[i] as i32 {
            self.next_char();
            n += 1;
            i += 1;
            if i == s.len() {
                if consume {
                    return true;
                }
                result = true;
                break;
            }
        }
        self.push_string_init();
        self.push_string_finish(s[..n].to_vec());
        result
    }

    /// The MATCH macro: does the input (with CH first) start with S?
    fn matches(&mut self, ch: i32, s: &[u8], consume: bool) -> bool {
        if s.is_empty() || s[0] as i32 != ch || ch == 0 {
            return false;
        }
        if s.len() == 1 {
            return true;
        }
        let rest: Vec<u8> = if consume { s[1..].to_vec() } else { s.to_vec() };
        self.match_input(&rest, consume)
    }

    pub fn next_token(&mut self) -> (Token, i32) {
        let ch = self.peek_input();
        if ch == CHAR_EOF {
            self.next_char();
            return (Token { kind: TokKind::Eof, text: Vec::new() }, self.current_line);
        }
        if ch == CHAR_MACRO {
            let b = self.peek_macro().expect("macro block");
            self.next_char();
            return (Token { kind: TokKind::MacDef(b), text: Vec::new() }, self.current_line);
        }
        let ch = self.next_char();
        let file = self.current_file.clone();
        let line = self.current_line;
        let mut text: Vec<u8> = Vec::new();
        let kind;
        let bcomm = self.bcomm.clone();
        let lquote = self.lquote.clone();
        if self.matches(ch, &bcomm, true) {
            text.extend_from_slice(&bcomm);
            let ecomm = self.ecomm.clone();
            let mut c;
            loop {
                c = self.next_char();
                if c == CHAR_EOF {
                    break;
                }
                if self.matches(c, &ecomm, true) {
                    break;
                }
                text.push(c as u8);
            }
            if c != CHAR_EOF {
                text.extend_from_slice(&ecomm);
            } else {
                self.failure_at(&file, line, b"ERROR: end of file in comment");
            }
            kind = TokKind::Str;
        } else if is_word_start(ch) {
            text.push(ch as u8);
            loop {
                let c = self.peek_input();
                if c != CHAR_EOF && is_word_char(c) {
                    text.push(c as u8);
                    self.next_char();
                } else {
                    break;
                }
            }
            kind = TokKind::Word;
        } else if !self.matches(ch, &lquote, true) {
            kind = match ch as u8 {
                b'(' => TokKind::Open,
                b',' => TokKind::Comma,
                b')' => TokKind::Close,
                _ => TokKind::Simple,
            };
            text.push(ch as u8);
        } else {
            let rquote = self.rquote.clone();
            let mut level = 1;
            loop {
                let c = self.next_char();
                if c == CHAR_EOF {
                    self.failure_at(&file, line, b"ERROR: end of file in string");
                }
                if self.matches(c, &rquote, true) {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                    text.extend_from_slice(&rquote);
                } else if self.matches(c, &lquote, true) {
                    level += 1;
                    text.extend_from_slice(&lquote);
                } else {
                    text.push(c as u8);
                }
            }
            kind = TokKind::Str;
        }
        (Token { kind, text }, line)
    }

    pub fn peek_token(&mut self) -> TokKind {
        let ch = self.peek_input();
        if ch == CHAR_EOF {
            return TokKind::Eof;
        }
        if ch == CHAR_MACRO {
            return TokKind::MacDef(self.peek_macro().expect("macro block"));
        }
        let bcomm = self.bcomm.clone();
        if self.matches(ch, &bcomm, false) {
            return TokKind::Str;
        }
        if is_word_start(ch) {
            return TokKind::Word;
        }
        let lquote = self.lquote.clone();
        if self.matches(ch, &lquote, false) {
            return TokKind::Str;
        }
        match ch as u8 {
            b'(' => TokKind::Open,
            b',' => TokKind::Comma,
            b')' => TokKind::Close,
            _ => TokKind::Simple,
        }
    }
}
