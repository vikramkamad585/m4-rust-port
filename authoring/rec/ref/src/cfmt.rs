// C printf-compatible %g and %f for doubles.

fn sci_digits(v: f64, prec: usize) -> (Vec<u8>, i32) {
    let s = format!("{:.*e}", prec, v.abs());
    let (m, e) = s.split_once('e').unwrap();
    (m.bytes().filter(|c| c.is_ascii_digit()).collect(), e.parse().unwrap())
}

fn special(v: f64) -> Option<String> {
    if v.is_nan() {
        return Some(if v.is_sign_negative() { "-nan".into() } else { "nan".into() });
    }
    if v.is_infinite() {
        return Some(if v < 0.0 { "-inf".into() } else { "inf".into() });
    }
    None
}

/// printf("%.*g", prec, v)
pub fn format_g(v: f64, prec: usize) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    let p = if prec == 0 { 1 } else { prec };
    let x = if v == 0.0 { 0 } else { sci_digits(v, p - 1).1 };
    let sign = if v.is_sign_negative() { "-" } else { "" };
    let body = if (x as i64) < p as i64 && x >= -4 {
        let fp = (p as i64 - 1 - x as i64) as usize;
        let mut s = format!("{:.*}", fp, v.abs());
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        s
    } else {
        let (d, e) = sci_digits(v, p - 1);
        let mut m = String::new();
        m.push(d[0] as char);
        m.push('.');
        for &c in &d[1..] {
            m.push(c as char);
        }
        while m.ends_with('0') {
            m.pop();
        }
        if m.ends_with('.') {
            m.pop();
        }
        format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.unsigned_abs())
    };
    format!("{}{}", sign, body)
}

/// printf("%f", v)
pub fn format_f(v: f64) -> String {
    if let Some(s) = special(v) {
        return s;
    }
    format!("{:.6}", v)
}
