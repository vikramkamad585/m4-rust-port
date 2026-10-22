// POSIX extended regular expressions as glibc's regcomp(REG_EXTENDED)
// compiles them (no REG_NEWLINE), with an existence-only search.

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Byte(u8),
    Any,
    Set(Box<[bool; 256]>),
    Bol,
    Eol,
    WordB,
    NotWordB,
    WordStart,
    WordEnd,
    Group(usize, Box<Node>),
    Backref(usize),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat(Box<Node>, u32, Option<u32>),
}

pub struct Ere {
    root: Node,
    ngroups: usize,
    icase: bool,
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    ngroups: usize,
    closed: Vec<bool>,
    icase: bool,
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn class_set(name: &str) -> Option<Box<dyn Fn(u8) -> bool>> {
    let f: Box<dyn Fn(u8) -> bool> = match name {
        "alpha" => Box::new(|c: u8| c.is_ascii_alphabetic()),
        "digit" => Box::new(|c: u8| c.is_ascii_digit()),
        "alnum" => Box::new(|c: u8| c.is_ascii_alphanumeric()),
        "upper" => Box::new(|c: u8| c.is_ascii_uppercase()),
        "lower" => Box::new(|c: u8| c.is_ascii_lowercase()),
        "space" => Box::new(|c: u8| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')),
        "blank" => Box::new(|c: u8| c == b' ' || c == b'\t'),
        "punct" => Box::new(|c: u8| c.is_ascii_punctuation()),
        "print" => Box::new(|c: u8| (0x20..0x7f).contains(&c)),
        "graph" => Box::new(|c: u8| (0x21..0x7f).contains(&c)),
        "cntrl" => Box::new(|c: u8| c < 0x20 || c == 0x7f),
        "xdigit" => Box::new(|c: u8| c.is_ascii_hexdigit()),
        _ => return None,
    };
    Some(f)
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn reg(&mut self, depth: usize) -> Result<Node, ()> {
        let mut alts = vec![self.branch(depth)?];
        while self.peek() == Some(b'|') {
            self.i += 1;
            alts.push(self.branch(depth)?);
        }
        Ok(if alts.len() == 1 { alts.pop().unwrap() } else { Node::Alt(alts) })
    }

    fn branch(&mut self, depth: usize) -> Result<Node, ()> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == b'|' || (c == b')' && depth > 0) {
                break;
            }
            items.push(self.expr()?);
        }
        Ok(if items.len() == 1 { items.pop().unwrap() } else { Node::Concat(items) })
    }

    fn expr(&mut self) -> Result<Node, ()> {
        let c = self.peek().unwrap();
        let mut node = match c {
            b'*' | b'+' | b'?' => return Err(()),
            b'{' => return Err(()),
            b'(' => {
                self.i += 1;
                let idx = self.ngroups;
                self.ngroups += 1;
                self.closed.push(false);
                let inner = if self.peek() == Some(b')') { Node::Empty } else { self.reg(1)? };
                if self.peek() != Some(b')') {
                    return Err(());
                }
                self.i += 1;
                self.closed[idx] = true;
                Node::Group(idx, Box::new(inner))
            }
            b')' => {
                self.i += 1;
                Node::Byte(b')')
            }
            b'^' => {
                self.i += 1;
                return Ok(Node::Bol);
            }
            b'$' => {
                self.i += 1;
                return Ok(Node::Eol);
            }
            b'.' => {
                self.i += 1;
                Node::Any
            }
            b'[' => self.bracket()?,
            b'\\' => {
                self.i += 1;
                let c2 = match self.peek() {
                    Some(c2) => c2,
                    None => return Err(()),
                };
                self.i += 1;
                match c2 {
                    b'1'..=b'9' => {
                        let n = (c2 - b'1') as usize;
                        if n >= self.closed.len() || !self.closed[n] {
                            return Err(());
                        }
                        Node::Backref(n)
                    }
                    b'w' | b'W' | b's' | b'S' => {
                        let mut set = [false; 256];
                        for (k, v) in set.iter_mut().enumerate() {
                            let b = k as u8;
                            *v = match c2 {
                                b'w' => is_word(b),
                                b'W' => !is_word(b),
                                b's' => matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'),
                                _ => !matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'),
                            };
                        }
                        Node::Set(Box::new(set))
                    }
                    b'b' => return Ok(Node::WordB),
                    b'B' => return Ok(Node::NotWordB),
                    b'<' => return Ok(Node::WordStart),
                    b'>' => return Ok(Node::WordEnd),
                    b'`' => return Ok(Node::Bol),
                    b'\'' => return Ok(Node::Eol),
                    _ => Node::Byte(c2),
                }
            }
            _ => {
                self.i += 1;
                Node::Byte(c)
            }
        };
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.i += 1;
                    node = Node::Repeat(Box::new(node), 0, None);
                }
                Some(b'+') => {
                    self.i += 1;
                    node = Node::Repeat(Box::new(node), 1, None);
                }
                Some(b'?') => {
                    self.i += 1;
                    node = Node::Repeat(Box::new(node), 0, Some(1));
                }
                Some(b'{') => {
                    self.i += 1;
                    let num = |p: &mut P| -> Option<u32> {
                        let st = p.i;
                        while p.peek().map_or(false, |c| c.is_ascii_digit()) {
                            p.i += 1;
                        }
                        if p.i == st {
                            None
                        } else {
                            std::str::from_utf8(&p.s[st..p.i]).ok()?.parse().ok()
                        }
                    };
                    let lo = num(self);
                    let (min, max);
                    if self.peek() == Some(b',') {
                        self.i += 1;
                        let hi = num(self);
                        min = lo.unwrap_or(0);
                        max = hi;
                    } else {
                        min = match lo {
                            Some(v) => v,
                            None => return Err(()),
                        };
                        max = Some(min);
                    }
                    if self.peek() != Some(b'}') {
                        return Err(());
                    }
                    self.i += 1;
                    if let Some(m) = max {
                        if m < min {
                            return Err(());
                        }
                    }
                    if min > 255 || max.map_or(false, |m| m > 255) {
                        return Err(());
                    }
                    node = Node::Repeat(Box::new(node), min, max);
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn bracket(&mut self) -> Result<Node, ()> {
        self.i += 1;
        let mut set = [false; 256];
        let mut neg = false;
        if self.peek() == Some(b'^') {
            neg = true;
            self.i += 1;
        }
        let mut first = true;
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err(()),
            };
            if c == b']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            // element
            let start: u8;
            if c == b'[' && matches!(self.s.get(self.i + 1), Some(b':') | Some(b'.') | Some(b'=')) {
                let kind = self.s[self.i + 1];
                let body_start = self.i + 2;
                let mut j = body_start;
                while j + 1 < self.s.len() && !(self.s[j] == kind && self.s[j + 1] == b']') {
                    j += 1;
                }
                if j + 1 >= self.s.len() {
                    return Err(());
                }
                let name = std::str::from_utf8(&self.s[body_start..j]).map_err(|_| ())?;
                self.i = j + 2;
                if kind == b':' {
                    let f = class_set(name).ok_or(())?;
                    for (k, v) in set.iter_mut().enumerate() {
                        if f(k as u8) {
                            *v = true;
                        }
                    }
                    continue;
                }
                if name.len() != 1 {
                    return Err(());
                }
                start = name.as_bytes()[0];
                if kind == b'=' {
                    set[start as usize] = true;
                    continue;
                }
            } else {
                start = c;
                self.i += 1;
            }
            if self.peek() == Some(b'-') && self.s.get(self.i + 1).map_or(false, |&x| x != b']') {
                self.i += 1;
                let mut end = self.peek().ok_or(())?;
                if end == b'[' && self.s.get(self.i + 1) == Some(&b'.') {
                    let body_start = self.i + 2;
                    let mut j = body_start;
                    while j + 1 < self.s.len() && !(self.s[j] == b'.' && self.s[j + 1] == b']') {
                        j += 1;
                    }
                    if j + 1 >= self.s.len() || j - body_start != 1 {
                        return Err(());
                    }
                    end = self.s[body_start];
                    self.i = j + 2;
                } else {
                    self.i += 1;
                }
                if end < start {
                    return Err(());
                }
                for k in start..=end {
                    set[k as usize] = true;
                }
            } else {
                set[start as usize] = true;
            }
        }
        if self.icase {
            for k in 0..256usize {
                if set[k] {
                    let b = k as u8;
                    set[b.to_ascii_lowercase() as usize] = true;
                    set[b.to_ascii_uppercase() as usize] = true;
                }
            }
        }
        if neg {
            for v in set.iter_mut() {
                *v = !*v;
            }
        }
        Ok(Node::Set(Box::new(set)))
    }
}

struct M<'a> {
    s: &'a [u8],
    icase: bool,
    caps: Vec<Option<(usize, usize)>>,
    steps: u64,
}

impl<'a> M<'a> {
    fn eq(&self, a: u8, b: u8) -> bool {
        if self.icase {
            a.to_ascii_lowercase() == b.to_ascii_lowercase()
        } else {
            a == b
        }
    }

    /// Continuation-passing matcher: does `n` match at `pos` such that
    /// `k(end)` succeeds?
    fn m(&mut self, n: &Node, pos: usize, k: &mut dyn FnMut(&mut M<'a>, usize) -> bool) -> bool {
        self.steps += 1;
        if self.steps > 5_000_000 {
            return false;
        }
        let s = self.s;
        match n {
            Node::Empty => k(self, pos),
            Node::Byte(c) => pos < s.len() && self.eq(s[pos], *c) && k(self, pos + 1),
            Node::Any => pos < s.len() && k(self, pos + 1),
            Node::Set(set) => pos < s.len() && set[s[pos] as usize] && k(self, pos + 1),
            Node::Bol => pos == 0 && k(self, pos),
            Node::Eol => pos == s.len() && k(self, pos),
            Node::WordB | Node::NotWordB | Node::WordStart | Node::WordEnd => {
                let pw = pos > 0 && is_word(s[pos - 1]);
                let nw = pos < s.len() && is_word(s[pos]);
                let ok = match n {
                    Node::WordB => pw != nw,
                    Node::NotWordB => pw == nw,
                    Node::WordStart => !pw && nw,
                    _ => pw && !nw,
                };
                ok && k(self, pos)
            }
            Node::Group(idx, inner) => {
                let idx = *idx;
                let old = self.caps[idx];
                let r = self.m(inner, pos, &mut |mm: &mut M<'a>, e: usize| {
                    let prev = mm.caps[idx];
                    mm.caps[idx] = Some((pos, e));
                    if k(mm, e) {
                        return true;
                    }
                    mm.caps[idx] = prev;
                    false
                });
                if !r {
                    self.caps[idx] = old;
                }
                r
            }
            Node::Backref(idx) => match self.caps[*idx] {
                Some((a, b)) => {
                    let len = b - a;
                    if pos + len > s.len() {
                        return false;
                    }
                    for t in 0..len {
                        if !self.eq(s[a + t], s[pos + t]) {
                            return false;
                        }
                    }
                    k(self, pos + len)
                }
                None => false,
            },
            Node::Concat(v) => self.seq(v, 0, pos, k),
            Node::Alt(v) => {
                for a in v {
                    if self.m(a, pos, k) {
                        return true;
                    }
                }
                false
            }
            Node::Repeat(inner, min, max) => self.rep(inner, *min, *max, 0, pos, usize::MAX, k),
        }
    }

    fn seq(&mut self, v: &[Node], i: usize, pos: usize, k: &mut dyn FnMut(&mut M<'a>, usize) -> bool) -> bool {
        if i == v.len() {
            return k(self, pos);
        }
        let (head, _) = v.split_at(i + 1);
        let node = &head[i];
        self.m(node, pos, &mut |mm: &mut M<'a>, e: usize| mm.seq(v, i + 1, e, k))
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(&mut self, n: &Node, min: u32, max: Option<u32>, count: u32, pos: usize, last: usize,
           k: &mut dyn FnMut(&mut M<'a>, usize) -> bool) -> bool {
        let can_more = max.map_or(true, |m| count < m);
        if can_more && !(count >= min && last == pos) {
            if self.m(n, pos, &mut |mm: &mut M<'a>, e: usize| {
                if e == pos && count >= min {
                    return false;
                }
                mm.rep(n, min, max, count + 1, e, pos, k)
            }) {
                return true;
            }
        }
        if count >= min {
            return k(self, pos);
        }
        false
    }
}

impl Ere {
    pub fn compile(pat: &str, icase: bool) -> Result<Ere, ()> {
        let mut p = P { s: pat.as_bytes(), i: 0, ngroups: 0, closed: Vec::new(), icase };
        let root = p.reg(0)?;
        if p.i != p.s.len() {
            return Err(());
        }
        Ok(Ere { root, ngroups: p.ngroups, icase })
    }

    pub fn is_match(&self, s: &str) -> bool {
        let b = s.as_bytes();
        for start in 0..=b.len() {
            let mut m = M { s: b, icase: self.icase, caps: vec![None; self.ngroups], steps: 0 };
            if m.m(&self.root, start, &mut |_mm: &mut M, _e: usize| true) {
                return true;
            }
        }
        false
    }
}

/// rec_match / rec_match_insensitive.
pub fn rec_match(s: &str, re: &str, icase: bool) -> bool {
    match Ere::compile(re, icase) {
        Ok(r) => r.is_match(s),
        Err(()) => {
            eprint!("internal error: rec_match: error compiling regexp.\n");
            false
        }
    }
}
