// GNU Emacs-syntax regular expressions (RE_SYNTAX_EMACS, as used by m4's
// regexp and patsubst), with POSIX leftmost-longest overall matching and
// priority-ordered sub-expression assignment.

use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Assert {
    LineStart,
    LineEnd,
    BufStart,
    BufEnd,
    WordBoundary,
    NotWordBoundary,
    WordStart,
    WordEnd,
}

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Byte(u8),
    Any,
    Set(Box<[bool; 256]>),
    Assert(Assert),
    Group(usize, Box<Node>),
    Backref(usize),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Star(Box<Node>),
    Plus(Box<Node>),
    Quest(Box<Node>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum T {
    Char(u8),
    Period,
    OpenBracket,
    Star,
    Plus,
    Quest,
    Alt,
    Open,
    Close,
    BackRef(usize),
    Anchor(Assert),
    Word,
    NotWord,
    Space,
    NotSpace,
    TrailingBackslash,
    End,
}

const MSG_EPAREN: &str = "Unmatched ( or \\(";
const MSG_ERPAREN: &str = "Unmatched ) or \\)";
const MSG_EBRACK: &str = "Unmatched [, [^, [:, [., or [=";
const MSG_EESCAPE: &str = "Trailing backslash";
const MSG_ESUBREG: &str = "Invalid back reference";
const MSG_ERANGE: &str = "Invalid range end";
const MSG_ECOLLATE: &str = "Invalid collation character";

struct Parser<'a> {
    p: &'a [u8],
    i: usize,
    tok: T,
    tok_len: usize,
    nsub: usize,
    completed: u32,
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

impl<'a> Parser<'a> {
    /// peek_token for syntax 0 (Emacs); caret_here mirrors RE_CARET_ANCHORS_HERE.
    fn peek(&self, at: usize, caret_here: bool) -> (T, usize) {
        let p = self.p;
        if at >= p.len() {
            return (T::End, 0);
        }
        let c = p[at];
        if c == b'\\' {
            if at + 1 >= p.len() {
                return (T::TrailingBackslash, 1);
            }
            let c2 = p[at + 1];
            let t = match c2 {
                b'|' => T::Alt,
                b'1'..=b'9' => T::BackRef((c2 - b'1') as usize),
                b'<' => T::Anchor(Assert::WordStart),
                b'>' => T::Anchor(Assert::WordEnd),
                b'b' => T::Anchor(Assert::WordBoundary),
                b'B' => T::Anchor(Assert::NotWordBoundary),
                b'w' => T::Word,
                b'W' => T::NotWord,
                b's' => T::Space,
                b'S' => T::NotSpace,
                b'`' => T::Anchor(Assert::BufStart),
                b'\'' => T::Anchor(Assert::BufEnd),
                b'(' => T::Open,
                b')' => T::Close,
                _ => T::Char(c2),
            };
            return (t, 2);
        }
        let t = match c {
            b'*' => T::Star,
            b'+' => T::Plus,
            b'?' => T::Quest,
            b'[' => T::OpenBracket,
            b'.' => T::Period,
            b'^' => {
                if at != 0 && !caret_here {
                    T::Char(c)
                } else {
                    T::Anchor(Assert::LineStart)
                }
            }
            b'$' => {
                if at + 1 != p.len() {
                    let (n, _) = self.peek(at + 1, false);
                    if n != T::Alt && n != T::Close {
                        return (T::Char(c), 1);
                    }
                }
                T::Anchor(Assert::LineEnd)
            }
            _ => T::Char(c),
        };
        (t, 1)
    }

    fn fetch(&mut self, caret_here: bool) {
        self.i += self.tok_len;
        let (t, l) = self.peek(self.i, caret_here);
        self.tok = t;
        self.tok_len = l;
    }

    fn parse_reg_exp(&mut self, nest: usize) -> Result<Node, &'static str> {
        // Groups closed in one alternative are not visible to back
        // references in a later alternative.
        let initial = self.completed;
        let mut branches = vec![self.parse_branch(nest)?];
        while self.tok == T::Alt {
            self.fetch(true);
            if self.tok != T::Alt && self.tok != T::End && (nest == 0 || self.tok != T::Close) {
                let accumulated = self.completed;
                self.completed = initial;
                branches.push(self.parse_branch(nest)?);
                self.completed |= accumulated;
            } else {
                branches.push(Node::Empty);
            }
        }
        if branches.len() == 1 {
            Ok(branches.pop().unwrap())
        } else {
            Ok(Node::Alt(branches))
        }
    }

    fn parse_branch(&mut self, nest: usize) -> Result<Node, &'static str> {
        let mut items = Vec::new();
        while self.tok != T::Alt && self.tok != T::End && (nest == 0 || self.tok != T::Close) {
            items.push(self.parse_expression(nest)?);
        }
        if items.len() == 1 {
            Ok(items.pop().unwrap())
        } else {
            Ok(Node::Concat(items))
        }
    }

    fn parse_expression(&mut self, nest: usize) -> Result<Node, &'static str> {
        let mut tree = match self.tok {
            T::Char(c) => Node::Byte(c),
            T::Period => Node::Any,
            T::OpenBracket => self.parse_bracket()?,
            T::Open => self.parse_sub_exp(nest + 1)?,
            T::BackRef(n) => {
                if self.completed & (1 << n) == 0 {
                    return Err(MSG_ESUBREG);
                }
                Node::Backref(n)
            }
            T::Anchor(a) => {
                self.fetch(false);
                return Ok(Node::Assert(a));
            }
            T::Word | T::NotWord | T::Space | T::NotSpace => {
                let mut s = [false; 256];
                for c in 0..256usize {
                    let b = c as u8;
                    s[c] = match self.tok {
                        T::Word => is_word(b),
                        T::NotWord => !is_word(b),
                        T::Space => crate::m4::is_space(b),
                        _ => !crate::m4::is_space(b),
                    };
                }
                Node::Set(Box::new(s))
            }
            T::Star => Node::Byte(b'*'),
            T::Plus => Node::Byte(b'+'),
            T::Quest => Node::Byte(b'?'),
            T::Close => return Err(MSG_ERPAREN),
            T::TrailingBackslash => return Err(MSG_EESCAPE),
            T::Alt | T::End => Node::Empty,
        };
        self.fetch(false);
        while matches!(self.tok, T::Star | T::Plus | T::Quest) {
            tree = match self.tok {
                T::Star => Node::Star(Box::new(tree)),
                T::Plus => Node::Plus(Box::new(tree)),
                _ => Node::Quest(Box::new(tree)),
            };
            self.fetch(false);
        }
        Ok(tree)
    }

    fn parse_sub_exp(&mut self, nest: usize) -> Result<Node, &'static str> {
        let idx = self.nsub;
        self.nsub += 1;
        self.fetch(true);
        let inner = if self.tok == T::Close {
            Node::Empty
        } else {
            let t = self.parse_reg_exp(nest)?;
            if self.tok != T::Close {
                return Err(MSG_EPAREN);
            }
            t
        };
        if idx < 9 {
            self.completed |= 1 << idx;
        }
        Ok(Node::Group(idx, Box::new(inner)))
    }

    // Bracket tokens: (kind, byte, length). kind: 0 char, 1 close, 2 range '-',
    // 3 coll-elem open "[.", 4 equiv open "[=", 5 end of pattern.
    fn peek_bracket(&self, at: usize) -> (u8, u8, usize) {
        let p = self.p;
        if at >= p.len() {
            return (5, 0, 0);
        }
        let c = p[at];
        if c == b'[' && at + 1 < p.len() {
            match p[at + 1] {
                b'.' => return (3, b'.', 2),
                b'=' => return (4, b'=', 2),
                _ => return (0, c, 1),
            }
        }
        match c {
            b']' => (1, c, 1),
            b'-' => (2, c, 1),
            _ => (0, c, 1),
        }
    }

    /// Parse one bracket element starting at self.i; returns Ok(Some(byte))
    /// for a single character element.
    fn parse_bracket_element(&mut self, kind: u8, ch: u8, len: usize, accept_hyphen: bool) -> Result<u8, &'static str> {
        self.i += len;
        if kind == 3 || kind == 4 {
            let delim = ch;
            if self.i >= self.p.len() {
                return Err(MSG_EBRACK);
            }
            let mut name = Vec::new();
            loop {
                if name.len() >= 32 {
                    return Err(MSG_EBRACK);
                }
                let c = self.p[self.i];
                self.i += 1;
                if self.i >= self.p.len() {
                    return Err(MSG_EBRACK);
                }
                if c == delim && self.p[self.i] == b']' {
                    break;
                }
                name.push(c);
            }
            self.i += 1;
            if name.len() != 1 {
                return Err(MSG_ECOLLATE);
            }
            return Ok(name[0]);
        }
        if kind == 2 && !accept_hyphen {
            let (k2, _, _) = self.peek_bracket(self.i);
            if k2 != 1 {
                return Err(MSG_ERANGE);
            }
        }
        Ok(ch)
    }

    fn parse_bracket(&mut self) -> Result<Node, &'static str> {
        // self.i points at '['
        self.i += 1;
        let mut set = [false; 256];
        let mut negate = false;
        if self.i < self.p.len() && self.p[self.i] == b'^' {
            negate = true;
            self.i += 1;
        }
        let (mut kind, mut ch, mut len) = self.peek_bracket(self.i);
        if kind == 1 {
            kind = 0;
        }
        let mut first = true;
        loop {
            if kind == 5 {
                return Err(MSG_EBRACK);
            }
            let start_is_equiv = kind == 4;
            let start = self.parse_bracket_element(kind, ch, len, first)?;
            first = false;
            let (mut k, c2, l) = self.peek_bracket(self.i);
            let mut is_range = false;
            if k == 2 {
                self.i += l;
                let (k2, _, _) = self.peek_bracket(self.i);
                if k2 == 5 {
                    return Err(MSG_EBRACK);
                }
                if k2 == 1 {
                    self.i -= l;
                    k = 0;
                    let _ = k;
                } else {
                    is_range = true;
                }
            }
            if is_range {
                let (k3, c3, l3) = self.peek_bracket(self.i);
                if k3 == 5 {
                    return Err(MSG_EBRACK);
                }
                let end_is_equiv = k3 == 4;
                let end = self.parse_bracket_element(k3, c3, l3, true)?;
                if start_is_equiv || end_is_equiv {
                    return Err(MSG_ERANGE);
                }
                if start <= end {
                    for c in start..=end {
                        set[c as usize] = true;
                    }
                }
            } else {
                set[start as usize] = true;
            }
            let _ = c2;
            let (nk, nc, nl) = self.peek_bracket(self.i);
            if nk == 5 {
                return Err(MSG_EBRACK);
            }
            if nk == 1 {
                self.i += nl;
                break;
            }
            kind = nk;
            ch = nc;
            len = nl;
        }
        if negate {
            for v in set.iter_mut() {
                *v = !*v;
            }
        }
        // Leave self.i just past ']' with a zero-length "current token" so
        // that the following fetch() resumes there.
        self.tok_len = 0;
        Ok(Node::Set(Box::new(set)))
    }
}

#[derive(Clone, Debug)]
enum Inst {
    Byte(u8),
    Any,
    Set(Box<[bool; 256]>),
    Assert(Assert),
    Split(usize, usize),
    Jmp(usize),
    Save(usize),
    Backref(usize),
    MarkSet(usize),
    MarkCheck(usize),
    Match,
}

fn nullable(n: &Node) -> bool {
    match n {
        Node::Empty | Node::Assert(_) | Node::Star(_) | Node::Quest(_) | Node::Backref(_) => true,
        Node::Byte(_) | Node::Any | Node::Set(_) => false,
        Node::Group(_, b) | Node::Plus(b) => nullable(b),
        Node::Concat(v) => v.iter().all(nullable),
        Node::Alt(v) => v.iter().any(nullable),
    }
}

struct Compiler {
    prog: Vec<Inst>,
    marks: usize,
}

impl Compiler {
    fn emit(&mut self, i: Inst) -> usize {
        self.prog.push(i);
        self.prog.len() - 1
    }

    fn star(&mut self, body: &Node) {
        let l1 = self.emit(Inst::Split(0, 0));
        let mark = if nullable(body) {
            let k = self.marks;
            self.marks += 1;
            self.emit(Inst::MarkSet(k));
            Some(k)
        } else {
            None
        };
        let body_start = l1 + 1;
        self.node(body);
        if let Some(k) = mark {
            self.emit(Inst::MarkCheck(k));
        }
        self.emit(Inst::Jmp(l1));
        let out = self.prog.len();
        self.prog[l1] = Inst::Split(body_start, out);
    }

    fn node(&mut self, n: &Node) {
        match n {
            Node::Empty => {}
            Node::Byte(c) => {
                self.emit(Inst::Byte(*c));
            }
            Node::Any => {
                self.emit(Inst::Any);
            }
            Node::Set(s) => {
                self.emit(Inst::Set(s.clone()));
            }
            Node::Assert(a) => {
                self.emit(Inst::Assert(*a));
            }
            Node::Group(k, b) => {
                self.emit(Inst::Save(2 * k));
                self.node(b);
                self.emit(Inst::Save(2 * k + 1));
            }
            Node::Backref(k) => {
                self.emit(Inst::Backref(*k));
            }
            Node::Concat(v) => {
                for x in v {
                    self.node(x);
                }
            }
            Node::Alt(v) => {
                let mut jumps = Vec::new();
                for (idx, x) in v.iter().enumerate() {
                    if idx + 1 < v.len() {
                        let s = self.emit(Inst::Split(0, 0));
                        self.node(x);
                        jumps.push(self.emit(Inst::Jmp(0)));
                        let next = self.prog.len();
                        self.prog[s] = Inst::Split(s + 1, next);
                    } else {
                        self.node(x);
                    }
                }
                let end = self.prog.len();
                for j in jumps {
                    self.prog[j] = Inst::Jmp(end);
                }
            }
            Node::Star(b) => self.star(b),
            Node::Plus(b) => {
                self.node(b);
                self.star(b);
            }
            Node::Quest(b) => {
                let s = self.emit(Inst::Split(0, 0));
                self.node(b);
                let out = self.prog.len();
                self.prog[s] = Inst::Split(s + 1, out);
            }
        }
    }
}

pub struct Regex {
    prog: Vec<Inst>,
    ngroups: usize,
    nmarks: usize,
    has_backrefs: bool,
}

pub struct Match {
    pub start: usize,
    pub end: usize,
    pub groups: Vec<Option<(usize, usize)>>,
}

const STEP_LIMIT: u64 = 20_000_000;

struct Exec<'a> {
    re: &'a Regex,
    s: &'a [u8],
    caps: Vec<Option<usize>>,
    marks: Vec<Option<usize>>,
    steps: u64,
    target: Option<usize>,
    best: Option<usize>,
    visited: HashSet<(usize, usize)>,
}

impl<'a> Exec<'a> {
    fn check(&self, a: Assert, pos: usize) -> bool {
        let s = self.s;
        let prev = if pos > 0 { Some(s[pos - 1]) } else { None };
        let next = if pos < s.len() { Some(s[pos]) } else { None };
        let pw = prev.map_or(false, is_word);
        let nw = next.map_or(false, is_word);
        match a {
            Assert::LineStart => pos == 0 || prev == Some(b'\n'),
            Assert::LineEnd => pos == s.len() || next == Some(b'\n'),
            Assert::BufStart => pos == 0,
            Assert::BufEnd => pos == s.len(),
            Assert::WordBoundary => pw != nw,
            Assert::NotWordBoundary => pw == nw,
            Assert::WordStart => !pw && nw,
            Assert::WordEnd => pw && !nw,
        }
    }

    /// Depth-first priority search. In "explore" mode (target None) records
    /// the longest reachable end and never stops early; in "capture" mode it
    /// stops at the first path that ends exactly at the target.
    fn run(&mut self, pc: usize, pos: usize) -> bool {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return false;
        }
        let memo = !self.re.has_backrefs;
        if memo {
            if self.visited.contains(&(pc, pos)) {
                return false;
            }
            self.visited.insert((pc, pos));
        }
        match &self.re.prog[pc] {
            Inst::Match => {
                match self.target {
                    Some(t) => return pos == t,
                    None => {
                        if self.best.map_or(true, |b| pos > b) {
                            self.best = Some(pos);
                        }
                        return false;
                    }
                }
            }
            Inst::Byte(c) => pos < self.s.len() && self.s[pos] == *c && self.run(pc + 1, pos + 1),
            Inst::Any => pos < self.s.len() && self.s[pos] != b'\n' && self.run(pc + 1, pos + 1),
            Inst::Set(set) => pos < self.s.len() && set[self.s[pos] as usize] && self.run(pc + 1, pos + 1),
            Inst::Assert(a) => self.check(*a, pos) && self.run(pc + 1, pos),
            Inst::Jmp(t) => {
                let t = *t;
                self.run(t, pos)
            }
            Inst::Split(a, b) => {
                let (a, b) = (*a, *b);
                if self.run(a, pos) {
                    return true;
                }
                self.run(b, pos)
            }
            Inst::Save(k) => {
                let k = *k;
                let old = self.caps[k];
                self.caps[k] = Some(pos);
                if self.run(pc + 1, pos) {
                    return true;
                }
                self.caps[k] = old;
                false
            }
            Inst::MarkSet(k) => {
                let k = *k;
                let old = self.marks[k];
                self.marks[k] = Some(pos);
                let r = self.run(pc + 1, pos);
                self.marks[k] = old;
                r
            }
            Inst::MarkCheck(k) => {
                if self.marks[*k] == Some(pos) {
                    return false;
                }
                self.run(pc + 1, pos)
            }
            Inst::Backref(k) => {
                let k = *k;
                match (self.caps[2 * k], self.caps[2 * k + 1]) {
                    (Some(a), Some(b)) if a <= b => {
                        let len = b - a;
                        if pos + len <= self.s.len() && self.s[a..b] == self.s[pos..pos + len] {
                            self.run(pc + 1, pos + len)
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
        }
    }
}

impl Regex {
    pub fn compile(p: &[u8]) -> Result<Regex, &'static str> {
        let mut ps = Parser { p, i: 0, tok: T::End, tok_len: 0, nsub: 0, completed: 0 };
        let (t, l) = ps.peek(0, true);
        ps.tok = t;
        ps.tok_len = l;
        let tree = ps.parse_reg_exp(0)?;
        if ps.tok != T::End {
            // Only a stray close paren can stop the top-level parse.
            return Err(MSG_ERPAREN);
        }
        fn has_br(n: &Node) -> bool {
            match n {
                Node::Backref(_) => true,
                Node::Group(_, b) | Node::Star(b) | Node::Plus(b) | Node::Quest(b) => has_br(b),
                Node::Concat(v) | Node::Alt(v) => v.iter().any(has_br),
                _ => false,
            }
        }
        let has_backrefs = has_br(&tree);
        let mut c = Compiler { prog: Vec::new(), marks: 0 };
        c.node(&tree);
        c.emit(Inst::Match);
        Ok(Regex { prog: c.prog, ngroups: ps.nsub, nmarks: c.marks, has_backrefs })
    }

    fn exec<'a>(&'a self, s: &'a [u8], target: Option<usize>) -> Exec<'a> {
        Exec {
            re: self,
            s,
            caps: vec![None; 2 * self.ngroups],
            marks: vec![None; self.nmarks],
            steps: 0,
            target,
            best: None,
            visited: HashSet::new(),
        }
    }

    pub fn search(&self, s: &[u8], start: usize, want_groups: bool) -> Option<Match> {
        for pos in start..=s.len() {
            let mut ex = self.exec(s, None);
            ex.run(0, pos);
            if let Some(end) = ex.best {
                let mut groups = vec![None; self.ngroups];
                if want_groups && self.ngroups > 0 {
                    let mut ex2 = self.exec(s, Some(end));
                    if ex2.run(0, pos) {
                        for g in 0..self.ngroups {
                            if let (Some(a), Some(b)) = (ex2.caps[2 * g], ex2.caps[2 * g + 1]) {
                                if a <= b {
                                    groups[g] = Some((a, b));
                                }
                            }
                        }
                    }
                }
                return Some(Match { start: pos, end, groups });
            }
        }
        None
    }
}
