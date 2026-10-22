// Records, record sets and databases; the rec-format parser and writer.

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub value: String,
    pub char_loc: Option<usize>,
}

impl Field {
    pub fn new(name: &str, value: &str) -> Field {
        Field { name: name.to_string(), value: value.to_string(), char_loc: None }
    }
}

#[derive(Clone, Debug)]
pub enum Elem {
    Field(Field),
    Comment(String),
}

#[derive(Clone, Debug, Default)]
pub struct Record {
    pub elems: Vec<Elem>,
    pub char_loc: Option<usize>,
}

impl Record {
    pub fn fields(&self) -> impl Iterator<Item = &Field> {
        self.elems.iter().filter_map(|e| match e {
            Elem::Field(f) => Some(f),
            _ => None,
        })
    }

    pub fn num_fields(&self) -> usize {
        self.fields().count()
    }

    pub fn get(&self, name: &str, n: usize) -> Option<&Field> {
        self.fields().filter(|f| f.name == name).nth(n)
    }

    pub fn count(&self, name: &str) -> usize {
        self.fields().filter(|f| f.name == name).count()
    }

    pub fn has(&self, name: &str) -> bool {
        self.fields().any(|f| f.name == name)
    }

    pub fn push_field(&mut self, f: Field) {
        self.elems.push(Elem::Field(f));
    }

    pub fn remove_all(&mut self, name: &str) {
        self.elems.retain(|e| !matches!(e, Elem::Field(f) if f.name == name));
    }
}

#[derive(Clone, Debug)]
pub enum RElem {
    Record(Record),
    Comment(String),
}

#[derive(Clone, Debug, Default)]
pub struct Rset {
    pub elems: Vec<RElem>,
    pub descriptor: Option<Record>,
    pub descriptor_pos: usize,
}

impl Rset {
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.elems.iter().filter_map(|e| match e {
            RElem::Record(r) => Some(r),
            _ => None,
        })
    }

    pub fn num_records(&self) -> usize {
        self.records().count()
    }

    /// The record type: the first field-name-like token of the %rec value.
    pub fn rtype(&self) -> Option<String> {
        let d = self.descriptor.as_ref()?;
        let f = d.get("%rec", 0)?;
        extract_type(&f.value)
    }
}

pub fn extract_type(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() || b[i] == b'%' {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            return Some(s[i..j].to_string());
        }
        i += 1;
    }
    None
}

#[derive(Default)]
pub struct Db {
    pub rsets: Vec<Rset>,
}

impl Db {
    pub fn get_by_type(&self, t: Option<&str>) -> Option<usize> {
        self.rsets.iter().position(|r| r.rtype().as_deref() == t)
    }
}

// ------------------------------------------------------------------ parser

const EOF: i32 = -1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PErr {
    None,
    EFname,
    EField,
    ERecord,
}

pub struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    pub eof: bool,
    pub error: PErr,
    pub line: usize,
    character: usize,
    prev_descriptor: Option<Record>,
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8]) -> Parser<'a> {
        Parser { data, pos: 0, eof: false, error: PErr::None, line: 1, character: 0, prev_descriptor: None }
    }

    pub fn error_string(&self) -> &'static str {
        match self.error {
            PErr::None => "no error (unused)",
            PErr::EFname => "expected a field name",
            PErr::EField => "expected a field",
            PErr::ERecord => "expected a record",
        }
    }

    fn getc(&mut self) -> i32 {
        if self.pos >= self.data.len() {
            self.eof = true;
            return EOF;
        }
        let c = self.data[self.pos];
        self.pos += 1;
        self.character += 1;
        if c == b'\n' {
            self.line += 1;
        }
        c as i32
    }

    fn ungetc(&mut self, c: i32) {
        self.character = self.character.wrapping_sub(1);
        if c == b'\n' as i32 {
            self.line -= 1;
        }
        if c != EOF && self.pos > 0 {
            self.pos -= 1;
        }
    }

    fn bad(&self) -> bool {
        self.eof || self.error != PErr::None
    }

    fn parse_field_value(&mut self) -> Option<String> {
        if self.bad() {
            return None;
        }
        let mut buf: Vec<u8> = Vec::new();
        let mut c: u8 = 0;
        let mut prev_newline = false;
        let mut ret = true;
        loop {
            let ci = self.getc();
            if ci == EOF {
                break;
            }
            c = ci as u8;
            if prev_newline && c != b'+' {
                self.ungetc(ci);
                buf.pop();
                break;
            }
            if c == b'\\' {
                let ci2 = self.getc();
                if ci2 == EOF {
                    self.eof = true;
                    ret = false;
                    break;
                }
                if ci2 as u8 != b'\n' {
                    buf.push(c);
                    self.ungetc(ci2);
                }
                prev_newline = false;
            } else if c == b'+' {
                if prev_newline {
                    let ci2 = self.getc();
                    if ci2 == EOF {
                        self.eof = true;
                        ret = false;
                        break;
                    }
                    if ci2 as u8 != b' ' {
                        self.ungetc(ci2);
                    }
                } else {
                    buf.push(c);
                }
                prev_newline = false;
            } else if c == b'\n' {
                buf.push(c);
                prev_newline = true;
            } else {
                buf.push(c);
                prev_newline = false;
            }
        }
        if !ret {
            return None;
        }
        if self.eof && c == b'\n' {
            buf.pop();
        }
        Some(String::from_utf8_lossy(&buf).into_owned())
    }

    fn parse_comment(&mut self) -> Option<String> {
        let ci = self.getc();
        if ci != b'#' as i32 {
            if ci != EOF {
                self.ungetc(ci);
            }
            return None;
        }
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let mut ci = self.getc();
            if ci == EOF {
                break;
            }
            if ci as u8 == b'\n' {
                ci = self.getc();
                if ci == EOF {
                    break;
                }
                if ci as u8 != b'#' {
                    self.ungetc(ci);
                    break;
                }
            }
            buf.push(ci as u8);
        }
        Some(String::from_utf8_lossy(&buf).into_owned())
    }

    fn parse_field_name(&mut self) -> Option<String> {
        if self.bad() {
            return None;
        }
        let mut name: Vec<u8> = Vec::new();
        let ci = self.getc();
        if ci == EOF {
            return None;
        }
        let c = ci as u8;
        if c.is_ascii_alphabetic() || c == b'%' {
            name.push(c);
        } else {
            self.error = PErr::EFname;
            return None;
        }
        loop {
            let ci = self.getc();
            if ci == EOF {
                break;
            }
            let c = ci as u8;
            if c.is_ascii_alphanumeric() || c == b'_' {
                name.push(c);
            } else if c == b':' {
                break;
            } else {
                self.error = PErr::EFname;
                return None;
            }
        }
        if self.eof {
            self.error = PErr::EFname;
            return None;
        }
        let ci = self.getc();
        if ci != EOF {
            let c = ci as u8;
            if c != b' ' && c != b'\t' {
                self.ungetc(ci);
            }
        }
        Some(String::from_utf8_lossy(&name).into_owned())
    }

    fn parse_field(&mut self) -> Option<Field> {
        if self.bad() {
            return None;
        }
        let mut char_loc = self.character;
        if char_loc != 0 {
            char_loc += 1;
        }
        let name = self.parse_field_name()?;
        let value = self.parse_field_value()?;
        Some(Field { name, value, char_loc: Some(char_loc) })
    }

    fn parse_record(&mut self) -> Option<Record> {
        if self.bad() {
            return None;
        }
        let mut char_loc = self.character;
        if char_loc != 0 {
            char_loc += 1;
        }
        let mut rec = Record { elems: Vec::new(), char_loc: Some(char_loc) };
        match self.parse_field() {
            Some(f) => rec.push_field(f),
            None => {
                self.error = PErr::EField;
                return None;
            }
        }
        loop {
            let ci = self.getc();
            if ci == EOF {
                break;
            }
            let c = ci as u8;
            if c == b'#' {
                self.ungetc(ci);
                if let Some(cm) = self.parse_comment() {
                    rec.elems.push(Elem::Comment(cm));
                }
            } else if c == b' ' || c == b'\t' {
                let mut ci = ci;
                let mut c = c;
                while ci != EOF && (c == b' ' || c == b'\t') {
                    ci = self.getc();
                    c = ci as u8;
                }
                if ci == EOF || c == b'\n' {
                    break;
                }
                self.error = PErr::EField;
                return None;
            } else if c == b'\n' {
                break;
            } else {
                self.ungetc(ci);
                match self.parse_field() {
                    Some(f) => rec.push_field(f),
                    None => {
                        self.error = PErr::EField;
                        return None;
                    }
                }
            }
        }
        Some(rec)
    }

    pub fn parse_rset(&mut self) -> Option<Rset> {
        let mut rset = Rset::default();
        rset.descriptor = self.prev_descriptor.take();
        let mut comments_added = 0;
        let mut ret = false;
        loop {
            let ci = self.getc();
            if ci == EOF {
                break;
            }
            let c = ci as u8;
            if c == b'\n' || c == b' ' || c == b'\t' {
                continue;
            } else if c == b'#' {
                self.ungetc(ci);
                if let Some(cm) = self.parse_comment() {
                    rset.elems.push(RElem::Comment(cm));
                }
                comments_added += 1;
            } else {
                self.ungetc(ci);
                match self.parse_record() {
                    Some(rec) => {
                        if rec.has("%rec") {
                            if rset.num_records() == 0 && rset.descriptor.is_none() {
                                rset.descriptor = Some(rec);
                                rset.descriptor_pos = comments_added;
                            } else {
                                self.prev_descriptor = Some(rec);
                                ret = true;
                                break;
                            }
                        } else {
                            rset.elems.push(RElem::Record(rec));
                        }
                    }
                    None => {
                        self.error = PErr::ERecord;
                        break;
                    }
                }
            }
        }
        if self.error == PErr::None && (rset.descriptor.is_some() || rset.num_records() > 0) {
            ret = true;
        }
        if ret {
            Some(rset)
        } else {
            None
        }
    }
}

// ------------------------------------------------------------------ writer

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Values,
    ValuesRow,
    Sexp,
}

pub struct Writer {
    pub out: Vec<u8>,
    pub collapse: bool,
    pub skip_comments: bool,
    pub mode: Mode,
}

impl Writer {
    pub fn new(mode: Mode) -> Writer {
        Writer { out: Vec::new(), collapse: false, skip_comments: false, mode }
    }

    fn puts(&mut self, s: &str) {
        self.out.extend_from_slice(s.as_bytes());
    }

    fn putc(&mut self, c: u8) {
        self.out.push(c);
    }

    pub fn write_comment(&mut self, text: &str) {
        if self.mode == Mode::Sexp {
            self.puts("(comment \"");
            for ch in text.bytes() {
                if ch == b'\n' {
                    self.puts("\\n");
                } else {
                    self.putc(ch);
                }
            }
            self.puts("\")");
        } else {
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 {
                    self.putc(b'\n');
                }
                self.putc(b'#');
                self.puts(line);
            }
        }
    }

    pub fn write_field(&mut self, f: &Field) {
        let mode = self.mode;
        if mode == Mode::Sexp {
            self.puts("(field ");
            let loc = f.char_loc.map(|l| l.to_string()).unwrap_or_default();
            self.puts(&loc);
            self.putc(b' ');
        }
        if mode != Mode::Values && mode != Mode::ValuesRow {
            if mode == Mode::Sexp {
                self.putc(b'"');
                self.puts(&f.name);
                self.putc(b'"');
            } else {
                self.puts(&f.name);
                self.putc(b':');
            }
        }
        if mode == Mode::Sexp {
            self.putc(b' ');
            self.putc(b'"');
        }
        if !f.value.is_empty() && mode == Mode::Normal {
            self.putc(b' ');
        }
        for ch in f.value.bytes() {
            if ch == b'\n' {
                match mode {
                    Mode::Sexp => self.puts("\\n"),
                    Mode::Normal => self.puts("\n+ "),
                    _ => self.putc(b'\n'),
                }
            } else if (ch == b'"' || ch == b'\\') && mode == Mode::Sexp {
                self.putc(b'\\');
                self.putc(ch);
            } else {
                self.putc(ch);
            }
        }
        if mode == Mode::Sexp {
            self.puts("\")");
        }
    }

    pub fn write_record(&mut self, r: &Record) {
        let mode = self.mode;
        if mode == Mode::Sexp {
            self.puts("(record ");
            let loc = r.char_loc.map(|l| l.to_string()).unwrap_or_default();
            self.puts(&loc);
            self.puts(" (\n");
        }
        let num_elems = r.elems.len();
        let num_fields = r.num_fields();
        let mut num_field = 0usize;
        for (num_elem, e) in r.elems.iter().enumerate() {
            match e {
                Elem::Field(f) => {
                    self.write_field(f);
                    if mode == Mode::ValuesRow && num_field != num_fields.wrapping_sub(1) {
                        self.putc(b' ');
                    } else if (self.skip_comments && num_field != num_fields.wrapping_sub(1))
                        || (!self.skip_comments && num_elem != num_elems.wrapping_sub(1))
                    {
                        self.putc(b'\n');
                    }
                    num_field += 1;
                }
                Elem::Comment(c) => {
                    if !self.skip_comments && mode != Mode::Values && mode != Mode::ValuesRow {
                        self.write_comment(c);
                        if num_elem != num_elems.wrapping_sub(1) {
                            self.putc(b'\n');
                        }
                    }
                }
            }
        }
        if mode == Mode::Sexp {
            self.puts("))");
        }
    }

    pub fn write_rset(&mut self, rset: &Rset) {
        let descriptor_pos = rset.descriptor_pos;
        let n = rset.elems.len();
        if n == 0 {
            if let Some(d) = &rset.descriptor {
                self.write_record(d);
                self.putc(b'\n');
            }
            return;
        }
        let mut wrote_descriptor = false;
        for (position, e) in rset.elems.iter().enumerate() {
            if position != 0 {
                self.putc(b'\n');
            }
            if position == descriptor_pos {
                if let Some(d) = &rset.descriptor {
                    self.write_record(d);
                    wrote_descriptor = true;
                    self.puts("\n\n");
                }
            }
            match e {
                RElem::Record(r) => self.write_record(r),
                RElem::Comment(c) => {
                    if !self.skip_comments {
                        self.write_comment(c)
                    }
                }
            }
            if !self.collapse || position == n - 1 {
                self.putc(b'\n');
            }
        }
        if !wrote_descriptor && descriptor_pos >= n {
            if let Some(d) = &rset.descriptor {
                self.putc(b'\n');
                self.write_record(d);
                self.putc(b'\n');
            }
        }
    }
}
