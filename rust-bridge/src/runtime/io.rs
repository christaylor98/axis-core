// IO primitives
// Extracted from emit_rust.rs generate_value_runtime()

use super::value::{Value, intern_str, get_str, intern_tag};
use std::io::{BufRead, Write};

pub fn io_print(val: Value) -> Value {
    match val {
        Value::Str(handle) => print!("{}", get_str(handle)),
        Value::Int(n) => print!("{}", n),
        Value::Bool(b) => print!("{}", b),
        Value::Unit => print!("()"),
        _ => print!("{:?}", val),
    }
    std::io::stdout().flush().unwrap();
    Value::Unit
}

pub fn io_eprint(val: Value) -> Value {
    match val {
        Value::Str(handle) => eprint!("{}", get_str(handle)),
        Value::Int(n) => eprint!("{}", n),
        Value::Bool(b) => eprint!("{}", b),
        Value::Unit => eprint!("()"),
        _ => eprint!("{:?}", val),
    }
    std::io::stderr().flush().unwrap();
    Value::Unit
}

// Debug trace (observational, non-semantic)
// Controlled by AXIS_TRACE environment variable
pub fn debug_trace(val: Value) -> Value {
    if std::env::var("AXIS_TRACE").ok().as_deref() == Some("1") {
        match val {
            Value::Str(handle) => {
                eprintln!("{}", get_str(handle));
            },
            Value::Int(n) => eprintln!("{}", n),
            Value::Bool(b) => eprintln!("{}", b),
            Value::Unit => eprintln!("()"),
            _ => eprintln!("{:?}", val),
        }
    }
    Value::Unit
}

pub fn io_read() -> Value {
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap_or(0);
    Value::Str(intern_str(&line))
}

// File IO primitives
pub fn fs_read_text(path: Value) -> Value {
    match path {
        Value::Str(handle) => {
            let path_str = get_str(handle);
            match std::fs::read_to_string(&path_str) {
                Ok(content) => Value::Ctor {
                    tag: intern_tag("Ok"),
                    fields: vec![Value::Str(intern_str(&content))],
                },
                Err(e) => Value::Ctor {
                    tag: intern_tag("Err"),
                    fields: vec![Value::Str(intern_str(&e.to_string()))],
                },
            }
        },
        _ => Value::Ctor {
            tag: intern_tag("Err"),
            fields: vec![Value::Str(intern_str("Invalid path"))],
        },
    }
}

pub fn fs_write_text(path: Value, content: Value) -> Value {
    match (path, content) {
        (Value::Str(path_handle), Value::Str(content_handle)) => {
            let path_str = get_str(path_handle);
            let content_str = get_str(content_handle);
            match std::fs::write(&path_str, &content_str) {
                Ok(_) => Value::Ctor {
                    tag: intern_tag("Ok"),
                    fields: vec![Value::Unit],
                },
                Err(e) => Value::Ctor {
                    tag: intern_tag("Err"),
                    fields: vec![Value::Str(intern_str(&e.to_string()))],
                },
            }
        },
        _ => Value::Ctor {
            tag: intern_tag("Err"),
            fields: vec![Value::Str(intern_str("Invalid arguments"))],
        },
    }
}
