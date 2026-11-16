//! The part of Molang that animation files use: numbers, arithmetic, comparisons, `?:`, `&&`,
//! `||`, `!`, parentheses, `math.*` functions and `query.*`/`variable.*` reads. Written from the
//! public Bedrock Molang documentation. Unknown names read 0, as Molang reads them.

use std::collections::HashMap;

/// A parsed expression, evaluated once a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    /// `query.anim_time`, `v.speed`... normalised to the long prefix (`query.`, `variable.`,
    /// `context.`, `temp.`) and lower case.
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `math.<name>(args)`.
    Call(String, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    /// `a ?? b`: `a`, or `b` when `a` is missing. Unknown names read 0 here, so it is `a`.
    Coalesce,
}

/// Where an expression reads its `query.*` and `variable.*` values.
pub trait Scope {
    fn get(&self, name: &str) -> Option<f64>;
}

impl Scope for HashMap<String, f64> {
    fn get(&self, name: &str) -> Option<f64> {
        HashMap::get(self, name).copied()
    }
}

impl<F: Fn(&str) -> Option<f64>> Scope for F {
    fn get(&self, name: &str) -> Option<f64> {
        self(name)
    }
}

/// A parse error, with the byte offset it was found at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub at: usize,
    pub what: &'static str,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "molang: {} at {}", self.what, self.at)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(src: &str) -> Result<Expr, ParseError> {
    // A statement list (`a; b; return c;`) is beyond what animation values use; take the last
    // `return`, else the first statement.
    let src = src.trim();
    let body = match src.rfind("return") {
        Some(i) => src[i + 6..].trim_end_matches(';'),
        None => src.split(';').next().unwrap_or(""),
    };
    let mut p = Parser { s: body.as_bytes(), i: 0 };
    let e = p.ternary()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("unexpected text"));
    }
    Ok(e)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, what: &'static str) -> ParseError {
        ParseError { at: self.i, what }
    }

    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn eat(&mut self, tok: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(tok.as_bytes()) {
            self.i += tok.len();
            true
        } else {
            false
        }
    }

    fn ternary(&mut self) -> Result<Expr, ParseError> {
        let c = self.coalesce()?;
        if self.eat("?") {
            let a = self.ternary()?;
            if !self.eat(":") {
                return Err(self.err("expected ':'"));
            }
            let b = self.ternary()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn coalesce(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.or()?;
        while self.eat("??") {
            let r = self.or()?;
            l = Expr::Bin(Op::Coalesce, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.and()?;
        while self.eat("||") {
            let r = self.and()?;
            l = Expr::Bin(Op::Or, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.cmp()?;
        while self.eat("&&") {
            let r = self.cmp()?;
            l = Expr::Bin(Op::And, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn cmp(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.sum()?;
        loop {
            let op = if self.eat("<=") {
                Op::Le
            } else if self.eat(">=") {
                Op::Ge
            } else if self.eat("==") {
                Op::Eq
            } else if self.eat("!=") {
                Op::Ne
            } else if self.eat("<") {
                Op::Lt
            } else if self.eat(">") {
                Op::Gt
            } else {
                return Ok(l);
            };
            let r = self.sum()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
    }

    fn sum(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.product()?;
        loop {
            let op = if self.eat("+") {
                Op::Add
            } else if self.eat("-") {
                Op::Sub
            } else {
                return Ok(l);
            };
            let r = self.product()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
    }

    fn product(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.unary()?;
        loop {
            let op = if self.eat("*") {
                Op::Mul
            } else if self.eat("/") {
                Op::Div
            } else {
                return Ok(l);
            };
            let r = self.unary()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        self.ws();
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat("+") {
            return self.unary();
        }
        if self.s[self.i..].starts_with(b"!") && !self.s[self.i..].starts_with(b"!=") && self.eat("!") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        self.atom()
    }

    fn atom(&mut self) -> Result<Expr, ParseError> {
        self.ws();
        if self.eat("(") {
            let e = self.ternary()?;
            if !self.eat(")") {
                return Err(self.err("expected ')'"));
            }
            return Ok(e);
        }
        let start = self.i;
        let Some(&c) = self.s.get(self.i) else {
            return Err(self.err("expected a value"));
        };
        if c.is_ascii_digit() || c == b'.' {
            while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                self.i += 1;
            }
            let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
            // Java float literals sometimes leak into files: `0.5f`.
            if self.i < self.s.len() && matches!(self.s[self.i], b'f' | b'F') {
                self.i += 1;
            }
            return text.parse().map(Expr::Num).map_err(|_| self.err("bad number"));
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while self.i < self.s.len()
                && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'_' | b'.'))
            {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("").to_ascii_lowercase();
            if name == "true" {
                return Ok(Expr::Num(1.0));
            }
            if name == "false" {
                return Ok(Expr::Num(0.0));
            }
            if let Some(f) = name.strip_prefix("math.") {
                let f = f.to_owned();
                if f == "pi" {
                    return Ok(Expr::Num(std::f64::consts::PI));
                }
                let mut args = Vec::new();
                if self.eat("(") {
                    if !self.eat(")") {
                        loop {
                            args.push(self.ternary()?);
                            if self.eat(")") {
                                break;
                            }
                            if !self.eat(",") {
                                return Err(self.err("expected ',' or ')'"));
                            }
                        }
                    }
                }
                return Ok(Expr::Call(f, args));
            }
            // A query with arguments (`query.is_item_name_any(...)`): its arguments are read and
            // dropped, and the query reads its value from the scope.
            if self.eat("(") {
                let mut depth = 1;
                while self.i < self.s.len() && depth > 0 {
                    match self.s[self.i] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    self.i += 1;
                }
            }
            return Ok(Expr::Var(long_name(&name)));
        }
        Err(self.err("expected a value"))
    }
}

/// `q.x` → `query.x`, `v.x` → `variable.x`, `c.x` → `context.x`, `t.x` → `temp.x`.
fn long_name(name: &str) -> String {
    for (short, long) in [("q.", "query."), ("v.", "variable."), ("c.", "context."), ("t.", "temp.")] {
        if let Some(rest) = name.strip_prefix(short) {
            return format!("{long}{rest}");
        }
    }
    name.to_owned()
}

fn truth(v: f64) -> bool {
    v != 0.0
}

impl Expr {
    /// A constant when the expression reads nothing, so keyframes can skip evaluation.
    pub fn constant(&self) -> Option<f64> {
        match self {
            Expr::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn eval(&self, scope: &dyn Scope) -> f64 {
        match self {
            Expr::Num(n) => *n,
            Expr::Var(name) => scope.get(name).unwrap_or(0.0),
            Expr::Neg(e) => -e.eval(scope),
            Expr::Not(e) => f64::from(!truth(e.eval(scope))),
            Expr::Cond(c, a, b) => {
                if truth(c.eval(scope)) {
                    a.eval(scope)
                } else {
                    b.eval(scope)
                }
            }
            Expr::Bin(op, l, r) => {
                let a = l.eval(scope);
                match op {
                    Op::And => return f64::from(truth(a) && truth(r.eval(scope))),
                    Op::Or => return f64::from(truth(a) || truth(r.eval(scope))),
                    Op::Coalesce => return a,
                    _ => {}
                }
                let b = r.eval(scope);
                match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    // Molang divides by zero to 0 rather than infinity.
                    Op::Div => {
                        if b == 0.0 {
                            0.0
                        } else {
                            a / b
                        }
                    }
                    Op::Lt => f64::from(a < b),
                    Op::Le => f64::from(a <= b),
                    Op::Gt => f64::from(a > b),
                    Op::Ge => f64::from(a >= b),
                    Op::Eq => f64::from(a == b),
                    Op::Ne => f64::from(a != b),
                    Op::And | Op::Or | Op::Coalesce => unreachable!(),
                }
            }
            Expr::Call(f, args) => {
                let arg = |i: usize| args.get(i).map_or(0.0, |a| a.eval(scope));
                call(f, &arg)
            }
        }
    }
}

/// `math.*`: angles in degrees, as Molang's trigonometry takes them.
fn call(f: &str, arg: &dyn Fn(usize) -> f64) -> f64 {
    let (a, b, c) = (arg(0), arg(1), arg(2));
    match f {
        "sin" => a.to_radians().sin(),
        "cos" => a.to_radians().cos(),
        "asin" => a.asin().to_degrees(),
        "acos" => a.acos().to_degrees(),
        "atan" => a.atan().to_degrees(),
        "atan2" => a.atan2(b).to_degrees(),
        "abs" => a.abs(),
        "ceil" => a.ceil(),
        "floor" => a.floor(),
        "round" => a.round(),
        "trunc" => a.trunc(),
        "sqrt" => a.sqrt(),
        "exp" => a.exp(),
        "ln" => a.ln(),
        "pow" => a.powf(b),
        "mod" => {
            if b == 0.0 {
                0.0
            } else {
                a % b
            }
        }
        "min" => a.min(b),
        "max" => a.max(b),
        "clamp" => a.max(b).min(c),
        "lerp" => a + (b - a) * c,
        "lerprotate" => {
            let d = ((b - a) % 360.0 + 540.0) % 360.0 - 180.0;
            a + d * c
        }
        "inverse_lerp" => {
            if b == a {
                0.0
            } else {
                (c - a) / (b - a)
            }
        }
        "hermite_blend" => 3.0 * a * a - 2.0 * a * a * a,
        "sign" => {
            if a > 0.0 {
                1.0
            } else if a < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        "copy_sign" => a.abs().copysign(b),
        "min_angle" => ((a % 360.0) + 540.0) % 360.0 - 180.0,
        "to_rad" => a.to_radians(),
        "to_deg" => a.to_degrees(),
        // Random functions would differ per frame from the original's; a renderer that needs
        // them supplies its own value through a variable.
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(src: &str, vars: &[(&str, f64)]) -> f64 {
        let scope: HashMap<String, f64> = vars.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        parse(src).unwrap().eval(&scope)
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(eval("1 + 2 * 3 - 4 / 2", &[]), 5.0);
        assert_eq!(eval("-(2 + 3) * 2", &[]), -10.0);
        assert_eq!(eval("1 / 0", &[]), 0.0);
    }

    #[test]
    fn queries_with_short_names_and_unknowns() {
        assert_eq!(eval("q.anim_time * 2", &[("query.anim_time", 1.5)]), 3.0);
        assert_eq!(eval("variable.unset + 1", &[]), 1.0);
    }

    #[test]
    fn math_functions_take_degrees() {
        assert!((eval("math.sin(90)", &[]) - 1.0).abs() < 1e-12);
        assert!((eval("math.cos(query.anim_time * 180)", &[("query.anim_time", 1.0)]) + 1.0).abs() < 1e-12);
        assert_eq!(eval("math.clamp(5, 0, 2)", &[]), 2.0);
        assert_eq!(eval("math.lerp(0, 10, 0.25)", &[]), 2.5);
    }

    #[test]
    fn logic_and_conditionals() {
        assert_eq!(eval("q.is_on_ground ? 10 : 20", &[("query.is_on_ground", 1.0)]), 10.0);
        assert_eq!(eval("!q.is_on_ground && 1 >= 1", &[]), 1.0);
        assert_eq!(eval("2 < 1 || 3 != 3", &[]), 0.0);
    }

    #[test]
    fn statements_take_the_return() {
        assert_eq!(eval("v.a = 3; return 4 * 2;", &[]), 8.0);
    }
}
