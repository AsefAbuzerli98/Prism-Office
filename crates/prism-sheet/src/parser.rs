//! Formula lexer, parser, serializer and reference shifting.

use crate::addr::{col_index, col_name, parse_a1, parse_r1c1, r1c1, CellRef, MAX_COLS, MAX_ROWS};
use crate::value::ErrorKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Percent,
    /// `@` implicit intersection.
    Implicit,
}

/// One end of a reference with its absolute flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub cell: CellRef,
    pub row_abs: bool,
    pub col_abs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefKind {
    Cell(Anchor),
    Range(Anchor, Anchor),
    /// Whole columns `A:C` (row field unused).
    Cols(Anchor, Anchor),
    /// Whole rows `1:3` (col field unused).
    Rows(Anchor, Anchor),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefExpr {
    pub sheet: Option<String>,
    pub kind: RefKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorKind),
    Array(Vec<Vec<Expr>>),
    Ref(RefExpr),
    Name(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// Function call by (upper-cased, prefix-stripped) name.
    Call(String, Vec<Expr>),
    /// Calling a lambda-valued expression: `LAMBDA(x,x+1)(2)`.
    Invoke(Box<Expr>, Vec<Expr>),
    /// `A1#`: the spill range anchored at a cell.
    Spill(Box<Expr>),
    /// `Table1[Col]`, `Table1[#All]`, `[@Col]` …
    Structured {
        table: Option<String>,
        spec: String,
    },
    /// An omitted argument: `IF(A1,,2)`.
    Missing,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("formula parse error at {pos}: {msg}")]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Err(ErrorKind),
    /// Identifier or reference text, with optional sheet prefix.
    Ident {
        sheet: Option<String>,
        text: String,
    },
    Structured {
        table: Option<String>,
        spec: String,
    },
    Op(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Hash,
    At,
    Percent,
}

struct Lexer<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
    r1c1: bool,
}

impl<'a> Lexer<'a> {
    fn err(&self, msg: &str) -> ParseError {
        ParseError {
            pos: self.i,
            msg: msg.into(),
        }
    }

    fn ident_char(c: u8, r1c1: bool) -> bool {
        c.is_ascii_alphanumeric()
            || c == b'_'
            || c == b'.'
            || c == b'$'
            || c == b'\\'
            || c >= 0x80
            || (r1c1 && (c == b'[' || c == b']' || c == b'-'))
    }

    fn bracket(&mut self) -> Result<String, ParseError> {
        // self.s[self.i] == '['; returns inner text with nesting
        let start = self.i + 1;
        let mut depth = 0;
        while self.i < self.s.len() {
            match self.s[self.i] {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        let inner = self.src[start..self.i].to_string();
                        self.i += 1;
                        return Ok(inner);
                    }
                }
                b'\'' => self.i += 1, // escape char inside structured refs
                _ => {}
            }
            self.i += 1;
        }
        Err(self.err("unterminated ["))
    }

    fn tokens(mut self) -> Result<Vec<(usize, Tok)>, ParseError> {
        let mut out = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            let pos = self.i;
            match c {
                b' ' | b'\t' | b'\r' | b'\n' => self.i += 1,
                b'"' => {
                    self.i += 1;
                    let mut s = String::new();
                    loop {
                        let Some(&ch) = self.s.get(self.i) else {
                            return Err(self.err("unterminated string"));
                        };
                        if ch == b'"' {
                            if self.s.get(self.i + 1) == Some(&b'"') {
                                s.push('"');
                                self.i += 2;
                                continue;
                            }
                            self.i += 1;
                            break;
                        }
                        let ch_len = utf8_len(ch);
                        s.push_str(&self.src[self.i..self.i + ch_len]);
                        self.i += ch_len;
                    }
                    out.push((pos, Tok::Str(s)));
                }
                b'#' => {
                    // error literal or spill operator
                    let rest = &self.src[self.i..];
                    let upper = rest.to_ascii_uppercase();
                    if let Some(e) = [
                        "#NULL!",
                        "#DIV/0!",
                        "#VALUE!",
                        "#REF!",
                        "#NAME?",
                        "#NUM!",
                        "#N/A",
                        "#SPILL!",
                        "#CALC!",
                        "#BUSY!",
                        "#GETTING_DATA",
                    ]
                    .iter()
                    .find(|e| upper.starts_with(*e))
                    {
                        self.i += e.len();
                        out.push((pos, Tok::Err(ErrorKind::parse(e).unwrap_or(ErrorKind::NA))));
                    } else {
                        self.i += 1;
                        out.push((pos, Tok::Hash));
                    }
                }
                b'0'..=b'9' | b'.'
                    if c != b'.' || self.s.get(self.i + 1).is_some_and(|d| d.is_ascii_digit()) =>
                {
                    let start = self.i;
                    while self.i < self.s.len()
                        && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.')
                    {
                        self.i += 1;
                    }
                    if self.i < self.s.len() && (self.s[self.i] == b'e' || self.s[self.i] == b'E') {
                        let save = self.i;
                        self.i += 1;
                        if self.i < self.s.len()
                            && (self.s[self.i] == b'+' || self.s[self.i] == b'-')
                        {
                            self.i += 1;
                        }
                        if self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                                self.i += 1;
                            }
                        } else {
                            self.i = save;
                        }
                    }
                    // a number immediately followed by letters is a cell ref like 1:1? no — treat "2A" as error
                    let text = &self.src[start..self.i];
                    let n: f64 = text.parse().map_err(|_| self.err("bad number"))?;
                    out.push((pos, Tok::Num(n)));
                }
                b'\'' => {
                    // quoted sheet name
                    self.i += 1;
                    let mut name = String::new();
                    loop {
                        let Some(&ch) = self.s.get(self.i) else {
                            return Err(self.err("unterminated sheet name"));
                        };
                        if ch == b'\'' {
                            if self.s.get(self.i + 1) == Some(&b'\'') {
                                name.push('\'');
                                self.i += 2;
                                continue;
                            }
                            self.i += 1;
                            break;
                        }
                        let l = utf8_len(ch);
                        name.push_str(&self.src[self.i..self.i + l]);
                        self.i += l;
                    }
                    if self.s.get(self.i) != Some(&b'!') {
                        return Err(self.err("expected ! after sheet name"));
                    }
                    self.i += 1;
                    let text = self.ident_text();
                    out.push((
                        pos,
                        Tok::Ident {
                            sheet: Some(name),
                            text,
                        },
                    ));
                }
                b'[' => {
                    let spec = self.bracket()?;
                    out.push((pos, Tok::Structured { table: None, spec }));
                }
                c if Self::ident_char(c, self.r1c1) => {
                    let text = self.ident_text();
                    if self.s.get(self.i) == Some(&b'!') {
                        self.i += 1;
                        let t2 = self.ident_text();
                        out.push((
                            pos,
                            Tok::Ident {
                                sheet: Some(text),
                                text: t2,
                            },
                        ));
                    } else if self.s.get(self.i) == Some(&b'[') && !self.r1c1 {
                        let spec = self.bracket()?;
                        out.push((
                            pos,
                            Tok::Structured {
                                table: Some(text),
                                spec,
                            },
                        ));
                    } else {
                        out.push((pos, Tok::Ident { sheet: None, text }));
                    }
                }
                b'(' => {
                    self.i += 1;
                    out.push((pos, Tok::LParen));
                }
                b')' => {
                    self.i += 1;
                    out.push((pos, Tok::RParen));
                }
                b'{' => {
                    self.i += 1;
                    out.push((pos, Tok::LBrace));
                }
                b'}' => {
                    self.i += 1;
                    out.push((pos, Tok::RBrace));
                }
                b',' => {
                    self.i += 1;
                    out.push((pos, Tok::Comma));
                }
                b';' => {
                    self.i += 1;
                    out.push((pos, Tok::Semi));
                }
                b'@' => {
                    self.i += 1;
                    out.push((pos, Tok::At));
                }
                b'%' => {
                    self.i += 1;
                    out.push((pos, Tok::Percent));
                }
                b'<' | b'>' => {
                    let two = &self.src[self.i..(self.i + 2).min(self.s.len())];
                    let op = if two == "<=" || two == ">=" || two == "<>" {
                        two
                    } else {
                        &self.src[self.i..self.i + 1]
                    };
                    self.i += op.len();
                    out.push((pos, Tok::Op(op.to_string())));
                }
                b'+' | b'-' | b'*' | b'/' | b'^' | b'&' | b'=' | b':' => {
                    self.i += 1;
                    out.push((pos, Tok::Op((c as char).to_string())));
                }
                _ => return Err(self.err(&format!("unexpected character '{}'", c as char))),
            }
        }
        Ok(out)
    }

    fn ident_text(&mut self) -> String {
        let start = self.i;
        while self.i < self.s.len() && Self::ident_char(self.s[self.i], self.r1c1) {
            self.i += 1;
        }
        self.src[start..self.i].to_string()
    }
}

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Strip Excel's future-function prefixes (`_xlfn.`, `_xlws.`, `_xlpm.`).
pub fn strip_prefixes(name: &str) -> &str {
    let mut n = name;
    for p in [
        "_xlfn.", "_xlws.", "_xlpm.", "_xleta.", "_XLFN.", "_XLWS.", "_XLPM.", "_XLETA.",
    ] {
        if let Some(rest) = n.strip_prefix(p) {
            n = rest;
        }
    }
    n
}

struct Parser {
    toks: Vec<(usize, Tok)>,
    i: usize,
    base: CellRef,
    r1c1: bool,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|t| &t.1)
    }
    fn pos(&self) -> usize {
        self.toks.get(self.i).map(|t| t.0).unwrap_or(usize::MAX)
    }
    fn err<T>(&self, msg: &str) -> Result<T, ParseError> {
        Err(ParseError {
            pos: self.pos(),
            msg: msg.into(),
        })
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).map(|t| t.1.clone());
        self.i += 1;
        t
    }
    fn eat_op(&mut self, ops: &[&str]) -> Option<String> {
        if let Some(Tok::Op(o)) = self.peek() {
            if ops.contains(&o.as_str()) {
                let o = o.clone();
                self.i += 1;
                return Some(o);
            }
        }
        None
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.concat()?;
        while let Some(op) = self.eat_op(&["=", "<>", "<", "<=", ">", ">="]) {
            let r = self.concat()?;
            let op = match op.as_str() {
                "=" => BinOp::Eq,
                "<>" => BinOp::Ne,
                "<" => BinOp::Lt,
                "<=" => BinOp::Le,
                ">" => BinOp::Gt,
                _ => BinOp::Ge,
            };
            l = Expr::Binary(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn concat(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.additive()?;
        while self.eat_op(&["&"]).is_some() {
            let r = self.additive()?;
            l = Expr::Binary(BinOp::Concat, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn additive(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.multiplicative()?;
        while let Some(op) = self.eat_op(&["+", "-"]) {
            let r = self.multiplicative()?;
            l = Expr::Binary(
                if op == "+" { BinOp::Add } else { BinOp::Sub },
                Box::new(l),
                Box::new(r),
            );
        }
        Ok(l)
    }

    fn multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.power()?;
        while let Some(op) = self.eat_op(&["*", "/"]) {
            let r = self.power()?;
            l = Expr::Binary(
                if op == "*" { BinOp::Mul } else { BinOp::Div },
                Box::new(l),
                Box::new(r),
            );
        }
        Ok(l)
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.unary()?;
        while self.eat_op(&["^"]).is_some() {
            let r = self.unary()?;
            l = Expr::Binary(BinOp::Pow, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if let Some(op) = self.eat_op(&["-", "+"]) {
            let e = self.unary()?;
            return Ok(Expr::Unary(
                if op == "-" { UnOp::Neg } else { UnOp::Plus },
                Box::new(e),
            ));
        }
        if self.peek() == Some(&Tok::At) {
            self.i += 1;
            let e = self.unary()?;
            return Ok(Expr::Unary(UnOp::Implicit, Box::new(e)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.range()?;
        loop {
            match self.peek() {
                Some(Tok::Percent) => {
                    self.i += 1;
                    e = Expr::Unary(UnOp::Percent, Box::new(e));
                }
                Some(Tok::Hash) => {
                    self.i += 1;
                    e = Expr::Spill(Box::new(e));
                }
                Some(Tok::LParen)
                    if matches!(e, Expr::Call(..) | Expr::Invoke(..) | Expr::Name(_))
                        && matches!(e, Expr::Call(ref n, _) if n == "LAMBDA") =>
                {
                    let args = self.args()?;
                    e = Expr::Invoke(Box::new(e), args);
                }
                _ => return Ok(e),
            }
        }
    }

    fn range(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.primary()?;
        while self.eat_op(&[":"]).is_some() {
            let r = self.primary()?;
            l = match (l, r) {
                (
                    Expr::Ref(RefExpr {
                        sheet,
                        kind: RefKind::Cell(a),
                    }),
                    Expr::Ref(RefExpr {
                        sheet: s2,
                        kind: RefKind::Cell(b),
                    }),
                ) if s2.is_none() || s2 == sheet => Expr::Ref(RefExpr {
                    sheet,
                    kind: RefKind::Range(a, b),
                }),
                (l, r) => Expr::Binary(BinOp::Range, Box::new(l), Box::new(r)),
            };
        }
        Ok(l)
    }

    fn args(&mut self) -> Result<Vec<Expr>, ParseError> {
        // at '('
        self.i += 1;
        let mut args = Vec::new();
        if self.peek() == Some(&Tok::RParen) {
            self.i += 1;
            return Ok(args);
        }
        loop {
            if matches!(self.peek(), Some(Tok::Comma) | Some(Tok::RParen)) {
                args.push(Expr::Missing);
            } else {
                args.push(self.expr()?);
            }
            match self.next() {
                Some(Tok::Comma) => continue,
                Some(Tok::RParen) => return Ok(args),
                _ => return self.err("expected , or )"),
            }
        }
    }

    fn array(&mut self) -> Result<Expr, ParseError> {
        // at '{'
        self.i += 1;
        let mut rows = vec![vec![]];
        loop {
            let neg = self.eat_op(&["-"]).is_some();
            let v = match self.next() {
                Some(Tok::Num(n)) => Expr::Number(if neg { -n } else { n }),
                Some(Tok::Str(s)) if !neg => Expr::Text(s),
                Some(Tok::Err(e)) if !neg => Expr::Error(e),
                Some(Tok::Ident { sheet: None, text })
                    if !neg && text.eq_ignore_ascii_case("TRUE") =>
                {
                    Expr::Bool(true)
                }
                Some(Tok::Ident { sheet: None, text })
                    if !neg && text.eq_ignore_ascii_case("FALSE") =>
                {
                    Expr::Bool(false)
                }
                _ => return self.err("array constants may only contain literals"),
            };
            rows.last_mut().unwrap().push(v);
            match self.next() {
                Some(Tok::Comma) => {}
                Some(Tok::Semi) => rows.push(vec![]),
                Some(Tok::RBrace) => break,
                _ => return self.err("expected , ; or }"),
            }
        }
        let w = rows[0].len();
        if rows.iter().any(|r| r.len() != w) {
            return self.err("array rows must have equal length");
        }
        Ok(Expr::Array(rows))
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let pos = self.pos();
        match self.next() {
            Some(Tok::Num(n)) => {
                // row range 1:3
                if n.fract() == 0.0 && n >= 1.0 && n <= MAX_ROWS as f64 {
                    if let (Some(Tok::Op(o)), Some(Tok::Num(m))) = (
                        self.peek().cloned(),
                        self.toks.get(self.i + 1).map(|t| t.1.clone()),
                    ) {
                        if o == ":" && m.fract() == 0.0 && m >= 1.0 && m <= MAX_ROWS as f64 {
                            self.i += 2;
                            let a = Anchor {
                                cell: CellRef::new(n as u32 - 1, 0),
                                row_abs: false,
                                col_abs: false,
                            };
                            let b = Anchor {
                                cell: CellRef::new(m as u32 - 1, 0),
                                row_abs: false,
                                col_abs: false,
                            };
                            return Ok(Expr::Ref(RefExpr {
                                sheet: None,
                                kind: RefKind::Rows(a, b),
                            }));
                        }
                    }
                }
                Ok(Expr::Number(n))
            }
            Some(Tok::Str(s)) => Ok(Expr::Text(s)),
            Some(Tok::Err(e)) => Ok(Expr::Error(e)),
            Some(Tok::LParen) => {
                let e = self.expr()?;
                if self.next() != Some(Tok::RParen) {
                    return self.err("expected )");
                }
                Ok(e)
            }
            Some(Tok::LBrace) => {
                self.i -= 1;
                self.array()
            }
            Some(Tok::Structured { table, spec }) => Ok(Expr::Structured { table, spec }),
            Some(Tok::Ident { sheet, text }) => {
                if self.peek() == Some(&Tok::LParen) && sheet.is_none() {
                    let name = strip_prefixes(&text).to_ascii_uppercase();
                    let args = self.args()?;
                    return Ok(Expr::Call(name, args));
                }
                if let Some(r) = self.reference(&sheet, &text) {
                    return Ok(r);
                }
                if sheet.is_none() && text.eq_ignore_ascii_case("TRUE") {
                    return Ok(Expr::Bool(true));
                }
                if sheet.is_none() && text.eq_ignore_ascii_case("FALSE") {
                    return Ok(Expr::Bool(false));
                }
                if sheet.is_some() {
                    return Err(ParseError {
                        pos,
                        msg: format!("invalid reference {text}"),
                    });
                }
                Ok(Expr::Name(strip_prefixes(&text).to_string()))
            }
            _ => Err(ParseError {
                pos,
                msg: "unexpected token".into(),
            }),
        }
    }

    fn reference(&mut self, sheet: &Option<String>, text: &str) -> Option<Expr> {
        let anchor = |c: CellRef, ra: bool, ca: bool| Anchor {
            cell: c,
            row_abs: ra,
            col_abs: ca,
        };
        let cell = if self.r1c1 {
            parse_r1c1(text, self.base).map(|(c, ra, ca)| anchor(c, ra, ca))
        } else {
            parse_a1(text).map(|(c, ca, ra)| anchor(c, ra, ca))
        };
        if let Some(a) = cell {
            return Some(Expr::Ref(RefExpr {
                sheet: sheet.clone(),
                kind: RefKind::Cell(a),
            }));
        }
        // whole columns A:C
        if !self.r1c1 {
            let col = |t: &str| {
                let (abs, t) = t.strip_prefix('$').map_or((false, t), |r| (true, r));
                col_index(t).map(|c| anchor(CellRef::new(0, c), false, abs))
            };
            if let Some(a) = col(text) {
                if let (
                    Some(Tok::Op(o)),
                    Some(Tok::Ident {
                        sheet: None,
                        text: t2,
                    }),
                ) = (
                    self.peek().cloned(),
                    self.toks.get(self.i + 1).map(|t| t.1.clone()),
                ) {
                    if o == ":" {
                        if let Some(b) = col(&t2) {
                            self.i += 2;
                            return Some(Expr::Ref(RefExpr {
                                sheet: sheet.clone(),
                                kind: RefKind::Cols(a, b),
                            }));
                        }
                    }
                }
            }
        }
        None
    }
}

/// Parse a formula (without the leading `=`) entered at `base`.
pub fn parse(src: &str, base: CellRef) -> Result<Expr, ParseError> {
    parse_mode(src, base, false)
}

pub fn parse_mode(src: &str, base: CellRef, r1c1: bool) -> Result<Expr, ParseError> {
    let src = src.strip_prefix('=').unwrap_or(src);
    let toks = Lexer {
        s: src.as_bytes(),
        src,
        i: 0,
        r1c1,
    }
    .tokens()?;
    let mut p = Parser {
        toks,
        i: 0,
        base,
        r1c1,
    };
    let e = p.expr()?;
    if p.i < p.toks.len() {
        return p.err("unexpected input after expression");
    }
    Ok(e)
}

// ---- serialisation ------------------------------------------------------------------

/// Functions Excel stores with a `_xlfn.` prefix (introduced after 2007).
pub const FUTURE_FUNCTIONS: &[&str] = &[
    "XLOOKUP",
    "XMATCH",
    "FILTER",
    "SORT",
    "SORTBY",
    "UNIQUE",
    "SEQUENCE",
    "RANDARRAY",
    "LET",
    "LAMBDA",
    "MAP",
    "REDUCE",
    "SCAN",
    "BYROW",
    "BYCOL",
    "MAKEARRAY",
    "ISOMITTED",
    "TEXTSPLIT",
    "TEXTBEFORE",
    "TEXTAFTER",
    "VSTACK",
    "HSTACK",
    "TOCOL",
    "TOROW",
    "WRAPROWS",
    "WRAPCOLS",
    "TAKE",
    "DROP",
    "CHOOSEROWS",
    "CHOOSECOLS",
    "EXPAND",
    "GROUPBY",
    "PIVOTBY",
    "PERCENTOF",
    "TRIMRANGE",
    "REGEXTEST",
    "REGEXEXTRACT",
    "REGEXREPLACE",
    "IFS",
    "SWITCH",
    "MAXIFS",
    "MINIFS",
    "CONCAT",
    "TEXTJOIN",
    "IFNA",
    "XOR",
    "STDEV.S",
    "STDEV.P",
    "VAR.S",
    "VAR.P",
    "PY",
    "ARRAYTOTEXT",
    "VALUETOTEXT",
    "DAYS",
    "ISOWEEKNUM",
    "NUMBERVALUE",
    "UNICHAR",
    "UNICODE",
    "CEILING.MATH",
    "FLOOR.MATH",
    "MODE.SNGL",
    "PERCENTILE.INC",
    "QUARTILE.INC",
    "RANK.EQ",
    "NORM.DIST",
    "NORM.INV",
    "AGGREGATE",
    "FORMULATEXT",
    "ISFORMULA",
    "SHEET",
    "SHEETS",
    "NETWORKDAYS.INTL",
    "WORKDAY.INTL",
    "MUNIT",
];

/// Functions that additionally live in the `_xlws.` namespace.
const WS_FUNCTIONS: &[&str] = &["FILTER", "SORT", "PY"];

#[derive(Clone, Copy, Debug, Default)]
pub struct WriteOptions {
    /// Add `_xlfn.` / `_xlpm.` prefixes for file storage.
    pub file_prefixes: bool,
    /// Write references in R1C1 relative to `base`.
    pub r1c1: bool,
    pub base: CellRef,
}

fn quote_sheet(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !s.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

fn anchor_text(a: &Anchor, o: &WriteOptions) -> String {
    if o.r1c1 {
        r1c1(a.cell, o.base, a.row_abs, a.col_abs)
    } else {
        format!(
            "{}{}{}{}",
            if a.col_abs { "$" } else { "" },
            col_name(a.cell.col),
            if a.row_abs { "$" } else { "" },
            a.cell.row + 1
        )
    }
}

pub fn ref_text(r: &RefExpr, o: &WriteOptions) -> String {
    let sheet = r
        .sheet
        .as_ref()
        .map(|s| format!("{}!", quote_sheet(s)))
        .unwrap_or_default();
    let body = match &r.kind {
        RefKind::Cell(a) => anchor_text(a, o),
        RefKind::Range(a, b) => format!("{}:{}", anchor_text(a, o), anchor_text(b, o)),
        RefKind::Cols(a, b) => format!(
            "{}{}:{}{}",
            if a.col_abs { "$" } else { "" },
            col_name(a.cell.col),
            if b.col_abs { "$" } else { "" },
            col_name(b.cell.col)
        ),
        RefKind::Rows(a, b) => format!(
            "{}{}:{}{}",
            if a.row_abs { "$" } else { "" },
            a.cell.row + 1,
            if b.row_abs { "$" } else { "" },
            b.cell.row + 1
        ),
    };
    format!("{sheet}{body}")
}

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Binary(op, ..) => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 1,
            BinOp::Concat => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div => 4,
            BinOp::Pow => 5,
            BinOp::Range => 8,
        },
        Expr::Unary(UnOp::Percent, _) => 6,
        Expr::Unary(..) => 7,
        _ => 9,
    }
}

fn num_text(n: f64) -> String {
    crate::value::format_number(n)
}

/// Serialise an expression back to formula text (without `=`).
pub fn to_formula(e: &Expr, o: &WriteOptions) -> String {
    fmt(e, o, &[])
}

/// `scope`: LET/LAMBDA parameter names visible at this point.
fn fmt(e: &Expr, o: &WriteOptions, scope: &[String]) -> String {
    let wrap = |child: &Expr, min: u8| {
        let s = fmt(child, o, scope);
        if prec(child) < min {
            format!("({s})")
        } else {
            s
        }
    };
    match e {
        Expr::Number(n) => num_text(*n),
        Expr::Text(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        Expr::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        Expr::Error(k) => k.code().into(),
        Expr::Array(rows) => format!(
            "{{{}}}",
            rows.iter()
                .map(|r| r
                    .iter()
                    .map(|x| fmt(x, o, scope))
                    .collect::<Vec<_>>()
                    .join(","))
                .collect::<Vec<_>>()
                .join(";")
        ),
        Expr::Ref(r) => ref_text(r, o),
        Expr::Name(n) if o.file_prefixes && scope.iter().any(|p| p.eq_ignore_ascii_case(n)) => {
            format!("_xlpm.{n}")
        }
        Expr::Name(n)
            if o.file_prefixes && crate::functions::is_builtin(&n.to_ascii_uppercase()) =>
        {
            format!("_xleta.{}", n.to_ascii_uppercase())
        }
        Expr::Name(n) => n.clone(),
        Expr::Unary(op, x) => match op {
            UnOp::Neg => format!("-{}", wrap(x, 7)),
            UnOp::Plus => format!("+{}", wrap(x, 7)),
            UnOp::Percent => format!("{}%", wrap(x, 6)),
            UnOp::Implicit => format!("@{}", wrap(x, 7)),
        },
        Expr::Binary(op, l, r) => {
            let p = prec(e);
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Pow => "^",
                BinOp::Concat => "&",
                BinOp::Eq => "=",
                BinOp::Ne => "<>",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
                BinOp::Range => ":",
            };
            // left-associative: right operand needs parens at equal precedence
            format!("{}{}{}", wrap(l, p), sym, wrap(r, p + 1))
        }
        Expr::Call(name, args) => {
            let shown = if o.file_prefixes && FUTURE_FUNCTIONS.contains(&name.as_str()) {
                if WS_FUNCTIONS.contains(&name.as_str()) {
                    format!("_xlfn._xlws.{name}")
                } else {
                    format!("_xlfn.{name}")
                }
            } else {
                name.clone()
            };
            let lambda_params: &[Expr] = if name == "LAMBDA" && !args.is_empty() {
                &args[..args.len() - 1]
            } else {
                &[]
            };
            let let_params: Vec<usize> = if name == "LET" {
                (0..args.len().saturating_sub(1)).step_by(2).collect()
            } else {
                vec![]
            };
            let mut inner: Vec<String> = scope.to_vec();
            if name == "LAMBDA" {
                inner.extend(lambda_params.iter().filter_map(|p| {
                    if let Expr::Name(n) = p {
                        Some(n.clone())
                    } else {
                        None
                    }
                }));
            }
            let mut parts: Vec<String> = Vec::with_capacity(args.len());
            for (i, a) in args.iter().enumerate() {
                parts.push(match a {
                    Expr::Name(n)
                        if o.file_prefixes
                            && (i < lambda_params.len() || let_params.contains(&i)) =>
                    {
                        format!("_xlpm.{n}")
                    }
                    Expr::Missing => String::new(),
                    _ => fmt(a, o, &inner),
                });
                if let_params.contains(&i) {
                    if let Expr::Name(n) = a {
                        inner.push(n.clone());
                    }
                }
            }
            format!("{shown}({})", parts.join(","))
        }
        Expr::Invoke(f, args) => format!(
            "{}({})",
            fmt(f, o, scope),
            args.iter()
                .map(|a| fmt(a, o, scope))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Expr::Spill(x) => format!("{}#", fmt(x, o, scope)),
        Expr::Structured { table, spec } => {
            format!("{}[{}]", table.clone().unwrap_or_default(), spec)
        }
        Expr::Missing => String::new(),
    }
}

// ---- shifting (fill, copy, shared formulas, row/column insert) ---------------------------

fn shift_anchor(a: &Anchor, dr: i64, dc: i64) -> Option<Anchor> {
    let row = if a.row_abs {
        a.cell.row as i64
    } else {
        a.cell.row as i64 + dr
    };
    let col = if a.col_abs {
        a.cell.col as i64
    } else {
        a.cell.col as i64 + dc
    };
    (row >= 0 && row < MAX_ROWS as i64 && col >= 0 && col < MAX_COLS as i64).then(|| Anchor {
        cell: CellRef::new(row as u32, col as u32),
        ..*a
    })
}

/// Move relative references by (dr, dc), as when copying a formula.
/// References pushed off the sheet become `#REF!`.
pub fn shift(e: &Expr, dr: i64, dc: i64) -> Expr {
    let s = |x: &Expr| shift(x, dr, dc);
    match e {
        Expr::Ref(r) => {
            let kind = match &r.kind {
                RefKind::Cell(a) => shift_anchor(a, dr, dc).map(RefKind::Cell),
                RefKind::Range(a, b) => shift_anchor(a, dr, dc)
                    .zip(shift_anchor(b, dr, dc))
                    .map(|(a, b)| RefKind::Range(a, b)),
                RefKind::Cols(a, b) => shift_anchor(a, 0, dc)
                    .zip(shift_anchor(b, 0, dc))
                    .map(|(a, b)| RefKind::Cols(a, b)),
                RefKind::Rows(a, b) => shift_anchor(a, dr, 0)
                    .zip(shift_anchor(b, dr, 0))
                    .map(|(a, b)| RefKind::Rows(a, b)),
            };
            match kind {
                Some(kind) => Expr::Ref(RefExpr {
                    sheet: r.sheet.clone(),
                    kind,
                }),
                None => Expr::Error(ErrorKind::Ref),
            }
        }
        Expr::Unary(op, x) => Expr::Unary(*op, Box::new(s(x))),
        Expr::Binary(op, l, r) => Expr::Binary(*op, Box::new(s(l)), Box::new(s(r))),
        Expr::Call(n, a) => Expr::Call(n.clone(), a.iter().map(s).collect()),
        Expr::Invoke(f, a) => Expr::Invoke(Box::new(s(f)), a.iter().map(s).collect()),
        Expr::Spill(x) => Expr::Spill(Box::new(s(x))),
        Expr::Array(rows) => Expr::Array(rows.iter().map(|r| r.iter().map(s).collect()).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(src: &str) -> String {
        to_formula(
            &parse(src, CellRef::new(0, 0)).unwrap(),
            &WriteOptions::default(),
        )
    }

    #[test]
    fn precedence_and_roundtrip() {
        assert_eq!(rt("=1+2*3"), "1+2*3");
        assert_eq!(rt("(1+2)*3"), "(1+2)*3");
        assert_eq!(rt("-2^2"), "-2^2");
        let e = parse("-2^2", CellRef::new(0, 0)).unwrap();
        assert!(
            matches!(e, Expr::Binary(BinOp::Pow, ..)),
            "negation binds tighter than ^ in Excel"
        );
        assert_eq!(rt("1-(2-3)"), "1-(2-3)");
        assert_eq!(rt("A1&\"x\"\"y\"=B$2"), "A1&\"x\"\"y\"=B$2");
        assert_eq!(
            rt("SUM(A1:B10,Sheet2!C:C,'My Sheet'!$A$1,3:5)"),
            "SUM(A1:B10,Sheet2!C:C,'My Sheet'!$A$1,3:5)"
        );
        assert_eq!(rt("{1,2;3,-4}"), "{1,2;3,-4}");
        assert_eq!(rt("IF(A1,,2)"), "IF(A1,,2)");
        assert_eq!(rt("50%"), "50%");
        assert_eq!(rt("A1#"), "A1#");
        assert_eq!(rt("@A1:A3"), "@A1:A3");
        assert_eq!(rt("Table1[Sales]+[@Qty]"), "Table1[Sales]+[@Qty]");
        assert_eq!(rt("LAMBDA(x,y,x+y)(1,2)"), "LAMBDA(x,y,x+y)(1,2)");
        assert_eq!(rt("_xlfn.XLOOKUP(1,A:A,B:B)"), "XLOOKUP(1,A:A,B:B)");
        assert_eq!(rt("#N/A"), "#N/A");
        assert_eq!(rt("1.5E+20*2"), "1.5E+20*2");
        assert_eq!(rt("TRUE<>FALSE"), "TRUE<>FALSE");
        assert_eq!(rt("\"ə₼\"&LEN(\"ü\")"), "\"ə₼\"&LEN(\"ü\")");
    }

    #[test]
    fn file_prefixes() {
        let e = parse(
            "LET(x,1,LAMBDA(y,XLOOKUP(y,A:A,B:B))(x))+SORT(A1:A3)",
            CellRef::new(0, 0),
        )
        .unwrap();
        let s = to_formula(
            &e,
            &WriteOptions {
                file_prefixes: true,
                ..Default::default()
            },
        );
        assert_eq!(s, "_xlfn.LET(_xlpm.x,1,_xlfn.LAMBDA(_xlpm.y,_xlfn.XLOOKUP(_xlpm.y,A:A,B:B))(_xlpm.x))+_xlfn._xlws.SORT(A1:A3)");
        // and reading it back strips them
        assert_eq!(
            to_formula(
                &parse(&s, CellRef::new(0, 0)).unwrap(),
                &WriteOptions::default()
            ),
            "LET(x,1,LAMBDA(y,XLOOKUP(y,A:A,B:B))(x))+SORT(A1:A3)"
        );
    }

    #[test]
    fn r1c1_mode() {
        let base = CellRef::new(4, 2);
        let e = parse_mode("=R[-1]C+R1C1", base, true).unwrap();
        assert_eq!(to_formula(&e, &WriteOptions::default()), "C4+$A$1");
        assert_eq!(
            to_formula(
                &e,
                &WriteOptions {
                    r1c1: true,
                    base,
                    ..Default::default()
                }
            ),
            "R[-1]C+R1C1"
        );
    }

    #[test]
    fn shifting_relative_refs() {
        let e = parse("A1+$B1+C$1+$D$1+SUM(A1:A3)+E:E", CellRef::new(0, 0)).unwrap();
        assert_eq!(
            to_formula(&shift(&e, 2, 1), &WriteOptions::default()),
            "B3+$B3+D$1+$D$1+SUM(B3:B5)+F:F"
        );
        assert_eq!(
            to_formula(&shift(&e, -1, 0), &WriteOptions::default()),
            "#REF!+#REF!+C$1+$D$1+SUM(#REF!)+E:E"
        );
    }

    #[test]
    fn errors() {
        for bad in ["1+", "SUM(1,2", "{1,2;3}", "\"abc", "A1 B1", "'S'A1"] {
            assert!(parse(bad, CellRef::new(0, 0)).is_err(), "{bad}");
        }
    }
}
