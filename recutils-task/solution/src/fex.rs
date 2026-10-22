// Field expressions: lists of field names with optional subscripts,
// aggregate calls and rewrite aliases.

use crate::types::fname_at;

#[derive(Clone, Debug)]
pub struct FexElem {
    pub field: String,
    pub function: Option<String>,
    pub alias: Option<String>,
    pub min: i64,
    pub max: i64,
}

/// `fname(.fname)?` at i -> (end, effective name with '.' turned into '_').
fn fexname_at(s: &str, i: usize) -> Option<(usize, String)> {
    let b = s.as_bytes();
    let e = fname_at(b, i)?;
    if e < b.len() && b[e] == b'.' {
        if let Some(e2) = fname_at(b, e + 1) {
            return Some((e2, format!("{}_{}", &s[i..e], &s[e + 1..e2])));
        }
    }
    Some((e, s[i..e].to_string()))
}

fn digits_at(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    if j > i {
        Some(j)
    } else {
        None
    }
}

fn funcname_at(b: &[u8], i: usize) -> Option<usize> {
    if i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'_') {
        let mut j = i + 1;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
            j += 1;
        }
        Some(j)
    } else {
        None
    }
}

/// Does one comma-free element match REC_FNAME_LIST_SUB_ELEM_RE exactly?
fn check_sub_elem(s: &str) -> bool {
    let b = s.as_bytes();
    let mut ends: Vec<usize> = Vec::new();
    // alternative 1: FNAME_SUB
    if let Some((e, _)) = fexname_at(s, 0) {
        ends.push(e);
        // with or without the dotted part, plus optional subscript
        let base_ends: Vec<usize> = {
            let mut v = vec![e];
            if let Some(e0) = fname_at(b, 0) {
                if e0 != e {
                    v.push(e0);
                }
            }
            v
        };
        for be in base_ends {
            ends.push(be);
            if be < b.len() && b[be] == b'[' {
                if let Some(d) = digits_at(b, be + 1) {
                    if d < b.len() && b[d] == b']' {
                        ends.push(d + 1);
                    }
                    if d < b.len() && b[d] == b'-' {
                        if let Some(d2) = digits_at(b, d + 2 - 1 + 0) {
                            let _ = d2;
                        }
                        if let Some(d2) = digits_at(b, d + 1) {
                            if d2 < b.len() && b[d2] == b']' {
                                ends.push(d2 + 1);
                            }
                        }
                    }
                }
            }
        }
    }
    // alternative 2: CALL
    if let Some(f) = funcname_at(b, 0) {
        if f < b.len() && b[f] == b'(' {
            let mut inner = vec![];
            if let Some(e0) = fname_at(b, f + 1) {
                inner.push(e0);
                if e0 < b.len() && b[e0] == b'.' {
                    if let Some(e1) = fname_at(b, e0 + 1) {
                        inner.push(e1);
                    }
                }
            }
            for ie in inner {
                if ie < b.len() && b[ie] == b')' {
                    ends.push(ie + 1);
                }
            }
        }
    }
    for e in ends {
        if e == b.len() {
            return true;
        }
        if e < b.len() && b[e] == b':' {
            if let Some(e0) = fname_at(b, e + 1) {
                if e0 == b.len() {
                    return true;
                }
                if e0 < b.len() && b[e0] == b'.' {
                    if let Some(e1) = fname_at(b, e0 + 1) {
                        if e1 == b.len() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn check_subscripts(s: &str) -> bool {
    s.split(',').all(check_sub_elem)
}

/// REC_FNAME_LIST_CS_RE anchored: fexname(,fexname)*
pub fn check_csv(s: &str) -> bool {
    s.split(',').all(|e| {
        let b = e.as_bytes();
        match fname_at(b, 0) {
            Some(x) if x == b.len() => true,
            Some(x) if b[x] == b'.' => matches!(fname_at(b, x + 1), Some(y) if y == b.len()),
            _ => false,
        }
    })
}

/// REC_FEX_SIMPLE / REC_FEX_CSV parsing: names separated by SEP characters.
pub fn parse_simple(s: &str, seps: &[char]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for part in s.split(|c| seps.contains(&c)) {
        if part.is_empty() {
            continue;
        }
        let (e, name) = fexname_at(part, 0)?;
        if e != part.len() {
            return None;
        }
        out.push(name);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// rec_parse_int-compatible scan used in subscripts.
fn parse_int(b: &[u8], i: usize) -> Option<(i64, usize)> {
    let mut j = i;
    while j < b.len()
        && (b[j].is_ascii_digit()
            || (j == i && b[j] == b'-')
            || (b'a'..=b'f').contains(&b[j])
            || (b'A'..=b'F').contains(&b[j])
            || b[j] == b'x'
            || b[j] == b'X')
    {
        j += 1;
    }
    let num = std::str::from_utf8(&b[i..j]).ok()?;
    crate::types::atoi(num).map(|v| (v as i64, j))
}

fn parse_elem(s: &str) -> Option<FexElem> {
    let b = s.as_bytes();
    let mut p = 0;
    let mut function = None;
    // "^" REC_FEX_CALL
    let is_call = {
        let mut ok = false;
        if let Some(f) = funcname_at(b, 0) {
            if f < b.len() && b[f] == b'(' {
                if let Some((e, _)) = fexname_at(s, f + 1) {
                    if e < b.len() && b[e] == b')' {
                        ok = true;
                    }
                    if let Some(e0) = fname_at(b, f + 1) {
                        if e0 < b.len() && b[e0] == b')' {
                            ok = true;
                        }
                    }
                }
            }
        }
        ok
    };
    if is_call {
        let f = funcname_at(b, 0)?;
        function = Some(s[..f].to_string());
        p = f + 1;
    }
    let (e, mut field) = {
        let e = fname_at(b, p)?;
        (e, s[p..e].to_string())
    };
    p = e;
    if p < b.len() && b[p] == b'.' {
        p += 1;
        let e2 = fname_at(b, p)?;
        field = format!("{}_{}", field, &s[p..e2]);
        p = e2;
    }
    let mut min = -1i64;
    let mut max = -1i64;
    if p < b.len() && b[p] == b'[' {
        p += 1;
        let (v, np) = parse_int(b, p)?;
        min = v;
        p = np;
        if p < b.len() && b[p] == b'-' {
            p += 1;
            let (v2, np2) = parse_int(b, p)?;
            max = v2;
            p = np2;
        }
        if !(p < b.len() && b[p] == b']') {
            return None;
        }
        p += 1;
    }
    if function.is_some() {
        p += 1;
    }
    let mut alias = None;
    if p < b.len() && b[p] == b':' {
        p += 1;
        let e = fname_at(b, p)?;
        alias = Some(s[p..e].to_string());
        p = e;
    }
    if p != b.len() {
        return None;
    }
    Some(FexElem { field, function, alias, min, max })
}

pub fn parse_subscripts(s: &str) -> Option<Vec<FexElem>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        out.push(parse_elem(part)?);
    }
    Some(out)
}
