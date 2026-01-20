//! Unit tests for the Axis runtime shim library
//! 
//! These tests validate that semantic distinctions are preserved
//! and that the shim functions behave correctly in isolation.

use crate::runtime::shim::*;
use crate::runtime::value::{Value, intern_str, init_runtime};

fn setup() {
    init_runtime();
}

// ============================================================================
// String Operation Tests
// ============================================================================

#[cfg(test)]
mod string_tests {
    use super::*;

    #[test]
    fn test_str_char_vs_str_char_at_semantic_distinction() {
        setup();
        
        let test_str = Value::Str(intern_str("hello"));
        let valid_index = Value::Int(1);
        let invalid_index = Value::Int(10);
        
        // str_char should return the character at valid index
        let result = str_char(Value::Tuple(vec![test_str.clone(), valid_index.clone()]));
        if let Value::Str(handle) = result {
            let ch = crate::runtime::value::get_str(handle);
            assert_eq!(ch, "e");
        } else {
            panic!("str_char should return a string");
        }
        
        // str_char_at should return Some for valid index
        let result = str_char_at(Value::Tuple(vec![test_str.clone(), valid_index.clone()]));
        assert!(option_is_some(&result).as_bool());
        let inner = option_unwrap(&result);
        if let Value::Str(handle) = inner {
            let ch = crate::runtime::value::get_str(handle);
            assert_eq!(ch, "e");
        }
        
        // str_char_at should return None for invalid index
        let result = str_char_at(Value::Tuple(vec![test_str.clone(), invalid_index.clone()]));
        assert!(option_is_none(&result).as_bool());
    }

    #[test]
    #[should_panic]
    fn test_str_char_panics_on_out_of_bounds() {
        setup();
        
        let test_str = Value::Str(intern_str("hello"));
        let invalid_index = Value::Int(10);
        
        // This should panic - demonstrating unchecked behavior
        str_char(Value::Tuple(vec![test_str.clone(), invalid_index.clone()]));
    }

    #[test]
    fn test_str_char_at_handles_negative_index() {
        setup();
        
        let test_str = Value::Str(intern_str("hello"));
        let negative_index = Value::Int(-1);
        
        let result = str_char_at(Value::Tuple(vec![test_str.clone(), negative_index.clone()]));
        assert!(option_is_none(&result).as_bool());
    }

    #[test]
    fn test_str_len() {
        setup();
        
        let empty_str = Value::Str(intern_str(""));
        let test_str = Value::Str(intern_str("hello"));
        let unicode_str = Value::Str(intern_str("héllo")); // accent should count as 1 char
        
        assert_eq!(str_len(empty_str).as_int(), 0);
        assert_eq!(str_len(test_str).as_int(), 5);
        assert_eq!(str_len(unicode_str).as_int(), 5);
    }

    #[test]
    fn test_str_concat() {
        setup();
        
        let str1 = Value::Str(intern_str("hello"));
        let str2 = Value::Str(intern_str(" world"));
        let empty = Value::Str(intern_str(""));
        
        let result = str_concat(Value::Tuple(vec![str1.clone(), str2.clone()]));
        if let Value::Str(handle) = result {
            let concat_result = crate::runtime::value::get_str(handle);
            assert_eq!(concat_result, "hello world");
        }
        
        // Test concatenation with empty string
        let result = str_concat(Value::Tuple(vec![str1.clone(), empty.clone()]));
        if let Value::Str(handle) = result {
            let concat_result = crate::runtime::value::get_str(handle);
            assert_eq!(concat_result, "hello");
        }
    }
}

// ============================================================================
// List Operation Tests
// ============================================================================

#[cfg(test)]
mod list_tests {
    use super::*;

    #[test]
    fn test_list_get_vs_list_get_at_semantic_distinction() {
        setup();
        
        let test_list = Value::List(vec![
            Value::Int(10),
            Value::Int(20),
            Value::Int(30)
        ]);
        let valid_index = Value::Int(1);
        let invalid_index = Value::Int(10);
        
        // list_get should return the element at valid index
        let result = list_get(&test_list, &valid_index);
        assert_eq!(result.as_int(), 20);
        
        // list_get_at should return Some for valid index
        let result = list_get_at(&test_list, &valid_index);
        assert!(option_is_some(&result).as_bool());
        let inner = option_unwrap(&result);
        assert_eq!(inner.as_int(), 20);
        
        // list_get_at should return None for invalid index
        let result = list_get_at(&test_list, &invalid_index);
        assert!(option_is_none(&result).as_bool());
    }

    #[test]
    #[should_panic]
    fn test_list_get_panics_on_out_of_bounds() {
        setup();
        
        let test_list = Value::List(vec![Value::Int(10)]);
        let invalid_index = Value::Int(10);
        
        // This should panic - demonstrating unchecked behavior
        list_get(&test_list, &invalid_index);
    }

    #[test]
    fn test_list_get_at_handles_negative_index() {
        setup();
        
        let test_list = Value::List(vec![Value::Int(10)]);
        let negative_index = Value::Int(-1);
        
        let result = list_get_at(&test_list, &negative_index);
        assert!(option_is_none(&result).as_bool());
    }

    #[test]
    fn test_list_len() {
        setup();
        
        let empty_list = Value::List(vec![]);
        let test_list = Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3)
        ]);
        
        assert_eq!(list_len(&empty_list).as_int(), 0);
        assert_eq!(list_len(&test_list).as_int(), 3);
    }

    #[test]
    fn test_list_append() {
        setup();
        
        let original_list = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let new_element = Value::Int(3);
        
        let result = list_append(Value::Tuple(vec![original_list.clone(), new_element.clone()]));
        
        // Original should be unchanged
        assert_eq!(original_list.as_list().len(), 2);
        
        // Result should have new element
        let result_list = result.as_list();
        assert_eq!(result_list.len(), 3);
        assert_eq!(result_list[0].as_int(), 1);
        assert_eq!(result_list[1].as_int(), 2);
        assert_eq!(result_list[2].as_int(), 3);
    }
}

// ============================================================================
// Arithmetic Operation Tests
// ============================================================================

#[cfg(test)]
mod arithmetic_tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        setup();
        
        let a = Value::Int(10);
        let b = Value::Int(3);
        
        assert_eq!(int_add(&a, &b).as_int(), 13);
        assert_eq!(int_sub(&a, &b).as_int(), 7);
        assert_eq!(int_mul(&a, &b).as_int(), 30);
    }

    #[test]
    fn test_int_div_checked_normal_division() {
        setup();
        
        let a = Value::Int(10);
        let b = Value::Int(3);
        
        let result = int_div_checked(&a, &b);
        assert!(option_is_some(&result).as_bool());
        let inner = option_unwrap(&result);
        assert_eq!(inner.as_int(), 3); // Integer division
    }

    #[test]
    fn test_int_div_checked_division_by_zero() {
        setup();
        
        let a = Value::Int(10);
        let zero = Value::Int(0);
        
        let result = int_div_checked(&a, &zero);
        assert!(option_is_none(&result).as_bool());
    }
}

// ============================================================================
// Comparison Operation Tests
// ============================================================================

#[cfg(test)]
mod comparison_tests {
    use super::*;

    #[test]
    fn test_value_equality() {
        setup();
        
        let int1 = Value::Int(42);
        let int2 = Value::Int(42);
        let int3 = Value::Int(43);
        let bool_val = Value::Bool(true);
        
        assert!(value_eq(&int1, &int2).as_bool());
        assert!(!value_eq(&int1, &int3).as_bool());
        assert!(!value_eq(&int1, &bool_val).as_bool());
    }

    #[test]
    fn test_integer_comparison() {
        setup();
        
        let a = Value::Int(5);
        let b = Value::Int(10);
        let c = Value::Int(5);
        
        assert!(int_lt(&a, &b).as_bool());
        assert!(!int_lt(&b, &a).as_bool());
        assert!(!int_lt(&a, &c).as_bool());
    }
}

// ============================================================================
// Option Type Tests
// ============================================================================

#[cfg(test)]
mod option_tests {
    use super::*;

    #[test]
    fn test_option_creation_and_testing() {
        setup();
        
        let none_val = option_none();
        let some_val = option_some(Value::Int(42));
        
        assert!(option_is_none(&none_val).as_bool());
        assert!(!option_is_some(&none_val).as_bool());
        
        assert!(option_is_some(&some_val).as_bool());
        assert!(!option_is_none(&some_val).as_bool());
    }

    #[test]
    fn test_option_unwrap_some() {
        setup();
        
        let inner_value = Value::Int(42);
        let some_val = option_some(inner_value.clone());
        
        let unwrapped = option_unwrap(&some_val);
        assert_eq!(unwrapped, inner_value);
    }

    #[test]
    #[should_panic(expected = "Called option_unwrap on None value")]
    fn test_option_unwrap_none_panics() {
        setup();
        
        let none_val = option_none();
        option_unwrap(&none_val);
    }
}

// ============================================================================
// Boolean Operation Tests
// ============================================================================

#[cfg(test)]
mod boolean_tests {
    use super::*;

    #[test]
    fn test_boolean_operations() {
        setup();
        
        let true_val = Value::Bool(true);
        let false_val = Value::Bool(false);
        
        // Test AND
        assert!(bool_and(&true_val, &true_val).as_bool());
        assert!(!bool_and(&true_val, &false_val).as_bool());
        assert!(!bool_and(&false_val, &true_val).as_bool());
        assert!(!bool_and(&false_val, &false_val).as_bool());
        
        // Test OR
        assert!(bool_or(&true_val, &true_val).as_bool());
        assert!(bool_or(&true_val, &false_val).as_bool());
        assert!(bool_or(&false_val, &true_val).as_bool());
        assert!(!bool_or(&false_val, &false_val).as_bool());
        
        // Test NOT
        assert!(!bool_not(&true_val).as_bool());
        assert!(bool_not(&false_val).as_bool());
    }
}

// ============================================================================
// Regression Tests (Anti-Semantic Collapse)
// ============================================================================

#[cfg(test)]
mod regression_tests {
    use super::*;

    /// This test ensures that str_char and str_char_at are truly different functions
    /// and cannot be confused by any form of dispatch mechanism
    #[test]
    fn test_string_functions_are_distinct() {
        setup();
        
        let test_str = Value::Str(intern_str("test"));
        let out_of_bounds = Value::Int(100);
        
        // These functions must behave differently on out-of-bounds access
        let char_at_result = str_char_at(Value::Tuple(vec![test_str.clone(), out_of_bounds.clone()]));
        assert!(option_is_none(&char_at_result).as_bool(), 
                "str_char_at should return None for out-of-bounds");
        
        // str_char should panic (we can't test this directly without killing the test,
        // but we've demonstrated it in a separate test marked with should_panic)
    }

    /// This test ensures that list_get and list_get_at are truly different functions
    #[test]
    fn test_list_functions_are_distinct() {
        setup();
        
        let test_list = Value::List(vec![Value::Int(42)]);
        let out_of_bounds = Value::Int(100);
        
        // These functions must behave differently on out-of-bounds access
        let get_at_result = list_get_at(&test_list, &out_of_bounds);
        assert!(option_is_none(&get_at_result).as_bool(),
                "list_get_at should return None for out-of-bounds");
        
        // list_get should panic (demonstrated in separate should_panic test)
    }

    /// Verify that division operations maintain their semantic distinction
    #[test]
    fn test_division_safety_distinction() {
        setup();
        
        let a = Value::Int(10);
        let zero = Value::Int(0);
        
        // int_div_checked should return None for division by zero
        let checked_result = int_div_checked(&a, &zero);
        assert!(option_is_none(&checked_result).as_bool(),
                "int_div_checked should return None for division by zero");
        
        // If we had an unchecked division function, it would panic or produce
        // undefined behavior - but we deliberately don't expose such a function
        // in this shim to maintain safety
    }
}