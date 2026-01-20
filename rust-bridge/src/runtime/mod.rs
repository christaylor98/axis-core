// Runtime module - extracted from emit_rust.rs
// This module contains all runtime helpers needed for generated Axis code

pub mod value;
pub mod tuple;
pub mod list;
pub mod io;
pub mod emit_rust;
pub mod core_emit;
pub mod shim;
pub mod integration_guide;

#[cfg(test)]
pub mod emit_rust_regression_tests;

#[cfg(test)]
mod shim_tests;

// Re-export all runtime items for convenient use
pub use value::*;
pub use tuple::*;
pub use list::*;
pub use io::*;
pub use core_emit::*;

// Re-export shim functions with explicit naming to avoid conflicts
pub use shim::{
    // String operations  
    str_char as shim_str_char,
    str_char_at as shim_str_char_at,
    str_char_code as shim_str_char_code,
    str_len as shim_str_len,
    str_concat as shim_str_concat,
    
    // List operations
    list_get, list_get_at, list_len, list_append,
    
    // Arithmetic
    int_add, int_sub, int_mul, int_div_checked,
    
    // Comparison
    value_eq, int_lt,
    
    // Options
    option_none, option_some, option_is_none, option_is_some, option_unwrap,
    
    // Boolean
    bool_and, bool_or, bool_not,
};
