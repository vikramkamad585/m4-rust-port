// Global processor state, diagnostics, the symbol table and the expansion
// engine (expand_input / expand_macro / collect_arguments).

use crate::builtins::Builtin;
use crate::input::{Block, TokKind, Token};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::rc::Rc;

#[derive(Clone)]
pub enum Def {
    Text(Vec<u8>),
    Func(Builtin),
}

#[derive(Clone)]
pub enum Arg {
    Text(Vec<u8>),
    Func(Builtin),
}

impl Arg {
    pub fn text(&self) -> &[u8] {
        match self {
            Arg::Text(t) => t,
            Arg::Func(_) => b"",
        }
    }
    pub fn is_text(&self) -> bool {
        matches!(self, Arg::Text(_))
    }
}

pub struct M4 {
    // input engine
    pub stack: Vec<Block>,
    pub wrapup: Vec<Block>,
    pub current_file: Rc<[u8]>,
    pub current_line: i32,
    pub start_of_input_line: bool,
    pub input_change: bool,
    pub next_valid: bool,
    pub next_file: Rc<[u8]>,
    pub next_line: i32,
    pub stdin_read: bool,
    pub lquote: Vec<u8>,
    pub rquote: Vec<u8>,
    pub bcomm: Vec<u8>,
    pub ecomm: Vec<u8>,
    // symbols
    pub symtab: HashMap<Vec<u8>, Vec<Def>>,
    // output
    pub stdout_buf: Vec<u8>,
    pub diversions: BTreeMap<i32, Vec<u8>>,
    pub current_diversion: i32,
    // options and status
    pub prefix_all_builtins: bool,
    pub suppress_warnings: bool,
    pub fatal_warnings: bool,
    pub warning_status: i32,
    pub include_dirs: Vec<Vec<u8>>,
    pub retcode: i32,
    pub substitute_warned: bool,
    pub expansion_level: usize,
}

pub fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

impl M4 {
    pub fn new() -> M4 {
        let empty: Rc<[u8]> = Rc::from(&b""[..]);
        M4 {
            stack: Vec::new(),
            wrapup: Vec::new(),
            current_file: empty.clone(),
            current_line: 0,
            start_of_input_line: false,
            input_change: false,
            next_valid: false,
            next_file: empty,
            next_line: 0,
            stdin_read: false,
            lquote: b"`".to_vec(),
            rquote: b"'".to_vec(),
            bcomm: b"#".to_vec(),
            ecomm: b"\n".to_vec(),
            symtab: HashMap::new(),
            stdout_buf: Vec::new(),
            diversions: BTreeMap::new(),
            current_diversion: 0,
            prefix_all_builtins: false,
            suppress_warnings: false,
            fatal_warnings: false,
            warning_status: 0,
            include_dirs: Vec::new(),
            retcode: 0,
            substitute_warned: false,
            expansion_level: 0,
        }
    }

    // ---- termination and diagnostics -------------------------------------

    pub fn flush_stdout(&mut self) {
        let mut out = std::io::stdout();
        let _ = out.write_all(&self.stdout_buf);
        let _ = out.flush();
        self.stdout_buf.clear();
    }

    pub fn exit(&mut self, code: i32) -> ! {
        self.flush_stdout();
        std::process::exit(code);
    }

    fn emit_diag(&mut self, file: Option<&[u8]>, line: i32, msg: &[u8]) {
        let mut s: Vec<u8> = b"m4:".to_vec();
        match file {
            Some(f) => {
                s.extend_from_slice(f);
                s.push(b':');
                s.extend_from_slice(line.to_string().as_bytes());
                s.extend_from_slice(b": ");
            }
            None => s.push(b' '),
        }
        s.extend_from_slice(msg);
        s.push(b'\n');
        let mut e = std::io::stderr();
        let _ = e.write_all(&s);
    }

    /// m4_error(warning_status, 0, ...) at the current location.
    pub fn warn(&mut self, msg: &[u8]) {
        let file = self.current_file.clone();
        let line = self.current_line;
        self.warn_at(&file, line, msg);
    }

    pub fn warn_at(&mut self, file: &[u8], line: i32, msg: &[u8]) {
        let f = if line != 0 { Some(file) } else { None };
        self.emit_diag(f, line, msg);
        if self.warning_status != 0 {
            let st = self.warning_status;
            self.exit(st);
        }
        if self.fatal_warnings && self.retcode == 0 {
            self.retcode = 1;
        }
    }

    /// Like `warn` but never subject to -E escalation (plain error(0,...)).
    pub fn plain_error(&mut self, msg: &[u8]) {
        self.emit_diag(None, 0, msg);
    }

    pub fn failure_at(&mut self, file: &[u8], line: i32, msg: &[u8]) -> ! {
        let f = if line != 0 { Some(file) } else { None };
        self.emit_diag(f, line, msg);
        self.exit(1);
    }

    // ---- symbol table ----------------------------------------------------

    pub fn lookup(&self, name: &[u8]) -> Option<&Def> {
        self.symtab.get(name).and_then(|v| v.last())
    }

    pub fn define(&mut self, name: &[u8], d: Def, push: bool) {
        let ent = self.symtab.entry(name.to_vec()).or_default();
        if push || ent.is_empty() {
            ent.push(d);
        } else {
            *ent.last_mut().unwrap() = d;
        }
    }

    pub fn define_user_macro(&mut self, name: &[u8], text: Vec<u8>, push: bool) {
        self.define(name, Def::Text(text), push);
    }

    pub fn undefine(&mut self, name: &[u8]) {
        self.symtab.remove(name);
    }

    pub fn popdef(&mut self, name: &[u8]) {
        if let Some(v) = self.symtab.get_mut(name) {
            v.pop();
            if v.is_empty() {
                self.symtab.remove(name);
            }
        }
    }

    pub fn init_builtins(&mut self) {
        for b in crate::builtins::ALL.iter() {
            let name = b.name().as_bytes();
            if self.prefix_all_builtins {
                let mut n = b"m4_".to_vec();
                n.extend_from_slice(name);
                self.define(&n, Def::Func(*b), false);
            } else {
                self.define(name, Def::Func(*b), false);
            }
        }
        self.define_user_macro(b"__unix__", Vec::new(), false);
        self.define_user_macro(b"__gnu__", Vec::new(), false);
    }

    pub fn add_include_directory(&mut self, dir: Vec<u8>) {
        if dir.is_empty() {
            self.include_dirs.push(b".".to_vec());
        } else {
            self.include_dirs.push(dir);
        }
    }

    // ---- top level -------------------------------------------------------

    pub fn process_file(&mut self, name: &[u8]) {
        if name == b"-" {
            let data = if self.stdin_read {
                Vec::new()
            } else {
                self.stdin_read = true;
                let mut v = Vec::new();
                let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut v);
                v
            };
            self.push_file(data, Rc::from(&b"stdin"[..]));
        } else {
            match self.path_search(name) {
                Ok((data, full)) => self.push_file(data, Rc::from(full.as_slice())),
                Err(e) => {
                    let mut msg = b"cannot open `".to_vec();
                    msg.extend_from_slice(name);
                    msg.extend_from_slice(b"': ");
                    msg.extend_from_slice(e.as_bytes());
                    self.plain_error(&msg);
                    self.retcode = 1;
                    return;
                }
            }
        }
        self.expand_input();
    }

    /// Search for FILE in `.` and then the -I directories.
    pub fn path_search(&self, file: &[u8]) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
        if file.is_empty() {
            return Err("No such file or directory");
        }
        let first = read_file(file);
        let e = match first {
            Ok(d) => return Ok((d, file.to_vec())),
            Err(e) => e,
        };
        if file[0] == b'/' {
            return Err(e);
        }
        for dir in &self.include_dirs {
            let mut name = dir.clone();
            if !name.ends_with(b"/") {
                name.push(b'/');
            }
            name.extend_from_slice(file);
            if let Ok(d) = read_file(&name) {
                return Ok((d, name));
            }
        }
        Err(e)
    }

    pub fn expand_input(&mut self) {
        loop {
            let (tok, line) = self.next_token();
            if let TokKind::Eof = tok.kind {
                break;
            }
            self.expand_token(None, tok, line);
        }
    }

    fn expand_token(&mut self, obs: Option<&mut Vec<u8>>, tok: Token, line: i32) {
        match tok.kind {
            TokKind::Eof | TokKind::MacDef(_) => {}
            TokKind::Word => {
                let def = self.lookup(&tok.text).cloned();
                match def {
                    None => self.shipout(obs, &tok.text, line),
                    Some(Def::Func(b)) if b.blind() && !self.peek_is_open() => {
                        self.shipout(obs, &tok.text, line)
                    }
                    Some(d) => self.expand_macro(tok.text, d),
                }
            }
            _ => self.shipout(obs, &tok.text, line),
        }
    }

    fn shipout(&mut self, obs: Option<&mut Vec<u8>>, text: &[u8], _line: i32) {
        match obs {
            Some(o) => o.extend_from_slice(text),
            None => self.output_text(text),
        }
    }

    fn peek_is_open(&mut self) -> bool {
        matches!(self.peek_token(), TokKind::Open)
    }

    /// Collect one argument; returns (more arguments follow, the argument).
    fn expand_argument(&mut self) -> (bool, Arg) {
        let file = self.current_file.clone();
        let line = self.current_line;
        let mut obs: Vec<u8> = Vec::new();
        let mut func: Option<Builtin> = None;
        let mut tok;
        loop {
            tok = self.next_token().0;
            match tok.kind {
                TokKind::Simple if is_space(tok.text[0]) => continue,
                _ => break,
            }
        }
        let mut paren_level = 0i32;
        loop {
            match tok.kind {
                TokKind::Comma | TokKind::Close if paren_level == 0 => {
                    let more = matches!(tok.kind, TokKind::Comma);
                    let arg = match func {
                        Some(b) => Arg::Func(b),
                        None => Arg::Text(obs),
                    };
                    return (more, arg);
                }
                TokKind::Comma | TokKind::Close | TokKind::Open | TokKind::Simple => {
                    if tok.text[0] == b'(' {
                        paren_level += 1;
                    } else if tok.text[0] == b')' {
                        paren_level -= 1;
                    }
                    self.expand_token(Some(&mut obs), tok, line);
                }
                TokKind::Eof => {
                    self.failure_at(&file, line, b"ERROR: end of file in argument list");
                }
                TokKind::Word | TokKind::Str => self.expand_token(Some(&mut obs), tok, line),
                TokKind::MacDef(b) => {
                    if obs.is_empty() {
                        func = Some(b);
                    }
                }
            }
            tok = self.next_token().0;
        }
    }

    fn collect_arguments(&mut self, name: &[u8], groks: bool) -> Vec<Arg> {
        let mut args = vec![Arg::Text(name.to_vec())];
        if self.peek_is_open() {
            self.next_token();
            loop {
                let (more, mut a) = self.expand_argument();
                if !groks {
                    if let Arg::Func(_) = a {
                        a = Arg::Text(Vec::new());
                    }
                }
                args.push(a);
                if !more {
                    break;
                }
            }
        }
        args
    }

    fn expand_macro(&mut self, name: Vec<u8>, def: Def) {
        self.expansion_level += 1;
        let open_file = self.current_file.clone();
        let open_line = self.current_line;
        let groks = match &def {
            Def::Func(b) => b.groks_macro_args(),
            Def::Text(_) => false,
        };
        let mut args = self.collect_arguments(&name, groks);
        let close_file = self.current_file.clone();
        let close_line = self.current_line;
        self.current_file = open_file;
        self.current_line = open_line;
        self.push_string_init();
        let mut obs: Vec<u8> = Vec::new();
        self.call_macro(&def, &mut args, &mut obs);
        self.push_string_finish(obs);
        self.current_file = close_file;
        self.current_line = close_line;
        self.expansion_level -= 1;
    }

    pub fn call_macro(&mut self, def: &Def, args: &mut [Arg], obs: &mut Vec<u8>) {
        match def {
            Def::Func(b) => self.call_builtin(*b, args, obs),
            Def::Text(t) => expand_user_macro(obs, t, args, &self.lquote, &self.rquote),
        }
    }

    pub fn dump_args(&self, obs: &mut Vec<u8>, args: &[Arg], sep: &[u8], quoted: bool) {
        for (i, a) in args.iter().enumerate().skip(1) {
            if i > 1 {
                obs.extend_from_slice(sep);
            }
            if quoted {
                obs.extend_from_slice(&self.lquote);
            }
            obs.extend_from_slice(a.text());
            if quoted {
                obs.extend_from_slice(&self.rquote);
            }
        }
    }
}

fn expand_user_macro(obs: &mut Vec<u8>, text: &[u8], args: &[Arg], lq: &[u8], rq: &[u8]) {
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if c != b'$' || i + 1 >= text.len() {
            obs.push(c);
            i += 1;
            continue;
        }
        let n = text[i + 1];
        match n {
            b'0'..=b'9' => {
                let mut j = i + 1;
                let mut k: usize = 0;
                while j < text.len() && text[j].is_ascii_digit() {
                    k = k.saturating_mul(10).saturating_add((text[j] - b'0') as usize);
                    j += 1;
                }
                if k < args.len() {
                    obs.extend_from_slice(args[k].text());
                }
                i = j;
            }
            b'#' => {
                obs.extend_from_slice((args.len() as i32 - 1).to_string().as_bytes());
                i += 2;
            }
            b'*' | b'@' => {
                for (idx, a) in args.iter().enumerate().skip(1) {
                    if idx > 1 {
                        obs.push(b',');
                    }
                    if n == b'@' {
                        obs.extend_from_slice(lq);
                    }
                    obs.extend_from_slice(a.text());
                    if n == b'@' {
                        obs.extend_from_slice(rq);
                    }
                }
                i += 2;
            }
            _ => {
                obs.push(b'$');
                i += 1;
            }
        }
    }
}

pub fn errno_text(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(2) => "No such file or directory",
        Some(13) => "Permission denied",
        Some(20) => "Not a directory",
        Some(21) => "Is a directory",
        Some(36) => "File name too long",
        Some(40) => "Too many levels of symbolic links",
        _ => match e.kind() {
            std::io::ErrorKind::NotFound => "No such file or directory",
            std::io::ErrorKind::PermissionDenied => "Permission denied",
            _ => "Input/output error",
        },
    }
}

pub fn read_file(name: &[u8]) -> Result<Vec<u8>, &'static str> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::path::Path::new(std::ffi::OsStr::from_bytes(name));
    let f = std::fs::File::open(path).map_err(|e| errno_text(&e))?;
    if let Ok(md) = f.metadata() {
        if md.is_dir() {
            return Err("Is a directory");
        }
    }
    let mut v = Vec::new();
    let mut f = f;
    std::io::Read::read_to_end(&mut f, &mut v).map_err(|e| errno_text(&e))?;
    Ok(v)
}
