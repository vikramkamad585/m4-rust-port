// The format builtin: a printf work-alike over m4 arguments, matching the
// C library's output in the C locale.

use crate::builtins::strtol;
use crate::m4::{is_space, Arg, M4};

const THOUSANDS: u8 = 0x01;
const PLUS: u8 = 0x02;
const MINUS: u8 = 0x04;
const SPACE: u8 = 0x08;
const ZERO: u8 = 0x10;
const ALT: u8 = 0x20;

struct Args<'a> {
    args: &'a [Arg],
    pos: usize,
}

impl<'a> Args<'a> {
    fn next(&mut self) -> Option<&'a [u8]> {
        if self.pos < self.args.len() {
            self.pos += 1;
            Some(self.args[self.pos - 1].text())
        } else {
            None
        }
    }
}

fn arg_long(m: &mut M4, s: &[u8], check_int: bool) -> i64 {
    if s.is_empty() {
        m.warn(b"empty string treated as 0");
        return 0;
    }
    let (v, end, erange) = strtol(s);
    if end != s.len() {
        let mut msg = b"non-numeric argument ".to_vec();
        msg.extend_from_slice(s);
        m.warn(&msg);
    } else if is_space(s[0]) {
        m.warn(b"leading whitespace ignored");
    } else if erange || (check_int && v as i32 as i64 != v) {
        m.warn(b"numeric overflow detected");
    }
    v
}

/// strtod: returns (value, bytes consumed, ERANGE).
pub fn strtod(s: &[u8]) -> (f64, usize, bool) {
    let mut i = 0;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let start = i;
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let lower: Vec<u8> = s[i..].iter().map(|c| c.to_ascii_lowercase()).collect();
    let sign = if neg { -1.0 } else { 1.0 };
    if lower.starts_with(b"inf") {
        let n = if lower.starts_with(b"infinity") { 8 } else { 3 };
        return (sign * f64::INFINITY, i + n, false);
    }
    if lower.starts_with(b"nan") {
        let mut n = 3;
        if lower.get(3) == Some(&b'(') {
            let mut k = 4;
            while k < lower.len() && (lower[k].is_ascii_alphanumeric() || lower[k] == b'_') {
                k += 1;
            }
            if lower.get(k) == Some(&b')') {
                n = k + 1;
            }
        }
        let v = if neg { -f64::NAN } else { f64::NAN };
        return (v, i + n, false);
    }
    if lower.starts_with(b"0x")
        && (lower.get(2).map_or(false, |c| c.is_ascii_hexdigit())
            || (lower.get(2) == Some(&b'.') && lower.get(3).map_or(false, |c| c.is_ascii_hexdigit())))
    {
        let mut k = 2;
        let mut mant: u128 = 0;
        let mut exp: i64 = 0;
        let mut seen_point = false;
        let mut digits = 0;
        while k < lower.len() {
            let c = lower[k];
            if c == b'.' && !seen_point {
                seen_point = true;
            } else if c.is_ascii_hexdigit() {
                let d = (c as char).to_digit(16).unwrap() as u128;
                if mant < (1u128 << 100) {
                    mant = mant * 16 + d;
                    if seen_point {
                        exp -= 4;
                    }
                } else if !seen_point {
                    exp += 4;
                }
                digits += 1;
            } else {
                break;
            }
            k += 1;
        }
        let _ = digits;
        if k < lower.len() && lower[k] == b'p' {
            let mut q = k + 1;
            let mut eneg = false;
            if q < lower.len() && (lower[q] == b'+' || lower[q] == b'-') {
                eneg = lower[q] == b'-';
                q += 1;
            }
            if q < lower.len() && lower[q].is_ascii_digit() {
                let mut e: i64 = 0;
                while q < lower.len() && lower[q].is_ascii_digit() {
                    e = (e * 10 + (lower[q] - b'0') as i64).min(100000);
                    q += 1;
                }
                exp += if eneg { -e } else { e };
                k = q;
            }
        }
        let v = (mant as f64) * 2f64.powi(exp.clamp(-2000, 2000) as i32);
        let erange = v.is_infinite();
        return (sign * v, i + k, erange);
    }
    // decimal
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
        return (0.0, 0, false);
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
    let v: f64 = text.parse().unwrap_or(0.0);
    let erange = v.is_infinite() || (v == 0.0 && s[i..k].iter().any(|&c| (b'1'..=b'9').contains(&c)) && {
        // only mantissa digits before an exponent count
        let mant_end = s[i..k].iter().position(|&c| c == b'e' || c == b'E').map(|p| i + p).unwrap_or(k);
        s[i..mant_end].iter().any(|&c| (b'1'..=b'9').contains(&c))
    }) || (v != 0.0 && v.abs() < f64::MIN_POSITIVE);
    (v, k, erange)
}

fn arg_double(m: &mut M4, s: &[u8]) -> f64 {
    if s.is_empty() {
        m.warn(b"empty string treated as 0");
        return 0.0;
    }
    let (v, end, erange) = strtod(s);
    if end != s.len() {
        let mut msg = b"non-numeric argument ".to_vec();
        msg.extend_from_slice(s);
        m.warn(&msg);
    } else if is_space(s[0]) {
        m.warn(b"leading whitespace ignored");
    } else if erange {
        m.warn(b"numeric overflow detected");
    }
    v
}

fn pad(body: Vec<u8>, width: usize, left: bool) -> Vec<u8> {
    if body.len() >= width {
        return body;
    }
    let mut out = Vec::with_capacity(width);
    if left {
        out.extend_from_slice(&body);
        out.resize(width, b' ');
    } else {
        out.resize(width - body.len(), b' ');
        out.extend_from_slice(&body);
    }
    out
}

/// Assemble sign/prefix + digits with width, zero padding and justification.
fn finish_number(sign: &[u8], digits: &[u8], width: usize, flags: u8, zero_ok: bool) -> Vec<u8> {
    let len = sign.len() + digits.len();
    if flags & MINUS != 0 {
        let mut v = sign.to_vec();
        v.extend_from_slice(digits);
        return pad(v, width, true);
    }
    if flags & ZERO != 0 && zero_ok && len < width {
        let mut v = sign.to_vec();
        v.resize(sign.len() + (width - len), b'0');
        v.extend_from_slice(digits);
        return v;
    }
    let mut v = sign.to_vec();
    v.extend_from_slice(digits);
    pad(v, width, false)
}

fn format_int(conv: u8, signed_val: i64, unsigned_val: u64, flags: u8, width: usize, prec: i32) -> Vec<u8> {
    let mut sign: Vec<u8> = Vec::new();
    let mut digits: Vec<u8>;
    match conv {
        b'd' | b'i' => {
            let neg = signed_val < 0;
            let mag = signed_val.unsigned_abs();
            digits = mag.to_string().into_bytes();
            if neg {
                sign.push(b'-');
            } else if flags & PLUS != 0 {
                sign.push(b'+');
            } else if flags & SPACE != 0 {
                sign.push(b' ');
            }
        }
        b'u' => digits = unsigned_val.to_string().into_bytes(),
        b'o' => digits = format!("{:o}", unsigned_val).into_bytes(),
        b'x' => digits = format!("{:x}", unsigned_val).into_bytes(),
        _ => digits = format!("{:X}", unsigned_val).into_bytes(),
    }
    let is_zero = unsigned_val == 0 && signed_val == 0;
    if prec >= 0 {
        if prec == 0 && is_zero {
            digits.clear();
        }
        while (digits.len() as i32) < prec {
            digits.insert(0, b'0');
        }
    }
    if flags & ALT != 0 {
        match conv {
            b'o' => {
                if digits.first() != Some(&b'0') {
                    digits.insert(0, b'0');
                }
            }
            b'x' if !is_zero => sign.extend_from_slice(b"0x"),
            b'X' if !is_zero => sign.extend_from_slice(b"0X"),
            _ => {}
        }
    }
    finish_number(&sign, &digits, width, flags, prec < 0)
}

/// Exact decimal digits of |v| in scientific form with `prec` fraction
/// digits: returns (mantissa digits without point, exponent).
fn sci_digits(v: f64, prec: usize) -> (Vec<u8>, i32) {
    let s = format!("{:.*e}", prec, v.abs());
    let (m, e) = s.split_once('e').unwrap();
    let digits: Vec<u8> = m.bytes().filter(|c| c.is_ascii_digit()).collect();
    (digits, e.parse().unwrap())
}

fn exp_suffix(e: i32, upper: bool) -> Vec<u8> {
    let mut s = vec![if upper { b'E' } else { b'e' }];
    s.push(if e < 0 { b'-' } else { b'+' });
    let a = e.unsigned_abs();
    if a < 10 {
        s.push(b'0');
    }
    s.extend_from_slice(a.to_string().as_bytes());
    s
}

fn format_hex_float(v: f64, prec: i32, upper: bool, alt: bool) -> Vec<u8> {
    let bits = v.abs().to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    let mant = bits & ((1u64 << 52) - 1);
    let (mut lead, exp) = if raw_exp == 0 {
        (0u64, if mant == 0 { 0 } else { -1022 })
    } else {
        (1u64, raw_exp - 1023)
    };
    let mut hex: Vec<u8>;
    if prec >= 0 && prec < 13 {
        // Round to PREC hex digits, half to even; a carry bumps the
        // leading digit (glibc prints 0x2p+N rather than renormalizing).
        let p = prec as u32;
        let drop = (13 - p) * 4;
        let full = (lead << 52) | mant;
        let rem = full & ((1u64 << drop) - 1);
        let mut kept = full >> drop;
        let half = 1u64 << (drop - 1);
        if rem > half || (rem == half && (kept & 1) == 1) {
            kept += 1;
        }
        lead = kept >> (p * 4);
        let frac = kept & ((1u64 << (p * 4)) - 1);
        hex = if p == 0 { Vec::new() } else { format!("{:0width$x}", frac, width = p as usize).into_bytes() };
    } else {
        hex = format!("{:013x}", mant).into_bytes();
        if prec < 0 {
            while hex.last() == Some(&b'0') {
                hex.pop();
            }
        } else {
            while (hex.len() as i32) < prec {
                hex.push(b'0');
            }
        }
    }
    let mut out = b"0x".to_vec();
    out.extend_from_slice(lead.to_string().as_bytes());
    if !hex.is_empty() || alt {
        out.push(b'.');
    }
    out.extend_from_slice(&hex);
    out.push(b'p');
    out.push(if exp < 0 { b'-' } else { b'+' });
    out.extend_from_slice(exp.unsigned_abs().to_string().as_bytes());
    if upper {
        out = out.to_ascii_uppercase();
    }
    out
}

fn format_double(conv: u8, v: f64, flags: u8, width: usize, prec: i32) -> Vec<u8> {
    let upper = conv.is_ascii_uppercase();
    let mut sign: Vec<u8> = Vec::new();
    if v.is_sign_negative() {
        sign.push(b'-');
    } else if flags & PLUS != 0 {
        sign.push(b'+');
    } else if flags & SPACE != 0 {
        sign.push(b' ');
    }
    if !v.is_finite() {
        let body: &[u8] = match (v.is_nan(), upper) {
            (true, false) => b"nan",
            (true, true) => b"NAN",
            (false, false) => b"inf",
            (false, true) => b"INF",
        };
        return finish_number(&sign, body, width, flags, false);
    }
    let alt = flags & ALT != 0;
    let body: Vec<u8> = match conv.to_ascii_lowercase() {
        b'f' => {
            let p = if prec < 0 { 6 } else { prec as usize };
            let mut s = format!("{:.*}", p, v.abs()).into_bytes();
            if p == 0 && alt {
                s.push(b'.');
            }
            s
        }
        b'e' => {
            let p = if prec < 0 { 6 } else { prec as usize };
            let (d, e) = sci_digits(v, p);
            let mut s = vec![d[0]];
            if p > 0 || alt {
                s.push(b'.');
            }
            s.extend_from_slice(&d[1..]);
            s.extend_from_slice(&exp_suffix(e, upper));
            s
        }
        b'g' => {
            let p = if prec < 0 { 6 } else if prec == 0 { 1 } else { prec as usize };
            let x = if v == 0.0 { 0 } else { sci_digits(v, p - 1).1 };
            let mut s: Vec<u8>;
            if (x as i64) < p as i64 && x >= -4 {
                let fp = (p as i64 - 1 - x as i64) as usize;
                s = format!("{:.*}", fp, v.abs()).into_bytes();
                if !alt {
                    if s.contains(&b'.') {
                        while s.last() == Some(&b'0') {
                            s.pop();
                        }
                        if s.last() == Some(&b'.') {
                            s.pop();
                        }
                    }
                } else if !s.contains(&b'.') {
                    s.push(b'.');
                }
            } else {
                let (d, e) = sci_digits(v, p - 1);
                let mut m = vec![d[0]];
                m.push(b'.');
                m.extend_from_slice(&d[1..]);
                if !alt {
                    while m.last() == Some(&b'0') {
                        m.pop();
                    }
                    if m.last() == Some(&b'.') {
                        m.pop();
                    }
                }
                s = m;
                s.extend_from_slice(&exp_suffix(e, upper));
            }
            s
        }
        _ => {
            let h = format_hex_float(v, prec, upper, alt);
            // zero padding goes after the 0x prefix
            if flags & ZERO != 0 && flags & MINUS == 0 {
                let mut pre = sign.clone();
                pre.extend_from_slice(&h[..2]);
                return finish_number(&pre, &h[2..], width, flags, true);
            }
            h
        }
    };
    finish_number(&sign, &body, width, flags, true)
}

pub fn expand_format(m: &mut M4, obs: &mut Vec<u8>, args: &[Arg]) {
    let mut a = Args { args, pos: 0 };
    let f: Vec<u8> = a.next().map(|x| x.to_vec()).unwrap_or_default();
    let mut i = 0usize;
    let at = |i: usize| -> u8 { if i < f.len() { f[i] } else { 0 } };
    loop {
        let percent = match f[i..].iter().position(|&c| c == b'%') {
            None => {
                obs.extend_from_slice(&f[i..]);
                return;
            }
            Some(p) => i + p,
        };
        obs.extend_from_slice(&f[i..percent]);
        i = percent + 1;
        if at(i) == b'%' {
            obs.push(b'%');
            i += 1;
            continue;
        }
        let mut ok = [false; 128];
        for &c in b"aAcdeEfFgGiosuxX" {
            ok[c as usize] = true;
        }
        let mut flags: u8 = 0;
        loop {
            match at(i) {
                b'\'' => {
                    for &c in b"aAceEosxX" {
                        ok[c as usize] = false;
                    }
                    flags |= THOUSANDS;
                }
                b'+' => {
                    for &c in b"cosuxX" {
                        ok[c as usize] = false;
                    }
                    flags |= PLUS;
                }
                b' ' => {
                    for &c in b"cosuxX" {
                        ok[c as usize] = false;
                    }
                    flags |= SPACE;
                }
                b'0' => {
                    for &c in b"cs" {
                        ok[c as usize] = false;
                    }
                    flags |= ZERO;
                }
                b'#' => {
                    for &c in b"cdisu" {
                        ok[c as usize] = false;
                    }
                    flags |= ALT;
                }
                b'-' => flags |= MINUS,
                _ => break,
            }
            i += 1;
        }
        let mut width: i64 = 0;
        if at(i) == b'*' {
            width = match a.next() {
                Some(s) => arg_long(m, s, true) as i32 as i64,
                None => 0,
            };
            i += 1;
        } else {
            while at(i).is_ascii_digit() {
                width = (10 * width + (at(i) - b'0') as i64).min(1 << 30);
                i += 1;
            }
        }
        let mut prec: i64 = -1;
        if at(i) == b'.' {
            ok[b'c' as usize] = false;
            i += 1;
            if at(i) == b'*' {
                prec = match a.next() {
                    Some(s) => arg_long(m, s, true) as i32 as i64,
                    None => 0,
                };
                i += 1;
            } else {
                prec = 0;
                while at(i).is_ascii_digit() {
                    prec = (10 * prec + (at(i) - b'0') as i64).min(1 << 30);
                    i += 1;
                }
            }
        }
        let mut lflag = false;
        let mut hcount = 0;
        if at(i) == b'l' {
            lflag = true;
            i += 1;
            ok[b'c' as usize] = false;
            ok[b's' as usize] = false;
        } else if at(i) == b'h' {
            hcount = 1;
            i += 1;
            if at(i) == b'h' {
                hcount = 2;
                i += 1;
            }
            for &c in b"aAceEfFgGs" {
                ok[c as usize] = false;
            }
        }
        let c = at(i);
        i += 1;
        if c as usize >= 128 || !ok[c as usize] {
            let mut msg = b"Warning: unrecognized specifier in `".to_vec();
            msg.extend_from_slice(&f);
            msg.push(b'\'');
            m.warn(&msg);
            if c == 0 {
                i -= 1;
            }
            continue;
        }
        if width < 0 {
            flags |= MINUS;
            width = -width;
        }
        if width > 1_000_000 || prec > 1_000_000 {
            // The C library refuses results this large; m4 then drops the
            // piece, but the argument has still been converted.
            if let Some(s) = a.next() {
                match c {
                    b's' => {}
                    b'c' => {
                        arg_long(m, s, true);
                    }
                    b'd' | b'i' | b'o' | b'u' | b'x' | b'X' => {
                        arg_long(m, s, !lflag);
                    }
                    _ => {
                        arg_double(m, s);
                    }
                }
            }
            continue;
        }
        let w = width as usize;
        let p: i32 = if prec < 0 { -1 } else { prec as i32 };
        let mut piece: Vec<u8> = match c {
            b'c' => {
                let v = match a.next() {
                    Some(s) => arg_long(m, s, true) as i32,
                    None => 0,
                };
                pad(vec![v as u8], w, flags & MINUS != 0)
            }
            b's' => {
                let s: Vec<u8> = a.next().map(|x| x.to_vec()).unwrap_or_default();
                let mut s = s;
                if p >= 0 && (p as usize) < s.len() {
                    s.truncate(p as usize);
                }
                pad(s, w, flags & MINUS != 0)
            }
            b'd' | b'i' | b'o' | b'u' | b'x' | b'X' => {
                let raw = match a.next() {
                    Some(s) => arg_long(m, s, !lflag),
                    None => 0,
                };
                let (sv, uv): (i64, u64) = if lflag {
                    (raw, raw as u64)
                } else {
                    let iv = raw as i32;
                    match hcount {
                        1 => ((iv as i16) as i64, (iv as u16) as u64),
                        2 => ((iv as i8) as i64, (iv as u8) as u64),
                        _ => (iv as i64, (iv as u32) as u64),
                    }
                };
                let (sv, uv) = if c == b'd' || c == b'i' { (sv, if sv == 0 { 0 } else { 1 }) } else { (if uv == 0 { 0 } else { 1 }, uv) };
                format_int(c, sv, uv, flags, w, p)
            }
            _ => {
                let v = match a.next() {
                    Some(s) => arg_double(m, s),
                    None => 0.0,
                };
                format_double(c, v, flags, w, p)
            }
        };
        if let Some(z) = piece.iter().position(|&ch| ch == 0) {
            piece.truncate(z);
        }
        obs.extend_from_slice(&piece);
    }
}
