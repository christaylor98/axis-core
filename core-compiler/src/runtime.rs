#![allow(dead_code)]
// Experimental runtime evaluator (not active in compiler pipeline)

// Minimal Axis Core evaluator for compiler
use std::collections::HashMap;
use std::rc::Rc;

// Source span for error reporting
#[derive(Clone, Debug)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

// Helper functions to access string table
fn intern_str(s: String) -> i64 {
    crate::intern_string(s)
}

fn get_str(handle: i64) -> String {
    crate::get_string(handle)
}

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(i64), // String handle into string table
    Unit,
    Tuple(Vec<Value>),
    Closure(Env, String, Rc<CoreTerm>),
    NativeFn(fn(Vec<Value>) -> Result<Value, i64>),
    Enum(String, Vec<Value>), // tag, fields
    // For surface lowering
    Var(String),
    Lam(String, Box<Value>),
    App(Box<Value>, Box<Value>),
    Let(String, Box<Value>, Box<Value>),
    If(Box<Value>, Box<Value>, Box<Value>), // cond, then, else
    Match(Box<Value>, Vec<(String, Value)>), // scrutinee, arms (pattern_str, body)
}

impl Value {
    pub fn as_int(&self) -> i64 {
        match self {
            Value::Int(n) => *n,
            _ => panic!("Expected Int, got {:?}", self),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            _ => panic!("Expected Bool"),
        }
    }

    pub fn as_str_handle(&self) -> i64 {
        match self {
            Value::Str(h) => *h,
            _ => panic!("Expected Str"),
        }
    }
}

pub type Env = Rc<HashMap<String, Value>>;

#[derive(Clone, Debug)]
pub enum CoreTerm {
    // Literals
    IntLit(i64, Option<Span>),
    BoolLit(bool, Option<Span>),
    UnitLit(Option<Span>),
    StrLit(String, Option<Span>), // Will be interned during evaluation
    // Variables
    Var(String, Option<Span>),
    // Lambda
    Lam(String, Rc<CoreTerm>, Option<Span>),
    // Application
    App(Rc<CoreTerm>, Rc<CoreTerm>, Option<Span>),
    // Tuple
    Tuple(Vec<CoreTerm>, Option<Span>),
    // Projection (1-based index)
    Proj(Rc<CoreTerm>, usize, Option<Span>),
    // Let binding
    Let(String, Rc<CoreTerm>, Rc<CoreTerm>, Option<Span>),
    // If expression
    If(Rc<CoreTerm>, Rc<CoreTerm>, Rc<CoreTerm>, Option<Span>),
    // Match expression
    Match(Rc<CoreTerm>, Vec<(Pattern, CoreTerm)>, Option<Span>),
    // Data constructor
    Ctor(String, Vec<CoreTerm>, Option<Span>),
}

#[derive(Clone, Debug)]
pub enum Pattern {
    PInt(i64),
    PBool(bool),
    PUnit,
    PVar(String),
    PTuple(Vec<Pattern>),
    PEnum(String, Vec<Pattern>), // Cons, Nil, Ok, Err, etc.
}

pub fn eval(term: &CoreTerm, env: &Env) -> Result<Value, i64> {
    match term {
        CoreTerm::IntLit(n, _) => Ok(Value::Int(*n)),
        CoreTerm::BoolLit(b, _) => Ok(Value::Bool(*b)),
        CoreTerm::UnitLit(_) => Ok(Value::Unit),
        CoreTerm::StrLit(s, _) => {
            let handle = intern_str(s.clone());
            Ok(Value::Str(handle))
        }
        CoreTerm::Var(name, _) => {
            env.get(name)
                .cloned()
                .ok_or_else(|| {
                    let err_msg = format!("Unbound variable: {}", name);
                    let handle = intern_str(err_msg);
                    -handle
                })
        }
        CoreTerm::Lam(param, body, _) => {
            Ok(Value::Closure(env.clone(), param.clone(), body.clone()))
        }
        CoreTerm::App(func_term, arg_term, _) => {
            let func = eval(func_term, env)?;
            let arg = eval(arg_term, env)?;
            apply(func, arg)
        }
        CoreTerm::Tuple(elems, _) => {
            let mut values = Vec::new();
            for elem in elems {
                values.push(eval(elem, env)?);
            }
            Ok(Value::Tuple(values))
        }
        CoreTerm::Proj(expr, index, _) => {
            let val = eval(expr, env)?;
            match val {
                Value::Tuple(elems) => {
                    // Axis uses 1-based indexing
                    let idx = index - 1;
                    if idx < elems.len() {
                        Ok(elems[idx].clone())
                    } else {
                        let err_msg = format!("Tuple index {} out of bounds", index);
                        let handle = intern_str(err_msg);
                        Err(-handle)
                    }
                }
                _ => {
                    let err_msg = "Projection on non-tuple".to_string();
                    let handle = intern_str(err_msg);
                    Err(-handle)
                }
            }
        }
        CoreTerm::Let(name, value_term, body_term, _) => {
            let value = eval(value_term, env)?;
            let mut new_env = (**env).clone();
            new_env.insert(name.clone(), value);
            eval(body_term, &Rc::new(new_env))
        }
        CoreTerm::If(cond_term, then_term, else_term, _) => {
            let cond = eval(cond_term, env)?;
            if cond.as_bool() {
                eval(then_term, env)
            } else {
                eval(else_term, env)
            }
        }
        CoreTerm::Match(scrutinee_term, arms, _) => {
            let scrutinee = eval(scrutinee_term, env)?;
            for (pattern, body) in arms {
                if let Some(bindings) = match_pattern(pattern, &scrutinee) {
                    let mut new_env = (**env).clone();
                    for (name, value) in bindings {
                        new_env.insert(name, value);
                    }
                    return eval(body, &Rc::new(new_env));
                }
            }
            let err_msg = "Non-exhaustive match".to_string();
            let handle = intern_str(err_msg);
            Err(-handle)
        }
        CoreTerm::Ctor(name, fields, _) => {
            let mut values = Vec::with_capacity(fields.len());
            for field in fields {
                values.push(eval(field, env)?);
            }
            Ok(Value::Enum(name.clone(), values))
        }
    }
}

fn apply(func: Value, arg: Value) -> Result<Value, i64> {
    match func {
        Value::Closure(env, param, body) => {
            let mut new_env = (*env).clone();
            new_env.insert(param, arg);
            eval(&body, &Rc::new(new_env))
        }
        Value::NativeFn(f) => f(vec![arg]),
        _ => {
            let err_msg = "Application of non-function".to_string();
            let handle = intern_str(err_msg);
            Err(-handle)
        }
    }
}

fn match_pattern(pattern: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
    match (pattern, value) {
        (Pattern::PInt(n), Value::Int(m)) if n == m => Some(vec![]),
        (Pattern::PBool(b), Value::Bool(c)) if b == c => Some(vec![]),
        (Pattern::PUnit, Value::Unit) => Some(vec![]),
        (Pattern::PVar(name), val) => Some(vec![(name.clone(), val.clone())]),
        (Pattern::PTuple(pat_elems), Value::Tuple(val_elems)) => {
            if pat_elems.len() != val_elems.len() {
                return None;
            }
            let mut bindings = Vec::new();
            for (pat, val) in pat_elems.iter().zip(val_elems.iter()) {
                match match_pattern(pat, val) {
                    Some(bs) => bindings.extend(bs),
                    None => return None,
                }
            }
            Some(bindings)
        }
        (Pattern::PEnum(tag1, pat_fields), Value::Enum(tag2, val_fields)) => {
            if tag1 != tag2 || pat_fields.len() != val_fields.len() {
                return None;
            }
            let mut bindings = Vec::new();
            for (pat, val) in pat_fields.iter().zip(val_fields.iter()) {
                match match_pattern(pat, val) {
                    Some(bs) => bindings.extend(bs),
                    None => return None,
                }
            }
            Some(bindings)
        }
        _ => None,
    }
}

pub fn empty_env() -> Env {
    Rc::new(HashMap::new())
}
