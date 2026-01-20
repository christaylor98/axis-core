// Library interface for axis-compiler
// Exposes the surface parser for testing

pub mod surface_parser;
pub mod runtime_value;
pub mod registry_loader;
pub mod validation_registry;

use runtime_value::Value;

/// Public compile entry that can be called from generated Rust code.
/// This is the bridge between generated main() and the compiler pipeline.
pub fn compile_entry(_args: Value) -> Value {
    println!("[compile_entry] start");
    println!("[compile_entry] compiler compile not yet implemented");
    println!("[compile_entry] this will be replaced by self-hosted compiler");
    Value::Unit
}
