// ANDL Loop 6: Value Runtime Implementation
// Extracted from emit_rust.rs generate_value_runtime()

use std::sync::{OnceLock, Mutex};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(u32),      // String handle
    Unit,
    Tuple(Vec<Value>),
    List(Vec<Value>),
    Ctor { tag: u32, fields: Vec<Value> }, // Constructor with tag and fields
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
            _ => panic!("Expected Bool, got {:?}", self),
        }
    }

    pub fn as_tuple(&self) -> &Vec<Value> {
        match self {
            Value::Tuple(elems) => elems,
            _ => panic!("Expected Tuple, got {:?}", self),
        }
    }

    pub fn as_list(&self) -> &Vec<Value> {
        match self {
            Value::List(elems) => elems,
            _ => panic!("Expected List, got {:?}", self),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(handle) => write!(f, "{}", get_str(*handle)),
            Value::Unit => write!(f, "()"),
            Value::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            },
            Value::List(elems) => {
                write!(f, "[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            },
            Value::Ctor { tag, fields } => {
                write!(f, "{}(", get_tag_name(*tag))?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", field)?;
                }
                write!(f, ")")
            },
        }
    }
}

// String table (thread-safe lazy statics)
static STRING_TABLE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static STRING_MAP: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

pub fn init_runtime() {
    let table_mutex = STRING_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let mut table = table_mutex.lock().unwrap();
    if table.is_empty() {
        table.push("".to_string()); // Reserve handle 0 for empty string
    }
    // ensure map exists
    STRING_MAP.get_or_init(|| Mutex::new(HashMap::new()));
}

pub fn intern_str(s: &str) -> u32 {
    let map_mutex = STRING_MAP.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map_mutex.lock().unwrap();
    if let Some(&handle) = map.get(s) {
        return handle;
    }
    let table_mutex = STRING_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let mut table = table_mutex.lock().unwrap();
    let handle = table.len() as u32;
    table.push(s.to_string());
    map.insert(s.to_string(), handle);
    handle
}

pub fn get_str(handle: u32) -> String {
    let table_mutex = STRING_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let table = table_mutex.lock().unwrap();
    table.get(handle as usize).cloned().unwrap_or_else(|| {
        // This should never happen in correct code - log the error
        eprintln!("[FATAL] get_str: invalid handle {} (table size: {})", handle, table.len());
        format!("<invalid-str-handle-{}>", handle)
    })
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Str(h) => *h != 0,
        Value::Unit => false,
        Value::Tuple(elems) => !elems.is_empty(),
        Value::List(elems) => !elems.is_empty(),
        Value::Ctor { .. } => true,
    }
}

// Tag table for constructors (similar to string table)
static TAG_TABLE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static TAG_MAP: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

pub fn intern_tag(name: &str) -> u32 {
    let map_mutex = TAG_MAP.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map_mutex.lock().unwrap();
    if let Some(&tag) = map.get(name) {
        return tag;
    }
    let table_mutex = TAG_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let mut table = table_mutex.lock().unwrap();
    let tag = table.len() as u32;
    table.push(name.to_string());
    map.insert(name.to_string(), tag);
    tag
}

pub fn get_tag_name(tag: u32) -> String {
    let table_mutex = TAG_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let table = table_mutex.lock().unwrap();
    table.get(tag as usize).cloned().unwrap_or_else(|| "Unknown".to_string())
}

// Arithmetic primitives - UNARY CONTRACT
pub fn __add__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_add(*y)),
                _ => Value::Int(0), // Error fallback
            }
        },
        _ => Value::Int(0),
    }
}

pub fn __sub__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_sub(*y)),
                _ => Value::Int(0),
            }
        },
        _ => Value::Int(0),
    }
}

pub fn __mul__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_mul(*y)),
                _ => Value::Int(0),
            }
        },
        _ => Value::Int(0),
    }
}

pub fn __div__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => {
                    if *y == 0 { Value::Int(0) } else { Value::Int(x / y) }
                },
                _ => Value::Int(0),
            }
        },
        _ => Value::Int(0),
    }
}

pub fn __mod__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => {
                    if *y == 0 { Value::Int(0) } else { Value::Int(x % y) }
                },
                _ => Value::Int(0),
            }
        },
        _ => Value::Int(0),
    }
}

// Comparison primitives - UNARY CONTRACT
pub fn __eq__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            Value::Bool(elems[0] == elems[1])
        },
        _ => Value::Bool(false),
    }
}

pub fn __neq__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            Value::Bool(elems[0] != elems[1])
        },
        _ => Value::Bool(false),
    }
}

pub fn __lt__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Bool(x < y),
                _ => Value::Bool(false),
            }
        },
        _ => Value::Bool(false),
    }
}

pub fn __lte__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Bool(x <= y),
                _ => Value::Bool(false),
            }
        },
        _ => Value::Bool(false),
    }
}

pub fn __gt__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Bool(x > y),
                _ => Value::Bool(false),
            }
        },
        _ => Value::Bool(false),
    }
}

pub fn __gte__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Int(x), Value::Int(y)) => Value::Bool(x >= y),
                _ => Value::Bool(false),
            }
        },
        _ => Value::Bool(false),
    }
}

// Logical primitives - UNARY CONTRACT
pub fn __and__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            Value::Bool(truthy(&elems[0]) && truthy(&elems[1]))
        },
        _ => Value::Bool(false),
    }
}

pub fn __or__(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            Value::Bool(truthy(&elems[0]) || truthy(&elems[1]))
        },
        _ => Value::Bool(false),
    }
}

pub fn __not__(a: Value) -> Value {
    Value::Bool(!truthy(&a))
}

// String primitives
pub fn str_len(s: Value) -> Value {
    match s {
        Value::Str(handle) => {
            let string = get_str(handle);
            Value::Int(string.len() as i64)
        },
        _ => Value::Int(0),
    }
}

pub fn str_char(s: Value, idx: Value) -> Value {
    match (s, idx) {
        (Value::Str(handle), Value::Int(i)) => {
            let string = get_str(handle);
            if let Some(c) = string.chars().nth(i as usize) {
                Value::Int(c as i64)
            } else {
                Value::Int(0)
            }
        },
        _ => Value::Int(0),
    }
}

pub fn str_char_at(s: Value, idx: Value) -> Value {
    str_char(s, idx)
}

pub fn str_slice(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 3 => {
            match (&elems[0], &elems[1], &elems[2]) {
                (Value::Str(handle), Value::Int(s_idx), Value::Int(e_idx)) => {
                    let string = get_str(*handle);
                    let start = (*s_idx).min(string.len() as i64) as usize;
                    let end = (*e_idx).min(string.len() as i64) as usize;
                    let slice = &string[start..end];
                    Value::Str(intern_str(slice))
                },
                _ => Value::Str(0),
            }
        },
        _ => Value::Str(0),
    }
}

pub fn str_to_int(s: Value) -> Value {
    match s {
        Value::Str(handle) => {
            let string = get_str(handle);
            Value::Int(string.parse().unwrap_or(0))
        },
        _ => Value::Int(0),
    }
}

pub fn str_concat(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::Str(h1), Value::Str(h2)) => {
                    let s1 = get_str(*h1);
                    let s2 = get_str(*h2);
                    Value::Str(intern_str(&format!("{}{}", s1, s2)))
                },
                _ => Value::Str(0),
            }
        },
        _ => Value::Str(0),
    }
}

pub fn __concat__(args: Value) -> Value {
    str_concat(args)
}

pub fn int_to_str(n: Value) -> Value {
    match n {
        Value::Int(i) => {
            let s = i.to_string();
            // Safety check: ensure we never return empty for valid integers
            if s.is_empty() {
                panic!("int_to_str: to_string() returned empty for {}", i);
            }
            let handle = intern_str(&s);
            // Verify the string can be retrieved correctly
            let retrieved = get_str(handle);
            if retrieved.is_empty() && i != 0 {
                panic!("int_to_str: intern_str/get_str corrupted string '{}' for {}", s, i);
            }
            Value::Str(handle)
        },
        _ => {
            // Non-integer values should never happen in well-typed code
            Value::Str(intern_str("<not-an-int>"))
        }
    }
}
