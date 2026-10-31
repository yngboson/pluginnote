//! 선언형 플러그인: JSON 스펙 + 안전한 수식 평가기. JS 코드를 실행하지 않는다.
//! 앱스토어 심사/보안 면에서 1순위 형식이다.

use crate::manifest::Manifest;
use note_core::{Attachment, DrawCmd, PluginOutput, Readout, SnapPoint, Style};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Spec {
    #[serde(flatten)]
    pub manifest: Manifest,
    /// 순서대로 평가되는 중간 변수
    #[serde(default)]
    pub vars: Vec<Var>,
    #[serde(default)]
    pub draw: Vec<Op>,
    #[serde(default)]
    pub attach: Vec<AttachSpec>,
    #[serde(default)]
    pub snaps: Vec<SnapSpec>,
    #[serde(default)]
    pub readouts: Vec<ReadoutSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Var {
    pub name: String,
    pub expr: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct AttachSpec {
    pub name: String,
    pub draw: Vec<Op>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct SnapSpec {
    pub name: String,
    pub at: [String; 2],
}
#[derive(Debug, Clone, Deserialize)]
pub struct ReadoutSpec {
    pub label: String,
    pub expr: String,
    #[serde(default)]
    pub formula: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Line { from: [String; 2], to: [String; 2], #[serde(default)] style: Option<Value> },
    Arrow { from: [String; 2], to: [String; 2], #[serde(default)] style: Option<Value> },
    Circle { cx: String, cy: String, r: String, #[serde(default)] style: Option<Value> },
    Arc { cx: String, cy: String, r: String, start: String, end: String, #[serde(default)] style: Option<Value> },
    Polyline { points: Vec<[String; 2]>, #[serde(default)] style: Option<Value> },
    Text { at: [String; 2], text: String, #[serde(default)] size: Option<f64>, #[serde(default)] style: Option<Value> },
}

pub fn style(v: &Option<Value>) -> Result<Style, String> {
    match v {
        None => Ok(named("main")?),
        Some(Value::String(s)) => named(s),
        Some(o) => serde_json::from_value(o.clone()).map_err(|e| format!("style: {e}")),
    }
}

fn named(n: &str) -> Result<Style, String> {
    Ok(match n {
        "main" => Style { stroke: "#111111".into(), width: 1.8, dash: None, fill: None },
        "gray" => Style { stroke: "#9a9a9a".into(), width: 1.0, dash: Some(vec![6.0, 4.0]), fill: None },
        "accent" => Style { stroke: "#d33a2c".into(), width: 1.8, dash: None, fill: None },
        _ => return Err(format!("unknown style: {n}")),
    })
}

pub fn render(spec: &Spec, params: &Map<String, Value>) -> Result<PluginOutput, String> {
    let mut env: HashMap<String, f64> = HashMap::new();
    for (k, v) in params {
        if let Some(x) = v.as_f64() {
            env.insert(k.clone(), x);
        }
    }
    for v in &spec.vars {
        let x = eval(&v.expr, &env).map_err(|e| format!("var {}: {e}", v.name))?;
        env.insert(v.name.clone(), x);
    }
    let mut out = PluginOutput::default();
    for op in &spec.draw {
        out.draws.push(op_to_cmd(op, &env)?);
    }
    for a in &spec.attach {
        let draws = a.draw.iter().map(|o| op_to_cmd(o, &env)).collect::<Result<_, _>>()?;
        out.attachments.push(Attachment { name: a.name.clone(), draws });
    }
    for s in &spec.snaps {
        out.snaps.push(SnapPoint { name: s.name.clone(), at: pt(&s.at, &env)? });
    }
    for r in &spec.readouts {
        let v = eval(&r.expr, &env).map_err(|e| format!("readout {}: {e}", r.label))?;
        out.readouts.push(Readout { label: r.label.clone(), value: Value::from(v), formula: r.formula.clone() });
    }
    Ok(out)
}

fn pt(p: &[String; 2], env: &HashMap<String, f64>) -> Result<[f64; 2], String> {
    Ok([eval(&p[0], env)?, eval(&p[1], env)?])
}

fn op_to_cmd(op: &Op, env: &HashMap<String, f64>) -> Result<DrawCmd, String> {
    Ok(match op {
        Op::Line { from, to, style: s } => DrawCmd::Line { from: pt(from, env)?, to: pt(to, env)?, style: style(s)? },
        Op::Arrow { from, to, style: s } => DrawCmd::Arrow { from: pt(from, env)?, to: pt(to, env)?, style: style(s)? },
        Op::Circle { cx, cy, r, style: s } => DrawCmd::Circle { c: [eval(cx, env)?, eval(cy, env)?], r: eval(r, env)?, style: style(s)? },
        Op::Arc { cx, cy, r, start, end, style: s } => DrawCmd::Arc {
            c: [eval(cx, env)?, eval(cy, env)?],
            r: eval(r, env)?,
            start: eval(start, env)?,
            end: eval(end, env)?,
            style: style(s)?,
        },
        Op::Polyline { points, style: s } => DrawCmd::Polyline { points: points.iter().map(|p| pt(p, env)).collect::<Result<_, _>>()?, style: style(s)? },
        Op::Text { at, text, size, style: s } => DrawCmd::Text { at: pt(at, env)?, text: text.clone(), size: size.unwrap_or(12.0), style: style(s)? },
    })
}

// ---- 수식 평가기: 숫자, 변수, + - * / ^, 괄호, 함수. 그 외는 모두 오류 ----

pub fn eval(src: &str, env: &HashMap<String, f64>) -> Result<f64, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { t: &toks, i: 0, env, depth: 0 };
    let v = p.expr()?;
    if p.i != toks.len() {
        return Err(format!("수식 끝에 남은 토큰: '{src}'"));
    }
    if !v.is_finite() {
        return Err(format!("'{src}' 의 결과가 유한하지 않음"));
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(char),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let (mut i, mut out) = (0, vec![]);
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            // 지수 표기 1e-3
            if i < cs.len() && (cs[i] == 'e' || cs[i] == 'E') {
                let mut j = i + 1;
                if j < cs.len() && (cs[j] == '+' || cs[j] == '-') {
                    j += 1;
                }
                if j < cs.len() && cs[j].is_ascii_digit() {
                    while j < cs.len() && cs[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let t: String = cs[st..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("잘못된 숫자: {t}"))?));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            out.push(Tok::Id(cs[st..i].iter().collect()));
        } else if "+-*/^(),".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else {
            return Err(format!("허용되지 않은 문자 '{c}' in '{s}'"));
        }
    }
    Ok(out)
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
    env: &'a HashMap<String, f64>,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Op(c)) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<f64, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("수식 중첩이 너무 깊음".into());
        }
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                break;
            }
        }
        self.depth -= 1;
        Ok(v)
    }
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') {
                v *= self.unary()?;
            } else if self.eat('/') {
                v /= self.unary()?;
            } else {
                break;
            }
        }
        Ok(v)
    }
    fn unary(&mut self) -> Result<f64, String> {
        if self.eat('-') {
            return Ok(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        let b = self.atom()?;
        if self.eat('^') {
            return Ok(b.powf(self.unary()?));
        }
        Ok(b)
    }
    fn atom(&mut self) -> Result<f64, String> {
        match self.t.get(self.i).cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(n)
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let v = self.expr()?;
                if !self.eat(')') {
                    return Err("')' 누락".into());
                }
                Ok(v)
            }
            Some(Tok::Id(id)) => {
                self.i += 1;
                if self.eat('(') {
                    let mut args = vec![];
                    if !self.eat(')') {
                        loop {
                            args.push(self.expr()?);
                            if self.eat(')') {
                                break;
                            }
                            if !self.eat(',') {
                                return Err("',' 또는 ')' 필요".into());
                            }
                        }
                    }
                    call(&id, &args)
                } else if id == "pi" {
                    Ok(std::f64::consts::PI)
                } else {
                    self.env.get(&id).copied().ok_or_else(|| format!("정의되지 않은 변수: {id}"))
                }
            }
            t => Err(format!("예상치 못한 토큰: {t:?}")),
        }
    }
}

fn call(f: &str, a: &[f64]) -> Result<f64, String> {
    let n = |k: usize| if a.len() == k { Ok(()) } else { Err(format!("{f}() 인자 {k}개 필요")) };
    Ok(match f {
        "sin" => { n(1)?; a[0].sin() }
        "cos" => { n(1)?; a[0].cos() }
        "tan" => { n(1)?; a[0].tan() }
        "atan" => { n(1)?; a[0].atan() }
        "sqrt" => { n(1)?; a[0].sqrt() }
        "abs" => { n(1)?; a[0].abs() }
        "exp" => { n(1)?; a[0].exp() }
        "ln" => { n(1)?; a[0].ln() }
        "atan2" => { n(2)?; a[0].atan2(a[1]) }
        "hypot" => { n(2)?; a[0].hypot(a[1]) }
        "min" => { n(2)?; a[0].min(a[1]) }
        "max" => { n(2)?; a[0].max(a[1]) }
        "pow" => { n(2)?; a[0].powf(a[1]) }
        _ => return Err(format!("허용되지 않은 함수: {f}")),
    })
}
