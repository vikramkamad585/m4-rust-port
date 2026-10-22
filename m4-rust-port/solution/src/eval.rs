// The eval builtin: a recursive-descent parser over 32-bit two's-complement
// integers with GNU m4's operator set, error classes and short-circuiting.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    Error,
    BadOp,
    Plus,
    Minus,
    Exponent,
    Times,
    Divide,
    Modulo,
    Assign,
    Eq,
    NotEq,
    Gt,
    GtEq,
    Ls,
    LsEq,
    LShift,
    RShift,
    LNot,
    LAnd,
    LOr,
    Not,
    And,
    Or,
    Xor,
    LeftP,
    RightP,
    Number,
    EoText,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Err {
    NoError,
    DivideZero,
    ModuloZero,
    NegativeExponent,
    Syntax,
    MissingRight,
    UnknownInput,
    ExcessInput,
    InvalidOperator,
}

struct Lexer<'a> {
    text: &'a [u8],
    pos: usize,
    last: usize,
    assign_warnings: usize,
}

fn is_space(c: u8) -> bool {
    crate::m4::is_space(c)
}

impl<'a> Lexer<'a> {
    fn at(&self, i: usize) -> u8 {
        if i < self.text.len() {
            self.text[i]
        } else {
            0
        }
    }

    fn undo(&mut self) {
        self.pos = self.last;
    }

    fn lex(&mut self, val: &mut i32) -> Tok {
        while self.pos < self.text.len() && is_space(self.text[self.pos]) {
            self.pos += 1;
        }
        self.last = self.pos;
        if self.pos >= self.text.len() {
            return Tok::EoText;
        }
        let c = self.text[self.pos];
        if c.is_ascii_digit() {
            let base: u32;
            if c == b'0' {
                self.pos += 1;
                match self.at(self.pos) {
                    b'x' | b'X' => {
                        base = 16;
                        self.pos += 1;
                    }
                    b'b' | b'B' => {
                        base = 2;
                        self.pos += 1;
                    }
                    b'r' | b'R' => {
                        self.pos += 1;
                        let mut b: u32 = 0;
                        while self.at(self.pos).is_ascii_digit() && b <= 36 {
                            b = 10 * b + (self.at(self.pos) - b'0') as u32;
                            self.pos += 1;
                        }
                        if b == 0 || b > 36 || self.at(self.pos) != b':' {
                            return Tok::Error;
                        }
                        self.pos += 1;
                        base = b;
                    }
                    _ => base = 8,
                }
            } else {
                base = 10;
            }
            let mut value: u32 = 0;
            while self.pos < self.text.len() {
                let ch = self.text[self.pos];
                let digit: u32 = if ch.is_ascii_digit() {
                    (ch - b'0') as u32
                } else if ch.is_ascii_lowercase() {
                    (ch - b'a') as u32 + 10
                } else if ch.is_ascii_uppercase() {
                    (ch - b'A') as u32 + 10
                } else {
                    break;
                };
                if base == 1 {
                    if digit == 1 {
                        value = value.wrapping_add(1);
                    } else if digit == 0 && value == 0 {
                        self.pos += 1;
                        continue;
                    } else {
                        break;
                    }
                } else if digit >= base {
                    break;
                } else {
                    value = value.wrapping_mul(base).wrapping_add(digit);
                }
                self.pos += 1;
            }
            *val = value as i32;
            return Tok::Number;
        }
        self.pos += 1;
        let n = self.at(self.pos);
        match c {
            b'+' => {
                if n == b'+' || n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Plus
                }
            }
            b'-' => {
                if n == b'-' || n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Minus
                }
            }
            b'*' => {
                if n == b'*' {
                    self.pos += 1;
                    Tok::Exponent
                } else if n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Times
                }
            }
            b'/' => {
                if n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Divide
                }
            }
            b'%' => {
                if n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Modulo
                }
            }
            b'=' => {
                if n == b'=' {
                    self.pos += 1;
                    Tok::Eq
                } else {
                    Tok::Assign
                }
            }
            b'!' => {
                if n == b'=' {
                    self.pos += 1;
                    Tok::NotEq
                } else {
                    Tok::LNot
                }
            }
            b'>' => {
                if n == b'=' {
                    self.pos += 1;
                    Tok::GtEq
                } else if n == b'>' {
                    self.pos += 1;
                    if self.at(self.pos) == b'=' {
                        Tok::BadOp
                    } else {
                        Tok::RShift
                    }
                } else {
                    Tok::Gt
                }
            }
            b'<' => {
                if n == b'=' {
                    self.pos += 1;
                    Tok::LsEq
                } else if n == b'<' {
                    self.pos += 1;
                    if self.at(self.pos) == b'=' {
                        Tok::BadOp
                    } else {
                        Tok::LShift
                    }
                } else {
                    Tok::Ls
                }
            }
            b'^' => {
                if n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Xor
                }
            }
            b'~' => Tok::Not,
            b'&' => {
                if n == b'&' {
                    self.pos += 1;
                    Tok::LAnd
                } else if n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::And
                }
            }
            b'|' => {
                if n == b'|' {
                    self.pos += 1;
                    Tok::LOr
                } else if n == b'=' {
                    Tok::BadOp
                } else {
                    Tok::Or
                }
            }
            b'(' => Tok::LeftP,
            b')' => Tok::RightP,
            _ => Tok::Error,
        }
    }

    fn logical_or(&mut self, et: Tok, v1: &mut i32) -> Err {
        let er = self.logical_and(et, v1);
        if er != Err::NoError {
            return er;
        }
        let mut v2 = 0;
        let mut et;
        loop {
            et = self.lex(&mut v2);
            if et != Tok::LOr {
                break;
            }
            et = self.lex(&mut v2);
            if et == Tok::Error {
                return Err::UnknownInput;
            }
            let er = self.logical_and(et, &mut v2);
            if er == Err::NoError {
                *v1 = (*v1 != 0 || v2 != 0) as i32;
            } else if *v1 != 0 && er < Err::Syntax {
                *v1 = 1;
            } else {
                return er;
            }
        }
        if et == Tok::Error {
            return Err::UnknownInput;
        }
        self.undo();
        Err::NoError
    }

    fn logical_and(&mut self, et: Tok, v1: &mut i32) -> Err {
        let er = self.or_term(et, v1);
        if er != Err::NoError {
            return er;
        }
        let mut v2 = 0;
        let mut et;
        loop {
            et = self.lex(&mut v2);
            if et != Tok::LAnd {
                break;
            }
            et = self.lex(&mut v2);
            if et == Tok::Error {
                return Err::UnknownInput;
            }
            let er = self.or_term(et, &mut v2);
            if er == Err::NoError {
                *v1 = (*v1 != 0 && v2 != 0) as i32;
            } else if *v1 == 0 && er < Err::Syntax {
            } else {
                return er;
            }
        }
        if et == Tok::Error {
            return Err::UnknownInput;
        }
        self.undo();
        Err::NoError
    }

    fn binary_loop(
        &mut self,
        et: Tok,
        v1: &mut i32,
        ops: &[Tok],
        sub: fn(&mut Self, Tok, &mut i32) -> Err,
        apply: fn(&mut Self, Tok, i32, i32) -> Result<i32, Err>,
    ) -> Err {
        let er = sub(self, et, v1);
        if er != Err::NoError {
            return er;
        }
        let mut v2 = 0;
        let mut op;
        loop {
            op = self.lex(&mut v2);
            if !ops.contains(&op) {
                break;
            }
            let et = self.lex(&mut v2);
            if et == Tok::Error {
                return Err::UnknownInput;
            }
            let er = sub(self, et, &mut v2);
            if er != Err::NoError {
                return er;
            }
            match apply(self, op, *v1, v2) {
                Ok(v) => *v1 = v,
                Result::Err(e) => return e,
            }
        }
        if op == Tok::Error {
            return Err::UnknownInput;
        }
        self.undo();
        Err::NoError
    }

    fn or_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(et, v1, &[Tok::Or], Self::xor_term, |_, _, a, b| Ok(a | b))
    }

    fn xor_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(et, v1, &[Tok::Xor], Self::and_term, |_, _, a, b| Ok(a ^ b))
    }

    fn and_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(et, v1, &[Tok::And], Self::equality_term, |_, _, a, b| Ok(a & b))
    }

    fn equality_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(
            et,
            v1,
            &[Tok::Eq, Tok::NotEq, Tok::Assign],
            Self::cmp_term,
            |lx, op, a, b| {
                let mut op = op;
                if op == Tok::Assign {
                    lx.assign_warnings += 1;
                    op = Tok::Eq;
                }
                Ok(((op == Tok::Eq) == (a == b)) as i32)
            },
        )
    }

    fn cmp_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(
            et,
            v1,
            &[Tok::Gt, Tok::GtEq, Tok::Ls, Tok::LsEq],
            Self::shift_term,
            |_, op, a, b| {
                Ok(match op {
                    Tok::Gt => (a > b) as i32,
                    Tok::GtEq => (a >= b) as i32,
                    Tok::Ls => (a < b) as i32,
                    _ => (a <= b) as i32,
                })
            },
        )
    }

    fn shift_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(et, v1, &[Tok::LShift, Tok::RShift], Self::add_term, |_, op, a, b| {
            let sh = (b & 0x1f) as u32;
            Ok(if op == Tok::LShift {
                ((a as u32) << sh) as i32
            } else {
                let u1: u32 = if a < 0 { !(a as u32) } else { a as u32 };
                let u1 = u1 >> sh;
                if a < 0 {
                    !u1 as i32
                } else {
                    u1 as i32
                }
            })
        })
    }

    fn add_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(et, v1, &[Tok::Plus, Tok::Minus], Self::mult_term, |_, op, a, b| {
            Ok(if op == Tok::Plus { a.wrapping_add(b) } else { a.wrapping_sub(b) })
        })
    }

    fn mult_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        self.binary_loop(
            et,
            v1,
            &[Tok::Times, Tok::Divide, Tok::Modulo],
            Self::exp_term,
            |_, op, a, b| match op {
                Tok::Times => Ok(a.wrapping_mul(b)),
                Tok::Divide => {
                    if b == 0 {
                        Result::Err(Err::DivideZero)
                    } else if b == -1 {
                        Ok(a.wrapping_neg())
                    } else {
                        Ok(a / b)
                    }
                }
                _ => {
                    if b == 0 {
                        Result::Err(Err::ModuloZero)
                    } else if b == -1 {
                        Ok(0)
                    } else {
                        Ok(a % b)
                    }
                }
            },
        )
    }

    fn exp_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        let er = self.unary_term(et, v1);
        if er != Err::NoError {
            return er;
        }
        let mut v2 = 0;
        let mut et;
        loop {
            et = self.lex(&mut v2);
            if et != Tok::Exponent {
                break;
            }
            et = self.lex(&mut v2);
            if et == Tok::Error {
                return Err::UnknownInput;
            }
            let er = self.exp_term(et, &mut v2);
            if er != Err::NoError {
                return er;
            }
            let mut result: u32 = 1;
            if v2 < 0 {
                return Err::NegativeExponent;
            }
            if *v1 == 0 && v2 == 0 {
                return Err::DivideZero;
            }
            let mut n = v2;
            while n > 0 {
                result = result.wrapping_mul(*v1 as u32);
                n -= 1;
            }
            *v1 = result as i32;
        }
        if et == Tok::Error {
            return Err::UnknownInput;
        }
        self.undo();
        Err::NoError
    }

    fn unary_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        if matches!(et, Tok::Plus | Tok::Minus | Tok::Not | Tok::LNot) {
            let et2 = self.lex(v1);
            if et2 == Tok::Error {
                return Err::UnknownInput;
            }
            let er = self.unary_term(et2, v1);
            if er != Err::NoError {
                return er;
            }
            match et {
                Tok::Minus => *v1 = v1.wrapping_neg(),
                Tok::Not => *v1 = !*v1,
                Tok::LNot => *v1 = (*v1 == 0) as i32,
                _ => {}
            }
            Err::NoError
        } else {
            self.simple_term(et, v1)
        }
    }

    fn simple_term(&mut self, et: Tok, v1: &mut i32) -> Err {
        match et {
            Tok::LeftP => {
                let et = self.lex(v1);
                if et == Tok::Error {
                    return Err::UnknownInput;
                }
                let er = self.logical_or(et, v1);
                if er != Err::NoError {
                    return er;
                }
                let mut v2 = 0;
                let et = self.lex(&mut v2);
                if et == Tok::Error {
                    return Err::UnknownInput;
                }
                if et != Tok::RightP {
                    return Err::MissingRight;
                }
                Err::NoError
            }
            Tok::Number => Err::NoError,
            Tok::BadOp => Err::InvalidOperator,
            _ => Err::Syntax,
        }
    }
}

pub struct EvalResult {
    /// Number of "recommend ==" warnings, emitted before any error.
    pub assign_warnings: usize,
    /// Ok(value) or Err((message prefix, sets exit status)).
    pub value: Result<i32, (&'static str, bool)>,
}

pub fn evaluate(expr: &[u8]) -> EvalResult {
    let mut lx = Lexer { text: expr, pos: 0, last: 0, assign_warnings: 0 };
    let mut val = 0;
    let et = lx.lex(&mut val);
    let mut err = lx.logical_or(et, &mut val);
    if err == Err::NoError && lx.pos < expr.len() {
        let mut dummy = 0;
        if lx.lex(&mut dummy) == Tok::BadOp {
            err = Err::InvalidOperator;
        } else {
            err = Err::ExcessInput;
        }
    }
    let value = match err {
        Err::NoError => Ok(val),
        Err::MissingRight => Result::Err(("bad expression in eval (missing right parenthesis): ", false)),
        Err::Syntax => Result::Err(("bad expression in eval: ", false)),
        Err::UnknownInput => Result::Err(("bad expression in eval (bad input): ", false)),
        Err::ExcessInput => Result::Err(("bad expression in eval (excess input): ", false)),
        Err::InvalidOperator => Result::Err(("invalid operator in eval: ", true)),
        Err::DivideZero => Result::Err(("divide by zero in eval: ", false)),
        Err::ModuloZero => Result::Err(("modulo by zero in eval: ", false)),
        Err::NegativeExponent => Result::Err(("negative exponent in eval: ", false)),
    };
    EvalResult { assign_warnings: lx.assign_warnings, value }
}
