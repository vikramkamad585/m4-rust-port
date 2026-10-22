// Field types declared in record descriptors and typed value comparison.

use crate::rec::{Rset, Record};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Int,
    Bool,
    Range,
    Real,
    Size,
    Line,
    Regexp,
    Date,
    Email,
    Enum,
    Field,
    Rec(String),
    Uuid,
}

fn blank(c: u8) -> bool {
    c == b' ' || c == b'\n' || c == b'\t'
}

fn skip_blanks(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && blank(s[i]) {
        i += 1;
    }
    i
}

/// Matches `[a-zA-Z%][a-zA-Z0-9_]*` at i; returns end.
pub fn fname_at(s: &[u8], i: usize) -> Option<usize> {
    if i < s.len() && (s[i].is_ascii_alphabetic() || s[i] == b'%') {
        let mut j = i + 1;
        while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'_') {
            j += 1;
        }
        Some(j)
    } else {
        None
    }
}

/// `[a-zA-Z][a-zA-Z0-9_-]*`
fn type_name_at(s: &[u8], i: usize) -> Option<usize> {
    if i < s.len() && s[i].is_ascii_alphabetic() {
        let mut j = i + 1;
        while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'_' || s[j] == b'-') {
            j += 1;
        }
        Some(j)
    } else {
        None
    }
}

/// C strtol with base 0 over the whole string (rec_atoi).
pub fn atoi(s: &str) -> Option<i32> {
    let (v, end) = strtol0(s.as_bytes());
    if !s.is_empty() && end == s.len() {
        Some(v as i32)
    } else {
        None
    }
}

/// strtol(s, &end, 0): returns (value clamped to i64, consumed bytes).
pub fn strtol0(s: &[u8]) -> (i64, usize) {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut base = 10u32;
    let start;
    if i + 1 < s.len() && s[i] == b'0' && (s[i + 1] == b'x' || s[i + 1] == b'X')
        && i + 2 < s.len() && s[i + 2].is_ascii_hexdigit()
    {
        base = 16;
        i += 2;
        start = i;
    } else if i < s.len() && s[i] == b'0' {
        base = 8;
        start = i;
    } else {
        start = i;
    }
    let mut v: i128 = 0;
    let mut j = start;
    while j < s.len() {
        let d = match (s[j] as char).to_digit(base) {
            Some(d) => d,
            None => break,
        };
        v = (v * base as i128 + d as i128).min(i128::from(i64::MAX) + 1);
        j += 1;
    }
    if j == start {
        // "0x" with no hex digits parses as 0 followed by "x"
        return (0, 0);
    }
    let v = if neg { -v } else { v };
    let v = v.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    (v, j)
}

/// C strtod over the whole string (rec_atod).
pub fn atod(s: &str) -> Option<f64> {
    let (v, end) = strtod(s.as_bytes());
    if !s.is_empty() && end == s.len() {
        Some(v)
    } else {
        None
    }
}

pub fn strtod(s: &[u8]) -> (f64, usize) {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let start = i;
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let low: Vec<u8> = s[i..].iter().map(|c| c.to_ascii_lowercase()).collect();
    if low.starts_with(b"infinity") {
        return (if neg { f64::NEG_INFINITY } else { f64::INFINITY }, i + 8);
    }
    if low.starts_with(b"inf") {
        return (if neg { f64::NEG_INFINITY } else { f64::INFINITY }, i + 3);
    }
    if low.starts_with(b"nan") {
        let mut n = 3;
        if low.get(3) == Some(&b'(') {
            let mut k = 4;
            while k < low.len() && (low[k].is_ascii_alphanumeric() || low[k] == b'_') {
                k += 1;
            }
            if low.get(k) == Some(&b')') {
                n = k + 1;
            }
        }
        return (f64::NAN, i + n);
    }
    if low.starts_with(b"0x") && (low.get(2).map_or(false, |c| c.is_ascii_hexdigit())
        || (low.get(2) == Some(&b'.') && low.get(3).map_or(false, |c| c.is_ascii_hexdigit())))
    {
        let mut k = 2;
        let mut mant = 0f64;
        let mut exp: i32 = 0;
        let mut point = false;
        while k < low.len() {
            let c = low[k];
            if c == b'.' && !point {
                point = true;
            } else if c.is_ascii_hexdigit() {
                mant = mant * 16.0 + (c as char).to_digit(16).unwrap() as f64;
                if point {
                    exp -= 4;
                }
            } else {
                break;
            }
            k += 1;
        }
        if k < low.len() && low[k] == b'p' {
            let mut q = k + 1;
            let mut eneg = false;
            if q < low.len() && (low[q] == b'+' || low[q] == b'-') {
                eneg = low[q] == b'-';
                q += 1;
            }
            if q < low.len() && low[q].is_ascii_digit() {
                let mut e = 0i32;
                while q < low.len() && low[q].is_ascii_digit() {
                    e = (e * 10 + (low[q] - b'0') as i32).min(100000);
                    q += 1;
                }
                exp += if eneg { -e } else { e };
                k = q;
            }
        }
        let v = mant * 2f64.powi(exp);
        return (if neg { -v } else { v }, i + k);
    }
    let mut k = i;
    let mut nd = 0;
    while k < s.len() && s[k].is_ascii_digit() {
        k += 1;
        nd += 1;
    }
    if k < s.len() && s[k] == b'.' {
        k += 1;
        while k < s.len() && s[k].is_ascii_digit() {
            k += 1;
            nd += 1;
        }
    }
    if nd == 0 {
        return (0.0, 0);
    }
    if k < s.len() && (s[k] == b'e' || s[k] == b'E') {
        let mut q = k + 1;
        if q < s.len() && (s[q] == b'+' || s[q] == b'-') {
            q += 1;
        }
        if q < s.len() && s[q].is_ascii_digit() {
            while q < s.len() && s[q].is_ascii_digit() {
                q += 1;
            }
            k = q;
        }
    }
    let text = std::str::from_utf8(&s[start..k]).unwrap_or("0");
    let text = text.trim_start_matches('+');
    let v: f64 = text.parse().unwrap_or(0.0);
    (v, k)
}

/// rec_parse_int: hex-ish characters then rec_atoi.
fn parse_int(s: &[u8], i: usize) -> Option<(i32, usize)> {
    let mut j = i;
    while j < s.len()
        && (s[j].is_ascii_digit()
            || (j == i && s[j] == b'-')
            || (b'a'..=b'f').contains(&s[j])
            || (b'A'..=b'F').contains(&s[j])
            || s[j] == b'x'
            || s[j] == b'X')
    {
        j += 1;
    }
    let num = std::str::from_utf8(&s[i..j]).ok()?;
    atoi(num).map(|v| (v, j))
}

/// rec_type_new: parse a type description; None if invalid.
pub fn parse_type(desc: &str) -> Option<Kind> {
    let s = desc.as_bytes();
    let mut p = skip_blanks(s, 0);
    // POSIX leftmost-longest over the alternatives at p.
    let names = ["int", "range", "real", "size", "line", "regexp", "date", "enum", "email", "bool",
                 "field", "rec", "uuid"];
    let mut best: Option<&str> = None;
    for n in names.iter() {
        if s[p..].starts_with(n.as_bytes()) && best.map_or(true, |b| n.len() > b.len()) {
            best = Some(n);
        }
    }
    let kname = best?;
    p += kname.len();
    let kind = match kname {
        "int" => Kind::Int,
        "bool" => Kind::Bool,
        "real" => Kind::Real,
        "line" => Kind::Line,
        "field" => Kind::Field,
        "date" => Kind::Date,
        "email" => Kind::Email,
        "uuid" => Kind::Uuid,
        "size" => {
            p = skip_blanks(s, p);
            let (v, np) = parse_int(s, p)?;
            if v < 0 {
                return None;
            }
            p = np;
            Kind::Size
        }
        "enum" => {
            let mut size = 0;
            while p < s.len() {
                p = skip_blanks(s, p);
                if p < s.len() && s[p] == b'(' {
                    p += 1;
                    while p < s.len() && s[p] != b')' && s[p] != b'(' {
                        p += 1;
                    }
                    if p < s.len() && s[p] == b')' {
                        p += 1;
                    } else {
                        return None;
                    }
                } else if p < s.len() {
                    // [a-zA-Z0-9][a-zA-Z0-9_-]*
                    if !s[p].is_ascii_alphanumeric() {
                        return None;
                    }
                    let mut j = p + 1;
                    while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'_' || s[j] == b'-') {
                        j += 1;
                    }
                    p = j;
                    size += 1;
                }
            }
            if size == 0 {
                return None;
            }
            Kind::Enum
        }
        "regexp" => {
            p = skip_blanks(s, p);
            if p >= s.len() {
                return None;
            }
            let delim = s[p];
            p += 1;
            let mut re = Vec::new();
            let mut end = false;
            while p < s.len() {
                if s[p] == delim {
                    if p + 1 < s.len() && s[p + 1] == delim {
                        re.push(delim);
                        p += 1;
                    } else {
                        p += 1;
                        end = true;
                        break;
                    }
                } else {
                    re.push(s[p]);
                }
                p += 1;
            }
            if !end {
                return None;
            }
            let re = String::from_utf8_lossy(&re).into_owned();
            if crate::ere::Ere::compile(&re, false).is_err() {
                return None;
            }
            Kind::Regexp
        }
        "range" => {
            p = skip_blanks(s, p);
            let point = |p: usize| -> Option<usize> {
                if s[p..].starts_with(b"MIN") || s[p..].starts_with(b"MAX") {
                    Some(p + 3)
                } else {
                    parse_int(s, p).map(|(_, np)| np)
                }
            };
            p = point(p)?;
            p = skip_blanks(s, p);
            if p < s.len() {
                p = point(p)?;
            }
            Kind::Range
        }
        "rec" => {
            p = skip_blanks(s, p);
            let e = fname_at(s, p)?;
            let name = desc[p..e].to_string();
            p = e;
            Kind::Rec(name)
        }
        _ => return None,
    };
    while p < s.len() {
        if !blank(s[p]) {
            return None;
        }
        p += 1;
    }
    Some(kind)
}

/// Is `str` a valid %type value: a field list then a type description or a
/// type name?
fn type_field_parts(value: &str) -> Option<(Vec<String>, String)> {
    let s = value.as_bytes();
    let p = skip_blanks(s, 0);
    // REC_FNAME_LIST_CS_RE: fexname(,fexname)*
    let mut names = Vec::new();
    let mut q = p;
    loop {
        let e = fname_at(s, q)?;
        let mut name = value[q..e].to_string();
        let mut e2 = e;
        if e2 < s.len() && s[e2] == b'.' {
            if let Some(e3) = fname_at(s, e2 + 1) {
                name = format!("{}_{}", name, &value[e2 + 1..e3]);
                e2 = e3;
            }
        }
        names.push(name);
        q = e2;
        if q < s.len() && s[q] == b',' && fname_at(s, q + 1).is_some() {
            q += 1;
            continue;
        }
        break;
    }
    let rest_start = skip_blanks(s, q);
    let rest = value[rest_start..].to_string();
    let ok = parse_type(&rest).is_some() || {
        let r = rest.as_bytes();
        match type_name_at(r, 0) {
            Some(e) => r[e..].iter().all(|&c| blank(c)),
            None => false,
        }
    };
    if ok {
        Some((names, rest))
    } else {
        None
    }
}

#[derive(Clone, Debug, Default)]
pub struct Props {
    /// field name -> (kind if a direct type, or named type)
    pub types: Vec<(String, Option<Kind>, Option<String>)>,
    pub typedefs: Vec<(String, Option<Kind>, Option<String>)>,
    pub key: Option<String>,
    pub sort: Option<Vec<String>>,
}

impl Props {
    pub fn from_rset(rset: &Rset) -> Props {
        let mut props = Props::default();
        if let Some(d) = &rset.descriptor {
            props.load(d);
        }
        props
    }

    fn set_type(&mut self, field: &str, kind: Option<Kind>, name: Option<String>) {
        // Newest props are prepended in C, but each field has a single slot.
        if let Some(slot) = self.types.iter_mut().find(|t| t.0 == field) {
            slot.1 = kind;
            slot.2 = name;
        } else {
            self.types.push((field.to_string(), kind, name));
        }
    }

    fn load(&mut self, d: &Record) {
        // typedefs
        for f in d.fields() {
            if f.name == "%typedef" {
                let s = f.value.as_bytes();
                let p = skip_blanks(s, 0);
                if let Some(e) = type_name_at(s, p) {
                    let tname = f.value[p..e].to_string();
                    let rest = &f.value[e..];
                    match parse_type(rest) {
                        Some(k) => self.typedefs.push((tname, Some(k), None)),
                        None => {
                            let r = rest.as_bytes();
                            let q = skip_blanks(r, 0);
                            if let Some(e2) = type_name_at(r, q) {
                                self.typedefs.push((tname, None, Some(rest[q..e2].to_string())));
                            }
                        }
                    }
                }
            }
        }
        let mut autos: Vec<String> = Vec::new();
        for f in d.fields() {
            if f.name == "%type" {
                if let Some((names, rest)) = type_field_parts(&f.value) {
                    for n in names {
                        match parse_type(&rest) {
                            Some(k) => self.set_type(&n, Some(k), None),
                            None => {
                                let r = rest.as_bytes();
                                let e = type_name_at(r, 0).unwrap_or(0);
                                self.set_type(&n, None, Some(rest[..e].to_string()));
                            }
                        }
                    }
                }
            }
            if f.name == "%key" {
                let s = f.value.as_bytes();
                let p = skip_blanks(s, 0);
                if let Some(e) = fname_at(s, p) {
                    let k = f.value[p..e].to_string();
                    // The first prop marked key in list order wins; C keeps
                    // props newest-first, so the most recently created one.
                    self.key = Some(k);
                }
            }
            if f.name == "%auto" {
                for n in f.value.split(|c| c == ' ' || c == '\t' || c == '\n') {
                    if !n.is_empty() {
                        autos.push(n.to_string());
                    }
                }
            }
            if f.name == "%sort" {
                if let Some(fex) = crate::fex::parse_simple(&f.value, &[' ', '\t', '\n']) {
                    self.sort = Some(fex);
                }
            }
        }
        for a in autos {
            if !self.types.iter().any(|t| t.0 == a) {
                self.types.push((a, Some(Kind::Int), None));
            }
        }
    }

    pub fn field_type(&self, field: &str) -> Option<Kind> {
        let t = self.types.iter().find(|t| t.0 == field)?;
        if let Some(k) = &t.1 {
            return Some(k.clone());
        }
        let mut name = t.2.clone()?;
        let mut seen: Vec<String> = Vec::new();
        loop {
            let td = self.typedefs.iter().find(|x| x.0 == name)?;
            if let Some(k) = &td.1 {
                return Some(k.clone());
            }
            if seen.contains(&name) {
                return None;
            }
            seen.push(name.clone());
            name = td.2.clone()?;
        }
    }
}

fn bool_true(v: &str) -> bool {
    let t = v.trim_matches(|c| c == ' ' || c == '\t' || c == '\n');
    // ZBLANKS (1|yes|true) ZBLANKS, unanchored search
    v.contains('1') || v.contains("yes") || v.contains("true") || t == "1"
}

pub fn values_cmp(kind: Option<&Kind>, v1: &str, v2: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let lexi = || v1.as_bytes().cmp(v2.as_bytes());
    match kind {
        Some(Kind::Int) | Some(Kind::Range) => match (atoi(v1), atoi(v2)) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => lexi(),
        },
        Some(Kind::Real) => match (atod(v1), atod(v2)) {
            (Some(a), Some(b)) => {
                if a < b {
                    Ordering::Less
                } else if a > b {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
            _ => lexi(),
        },
        Some(Kind::Bool) => {
            let b1 = bool_true(v1);
            let b2 = bool_true(v2);
            if !b1 && b2 {
                Ordering::Less
            } else if b1 == b2 {
                Ordering::Equal
            } else {
                Ordering::Greater
            }
        }
        _ => lexi(),
    }
}
