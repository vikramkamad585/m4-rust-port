// The builtin macro table and the implementation of every builtin.

use crate::m4::{is_space, Arg, Def, M4};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Builtin {
    File,
    Line,
    Program,
    Builtin,
    Changecom,
    Changequote,
    Debugmode,
    Debugfile,
    Decr,
    Define,
    Defn,
    Divert,
    Divnum,
    Dnl,
    Dumpdef,
    Errprint,
    Esyscmd,
    Eval,
    Format,
    Ifdef,
    Ifelse,
    Include,
    Incr,
    Index,
    Indir,
    Len,
    M4exit,
    M4wrap,
    Maketemp,
    Mkstemp,
    Patsubst,
    Popdef,
    Pushdef,
    Regexp,
    Shift,
    Sinclude,
    Substr,
    Syscmd,
    Sysval,
    Traceoff,
    Traceon,
    Translit,
    Undefine,
    Undivert,
}

use Builtin as B;

pub const ALL: [Builtin; 44] = [
    B::File, B::Line, B::Program, B::Builtin, B::Changecom, B::Changequote, B::Debugmode,
    B::Debugfile, B::Decr, B::Define, B::Defn, B::Divert, B::Divnum, B::Dnl, B::Dumpdef,
    B::Errprint, B::Esyscmd, B::Eval, B::Format, B::Ifdef, B::Ifelse, B::Include, B::Incr,
    B::Index, B::Indir, B::Len, B::M4exit, B::M4wrap, B::Maketemp, B::Mkstemp, B::Patsubst,
    B::Popdef, B::Pushdef, B::Regexp, B::Shift, B::Sinclude, B::Substr, B::Syscmd, B::Sysval,
    B::Traceoff, B::Traceon, B::Translit, B::Undefine, B::Undivert,
];

impl Builtin {
    pub fn name(self) -> &'static str {
        match self {
            B::File => "__file__",
            B::Line => "__line__",
            B::Program => "__program__",
            B::Builtin => "builtin",
            B::Changecom => "changecom",
            B::Changequote => "changequote",
            B::Debugmode => "debugmode",
            B::Debugfile => "debugfile",
            B::Decr => "decr",
            B::Define => "define",
            B::Defn => "defn",
            B::Divert => "divert",
            B::Divnum => "divnum",
            B::Dnl => "dnl",
            B::Dumpdef => "dumpdef",
            B::Errprint => "errprint",
            B::Esyscmd => "esyscmd",
            B::Eval => "eval",
            B::Format => "format",
            B::Ifdef => "ifdef",
            B::Ifelse => "ifelse",
            B::Include => "include",
            B::Incr => "incr",
            B::Index => "index",
            B::Indir => "indir",
            B::Len => "len",
            B::M4exit => "m4exit",
            B::M4wrap => "m4wrap",
            B::Maketemp => "maketemp",
            B::Mkstemp => "mkstemp",
            B::Patsubst => "patsubst",
            B::Popdef => "popdef",
            B::Pushdef => "pushdef",
            B::Regexp => "regexp",
            B::Shift => "shift",
            B::Sinclude => "sinclude",
            B::Substr => "substr",
            B::Syscmd => "syscmd",
            B::Sysval => "sysval",
            B::Traceoff => "traceoff",
            B::Traceon => "traceon",
            B::Translit => "translit",
            B::Undefine => "undefine",
            B::Undivert => "undivert",
        }
    }

    pub fn groks_macro_args(self) -> bool {
        matches!(self, B::Builtin | B::Define | B::Indir | B::Pushdef)
    }

    pub fn blind(self) -> bool {
        matches!(
            self,
            B::Builtin | B::Decr | B::Define | B::Defn | B::Errprint | B::Esyscmd | B::Eval
                | B::Format | B::Ifdef | B::Ifelse | B::Include | B::Incr | B::Index | B::Indir
                | B::Len | B::M4wrap | B::Maketemp | B::Mkstemp | B::Patsubst | B::Popdef
                | B::Pushdef | B::Regexp | B::Shift | B::Sinclude | B::Substr | B::Syscmd
                | B::Translit | B::Undefine
        )
    }

    pub fn by_name(name: &[u8]) -> Option<Builtin> {
        ALL.iter().copied().find(|b| b.name().as_bytes() == name)
    }
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for p in parts {
        v.extend_from_slice(p);
    }
    v
}

/// C strtol(s, &end, 10): (value saturated to i64, bytes consumed, ERANGE).
pub fn strtol(s: &[u8]) -> (i64, usize, bool) {
    let mut i = 0;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let digits_start = i;
    let mut v: i128 = 0;
    let mut over = false;
    while i < s.len() && s[i].is_ascii_digit() {
        if !over {
            v = v * 10 + (s[i] - b'0') as i128;
            if v > (i64::MAX as i128) + 1 {
                over = true;
            }
        }
        i += 1;
    }
    if i == digits_start {
        return (0, 0, false);
    }
    let v = if neg { -v } else { v };
    if over || v > i64::MAX as i128 || v < i64::MIN as i128 {
        return (if neg { i64::MIN } else { i64::MAX }, i, true);
    }
    (v as i64, i, false)
}

pub fn ntoa(value: i32, radix: u32) -> Vec<u8> {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let neg = value < 0;
    let mut u: u32 = if neg { (value as u32).wrapping_neg() } else { value as u32 };
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(u % radix) as usize]);
        u /= radix;
        if u == 0 {
            break;
        }
    }
    if neg {
        out.push(b'-');
    }
    out.reverse();
    out
}

fn shipout_int(obs: &mut Vec<u8>, v: i32) {
    obs.extend_from_slice(v.to_string().as_bytes());
}

impl M4 {
    fn bad_argc(&mut self, name: &[u8], argc: usize, min: i32, max: i32) -> bool {
        let argc = argc as i32;
        let mut isbad = false;
        if min > 0 && argc < min {
            if !self.suppress_warnings {
                self.warn(&cat(&[b"Warning: too few arguments to builtin `", name, b"'"]));
            }
            isbad = true;
        } else if max > 0 && argc > max && !self.suppress_warnings {
            self.warn(&cat(&[b"Warning: excess arguments to builtin `", name, b"' ignored"]));
        }
        isbad
    }

    fn numeric_arg(&mut self, name: &[u8], arg: &[u8]) -> Option<i32> {
        if arg.is_empty() {
            self.warn(&cat(&[b"empty string treated as 0 in builtin `", name, b"'"]));
            return Some(0);
        }
        let (v, end, erange) = strtol(arg);
        if end != arg.len() {
            self.warn(&cat(&[b"non-numeric argument to builtin `", name, b"'"]));
            return None;
        }
        if is_space(arg[0]) {
            self.warn(&cat(&[b"leading whitespace ignored in builtin `", name, b"'"]));
        } else if erange {
            self.warn(&cat(&[b"numeric overflow detected in builtin `", name, b"'"]));
        }
        Some(v as i32)
    }

    pub fn call_builtin(&mut self, b: Builtin, args: &mut [Arg], obs: &mut Vec<u8>) {
        let argc = args.len();
        let name: Vec<u8> = args[0].text().to_vec();
        let arg = |i: usize| -> Vec<u8> {
            if i < args.len() {
                args[i].text().to_vec()
            } else {
                Vec::new()
            }
        };
        match b {
            B::Define | B::Pushdef => {
                let push = b == B::Pushdef;
                if self.bad_argc(&name, argc, 2, 3) {
                    return;
                }
                if !args[1].is_text() {
                    self.warn(&cat(&[b"Warning: ", &name, b": invalid macro name ignored"]));
                    return;
                }
                let mname = arg(1);
                if argc == 2 {
                    self.define(&mname, Def::Text(Vec::new()), push);
                    return;
                }
                match &args[2] {
                    Arg::Text(t) => {
                        let t = t.clone();
                        self.define(&mname, Def::Text(t), push)
                    }
                    Arg::Func(f) => {
                        let f = *f;
                        self.define(&mname, Def::Func(f), push)
                    }
                }
            }
            B::Undefine => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                for i in 1..argc {
                    self.undefine(&arg(i));
                }
            }
            B::Popdef => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                for i in 1..argc {
                    self.popdef(&arg(i));
                }
            }
            B::Ifdef => {
                if self.bad_argc(&name, argc, 3, 4) {
                    return;
                }
                if self.lookup(&arg(1)).is_some() {
                    obs.extend_from_slice(&arg(2));
                } else if argc >= 4 {
                    obs.extend_from_slice(&arg(3));
                }
            }
            B::Ifelse => {
                if argc == 2 {
                    return;
                }
                if self.bad_argc(&name, argc, 4, -1) {
                    return;
                }
                self.bad_argc(&name, (argc + 2) % 3, -1, 1);
                let mut base = 1;
                let mut n = argc - 1;
                loop {
                    let a0 = if base < argc { args[base].text() } else { b"" };
                    let a1 = if base + 1 < argc { args[base + 1].text() } else { b"" };
                    if a0 == a1 {
                        let r = if base + 2 < argc { args[base + 2].text().to_vec() } else { Vec::new() };
                        obs.extend_from_slice(&r);
                        return;
                    }
                    match n {
                        3 => return,
                        4 | 5 => {
                            let r = if base + 3 < argc { args[base + 3].text().to_vec() } else { Vec::new() };
                            obs.extend_from_slice(&r);
                            return;
                        }
                        _ => {
                            n -= 3;
                            base += 3;
                        }
                    }
                }
            }
            B::Dumpdef => {
                let mut names: Vec<Vec<u8>> = Vec::new();
                if argc == 1 {
                    names = self.symtab.keys().cloned().collect();
                } else {
                    for i in 1..argc {
                        let n = arg(i);
                        if self.lookup(&n).is_some() {
                            names.push(n);
                        } else {
                            self.warn(&cat(&[b"undefined macro `", &n, b"'"]));
                        }
                    }
                }
                names.sort();
                let mut out = Vec::new();
                for n in names {
                    out.extend_from_slice(&n);
                    out.extend_from_slice(b":\t");
                    match self.lookup(&n) {
                        Some(Def::Text(t)) => out.extend_from_slice(t),
                        Some(Def::Func(f)) => {
                            out.push(b'<');
                            out.extend_from_slice(f.name().as_bytes());
                            out.push(b'>');
                        }
                        None => {}
                    }
                    out.push(b'\n');
                }
                use std::io::Write;
                let _ = std::io::stderr().write_all(&out);
            }
            B::Builtin => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                if !args[1].is_text() {
                    self.warn(&cat(&[b"Warning: ", &name, b": invalid macro name ignored"]));
                    return;
                }
                let target = arg(1);
                match Builtin::by_name(&target) {
                    None => self.warn(&cat(&[b"undefined builtin `", &target, b"'"])),
                    Some(t) => {
                        if !t.groks_macro_args() {
                            for a in args.iter_mut().skip(2) {
                                if !a.is_text() {
                                    *a = Arg::Text(Vec::new());
                                }
                            }
                        }
                        self.call_builtin(t, &mut args[1..], obs);
                    }
                }
            }
            B::Indir => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                if !args[1].is_text() {
                    self.warn(&cat(&[b"Warning: ", &name, b": invalid macro name ignored"]));
                    return;
                }
                let target = arg(1);
                match self.lookup(&target).cloned() {
                    None => self.warn(&cat(&[b"undefined macro `", &target, b"'"])),
                    Some(d) => {
                        let groks = matches!(d, Def::Func(f) if f.groks_macro_args());
                        if !groks {
                            for a in args.iter_mut().skip(2) {
                                if !a.is_text() {
                                    *a = Arg::Text(Vec::new());
                                }
                            }
                        }
                        self.call_macro(&d, &mut args[1..], obs);
                    }
                }
            }
            B::Defn => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                for i in 1..argc {
                    let n = arg(i);
                    match self.lookup(&n).cloned() {
                        None => {}
                        Some(Def::Text(t)) => {
                            obs.extend_from_slice(&self.lquote);
                            obs.extend_from_slice(&t);
                            obs.extend_from_slice(&self.rquote);
                        }
                        Some(Def::Func(f)) => {
                            if argc != 2 {
                                self.warn(&cat(&[b"Warning: cannot concatenate builtin `", &n, b"'"]));
                            } else {
                                self.push_macro(f);
                            }
                        }
                    }
                }
            }
            B::Syscmd | B::Esyscmd => {
                // Shell escapes are not supported by this implementation.
                self.bad_argc(&name, argc, 2, 2);
            }
            B::Sysval => shipout_int(obs, 0),
            B::Eval => self.m4_eval(&name, args, obs),
            B::Incr | B::Decr => {
                if self.bad_argc(&name, argc, 2, 2) {
                    return;
                }
                if let Some(v) = self.numeric_arg(&name, &arg(1)) {
                    let w = if b == B::Incr { v.wrapping_add(1) } else { v.wrapping_sub(1) };
                    shipout_int(obs, w);
                }
            }
            B::Divert => {
                if self.bad_argc(&name, argc, 1, 2) {
                    return;
                }
                let mut i = 0;
                if argc >= 2 {
                    match self.numeric_arg(&name, &arg(1)) {
                        Some(v) => i = v,
                        None => return,
                    }
                }
                self.make_diversion(i);
            }
            B::Divnum => {
                if self.bad_argc(&name, argc, 1, 1) {
                    return;
                }
                shipout_int(obs, self.current_diversion);
            }
            B::Undivert => {
                if argc == 1 {
                    self.undivert_all();
                } else {
                    for i in 1..argc {
                        let a = arg(i);
                        let (v, end, _) = strtol(&a);
                        let numeric = end == a.len() && !(a.first().map_or(false, |&c| is_space(c)));
                        if numeric {
                            self.insert_diversion(v as i32);
                        } else {
                            match self.path_search(&a) {
                                Ok((data, _)) => {
                                    if self.current_diversion >= 0 {
                                        self.output_text(&data);
                                    }
                                }
                                Err(e) => {
                                    self.warn(&cat(&[b"cannot undivert `", &a, b"': ", e.as_bytes()]));
                                }
                            }
                        }
                    }
                }
            }
            B::Dnl => {
                if self.bad_argc(&name, argc, 1, 1) {
                    return;
                }
                self.skip_line();
            }
            B::Shift => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                let lq = self.lquote.clone();
                let rq = self.rquote.clone();
                for i in 2..argc {
                    if i > 2 {
                        obs.push(b',');
                    }
                    obs.extend_from_slice(&lq);
                    obs.extend_from_slice(args[i].text());
                    obs.extend_from_slice(&rq);
                }
            }
            B::Changequote => {
                if self.bad_argc(&name, argc, 1, 3) {
                    return;
                }
                let lq = if argc >= 2 { Some(arg(1)) } else { None };
                let rq = if argc >= 3 { Some(arg(2)) } else { None };
                match lq {
                    None => {
                        self.lquote = b"`".to_vec();
                        self.rquote = b"'".to_vec();
                    }
                    Some(l) => {
                        let r = match rq {
                            Some(r) if !(!l.is_empty() && r.is_empty()) => r,
                            _ => b"'".to_vec(),
                        };
                        self.lquote = l;
                        self.rquote = r;
                    }
                }
            }
            B::Changecom => {
                if self.bad_argc(&name, argc, 1, 3) {
                    return;
                }
                let bc = if argc >= 2 { Some(arg(1)) } else { None };
                let ec = if argc >= 3 { Some(arg(2)) } else { None };
                match bc {
                    None => {
                        self.bcomm = Vec::new();
                        self.ecomm = Vec::new();
                    }
                    Some(c) => {
                        let e = match ec {
                            Some(e) if !(!c.is_empty() && e.is_empty()) => e,
                            _ => b"\n".to_vec(),
                        };
                        self.bcomm = c;
                        self.ecomm = e;
                    }
                }
            }
            B::Include | B::Sinclude => {
                if self.bad_argc(&name, argc, 2, 2) {
                    return;
                }
                let f = arg(1);
                match self.path_search(&f) {
                    Ok((data, full)) => self.push_file(data, std::rc::Rc::from(full.as_slice())),
                    Err(e) => {
                        if b == B::Include {
                            self.warn(&cat(&[b"cannot open `", &f, b"': ", e.as_bytes()]));
                            self.retcode = 1;
                        }
                    }
                }
            }
            B::Maketemp | B::Mkstemp => {
                self.bad_argc(&name, argc, 2, 2);
            }
            B::Errprint => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                let mut out = Vec::new();
                self.dump_args(&mut out, args, b" ", false);
                if let Some(p) = out.iter().position(|&c| c == 0) {
                    out.truncate(p);
                }
                use std::io::Write;
                let _ = std::io::stderr().write_all(&out);
            }
            B::File => {
                if self.bad_argc(&name, argc, 1, 1) {
                    return;
                }
                obs.extend_from_slice(&self.lquote);
                obs.extend_from_slice(&self.current_file);
                obs.extend_from_slice(&self.rquote);
            }
            B::Line => {
                if self.bad_argc(&name, argc, 1, 1) {
                    return;
                }
                shipout_int(obs, self.current_line);
            }
            B::Program => {
                if self.bad_argc(&name, argc, 1, 1) {
                    return;
                }
                obs.extend_from_slice(&self.lquote);
                obs.extend_from_slice(b"m4");
                obs.extend_from_slice(&self.rquote);
            }
            B::M4exit => {
                self.bad_argc(&name, argc, 1, 2);
                let mut code = 0;
                if argc >= 2 {
                    code = match self.numeric_arg(&name, &arg(1)) {
                        Some(v) => v,
                        None => 1,
                    };
                }
                if !(0..=255).contains(&code) {
                    self.warn(format!("exit status out of range: `{}'", code).as_bytes());
                    code = 1;
                }
                if code == 0 && self.retcode != 0 {
                    code = self.retcode;
                }
                self.exit(code);
            }
            B::M4wrap => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                let mut out = Vec::new();
                self.dump_args(&mut out, args, b" ", false);
                self.push_wrapup(out);
            }
            B::Traceon | B::Traceoff => {}
            B::Debugmode => {
                self.bad_argc(&name, argc, 1, 2);
            }
            B::Debugfile => {
                self.bad_argc(&name, argc, 1, 2);
            }
            B::Len => {
                if self.bad_argc(&name, argc, 2, 2) {
                    return;
                }
                shipout_int(obs, arg(1).len() as i32);
            }
            B::Index => {
                if self.bad_argc(&name, argc, 3, 3) {
                    if argc == 2 {
                        shipout_int(obs, 0);
                    }
                    return;
                }
                let hay = arg(1);
                let needle = arg(2);
                let r = find(&hay, &needle).map(|p| p as i32).unwrap_or(-1);
                shipout_int(obs, r);
            }
            B::Substr => {
                if self.bad_argc(&name, argc, 3, 4) {
                    if argc == 2 {
                        obs.extend_from_slice(&arg(1));
                    }
                    return;
                }
                let s = arg(1);
                let avail = s.len() as i32;
                let mut length = avail;
                let start = match self.numeric_arg(&name, &arg(2)) {
                    Some(v) => v,
                    None => return,
                };
                if argc >= 4 {
                    match self.numeric_arg(&name, &arg(3)) {
                        Some(v) => length = v,
                        None => return,
                    }
                }
                if start < 0 || length <= 0 || start >= avail {
                    return;
                }
                if start as i64 + length as i64 > avail as i64 {
                    length = avail - start;
                }
                obs.extend_from_slice(&s[start as usize..(start + length) as usize]);
            }
            B::Translit => self.m4_translit(&name, args, obs),
            B::Format => {
                if self.bad_argc(&name, argc, 2, -1) {
                    return;
                }
                crate::format::expand_format(self, obs, &args[1..]);
            }
            B::Regexp => self.m4_regexp(&name, args, obs),
            B::Patsubst => self.m4_patsubst(&name, args, obs),
        }
    }

    fn m4_eval(&mut self, name: &[u8], args: &[Arg], obs: &mut Vec<u8>) {
        let argc = args.len();
        if self.bad_argc(name, argc, 2, 4) {
            return;
        }
        let a = |i: usize| -> Vec<u8> { if i < argc { args[i].text().to_vec() } else { Vec::new() } };
        let mut radix = 10;
        let mut min = 1;
        if !a(2).is_empty() {
            match self.numeric_arg(name, &a(2)) {
                Some(v) => radix = v,
                None => return,
            }
        }
        if !(1..=36).contains(&radix) {
            self.warn(&cat(&[format!("radix {} in builtin `", radix).as_bytes(), name, b"' out of range"]));
            return;
        }
        if argc >= 4 {
            match self.numeric_arg(name, &a(3)) {
                Some(v) => min = v,
                None => return,
            }
        }
        if min < 0 {
            self.warn(&cat(&[b"negative width to builtin `", name, b"'"]));
            return;
        }
        let expr = a(1);
        let mut value: i32 = 0;
        if expr.is_empty() {
            self.warn(&cat(&[b"empty string treated as 0 in builtin `", name, b"'"]));
        } else {
            let r = crate::eval::evaluate(&expr);
            for _ in 0..r.assign_warnings {
                self.warn(b"Warning: recommend ==, not =, for equality operator");
            }
            match r.value {
                Ok(v) => value = v,
                Err((msg, fatal)) => {
                    let mut m = msg.as_bytes().to_vec();
                    m.extend_from_slice(&expr);
                    self.warn(&m);
                    if fatal {
                        self.retcode = 1;
                    }
                    return;
                }
            }
        }
        if radix == 1 {
            if value < 0 {
                obs.push(b'-');
                value = value.wrapping_neg();
            }
            let mut m = min;
            while m.wrapping_sub(value) > 0 {
                obs.push(b'0');
                m -= 1;
            }
            let mut v = value as u32;
            while v != 0 {
                obs.push(b'1');
                v -= 1;
            }
            return;
        }
        let s = ntoa(value, radix as u32);
        let digits: &[u8] = if s[0] == b'-' {
            obs.push(b'-');
            &s[1..]
        } else {
            &s
        };
        let mut pad = min - digits.len() as i32;
        while pad > 0 {
            obs.push(b'0');
            pad -= 1;
        }
        obs.extend_from_slice(digits);
    }

    fn m4_translit(&mut self, name: &[u8], args: &[Arg], obs: &mut Vec<u8>) {
        let argc = args.len();
        let data: Vec<u8> = if argc > 1 { args[1].text().to_vec() } else { Vec::new() };
        let from: Vec<u8> = if argc > 2 { args[2].text().to_vec() } else { Vec::new() };
        if self.bad_argc(name, argc, 3, 4) || data.is_empty() || from.is_empty() {
            if argc >= 2 {
                obs.extend_from_slice(&data);
            }
            return;
        }
        let mut to: Vec<u8> = if argc > 3 { args[3].text().to_vec() } else { Vec::new() };
        if to.contains(&b'-') {
            to = expand_ranges(&to);
        }
        if from.len() <= 2 {
            let f0 = from[0];
            let f1 = from.get(1).copied();
            for &c in &data {
                if c == f0 {
                    if !to.is_empty() {
                        obs.push(to[0]);
                    }
                } else if Some(c) == f1 {
                    if to.len() >= 2 {
                        obs.push(to[1]);
                    }
                } else {
                    obs.push(c);
                }
            }
            return;
        }
        let from = if from.contains(&b'-') { expand_ranges(&from) } else { from };
        let mut map = [0u8; 256];
        let mut found = [false; 256];
        let mut ti = 0;
        for &c in &from {
            if !found[c as usize] {
                found[c as usize] = true;
                map[c as usize] = if ti < to.len() { to[ti] } else { 0 };
            }
            if ti < to.len() {
                ti += 1;
            }
        }
        for &c in &data {
            if !found[c as usize] {
                obs.push(c);
            } else if map[c as usize] != 0 {
                obs.push(map[c as usize]);
            }
        }
    }

    fn m4_regexp(&mut self, name: &[u8], args: &[Arg], obs: &mut Vec<u8>) {
        let argc = args.len();
        if self.bad_argc(name, argc, 3, 4) {
            if argc == 2 {
                shipout_int(obs, 0);
            }
            return;
        }
        let victim = args[1].text().to_vec();
        let pat = args[2].text().to_vec();
        let re = match crate::regex::Regex::compile(&pat) {
            Ok(r) => r,
            Err(msg) => {
                self.warn(&cat(&[b"bad regular expression: `", &pat, b"': ", msg.as_bytes()]));
                return;
            }
        };
        let m = re.search(&victim, 0, argc != 3);
        if argc == 3 {
            shipout_int(obs, m.as_ref().map(|x| x.start as i32).unwrap_or(-1));
        } else if let Some(m) = m {
            let repl = args[3].text().to_vec();
            self.substitute(obs, &victim, &repl, &m);
        }
    }

    fn m4_patsubst(&mut self, name: &[u8], args: &[Arg], obs: &mut Vec<u8>) {
        let argc = args.len();
        if self.bad_argc(name, argc, 3, 4) {
            if argc == 2 {
                obs.extend_from_slice(args[1].text());
            }
            return;
        }
        let pat = args[2].text().to_vec();
        let re = match crate::regex::Regex::compile(&pat) {
            Ok(r) => r,
            Err(msg) => {
                self.warn(&cat(&[b"bad regular expression `", &pat, b"': ", msg.as_bytes()]));
                return;
            }
        };
        let victim = args[1].text().to_vec();
        let repl: Vec<u8> = if argc > 3 { args[3].text().to_vec() } else { Vec::new() };
        let length = victim.len();
        let mut offset = 0;
        while offset <= length {
            let m = match re.search(&victim, offset, true) {
                None => {
                    if offset < length {
                        obs.extend_from_slice(&victim[offset..]);
                    }
                    break;
                }
                Some(m) => m,
            };
            if m.start > offset {
                obs.extend_from_slice(&victim[offset..m.start]);
            }
            self.substitute(obs, &victim, &repl, &m);
            offset = m.end;
            if m.start == m.end {
                if offset < length {
                    obs.push(victim[offset]);
                } else {
                    obs.push(0);
                }
                offset += 1;
            }
        }
    }

    fn substitute(&mut self, obs: &mut Vec<u8>, victim: &[u8], repl: &[u8], m: &crate::regex::Match) {
        let mut i = 0;
        while i < repl.len() {
            let c = repl[i];
            if c != b'\\' {
                obs.push(c);
                i += 1;
                continue;
            }
            i += 1;
            if i >= repl.len() {
                self.warn(b"Warning: trailing \\ ignored in replacement");
                return;
            }
            let ch = repl[i];
            match ch {
                b'0' | b'&' => {
                    if ch == b'0' && !self.substitute_warned {
                        self.warn(b"Warning: \\0 will disappear, use \\& instead in replacements");
                        self.substitute_warned = true;
                    }
                    obs.extend_from_slice(&victim[m.start..m.end]);
                    i += 1;
                }
                b'1'..=b'9' => {
                    let ind = (ch - b'0') as usize;
                    if ind > m.groups.len() {
                        self.warn(format!("Warning: sub-expression {} not present", ind).as_bytes());
                    } else if let Some((s, e)) = m.groups[ind - 1] {
                        if e > 0 {
                            obs.extend_from_slice(&victim[s..e]);
                        }
                    }
                    i += 1;
                }
                _ => {
                    obs.push(ch);
                    i += 1;
                }
            }
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn expand_ranges(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut from: u8 = 0;
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'-' && from != 0 {
            i += 1;
            if i >= s.len() {
                out.push(b'-');
                return out;
            }
            let to = s[i];
            if from <= to {
                let mut f = from;
                while f < to {
                    f += 1;
                    out.push(f);
                }
            } else {
                let mut f = from as i32;
                loop {
                    f -= 1;
                    if f < to as i32 {
                        break;
                    }
                    out.push(f as u8);
                }
            }
        } else {
            out.push(s[i]);
        }
        from = s[i];
        i += 1;
    }
    out
}
