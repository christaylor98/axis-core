// Registry authority for validation - NO FILESYSTEM ACCESS
// Validation uses the CLI-loaded Registry as the sole authority.
// No filesystem access is permitted here.

use crate::registry_loader::Registry;

// Check if a function is known (builtin or in registry)
pub fn is_known_function(registry: &Registry, name: &str) -> bool {
    // Check global registry
    if registry.entries.contains_key(name) {
        return true;
    }
    
    // Check builtins
    is_builtin_function(name)
}

// Builtin function check (same as in registry_loader.rs but duplicated to avoid circular deps)
fn is_builtin_function(name: &str) -> bool {
    match name {
        // Core arithmetic
        "+" | "-" | "*" | "/" | "%" => true,
        "__add__" | "__sub__" | "__mul__" | "__div__" | "__mod__" => true,
        
        // Comparisons  
        "==" | "!=" | "<" | "<=" | ">" | ">=" => true,
        "__eq__" | "__lt__" | "__lte__" | "__gt__" | "__gte__" => true,
        
        // Logical
        "&&" | "||" | "!" => true,
        "__and__" | "__or__" | "__not__" => true,
        
        // Tuple operations
        "tuple_field" => true,
        
        // String operations
        "str_len" | "str_char" | "str_slice" | "str_concat" | "__concat__" => true,
        "int_to_str" | "str_to_int" => true,
        
        // Any function that ends with builtin pattern
        _ if name.ends_with("___main") => true,
        _ if name.contains("__add__") || name.contains("__sub__") => true,
        _ if name.contains("__mul__") || name.contains("__div__") => true,
        
        // Special debug function
        "debug_trace" => true,
        
        _ => false,
    }
}