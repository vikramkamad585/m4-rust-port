// m4rs: a GNU m4 1.4.19 compatible macro processor.

mod builtins;
mod eval;
mod format;
mod input;
mod m4;
mod output;
mod regex;

use m4::M4;

fn usage_error(msg: &str) -> ! {
    use std::io::Write;
    let mut e = std::io::stderr();
    let _ = write!(e, "m4: {}\nTry `m4 --help' for more information.\n", msg);
    std::process::exit(1);
}

enum Deferred {
    Define(Vec<u8>),
    Undefine(Vec<u8>),
    File(Vec<u8>),
}

fn run() {
    use std::os::unix::ffi::OsStringExt;
    let argv: Vec<Vec<u8>> = std::env::args_os().skip(1).map(|a| a.into_vec()).collect();
    let mut m = M4::new();
    let mut deferred: Vec<Deferred> = Vec::new();
    let mut trailing: Vec<Vec<u8>> = Vec::new();

    let long_opts: &[(&str, char, bool)] = &[
        ("define", 'D', true),
        ("undefine", 'U', true),
        ("include", 'I', true),
        ("prefix-builtins", 'P', false),
        ("quiet", 'Q', false),
        ("silent", 'Q', false),
        ("fatal-warnings", 'E', false),
    ];

    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        i += 1;
        if a.as_slice() == b"--" {
            trailing.extend(argv[i..].iter().cloned());
            break;
        }
        if a.len() >= 2 && a.starts_with(b"--") {
            let body = &a[2..];
            let (name, val) = match body.iter().position(|&c| c == b'=') {
                Some(p) => (&body[..p], Some(body[p + 1..].to_vec())),
                None => (body, None),
            };
            let name_s = String::from_utf8_lossy(name).to_string();
            let matches: Vec<&(&str, char, bool)> = long_opts
                .iter()
                .filter(|(n, _, _)| n.starts_with(name_s.as_str()))
                .collect();
            let exact = long_opts.iter().find(|(n, _, _)| *n == name_s);
            let opt = match exact {
                Some(o) => o,
                None => {
                    let mut chars: Vec<char> = matches.iter().map(|o| o.1).collect();
                    chars.dedup();
                    if matches.is_empty() {
                        usage_error(&format!("unrecognized option '--{}'", name_s));
                    } else if chars.len() > 1 {
                        usage_error(&format!("option '--{}' is ambiguous", name_s));
                    }
                    matches[0]
                }
            };
            let arg = if opt.2 {
                match val {
                    Some(v) => Some(v),
                    None => {
                        if i < argv.len() {
                            i += 1;
                            Some(argv[i - 1].clone())
                        } else {
                            usage_error(&format!("option '--{}' requires an argument", opt.0));
                        }
                    }
                }
            } else {
                if val.is_some() {
                    usage_error(&format!("option '--{}' doesn't allow an argument", opt.0));
                }
                None
            };
            apply_opt(&mut m, &mut deferred, opt.1, arg);
            continue;
        }
        if a.len() >= 2 && a[0] == b'-' {
            let mut j = 1;
            while j < a.len() {
                let c = a[j] as char;
                j += 1;
                match c {
                    'D' | 'U' | 'I' => {
                        let arg = if j < a.len() {
                            let v = a[j..].to_vec();
                            j = a.len();
                            v
                        } else if i < argv.len() {
                            i += 1;
                            argv[i - 1].clone()
                        } else {
                            usage_error(&format!("option requires an argument -- '{}'", c));
                        };
                        apply_opt(&mut m, &mut deferred, c, Some(arg));
                    }
                    'P' | 'Q' | 'E' => apply_opt(&mut m, &mut deferred, c, None),
                    _ => usage_error(&format!("invalid option -- '{}'", c)),
                }
            }
            continue;
        }
        deferred.push(Deferred::File(a.clone()));
    }

    m.init_builtins();
    let mut seen_file = false;
    for d in deferred {
        match d {
            Deferred::Define(arg) => {
                let (name, val) = match arg.iter().position(|&c| c == b'=') {
                    Some(p) => (arg[..p].to_vec(), arg[p + 1..].to_vec()),
                    None => (arg.clone(), Vec::new()),
                };
                m.define_user_macro(&name, val, false);
            }
            Deferred::Undefine(name) => m.undefine(&name),
            Deferred::File(f) => {
                seen_file = true;
                m.process_file(&f);
            }
        }
    }
    if trailing.is_empty() && !seen_file {
        m.process_file(b"-");
    } else {
        for f in trailing {
            m.process_file(&f);
        }
    }
    while m.pop_wrapup() {
        m.expand_input();
    }
    m.make_diversion(0);
    m.undivert_all();
    let code = m.retcode;
    m.exit(code);
}

fn apply_opt(m: &mut M4, deferred: &mut Vec<Deferred>, c: char, arg: Option<Vec<u8>>) {
    match c {
        'D' => deferred.push(Deferred::Define(arg.unwrap())),
        'U' => deferred.push(Deferred::Undefine(arg.unwrap())),
        'I' => m.add_include_directory(arg.unwrap()),
        'P' => m.prefix_all_builtins = true,
        'Q' => m.suppress_warnings = true,
        'E' => {
            if !m.fatal_warnings {
                m.fatal_warnings = true;
            } else {
                m.warning_status = 1;
            }
        }
        _ => {}
    }
}

fn main() {
    // Deeply recursive macro programs need far more stack than the default.
    let child = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(run)
        .expect("spawn");
    let _ = child.join();
    std::process::exit(1);
}
