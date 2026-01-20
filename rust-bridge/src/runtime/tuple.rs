// Tuple and Constructor field access helpers
// Extracted from emit_rust.rs generate_value_runtime()

use super::value::Value;

// Tuple constructor
pub fn tuple(args: Value) -> Value {
    match args {
        Value::Tuple(elems) => Value::Tuple(elems),
        _ => Value::Tuple(vec![args]),
    }
}

// Tuple field access
// UNARY CONTRACT: Accepts Value::Tuple containing [tuple_data, index]
pub fn tuple_field(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let tuple_data = &elems[0];
            let idx = match &elems[1] {
                Value::Int(n) => *n as usize,
                _ => return Value::Unit,
            };
            
            match tuple_data {
                Value::Tuple(ref fields) => {
                    fields.get(idx).cloned().unwrap_or(Value::Unit)
                },
                _ => Value::Unit,
            }
        },
        _ => Value::Unit,
    }
}

// Constructor field access
// UNARY CONTRACT: Accepts Value::Tuple containing [ctor_data, index]
pub fn ctor_field(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let ctor_data = &elems[0];
            let idx = match &elems[1] {
                Value::Int(n) => *n as usize,
                _ => return Value::Unit,
            };
            
            match ctor_data {
                Value::Ctor { ref fields, .. } => {
                    fields.get(idx).cloned().unwrap_or(Value::Unit)
                },
                _ => Value::Unit,
            }
        },
        _ => Value::Unit,
    }
}
