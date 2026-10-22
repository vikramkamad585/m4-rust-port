// recrs: GNU recutils 1.9 compatible recsel, recinf, recfmt and rec2csv.
//
//   recrs TOOL [OPTION]... [FILE]...

mod cfmt;
mod ere;
mod fex;
mod query;
mod rec;
mod sex;
mod types;

use rec::{Db, Mode, Parser, RElem, Rset, Writer};
use std::io::{Read, Write};

struct Out {
    buf: Vec<u8>,
}

impl Out {
    fn flush(&mut self) {
        let mut o = std::io::stdout();
        let _ = o.write_all(&self.buf);
        let _ = o.flush();
        self.buf.clear();
    }
}

fn die(out: &mut Out, prog: &str, msg: &str) -> ! {
    out.flush();
    eprint!("{}: error: {}", prog, msg);
    std::process::exit(1);
}

fn exit(out: &mut Out, code: i32) -> ! {
    out.flush();
    std::process::exit(code);
}

// ------------------------------------------------------------- getopt_long

#[derive(Clone)]
struct LongOpt {
    name: &'static str,
    has_arg: bool,
    val: char,
}

enum Opt {
    Opt(char, Option<String>),
    Err,
}

/// GNU getopt_long with argument permutation; returns options in order and
/// the operands.
fn getopt(prog: &str, args: &[String], short: &str, longs: &[LongOpt]) -> (Vec<Opt>, Vec<String>) {
    let mut opts = Vec::new();
    let mut operands = Vec::new();
    let sb = short.as_bytes();
    let takes_arg = |c: u8| -> Option<bool> {
        let p = sb.iter().position(|&x| x == c && x != b':')?;
        Some(sb.get(p + 1) == Some(&b':'))
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if a == "--" {
            operands.extend(args[i..].iter().cloned());
            break;
        }
        if a.len() > 2 && a.starts_with("--") {
            let body = &a[2..];
            let (name, val) = match body.find('=') {
                Some(p) => (&body[..p], Some(body[p + 1..].to_string())),
                None => (body, None),
            };
            let exact: Vec<&LongOpt> = longs.iter().filter(|o| o.name == name).collect();
            let cands: Vec<&LongOpt> = if !exact.is_empty() {
                exact
            } else {
                longs.iter().filter(|o| o.name.starts_with(name)).collect()
            };
            if cands.is_empty() {
                eprint!("{}: unrecognized option '--{}'\n", prog, name);
                opts.push(Opt::Err);
                return (opts, operands);
            }
            let distinct = cands.iter().any(|c| c.val != cands[0].val || c.has_arg != cands[0].has_arg);
            if cands.len() > 1 && distinct {
                let mut msg = format!("{}: option '--{}' is ambiguous; possibilities:", prog, name);
                for c in &cands {
                    msg.push_str(&format!(" '--{}'", c.name));
                }
                eprint!("{}\n", msg);
                opts.push(Opt::Err);
                return (opts, operands);
            }
            let o = cands[0];
            if o.has_arg {
                let v = match val {
                    Some(v) => v,
                    None => {
                        if i < args.len() {
                            i += 1;
                            args[i - 1].clone()
                        } else {
                            eprint!("{}: option '--{}' requires an argument\n", prog, o.name);
                            opts.push(Opt::Err);
                            return (opts, operands);
                        }
                    }
                };
                opts.push(Opt::Opt(o.val, Some(v)));
            } else {
                if val.is_some() {
                    eprint!("{}: option '--{}' doesn't allow an argument\n", prog, o.name);
                    opts.push(Opt::Err);
                    return (opts, operands);
                }
                opts.push(Opt::Opt(o.val, None));
            }
            continue;
        }
        if a.len() > 1 && a.starts_with('-') {
            let b = a.as_bytes();
            let mut j = 1;
            while j < b.len() {
                let c = b[j];
                j += 1;
                match takes_arg(c) {
                    None => {
                        eprint!("{}: invalid option -- '{}'\n", prog, c as char);
                        opts.push(Opt::Err);
                        return (opts, operands);
                    }
                    Some(false) => opts.push(Opt::Opt(c as char, None)),
                    Some(true) => {
                        let v = if j < b.len() {
                            let v = a[j..].to_string();
                            j = b.len();
                            v
                        } else if i < args.len() {
                            i += 1;
                            args[i - 1].clone()
                        } else {
                            eprint!("{}: option requires an argument -- '{}'\n", prog, c as char);
                            opts.push(Opt::Err);
                            return (opts, operands);
                        };
                        opts.push(Opt::Opt(c as char, Some(v)));
                    }
                }
            }
            continue;
        }
        operands.push(a.clone());
    }
    (opts, operands)
}

fn lo(name: &'static str, has_arg: bool, val: char) -> LongOpt {
    LongOpt { name, has_arg, val }
}

// --------------------------------------------------------------- database

fn read_input(name: &str) -> Option<Vec<u8>> {
    if name == "stdin" {
        let mut v = Vec::new();
        std::io::stdin().read_to_end(&mut v).ok()?;
        Some(v)
    } else {
        let p = std::path::Path::new(name);
        if p.is_dir() {
            return Some(Vec::new());
        }
        std::fs::read(p).ok()
    }
}

/// recutl_parse_db_from_file: returns false on a parse error (after
/// printing it).
fn parse_into(out: &mut Out, prog: &str, data: &[u8], file_name: &str, db: &mut Db) -> bool {
    let mut parser = Parser::new(data);
    while let Some(rset) = parser.parse_rset() {
        let t = rset.rtype();
        if let Some(i) = db.get_by_type(t.as_deref()) {
            match t {
                Some(t) => die(out, prog, &format!("duplicated record set '{}' from {}.\n", t, file_name)),
                None => {
                    let elems = rset.elems;
                    db.rsets[i].elems.extend(elems);
                    return true;
                }
            }
        }
        db.rsets.push(rset);
    }
    if parser.error != rec::PErr::None {
        out.flush();
        eprint!("{}: {}: error: {}\n", file_name, parser.line, parser.error_string());
        return false;
    }
    true
}

fn build_db(out: &mut Out, prog: &str, files: &[String]) -> Option<Db> {
    let mut db = Db::default();
    if files.is_empty() {
        let data = read_input("stdin").unwrap_or_default();
        if !parse_into(out, prog, &data, "stdin", &mut db) {
            return None;
        }
        return Some(db);
    }
    for f in files {
        let data = match read_input(f) {
            Some(d) => d,
            None => die(out, prog, &format!("cannot read file {}\n", f)),
        };
        if !parse_into(out, prog, &data, f, &mut db) {
            return None;
        }
    }
    Some(db)
}

fn field_name_p(s: &str) -> bool {
    let b = s.as_bytes();
    matches!(types::fname_at(b, 0), Some(e) if e == b.len())
}

fn index_list(s: &str) -> Option<Vec<(usize, Option<usize>)>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let (a, b) = match part.find('-') {
            Some(p) => (&part[..p], Some(&part[p + 1..])),
            None => (part, None),
        };
        if a.is_empty() || !a.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if let Some(b) = b {
            if b.is_empty() || !b.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
        }
        let parse = |x: &str| -> usize {
            let v: i128 = x.parse::<i128>().unwrap_or(i128::from(i64::MAX)).min(i128::from(i64::MAX));
            v as i64 as usize
        };
        out.push((parse(a), b.map(parse)));
    }
    Some(out)
}

// ------------------------------------------------------------------ recsel

fn recsel(args: &[String]) {
    let prog = "recsel";
    let mut out = Out { buf: Vec::new() };
    let longs = vec![
        lo("help", false, '\u{1}'), lo("version", false, '\u{2}'),
        lo("type", true, 't'), lo("expression", true, 'e'), lo("quick", true, 'q'),
        lo("number", true, 'n'), lo("case-insensitive", false, 'i'), lo("random", true, 'm'),
        lo("print", true, 'p'), lo("print-values", true, 'P'), lo("print-row", true, 'R'),
        lo("collapse", false, 'C'), lo("count", false, 'c'), lo("include-descriptors", false, 'd'),
        lo("print-sexps", false, '\u{3}'), lo("sort", true, 'S'), lo("password", true, 's'),
        lo("uniq", false, 'U'), lo("group-by", true, 'G'), lo("join", true, 'j'),
    ];
    let (opts, files) = getopt(prog, args, "it:e:n:q:m:s:S:Cdcp:P:R:UG:j:", &longs);
    let mut rtype: Option<String> = None;
    let mut sex_str: Option<String> = None;
    let mut sexc: Option<sex::Sex> = None;
    let mut icase = false;
    let mut quick: Option<String> = None;
    let mut index: Option<Vec<(usize, Option<usize>)>> = None;
    let mut random = false;
    let mut descriptors = false;
    let mut uniq = false;
    let mut sort_by: Option<Vec<String>> = None;
    let mut join: Option<String> = None;
    let mut group_by: Option<Vec<String>> = None;
    let mut fex_str: Option<String> = None;
    let mut fexv: Option<Vec<fex::FexElem>> = None;
    let mut mode = Mode::Normal;
    let mut collapse = false;
    let mut count = false;
    for o in opts {
        let (c, arg) = match o {
            Opt::Err => exit(&mut out, 1),
            Opt::Opt(c, a) => (c, a.unwrap_or_default()),
        };
        match c {
            't' => {
                if !field_name_p(&arg) {
                    die(&mut out, prog, &format!("invalid record type {}\n", arg));
                }
                rtype = Some(arg);
            }
            'e' => {
                if index.is_some() {
                    die(&mut out, prog, "cannot specify -e and also -n\n");
                }
                if quick.is_some() {
                    die(&mut out, prog, "cannot specify -e and also -q\n");
                }
                match sex::Sex::compile(&arg, icase) {
                    Some(s) => sexc = Some(s),
                    None => die(&mut out, prog, "invalid selection expression\n"),
                }
                sex_str = Some(arg);
            }
            'n' => {
                if sexc.is_some() {
                    die(&mut out, prog, "cannot specify -n and also -e\n");
                }
                if quick.is_some() {
                    die(&mut out, prog, "cannot specify -n and also -q\n");
                }
                if index.is_some() {
                    die(&mut out, prog, "please specify just one -n\n");
                }
                match index_list(&arg) {
                    Some(v) => index = Some(v),
                    None => die(&mut out, prog, "invalid list of indexes in -n\n"),
                }
            }
            'm' => {
                if sexc.is_some() {
                    die(&mut out, prog, "cannot specify -m and also -e\n");
                }
                if quick.is_some() {
                    die(&mut out, prog, "cannot specify -m and also -q\n");
                }
                if index.is_some() {
                    die(&mut out, prog, "cannot specify -m and also -n\n");
                }
                if random {
                    die(&mut out, prog, "please specify just one -m\n");
                }
                random = true;
            }
            'q' => {
                if sexc.is_some() || index.is_some() {
                    die(&mut out, prog, "cannot specify -e and also -n\n");
                }
                quick = Some(arg);
            }
            'i' => icase = true,
            'd' => descriptors = true,
            '\u{3}' => mode = Mode::Sexp,
            'U' => uniq = true,
            's' => {}
            'S' => {
                if sort_by.is_some() {
                    die(&mut out, prog, "only one field list can be specified as a sorting criteria.\n");
                }
                if !fex::check_csv(&arg) {
                    die(&mut out, prog, "invalid field names in -S.\n");
                }
                sort_by = fex::parse_simple(&arg, &[',']);
            }
            'j' => {
                if join.is_some() {
                    die(&mut out, prog, "only one field can be specified as join criteria.\n");
                }
                if !field_name_p(&arg) {
                    die(&mut out, prog, "please specify a correct field name to -j|--join.\n");
                }
                join = Some(arg);
            }
            'G' => {
                if group_by.is_some() {
                    die(&mut out, prog, "only one field list can be specified as a grouping criteria.\n");
                }
                if !fex::check_csv(&arg) {
                    die(&mut out, prog, "invalid field names in -G.\n");
                }
                group_by = fex::parse_simple(&arg, &[',']);
            }
            'p' | 'P' | 'R' => {
                if count {
                    die(&mut out, prog, "cannot specify -[pPR] and also -c.\n");
                }
                if c == 'P' {
                    mode = Mode::Values;
                }
                if c == 'R' {
                    mode = Mode::ValuesRow;
                }
                if !fex::check_subscripts(&arg) {
                    die(&mut out, prog, &format!("invalid list of fields in -{}\n", c));
                }
                let f = match fex::parse_subscripts(&arg) {
                    Some(f) => f,
                    None => die(&mut out, prog, "internal error creating the field expression.\n"),
                };
                for e in &f {
                    if let Some(fname) = &e.function {
                        if !query::known_aggregate(fname) {
                            die(&mut out, prog, &format!("invalid aggregate function '{}'\n", fname));
                        }
                    }
                }
                fex_str = Some(arg);
                fexv = Some(f);
            }
            'C' => collapse = true,
            'c' => {
                if fex_str.is_some() {
                    die(&mut out, prog, "cannot specify -c and also -p.\n");
                }
                count = true;
            }
            _ => exit(&mut out, 1),
        }
    }
    let _ = sex_str;
    if rtype.is_none() && join.is_some() {
        die(&mut out, prog, "joins can only be used when a named record set is selected.\n");
    }
    let db = match build_db(&mut out, prog, &files) {
        Some(db) => db,
        None => exit(&mut out, 1),
    };
    if rtype.is_none() && db.rsets.len() > 1 {
        die(&mut out, prog, "several record types found.  Please use -t to specify one.\n");
    }
    if random {
        // Random selection is not reproducible; select nothing extra.
    }
    let q = query::Query {
        rtype,
        join,
        index,
        sex: sexc.as_ref(),
        quick,
        fex: fexv,
        group_by,
        sort_by,
        icase,
        descriptor: descriptors,
        uniq,
    };
    let res = match query::run(&db, &q) {
        Some(r) => r,
        None => die(&mut out, prog, "out of memory\n"),
    };
    if count {
        out.buf.extend_from_slice(format!("{}\n", res.num_records()).as_bytes());
    } else {
        let mut w = Writer::new(mode);
        w.collapse = collapse;
        w.skip_comments = true;
        w.write_rset(&res);
        out.buf.extend_from_slice(&w.out);
    }
    exit(&mut out, 0);
}

// ------------------------------------------------------------------ recinf

fn recinf(args: &[String]) {
    let prog = "recinf";
    let mut out = Out { buf: Vec::new() };
    let longs = vec![
        lo("help", false, '\u{1}'), lo("version", false, '\u{2}'), lo("descriptor", false, 'd'),
        lo("names-only", false, 'n'), lo("type", true, 't'), lo("print-sexps", false, 'S'),
    ];
    let (opts, files) = getopt(prog, args, "Sdnt:", &longs);
    let mut mode = Mode::Normal;
    let mut descriptor = false;
    let mut names_only = false;
    let mut rtype: Option<String> = None;
    for o in opts {
        match o {
            Opt::Err => exit(&mut out, 1),
            Opt::Opt('S', _) => mode = Mode::Sexp,
            Opt::Opt('d', _) => descriptor = true,
            Opt::Opt('n', _) => names_only = true,
            Opt::Opt('t', a) => rtype = a,
            _ => exit(&mut out, 1),
        }
    }
    let inputs: Vec<String> = if files.is_empty() { vec!["stdin".to_string()] } else { files };
    for f in inputs {
        let data = match read_input(&f) {
            Some(d) => d,
            None => {
                out.buf.extend_from_slice(format!("error: cannot read file {}\n", f).as_bytes());
                exit(&mut out, 1);
            }
        };
        let mut parser = Parser::new(&data);
        let mut db = Db::default();
        while let Some(rs) = parser.parse_rset() {
            db.rsets.push(rs);
        }
        let ok = parser.error == rec::PErr::None;
        if ok {
            let size = db.rsets.len();
            for (position, rset) in db.rsets.iter().enumerate() {
                let d = rset.descriptor.as_ref();
                if let (Some(t), Some(_)) = (&rtype, d) {
                    if rset.rtype().as_deref() != Some(t.as_str()) {
                        continue;
                    }
                }
                if descriptor {
                    match d {
                        Some(d) => {
                            let mut w = Writer::new(mode);
                            w.write_record(d);
                            out.buf.extend_from_slice(&w.out);
                            out.buf.push(b'\n');
                        }
                        None => {
                            if mode == Mode::Normal {
                                out.buf.extend_from_slice(b"unknown\n");
                            }
                        }
                    }
                    if position + 1 < size {
                        out.buf.push(b'\n');
                    }
                } else {
                    match d {
                        Some(_) => {
                            if !names_only {
                                out.buf.extend_from_slice(format!("{} ", rset.num_records()).as_bytes());
                            }
                            out.buf.extend_from_slice(format!("{}\n", rset.rtype().unwrap_or_default()).as_bytes());
                        }
                        None => {
                            if !names_only {
                                out.buf.extend_from_slice(format!("{}\n", rset.num_records()).as_bytes());
                            }
                        }
                    }
                }
            }
        } else {
            out.flush();
            eprint!("{}: {}: error: {}\n", f, parser.line, parser.error_string());
            exit(&mut out, 1);
        }
    }
    exit(&mut out, 0);
}

// ----------------------------------------------------------------- rec2csv

fn rec2csv(args: &[String]) {
    let prog = "rec2csv";
    let mut out = Out { buf: Vec::new() };
    let longs = vec![
        lo("help", false, '\u{1}'), lo("version", false, '\u{2}'), lo("delim", true, 'd'),
        lo("type", true, 't'), lo("sort", true, 'S'),
    ];
    let (opts, files) = getopt(prog, args, "t:S:d:", &longs);
    let mut delim = b',';
    let mut rtype: Option<String> = None;
    let mut sort_by: Option<Vec<String>> = None;
    for o in opts {
        match o {
            Opt::Err => exit(&mut out, 1),
            Opt::Opt('d', a) => delim = a.unwrap_or_default().bytes().next().unwrap_or(0),
            Opt::Opt('t', a) => rtype = a,
            Opt::Opt('S', a) => {
                let a = a.unwrap_or_default();
                if sort_by.is_some() {
                    die(&mut out, prog, "only one list of fields can be specified as a sorting criteria.\n");
                }
                if !fex::check_csv(&a) {
                    die(&mut out, prog, "invalid field name list in -S.\n");
                }
                sort_by = fex::parse_simple(&a, &[',']);
            }
            _ => exit(&mut out, 1),
        }
    }
    let db = match build_db(&mut out, prog, &files) {
        Some(db) => db,
        None => exit(&mut out, 1),
    };
    let size = db.rsets.len();
    for rset in &db.rsets {
        let t = rset.rtype();
        let selected = match &rtype {
            Some(want) => t.as_deref() == Some(want.as_str()),
            None => t.is_none() || size == 1,
        };
        if !selected {
            continue;
        }
        let mut rs: Rset = rset.clone();
        let mut props = types::Props::from_rset(&rs);
        if let Some(s) = &sort_by {
            props.sort = Some(s.clone());
        }
        if let Some(order) = props.sort.clone() {
            query::sort_rset(&mut rs, &props, &order);
        }
        // determine the columns: (name, index)
        let mut cols: Vec<(String, usize)> = Vec::new();
        for r in rs.records() {
            let mut seen: Vec<(String, usize)> = Vec::new();
            for f in r.fields() {
                let idx = seen.iter().filter(|(n, _)| *n == f.name).count();
                seen.push((f.name.clone(), idx));
                if !cols.iter().any(|(n, i)| *n == f.name && *i == idx) {
                    cols.push((f.name.clone(), idx));
                }
            }
        }
        let csv = |s: &str, o: &mut Vec<u8>| {
            o.push(b'"');
            for b in s.bytes() {
                if b == b'"' {
                    o.push(b'"');
                }
                o.push(b);
            }
            o.push(b'"');
        };
        for (k, (n, i)) in cols.iter().enumerate() {
            if k != 0 {
                out.buf.push(delim);
            }
            let name = if *i != 0 { format!("{}_{}", n, i + 1) } else { n.clone() };
            csv(&name, &mut out.buf);
        }
        out.buf.push(b'\n');
        for r in rs.records() {
            for (k, (n, i)) in cols.iter().enumerate() {
                if k != 0 {
                    out.buf.push(delim);
                }
                if let Some(f) = r.get(n, *i) {
                    csv(&f.value, &mut out.buf);
                }
            }
            out.buf.push(b'\n');
        }
    }
    exit(&mut out, 0);
}

// ------------------------------------------------------------------ recfmt

fn recfmt(args: &[String]) {
    let prog = "recfmt";
    let mut out = Out { buf: Vec::new() };
    let longs = vec![lo("help", false, '\u{1}'), lo("version", false, '\u{2}'), lo("file", true, 'f')];
    let (opts, operands) = getopt(prog, args, "f:", &longs);
    let mut template: Option<String> = None;
    for o in opts {
        match o {
            Opt::Err => exit(&mut out, 1),
            Opt::Opt('f', a) => {
                let a = a.unwrap_or_default();
                match std::fs::read(&a) {
                    Ok(d) => template = Some(String::from_utf8_lossy(&d).into_owned()),
                    Err(_) => die(&mut out, prog, &format!("can't open file {} for reading.\n", a)),
                }
            }
            _ => exit(&mut out, 1),
        }
    }
    if !operands.is_empty() {
        if template.is_some() {
            die(&mut out, prog, "don't specify a template in the command line and -f at the same time.\n");
        }
        if operands.len() != 1 {
            exit(&mut out, 1);
        }
        template = Some(operands[0].clone());
    }
    let data = read_input("stdin").unwrap_or_default();
    let mut db = Db::default();
    if !parse_into(&mut out, prog, &data, "stdin", &mut db) {
        exit(&mut out, 0);
    }
    let template = match template {
        Some(t) => t,
        None => exit(&mut out, 0),
    };
    for rset in &db.rsets {
        for e in &rset.elems {
            if let RElem::Record(r) = e {
                let res = apply_template(&mut out, prog, r, &template);
                if !res.is_empty() {
                    out.buf.extend_from_slice(res.as_bytes());
                }
            }
        }
    }
    exit(&mut out, 0);
}

fn apply_template(out: &mut Out, prog: &str, r: &rec::Record, t: &str) -> String {
    let b = t.as_bytes();
    let mut res = String::new();
    let mut p = 0;
    loop {
        if p >= b.len() {
            break;
        }
        // leftmost \{\{[^{}]*\}\}
        let mut found = None;
        let mut s = p;
        while s + 1 < b.len() {
            if b[s] == b'{' && b[s + 1] == b'{' {
                let mut e = s + 2;
                while e < b.len() && b[e] != b'{' && b[e] != b'}' {
                    e += 1;
                }
                if e + 1 < b.len() && b[e] == b'}' && b[e + 1] == b'}' {
                    found = Some((s, e + 2));
                    break;
                }
            }
            s += 1;
        }
        let (ms, me) = match found {
            Some(x) => x,
            None => break,
        };
        res.push_str(&t[p..ms]);
        let expr = &t[ms + 2..me - 2];
        p = me;
        let sx = match sex::Sex::compile(expr, false) {
            Some(s) => s,
            None => die(out, prog, "invalid expression in a template slot.\n"),
        };
        match sx.eval_str(r) {
            Some(v) => res.push_str(&v),
            None => die(out, prog, "error evaluating expression in a template slot.\n"),
        }
    }
    if p < b.len() {
        res.push_str(&t[p..]);
    }
    res
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("usage: recrs {{recsel|recinf|recfmt|rec2csv}} [OPTION]... [FILE]...\n");
        std::process::exit(2);
    }
    let tool = args[0].clone();
    let rest: Vec<String> = args[1..].to_vec();
    let child = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            match tool.as_str() {
                "recsel" => recsel(&rest),
                "recinf" => recinf(&rest),
                "recfmt" => recfmt(&rest),
                "rec2csv" => rec2csv(&rest),
                _ => {
                    eprint!("recrs: unknown tool '{}'\n", tool);
                    std::process::exit(2);
                }
            }
        })
        .expect("spawn");
    let _ = child.join();
    std::process::exit(1);
}
