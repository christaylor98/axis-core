// Regression test for emit_rust.rs refactoring
// Tests that:
// 1. Core IR functions generate `fn foo` definitions  
// 2. Foreign calls generate `shim::foreign_func` calls
// 3. NO foreign stub functions are generated

use crate::runtime::emit_rust::emit_rust_from_core;
use crate::core_ir::CoreTerm;
use std::rc::Rc;

#[test]
fn test_emit_rust_no_foreign_stubs() {
    // Create a Core IR structure that:
    // 1. Defines a function `foo` in Core IR (should emit `fn foo`)
    // 2. Calls foreign `str_char_at` (should emit `shim::str_char_at`)
    
    // foo = λx. str_char_at(x, 0) 
    let core_ir = CoreTerm::Let(
        "foo".to_string(),
        Rc::new(CoreTerm::Lam(
            "x".to_string(),
            Rc::new(CoreTerm::App(
                Rc::new(CoreTerm::App(
                    Rc::new(CoreTerm::Var("str_char_at".to_string(), None)),
                    Rc::new(CoreTerm::Var("x".to_string(), None)),
                    None
                )),
                Rc::new(CoreTerm::IntLit(0, None)),
                None
            )),
            None
        )),
        Rc::new(CoreTerm::UnitLit(None)),
        None
    );
    
    let generated_rust = emit_rust_from_core(&core_ir, "test.ax", "foo");
    
    println!("Generated Rust code:");
    println!("{}", generated_rust);
    
    // ASSERTION 1: Core IR function generates a Rust `fn` definition
    assert!(generated_rust.contains("fn foo("), 
        "Expected Core IR function 'foo' to generate 'fn foo(' definition");
    
    // ASSERTION 2: Foreign call generates direct shim call
    assert!(generated_rust.contains("shim::str_char_at("), 
        "Expected foreign call 'str_char_at' to generate 'shim::str_char_at(' call");
    
    // ASSERTION 3: NO foreign stub function is generated
    assert!(!generated_rust.contains("fn str_char_at("), 
        "Foreign function 'str_char_at' should NOT generate a stub 'fn str_char_at(' definition");
        
    // ASSERTION 4: NO other stub generation patterns
    assert!(!generated_rust.contains("// ========== AUTO-GENERATED RUNTIME"), 
        "Should not contain any auto-generated runtime section");
        
    println!("✅ All assertions passed: emit_rust.rs refactoring works correctly");
}

#[test] 
fn test_unmapped_foreign_symbol_fails_fast() {
    // Create Core IR that calls an unmapped foreign function
    let core_ir = CoreTerm::Let(
        "test_func".to_string(),
        Rc::new(CoreTerm::Lam(
            "x".to_string(),
            Rc::new(CoreTerm::App(
                Rc::new(CoreTerm::Var("unmapped_foreign_function".to_string(), None)),
                Rc::new(CoreTerm::Var("x".to_string(), None)), 
                None
            )),
            None
        )),
        Rc::new(CoreTerm::UnitLit(None)),
        None
    );
    
    // Should panic with the specific error message
    let result = std::panic::catch_unwind(|| {
        emit_rust_from_core(&core_ir, "test.ax", "test_func")
    });
    
    assert!(result.is_err(), "Should panic on unmapped foreign symbol");
    
    // Check the panic message contains the expected text
    if let Err(panic_info) = result {
        if let Some(msg) = panic_info.downcast_ref::<String>() {
            assert!(msg.contains("Foreign symbol 'unmapped_foreign_function' is not mapped in shim"), 
                "Panic message should contain expected error text");
        } else if let Some(msg) = panic_info.downcast_ref::<&str>() {
            assert!(msg.contains("Foreign symbol 'unmapped_foreign_function' is not mapped in shim"), 
                "Panic message should contain expected error text");
        }
    }
    
    println!("✅ Fail-fast test passed: unmapped foreign symbols cause proper error");
}