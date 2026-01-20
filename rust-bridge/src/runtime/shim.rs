//! Axis Runtime Shim Library
//!
//! This module provides a stable Rust API for Axis runtime operations.
//! It serves as the load-bearing semantic boundary between generated
//! Axis code and Rust runtime implementations.
//!
//! ## Design Principles
//! 
//! 1. **Preserve Semantic Distinctions**: Each operation has a unique function
//! 2. **No String Dispatch**: All operations are statically typed Rust functions
//! 3. **Explicit Variants**: Checked vs unchecked operations are separate functions
//! 4. **Testable**: Each function can be tested in isolation
//! 5. **Load-bearing**: Semantic collapse here is considered a bug

use crate::runtime::value::{Value, get_str, intern_str};
use crate::runtime::io;

// ============================================================================
// IO Operations
// ============================================================================

/// Print a value to stdout
pub fn io_print(val: Value) -> Value {
    io::io_print(val)
}

/// Print a value to stderr
pub fn io_eprint(val: Value) -> Value {
    io::io_eprint(val)
}

/// Read a line from stdin
pub fn io_read() -> Value {
    io::io_read()
}

// ============================================================================
// String Operations
// ============================================================================

/// Get a character from a string without bounds checking
/// 
/// # Safety
/// Caller must ensure index is valid
/// UNARY CONTRACT: Accepts Value::Tuple containing [string, index]
pub fn str_char(args: Value) -> Value {
    let (string_handle, idx) = match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let handle = match elems[0] {
                Value::Str(h) => h,
                _ => panic!("str_char: first argument must be a string"),
            };
            let index = match elems[1] {
                Value::Int(n) => n as usize,
                _ => panic!("str_char: second argument must be an integer"),
            };
            (handle, index)
        },
        _ => panic!("str_char: expects tuple of (string, index)"),
    };
    
    let string_content = get_str(string_handle);
    
    // UNCHECKED: Direct indexing without bounds validation
    let chars: Vec<char> = string_content.chars().collect();
    let ch = chars[idx]; // Will panic on out-of-bounds - this is the unchecked behavior
    
    Value::Str(intern_str(&ch.to_string()))
}

/// Get a character from a string with bounds checking
/// 
/// Returns None (as a Value) if index is out of bounds
/// UNARY CONTRACT: Accepts Value::Tuple containing [string, index]
pub fn str_char_at(args: Value) -> Value {
    let (string_handle, idx) = match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let handle = match elems[0] {
                Value::Str(h) => h,
                _ => panic!("str_char_at: first argument must be a string"),
            };
            let index = match elems[1] {
                Value::Int(n) => {
                    if n < 0 {
                        return option_none();
                    }
                    n as usize
                },
                _ => panic!("str_char_at: second argument must be an integer"),
            };
            (handle, index)
        },
        _ => panic!("str_char_at: expects tuple of (string, index)"),
    };
    
    let string_content = get_str(string_handle);
    
    // CHECKED: Bounds validation before access
    let chars: Vec<char> = string_content.chars().collect();
    if idx >= chars.len() {
        return option_none();
    }
    
    let ch = chars[idx];
    option_some(Value::Str(intern_str(&ch.to_string())))
}

/// Get character code from a string (returns integer, defensive against emitter bugs)
pub fn str_char_code(args: Value) -> Value {
    let (string_handle, idx) = match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let handle = match elems[0] {
                Value::Str(h) => h,
                _ => panic!("str_char_code: first argument must be a string"),
            };
            let index = match elems[1] {
                Value::Int(n) => n as usize,
                // Handle string literals that should be integers (emitter bug defensive)
                Value::Str(id) => {
                    let s = get_str(id);
                    s.parse::<i32>().unwrap_or(0) as usize
                }
                _ => panic!("str_char_code: second argument must be an integer or numeric string"),
            };
            (handle, index)
        },
        _ => panic!("str_char_code: expects tuple of (string, index)"),
    };
    
    let string_content = get_str(string_handle);
    let chars: Vec<char> = string_content.chars().collect();
    
    if idx >= chars.len() {
        return Value::Int(0); // Out of bounds
    }
    
    let ch = chars[idx];
    Value::Int(ch as u32 as i64)
}

/// Get the length of a string
pub fn str_len(s: Value) -> Value {
    let string_handle = match s {
        Value::Str(handle) => handle,
        _ => panic!("str_len: argument must be a string"),
    };
    
    let string_content = get_str(string_handle);
    Value::Int(string_content.chars().count() as i64)
}

/// Convert a character (single-char string) to a string
/// This is a no-op in the Value representation but included for semantic clarity
pub fn char_to_str(c: Value) -> Value {
    match c {
        Value::Str(_) => c, // Already a string
        Value::Int(n) if n >= 0 && n <= 0x10FFFF => {
            // Treat integer as Unicode code point
            if let Some(ch) = char::from_u32(n as u32) {
                Value::Str(intern_str(&ch.to_string()))
            } else {
                Value::Str(intern_str("�")) // Replacement character
            }
        }
        _ => Value::Str(intern_str("")),
    }
}

/// Concatenate two strings
pub fn str_concat(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let handle_a = match &elems[0] {
                Value::Str(handle) => *handle,
                _ => panic!("str_concat: first argument must be a string"),
            };
            
            let handle_b = match &elems[1] {
                Value::Str(handle) => *handle,
                _ => panic!("str_concat: second argument must be a string"),
            };
            
            let str_a = get_str(handle_a);
            let str_b = get_str(handle_b);
            let result = format!("{}{}", str_a, str_b);
            
            Value::Str(intern_str(&result))
        },
        _ => panic!("str_concat: expected tuple with 2 elements"),
    }
}

// ============================================================================
// List Operations
// ============================================================================

/// Get an element from a list without bounds checking
/// 
/// # Safety
/// Caller must ensure index is valid
pub fn list_get(list: &Value, index: &Value) -> Value {
    let elements = match list {
        Value::List(elems) => elems,
        _ => panic!("list_get: first argument must be a list"),
    };
    
    let idx = match index {
        Value::Int(n) => *n as usize,
        _ => panic!("list_get: second argument must be an integer"),
    };
    
    // UNCHECKED: Direct indexing without bounds validation
    elements[idx].clone() // Will panic on out-of-bounds - this is the unchecked behavior
}

/// Get an element from a list with bounds checking
/// 
/// Returns None (as a Value) if index is out of bounds
pub fn list_get_at(list: &Value, index: &Value) -> Value {
    let elements = match list {
        Value::List(elems) => elems,
        _ => panic!("list_get_at: first argument must be a list"),
    };
    
    let idx = match index {
        Value::Int(n) => {
            if *n < 0 {
                return option_none();
            }
            *n as usize
        },
        _ => panic!("list_get_at: second argument must be an integer"),
    };
    
    // CHECKED: Bounds validation before access
    if idx >= elements.len() {
        return option_none();
    }
    
    option_some(elements[idx].clone())
}

/// Get the length of a list
pub fn list_len(list: &Value) -> Value {
    let elements = match list {
        Value::List(elems) => elems,
        _ => panic!("list_len: argument must be a list"),
    };
    
    Value::Int(elements.len() as i64)
}

/// Append an element to a list (creates new list)
pub fn list_append(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let elements = match &elems[0] {
                Value::List(list_elems) => list_elems,
                _ => panic!("list_append: first argument must be a list"),
            };
            
            let mut new_elements = elements.clone();
            new_elements.push(elems[1].clone());
            
            Value::List(new_elements)
        },
        _ => panic!("list_append: expected tuple with 2 elements"),
    }
}

// ============================================================================
// Arithmetic Operations
// ============================================================================

/// Add two integers
pub fn int_add(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Int(n) => *n,
        _ => panic!("int_add: first argument must be an integer"),
    };
    
    let y = match b {
        Value::Int(n) => *n,
        _ => panic!("int_add: second argument must be an integer"),
    };
    
    Value::Int(x + y)
}

/// Subtract two integers
pub fn int_sub(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Int(n) => *n,
        _ => panic!("int_sub: first argument must be an integer"),
    };
    
    let y = match b {
        Value::Int(n) => *n,
        _ => panic!("int_sub: second argument must be an integer"),
    };
    
    Value::Int(x - y)
}

/// Multiply two integers
pub fn int_mul(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Int(n) => *n,
        _ => panic!("int_mul: first argument must be an integer"),
    };
    
    let y = match b {
        Value::Int(n) => *n,
        _ => panic!("int_mul: second argument must be an integer"),
    };
    
    Value::Int(x * y)
}

/// Divide two integers with bounds checking
/// 
/// Returns None if division by zero
pub fn int_div_checked(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Int(n) => *n,
        _ => panic!("int_div_checked: first argument must be an integer"),
    };
    
    let y = match b {
        Value::Int(n) => *n,
        _ => panic!("int_div_checked: second argument must be an integer"),
    };
    
    if y == 0 {
        option_none()
    } else {
        option_some(Value::Int(x / y))
    }
}

// ============================================================================
// Comparison Operations
// ============================================================================

/// Test equality between two values
pub fn value_eq(a: &Value, b: &Value) -> Value {
    Value::Bool(a == b)
}

/// Test if first value is less than second
pub fn int_lt(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Int(n) => *n,
        _ => panic!("int_lt: first argument must be an integer"),
    };
    
    let y = match b {
        Value::Int(n) => *n,
        _ => panic!("int_lt: second argument must be an integer"),
    };
    
    Value::Bool(x < y)
}

// ============================================================================
// Option Type Helpers
// ============================================================================

/// Create a None option value
pub fn option_none() -> Value {
    Value::Ctor { 
        tag: intern_str("None"), 
        fields: vec![] 
    }
}

/// Create a Some option value
pub fn option_some(value: Value) -> Value {
    Value::Ctor { 
        tag: intern_str("Some"), 
        fields: vec![value] 
    }
}

/// Test if a value is None
pub fn option_is_none(opt: &Value) -> Value {
    match opt {
        Value::Ctor { tag, fields } if get_str(*tag) == "None" && fields.is_empty() => {
            Value::Bool(true)
        }
        _ => Value::Bool(false)
    }
}

/// Test if a value is Some
pub fn option_is_some(opt: &Value) -> Value {
    match opt {
        Value::Ctor { tag, fields } if get_str(*tag) == "Some" && fields.len() == 1 => {
            Value::Bool(true)
        }
        _ => Value::Bool(false)
    }
}

/// Unwrap a Some value, panic if None
pub fn option_unwrap(opt: &Value) -> Value {
    match opt {
        Value::Ctor { tag, fields } if get_str(*tag) == "Some" && fields.len() == 1 => {
            fields[0].clone()
        }
        Value::Ctor { tag, fields } if get_str(*tag) == "None" && fields.is_empty() => {
            panic!("Called option_unwrap on None value")
        }
        _ => panic!("option_unwrap called on non-option value")
    }
}

// ============================================================================
// Boolean Operations
// ============================================================================

/// Logical AND
pub fn bool_and(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Bool(b) => *b,
        _ => panic!("bool_and: first argument must be a boolean"),
    };
    
    let y = match b {
        Value::Bool(b) => *b,
        _ => panic!("bool_and: second argument must be a boolean"),
    };
    
    Value::Bool(x && y)
}

/// Logical OR
pub fn bool_or(a: &Value, b: &Value) -> Value {
    let x = match a {
        Value::Bool(b) => *b,
        _ => panic!("bool_or: first argument must be a boolean"),
    };
    
    let y = match b {
        Value::Bool(b) => *b,
        _ => panic!("bool_or: second argument must be a boolean"),
    };
    
    Value::Bool(x || y)
}

/// Logical NOT
pub fn bool_not(a: &Value) -> Value {
    let x = match a {
        Value::Bool(b) => *b,
        _ => panic!("bool_not: argument must be a boolean"),
    };
    
    Value::Bool(!x)
}

// ============================================================================
// Result/Option Constructors
// ============================================================================

/// Create an Err value wrapping a message
pub fn axis_io_make_error(msg: Value) -> Value {
    use crate::runtime::value::intern_tag;
    Value::Ctor { tag: intern_tag("Err"), fields: vec![msg] }
}

// ============================================================================
// JSON Parsing (Minimal compiler Implementation)
// ============================================================================

/// Parse a simple JSON object into a list of (key, value) tuples
/// This is a minimal implementation for compiler invocation parsing only.
/// Supports: {"key": "value", "key2": "value2"}
/// Returns: List[(Str, Str)]
pub fn axis_json_parse(json_str: Value) -> Value {
    let json_text = match json_str {
        Value::Str(handle) => get_str(handle),
        _ => return Value::List(vec![]),
    };
    
    // Minimal JSON object parser
    let trimmed = json_text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Value::List(vec![]);
    }
    
    let content = &trimmed[1..trimmed.len()-1];
    let mut pairs = Vec::new();
    
    // Split by commas (simplified - doesn't handle nested objects)
    for pair_str in content.split(',') {
        let parts: Vec<&str> = pair_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }
        
        let key = parts[0].trim().trim_matches('"');
        let value = parts[1].trim().trim_matches('"');
        
        let key_val = Value::Str(intern_str(key));
        let value_val = Value::Str(intern_str(value));
        
        pairs.push(Value::Tuple(vec![key_val, value_val]));
    }
    
    Value::List(pairs)
}

// ============================================================================
// Arithmetic Operations (re-exported from value.rs)
// ============================================================================

pub use crate::runtime::value::{
    __add__, __sub__, __mul__, __div__, __mod__,
    __eq__, __lt__, __lte__, __gt__, __gte__,
    __and__, __or__, __not__, __concat__,
    int_to_str, str_to_int, str_slice
};

pub use crate::runtime::tuple::{
    tuple, tuple_field, ctor_field
};

pub use crate::runtime::list::{
    list_nil, list_cons, list_reverse, list_concat, list_contains_str, list_index_of_str
};

pub use crate::runtime::io::{
    fs_read_text, fs_write_text
};

pub use crate::runtime::core_emit::axis_emit_core_bundle_to_file;