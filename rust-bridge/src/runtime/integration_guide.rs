//! Integration Guide for Axis Semantic Compiler
//! 
//! This file demonstrates how the semantic compiler should call into
//! the runtime shim library to preserve semantic distinctions.

use crate::runtime::shim::*;
use crate::runtime::value::{Value, intern_str};

/// Example: How the semantic compiler should generate calls for string operations
/// 
/// When the compiler encounters Axis source code like:
/// ```axis
/// let ch = str_char("hello", 2)    // Unchecked operation
/// let ch_safe = str_char_at("hello", 10)  // Checked operation
/// ```
/// 
/// It should generate Rust code like:
pub fn example_generated_string_code() {
    // Initialize runtime before any operations
    crate::runtime::value::init_runtime();
    
    let hello_str = Value::Str(intern_str("hello"));
    let index_2 = Value::Int(2);
    let index_10 = Value::Int(10);
    
    // For unchecked operation: direct call to str_char
    // This will panic if index is out of bounds - that's the intended semantic
    let ch = str_char(Value::Tuple(vec![hello_str.clone(), index_2]));
    println!("Character at index 2: {}", ch);
    
    // For checked operation: direct call to str_char_at  
    // This returns Option-wrapped result
    let ch_safe = str_char_at(Value::Tuple(vec![hello_str.clone(), index_10]));
    match ch_safe {
        Value::Ctor { tag, fields } if crate::runtime::value::get_str(tag) == "Some" => {
            println!("Character found: {}", fields[0]);
        }
        Value::Ctor { tag, fields: _ } if crate::runtime::value::get_str(tag) == "None" => {
            println!("No character at index 10");
        }
        _ => panic!("Unexpected result from str_char_at"),
    }
}

/// Example: How the semantic compiler should generate calls for arithmetic
/// 
/// When the compiler encounters:
/// ```axis
/// let result = a + b           // Basic addition
/// let safe_div = a / b         // Should use checked division
/// ```
pub fn example_generated_arithmetic_code() {
    let a = Value::Int(10);
    let b = Value::Int(3);
    let zero = Value::Int(0);
    
    // Basic arithmetic: direct calls
    let sum = int_add(&a, &b);
    let diff = int_sub(&a, &b);
    let product = int_mul(&a, &b);
    
    println!("Sum: {}, Diff: {}, Product: {}", sum, diff, product);
    
    // Division should always use the checked variant to prevent runtime errors
    let division_result = int_div_checked(&a, &b);
    let division_by_zero = int_div_checked(&a, &zero);
    
    // Handle division results
    if option_is_some(&division_result).as_bool() {
        let quotient = option_unwrap(&division_result);
        println!("Division result: {}", quotient);
    }
    
    if option_is_none(&division_by_zero).as_bool() {
        println!("Division by zero detected and handled safely");
    }
}

/// Example: How to maintain semantic boundaries in pattern matching
/// 
/// The compiler should generate different code paths for:
/// ```axis
/// match list[index] { ... }     // Unchecked access
/// match list.get(index) { ... } // Checked access
/// ```
pub fn example_generated_pattern_matching() {
    let test_list = Value::List(vec![
        Value::Int(10),
        Value::Int(20),
        Value::Int(30),
    ]);
    let index = Value::Int(1);
    let bad_index = Value::Int(10);
    
    // For unchecked list access (would panic on bad index)
    let element = list_get(&test_list, &index);
    println!("Element at index 1: {}", element);
    
    // For checked list access (returns Option)
    let maybe_element = list_get_at(&test_list, &bad_index);
    if option_is_none(&maybe_element).as_bool() {
        println!("No element at index 10");
    }
}

/// Code Generation Principles for the Semantic Compiler
/// 
/// 1. **Direct Function Calls**: No string-based dispatch. Generate direct
///    calls like `str_char(&s, &i)` not `call_runtime("str_char", args)`.
/// 
/// 2. **Preserve Semantics**: 
///    - Unchecked operations → panic on error (str_char, list_get)
///    - Checked operations → return Option (str_char_at, list_get_at)
/// 
/// 3. **Static Dispatch**: All runtime function names are known at compile time.
///    The compiler should have a mapping from Axis operations to shim functions.
/// 
/// 4. **Error Handling**: Let the shim functions handle their own error conditions.
///    Don't add extra bounds checking in generated code.
/// 
/// 5. **Type Safety**: Always pass &Value references to shim functions.
///    The shim handles all type validation and panics on type mismatches.

/// Suggested mapping from Axis operations to shim functions:
/// 
/// Axis Source → Rust Shim Function
/// ================================
/// str_char(s, i)      → str_char(&s, &i)
/// str_char_at(s, i)   → str_char_at(&s, &i) 
/// str_len(s)          → str_len(&s)
/// str_concat(a, b)    → str_concat(&a, &b)
/// 
/// list[i]             → list_get(&list, &i)
/// list.get(i)         → list_get_at(&list, &i)
/// list.len()          → list_len(&list)
/// list.append(x)      → list_append(&list, &x)
/// 
/// a + b               → int_add(&a, &b)
/// a - b               → int_sub(&a, &b)
/// a * b               → int_mul(&a, &b)
/// a / b               → int_div_checked(&a, &b)  // Always checked!
/// 
/// a == b              → value_eq(&a, &b)
/// a < b               → int_lt(&a, &b)
/// 
/// Some(x)             → option_some(x)
/// None                → option_none()
/// is_some(opt)        → option_is_some(&opt)
/// is_none(opt)        → option_is_none(&opt)
/// unwrap(opt)         → option_unwrap(&opt)

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_semantic_compiler_integration_example() {
        // This test demonstrates that the generated code works as expected
        example_generated_string_code();
        example_generated_arithmetic_code();
        example_generated_pattern_matching();
        // If we get here without panicking, integration is working
    }
}