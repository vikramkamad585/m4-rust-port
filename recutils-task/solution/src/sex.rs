// Selection expressions: lexer, parser and evaluator.

use crate::rec::{Field, Record};
use crate::types::{atod, atoi};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Add,
    Sub,
    Div,
    Mod,
    After,
    Before,
    Lte,
    Gte,
    SameTime,
    Implies,
    Mul,
    Eql,
    Neq,
    Not,
    Mat,
    Gt,
    Lt,
    Bp,
    Ep,
    And,
    Or,
    Sharp,
    Qm,
    Colon,
    Amp,
}

#[derive(Clone, Debug)]
enum Tok {
    Int(i32),
    Real(f64),
    Str(String),
    Name(String, Option<String>, i32),
    Op(Op),
    Err,
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

/// Longest-match lexer with flex rule order as the tie breaker.
fn lex(src: &str) -> Vec<Tok> {
    let s = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < s.len() {
        let c = s[i];
        if c == b' ' || c == b'\t' || c == b'\n' {
            i += 1;
            continue;
        }
        // candidates: (len, rule order, token)
        let mut best: Option<(usize, usize, Tok)> = None;
        let consider = |len: usize, order: usize, t: Tok, best: &mut Option<(usize, usize, Tok)>| {
            if len == 0 {
                return;
            }
            match best {
                Some((bl, bo, _)) if *bl > len || (*bl == len && *bo <= order) => {}
                _ => *best = Some((len, order, t)),
            }
        };
        // REAL: -?DIGIT*\.DIGIT+
        {
            let mut j = i;
            if j < s.len() && s[j] == b'-' {
                j += 1;
            }
            while j < s.len() && is_digit(s[j]) {
                j += 1;
            }
            if j < s.len() && s[j] == b'.' {
                let k0 = j + 1;
                let mut k = k0;
                while k < s.len() && is_digit(s[k]) {
                    k += 1;
                }
                if k > k0 {
                    let text = &src[i..k];
                    let v = atod(text).unwrap_or(0.0);
                    consider(k - i, 0, Tok::Real(v), &mut best);
                }
            }
        }
        // INT: -?((0x[0-9a-fA-F]+)|DIGIT+)
        {
            let mut j = i;
            if j < s.len() && s[j] == b'-' {
                j += 1;
            }
            let st = j;
            let mut end = None;
            if j + 2 < s.len() + 1 && s.get(j) == Some(&b'0') && s.get(j + 1) == Some(&b'x') {
                let mut k = j + 2;
                while k < s.len() && s[k].is_ascii_hexdigit() {
                    k += 1;
                }
                if k > j + 2 {
                    end = Some(k);
                }
            }
            let mut k = st;
            while k < s.len() && is_digit(s[k]) {
                k += 1;
            }
            if k > st && end.map_or(true, |e| k > e) {
                end = Some(k);
            }
            if let Some(e) = end {
                let text = &src[i..e];
                let v = atoi(text).unwrap_or(0);
                consider(e - i, 1, Tok::Int(v), &mut best);
            }
        }
        // operators, in rule order
        let ops: [(&str, Op); 25] = [
            ("+", Op::Add), ("-", Op::Sub), ("/", Op::Div), ("%", Op::Mod), (">>", Op::After),
            ("<<", Op::Before), ("<=", Op::Lte), (">=", Op::Gte), ("==", Op::SameTime),
            ("=>", Op::Implies), ("*", Op::Mul), ("=", Op::Eql), ("!=", Op::Neq), ("!", Op::Not),
            ("~", Op::Mat), (">", Op::Gt), ("<", Op::Lt), ("(", Op::Bp), (")", Op::Ep),
            ("&&", Op::And), ("||", Op::Or), ("#", Op::Sharp), ("?", Op::Qm), (":", Op::Colon),
            ("&", Op::Amp),
        ];
        for (k, (txt, op)) in ops.iter().enumerate() {
            if s[i..].starts_with(txt.as_bytes()) {
                consider(txt.len(), 2 + k, Tok::Op(*op), &mut best);
            }
        }
        // NAME: LETTER[a-zA-Z0-9_]*(\.LETTER[a-zA-Z0-9_]*)?(\[[0-9]+\])?
        if c.is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'_') {
                j += 1;
            }
            let name = src[i..j].to_string();
            let mut sub = None;
            if j + 1 < s.len() && s[j] == b'.' && s[j + 1].is_ascii_alphabetic() {
                let mut k = j + 2;
                while k < s.len() && (s[k].is_ascii_alphanumeric() || s[k] == b'_') {
                    k += 1;
                }
                sub = Some(src[j + 1..k].to_string());
                j = k;
            }
            let mut index = -1;
            if j < s.len() && s[j] == b'[' {
                let mut k = j + 1;
                while k < s.len() && is_digit(s[k]) {
                    k += 1;
                }
                if k > j + 1 && k < s.len() && s[k] == b']' {
                    index = atoi(&src[j + 1..k]).unwrap_or(0);
                    j = k + 1;
                }
            }
            consider(j - i, 100, Tok::Name(name, sub, index), &mut best);
        }
        // STRING: '([^']|\\(.|\n))*' | "([^"]|\\(.|\n))*"
        if c == b'\'' || c == b'"' {
            // The longest match ends at the last closing quote reachable.
            let q = c;
            let mut j = i + 1;
            let mut last_close = None;
            // states: scanning; a backslash may consume the next char
            // (including a quote); find the farthest closing quote such that
            // the body is matchable: every quote in the body must be escaped
            // by a preceding backslash that is itself "free".
            // Simulate with a set of positions (NFA).
            let mut states = vec![j];
            let mut seen = vec![false; s.len() + 2];
            while let Some(p) = states.pop() {
                if p > s.len() || seen[p] {
                    continue;
                }
                seen[p] = true;
                if p < s.len() {
                    if s[p] == q {
                        last_close = Some(last_close.map_or(p, |l: usize| l.max(p)));
                    } else {
                        states.push(p + 1);
                    }
                    if s[p] == b'\\' && p + 1 < s.len() {
                        states.push(p + 2);
                    }
                }
            }
            j = 0;
            let _ = j;
            if let Some(e) = last_close {
                let body = src[i + 1..e].to_string();
                consider(e + 1 - i, 101, Tok::Str(body), &mut best);
            }
        }
        consider(1, 102, Tok::Err, &mut best);
        let (len, _, t) = best.unwrap();
        out.push(t);
        i += len;
    }
    out
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Node {
    Int(i32),
    Real(f64),
    Str(String),
    Name { name: String, sub: Option<String>, index: i32, fixed: RefCell<Option<String>> },
    Un(Op, Box<Node>),
    Bin(Op, Box<Node>, Box<Node>),
    Cond(Box<Node>, Box<Node>, Box<Node>),
    Sharp(Box<Node>),
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
}

fn binprec(op: Op) -> Option<(u8, bool)> {
    // (level, right associative)
    Some(match op {
        Op::Qm => (1, false),
        Op::Implies => (2, true),
        Op::And | Op::Or => (3, false),
        Op::Eql | Op::Neq | Op::Lt | Op::Gt | Op::Lte | Op::Gte => (4, false),
        Op::SameTime | Op::After | Op::Before => (5, false),
        Op::Sub | Op::Add => (6, false),
        Op::Mul | Op::Div | Op::Mod | Op::Mat => (7, false),
        Op::Amp => (8, false),
        _ => return None,
    })
}

impl Parser {
    fn peek_op(&self) -> Option<Op> {
        match self.toks.get(self.i) {
            Some(Tok::Op(o)) => Some(*o),
            _ => None,
        }
    }

    fn primary(&mut self) -> Result<Node, ()> {
        let t = self.toks.get(self.i).cloned().ok_or(())?;
        self.i += 1;
        match t {
            Tok::Int(v) => Ok(Node::Int(v)),
            Tok::Real(v) => Ok(Node::Real(v)),
            Tok::Str(v) => Ok(Node::Str(v)),
            Tok::Name(n, s, idx) => Ok(Node::Name { name: n, sub: s, index: idx, fixed: RefCell::new(None) }),
            Tok::Op(Op::Not) => {
                let e = self.primary()?;
                Ok(Node::Un(Op::Not, Box::new(e)))
            }
            Tok::Op(Op::Sharp) => match self.toks.get(self.i).cloned() {
                Some(Tok::Name(n, s, idx)) => {
                    self.i += 1;
                    Ok(Node::Sharp(Box::new(Node::Name { name: n, sub: s, index: idx, fixed: RefCell::new(None) })))
                }
                _ => Err(()),
            },
            Tok::Op(Op::Bp) => {
                let e = self.expr(1)?;
                if self.peek_op() != Some(Op::Ep) {
                    return Err(());
                }
                self.i += 1;
                Ok(e)
            }
            _ => Err(()),
        }
    }

    fn expr(&mut self, min: u8) -> Result<Node, ()> {
        let mut lhs = self.primary()?;
        loop {
            let op = match self.peek_op() {
                Some(o) => o,
                None => break,
            };
            let (lvl, right) = match binprec(op) {
                Some(x) => x,
                None => break,
            };
            if lvl < min {
                break;
            }
            self.i += 1;
            if op == Op::Qm {
                let mid = self.expr(1)?;
                if self.peek_op() != Some(Op::Colon) {
                    return Err(());
                }
                self.i += 1;
                let rhs = self.expr(lvl + 1)?;
                lhs = Node::Cond(Box::new(lhs), Box::new(mid), Box::new(rhs));
                continue;
            }
            let rhs = self.expr(if right { lvl } else { lvl + 1 })?;
            if op == Op::Mat && (matches!(lhs, Node::Int(_)) || matches!(rhs, Node::Int(_))) {
                return Err(());
            }
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
}

pub struct Sex {
    top: Node,
    pub icase: bool,
}

#[derive(Clone, Debug)]
pub enum Val {
    Int(i32),
    Real(f64),
    Str(String),
}

fn atoi_val(v: &Val) -> Option<i32> {
    match v {
        Val::Int(i) => Some(*i),
        Val::Str(s) => {
            if s.is_empty() {
                Some(0)
            } else {
                atoi(s)
            }
        }
        // C leaves the destination uninitialised here; treat as truncation.
        Val::Real(r) => Some(*r as i32),
    }
}

fn atod_val(v: &Val) -> Option<f64> {
    match v {
        Val::Int(i) => Some(*i as f64),
        Val::Real(r) => Some(*r),
        Val::Str(s) => {
            if s.is_empty() {
                Some(0.0)
            } else {
                atod(s)
            }
        }
    }
}

fn real_p(a: &Val, b: &Val) -> bool {
    let is_int = |v: &Val| match v {
        Val::Int(_) => true,
        Val::Str(s) => atoi(s).is_some(),
        _ => false,
    };
    let is_real_only = |v: &Val| match v {
        Val::Real(_) => true,
        Val::Str(s) => atod(s).is_some() && atoi(s).is_none(),
        _ => false,
    };
    let mut ret = true;
    if is_int(a) {
        ret = match b {
            Val::Int(_) => false,
            Val::Real(_) => true,
            Val::Str(s) => atod(s).is_some() && atoi(s).is_none(),
        };
    }
    if is_real_only(a) {
        ret = match b {
            Val::Int(_) | Val::Real(_) => true,
            Val::Str(s) => atod(s).is_some(),
        };
    }
    ret
}

fn strcasecmp_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

impl Sex {
    pub fn compile(src: &str, icase: bool) -> Option<Sex> {
        let toks = lex(src);
        if toks.iter().any(|t| matches!(t, Tok::Err)) {
            return None;
        }
        let mut p = Parser { toks, i: 0 };
        let top = p.expr(1).ok()?;
        if p.i != p.toks.len() {
            return None;
        }
        Some(Sex { top, icase })
    }

    fn unfix(n: &Node) {
        match n {
            Node::Name { fixed, .. } => *fixed.borrow_mut() = None,
            Node::Un(_, a) | Node::Sharp(a) => Self::unfix(a),
            Node::Bin(_, a, b) => {
                Self::unfix(a);
                Self::unfix(b);
            }
            Node::Cond(a, b, c) => {
                Self::unfix(a);
                Self::unfix(b);
                Self::unfix(c);
            }
            _ => {}
        }
    }

    fn eval(&self, r: &Record, n: &Node) -> Option<Val> {
        Some(match n {
            Node::Int(v) => Val::Int(*v),
            Node::Real(v) => Val::Real(*v),
            Node::Str(s) => Val::Str(s.clone()),
            Node::Name { name, sub, index, fixed } => {
                if let Some(v) = fixed.borrow().as_ref() {
                    return Some(Val::Str(v.clone()));
                }
                let eff = match sub {
                    Some(s) => format!("{}_{}", name, s),
                    None => name.clone(),
                };
                let idx = if *index == -1 { 0 } else { *index as usize };
                let v = r.get(&eff, idx).map(|f| f.value.clone()).unwrap_or_default();
                if *index != -1 {
                    *fixed.borrow_mut() = Some(v.clone());
                }
                Val::Str(v)
            }
            Node::Sharp(c) => match c.as_ref() {
                Node::Name { name, sub, .. } => {
                    let eff = match sub {
                        Some(s) => format!("{}_{}", name, s),
                        None => name.clone(),
                    };
                    Val::Int(r.count(&eff) as i32)
                }
                _ => return None,
            },
            Node::Un(_, c) => {
                let v = self.eval(r, c)?;
                Val::Int((atoi_val(&v)? == 0) as i32)
            }
            Node::Cond(a, b, c) => {
                let va = self.eval(r, a)?;
                let vb = self.eval(r, b)?;
                let vc = self.eval(r, c)?;
                if atoi_val(&va)? != 0 {
                    vb
                } else {
                    vc
                }
            }
            Node::Bin(op, a, b) => {
                let va = self.eval(r, a)?;
                let vb = self.eval(r, b)?;
                match op {
                    Op::Add | Op::Sub | Op::Mul | Op::Div => {
                        if real_p(&va, &vb) {
                            let x = atod_val(&va)?;
                            let y = atod_val(&vb)?;
                            Val::Real(match op {
                                Op::Add => x + y,
                                Op::Sub => x - y,
                                Op::Mul => x * y,
                                _ => x / y,
                            })
                        } else {
                            let x = atoi_val(&va)?;
                            let y = atoi_val(&vb)?;
                            Val::Int(match op {
                                Op::Add => x.wrapping_add(y),
                                Op::Sub => x.wrapping_sub(y),
                                Op::Mul => x.wrapping_mul(y),
                                _ => {
                                    if y == 0 {
                                        return None;
                                    }
                                    x.wrapping_div(y)
                                }
                            })
                        }
                    }
                    Op::Mod => {
                        let x = atoi_val(&va)?;
                        let y = atoi_val(&vb)?;
                        if y == 0 {
                            return None;
                        }
                        Val::Int(x.wrapping_rem(y))
                    }
                    Op::Eql | Op::Neq => {
                        let eq = if let (Val::Str(x), Val::Str(y)) = (&va, &vb) {
                            if self.icase {
                                strcasecmp_eq(x, y)
                            } else {
                                x == y
                            }
                        } else if real_p(&va, &vb) {
                            atod_val(&va)? == atod_val(&vb)?
                        } else {
                            atoi_val(&va)? == atoi_val(&vb)?
                        };
                        Val::Int(if *op == Op::Eql { eq as i32 } else { (!eq) as i32 })
                    }
                    Op::Mat => {
                        if let (Val::Str(x), Val::Str(y)) = (&va, &vb) {
                            Val::Int(crate::ere::rec_match(x, y, self.icase) as i32)
                        } else {
                            return None;
                        }
                    }
                    Op::Lt | Op::Lte | Op::Gt | Op::Gte => {
                        let res = if real_p(&va, &vb) {
                            let x = atod_val(&va)?;
                            let y = atod_val(&vb)?;
                            match op {
                                Op::Lt => x < y,
                                Op::Lte => x <= y,
                                Op::Gt => x > y,
                                _ => x >= y,
                            }
                        } else {
                            let x = atoi_val(&va)?;
                            let y = atoi_val(&vb)?;
                            match op {
                                Op::Lt => x < y,
                                Op::Lte => x <= y,
                                Op::Gt => x > y,
                                _ => x >= y,
                            }
                        };
                        Val::Int(res as i32)
                    }
                    Op::Implies | Op::And | Op::Or => {
                        let x = atoi_val(&va)? != 0;
                        let y = atoi_val(&vb)? != 0;
                        Val::Int(match op {
                            Op::Implies => !x || (x && y),
                            Op::And => x && y,
                            _ => x || y,
                        } as i32)
                    }
                    Op::Amp => {
                        if let (Val::Str(x), Val::Str(y)) = (&va, &vb) {
                            Val::Str(format!("{}{}", x, y))
                        } else {
                            return None;
                        }
                    }
                    // Date comparisons need a date parser; unsupported here.
                    _ => return None,
                }
            }
        })
    }

    fn truthy(&self, r: &Record) -> bool {
        matches!(self.eval(r, &self.top), Some(Val::Int(v)) if v != 0)
    }

    fn name_p(n: &Node, name: &str, idx: usize) -> bool {
        match n {
            Node::Name { name: nm, index, .. } => (*index == -1 || (*index as i64) < idx as i64) && nm == name,
            Node::Un(_, a) | Node::Sharp(a) => Self::name_p(a, name, idx),
            Node::Bin(_, a, b) => Self::name_p(a, name, idx) || Self::name_p(b, name, idx),
            Node::Cond(a, b, c) => Self::name_p(a, name, idx) || Self::name_p(b, name, idx) || Self::name_p(c, name, idx),
            _ => false,
        }
    }

    fn hash_name_p(n: &Node, name: &str) -> bool {
        match n {
            Node::Sharp(c) => matches!(c.as_ref(), Node::Name { name: nm, .. } if nm == name),
            Node::Un(_, a) => Self::hash_name_p(a, name),
            Node::Bin(_, a, b) => Self::hash_name_p(a, name) || Self::hash_name_p(b, name),
            Node::Cond(a, b, c) => Self::hash_name_p(a, name) || Self::hash_name_p(b, name) || Self::hash_name_p(c, name),
            _ => false,
        }
    }

    /// rec_sex_eval: true if the record matches, trying each value of
    /// multi-valued fields mentioned in the expression.
    pub fn matches(&self, r: &Record) -> bool {
        Self::unfix(&self.top);
        if self.truthy(r) {
            return true;
        }
        let mut done: Vec<String> = Vec::new();
        let names: Vec<String> = r.fields().map(|f| f.name.clone()).collect();
        for name in names {
            let nf = r.count(&name);
            if nf > 1 && !done.contains(&name) && Self::name_p(&self.top, &name, nf) && !Self::hash_name_p(&self.top, &name) {
                done.push(name.clone());
                for j in 0..nf {
                    let wf: Field = r.get(&name, j).unwrap().clone();
                    let mut w = r.clone();
                    w.remove_all(&name);
                    w.push_field(wf);
                    if self.truthy(&w) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// rec_sex_eval_str, for recfmt templates.
    pub fn eval_str(&self, r: &Record) -> Option<String> {
        Self::unfix(&self.top);
        match self.eval(r, &self.top)? {
            Val::Int(v) => Some(v.to_string()),
            Val::Real(v) => Some(crate::cfmt::format_f(v)),
            Val::Str(s) => Some(s),
        }
    }
}
