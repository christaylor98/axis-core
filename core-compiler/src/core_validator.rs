// Core IR validation for deterministic failure behavior
use crate::runtime::CoreTerm;
use crate::validation_registry;
use crate::registry_loader::Registry;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
}

impl ValidationError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[derive(Clone, Debug)]
enum VarInfo {
    Lambda,    // Variable is bound to a lambda
    NonLambda, // Variable is bound to a non-lambda value
    Unknown,   // Variable binding is complex (could be lambda or not)
}

/// Validate Core IR and return deterministic error messages
/// 
/// Validation uses the CLI-loaded Registry as the sole authority.
/// No filesystem access is permitted here.
/// 
/// Invariants:
/// C1: Unbound variable detection - Any Var(name) not bound by Let or Lam is an error
/// C2: Application correctness - Reject App where function position is not a function
pub fn validate_core(term: &CoreTerm, registry: &Registry) -> Result<(), ValidationError> {
    //  Pre-bind all top-level function names for mutual recursion
    // Scan through nested Let bindings at the top level and collect all names
    let mut bound_vars = HashMap::new();    collect_top_level_bindings(term, &mut bound_vars);
    
    // Validate the entire term tree
    validate_term(term, &bound_vars, registry)?;
    
    Ok(())
}

/// Collect all top-level Let bindings (assumes module structure is nested Lets)
/// This enables mutual recursion among top-level functions
fn collect_top_level_bindings(term: &CoreTerm, bound_vars: &mut HashMap<String, VarInfo>) {
    match term {
        CoreTerm::Let(name, val, body, _) => {
            // Determine what kind of value we're binding to
            let var_info = match val.as_ref() {
                CoreTerm::Lam(_, _, _) => VarInfo::Lambda,
                CoreTerm::IntLit(_, _) | CoreTerm::BoolLit(_, _) | CoreTerm::UnitLit(_) | 
                CoreTerm::StrLit(_, _) | CoreTerm::Tuple(_, _) | CoreTerm::Ctor(_, _, _) => VarInfo::NonLambda,
                _ => VarInfo::Unknown,
            };
            bound_vars.insert(name.clone(), var_info);
            
            // Continue scanning through the Let chain
            collect_top_level_bindings(body, bound_vars);
        }
        _ => {
            // End of Let chain, stop collecting
        }
    }
}

fn validate_term(term: &CoreTerm, bound_vars: &HashMap<String, VarInfo>, registry: &Registry) -> Result<(), ValidationError> {
    match term {
        CoreTerm::IntLit(_, _) | CoreTerm::BoolLit(_, _) | CoreTerm::UnitLit(_) | CoreTerm::StrLit(_, _) => {
            Ok(())
        }

        CoreTerm::Ctor(_, fields, _) => {
            for field in fields {
                validate_term(field, bound_vars, registry)?;
            }
            Ok(())
        }
        
        CoreTerm::Var(name, span) => {
            // Step 4: Resolution attempt logging
            if name == "axis_io_print" {
                eprintln!("[RESOLVE] attempting axis_io_print");
                eprintln!("[RESOLVE]   in bound_vars: {}", bound_vars.contains_key(name));
            }
            
            // C1: Use registry-based classification for foreign function resolution
            if bound_vars.contains_key(name) {
                // Variable is locally bound
                Ok(())
            } else if validation_registry::is_known_function(registry, name) {
                // Known builtin or foreign function
                Ok(())
            } else {
                // Unknown function
                let mut msg = format!("E_UNBOUND_VAR: {}", name);
                if let Some(s) = span {
                    msg.push_str(&format!("\n  at {}:{}:{}", s.file, s.line, s.column));
                }
                return Err(ValidationError::new(msg));
            }
        }
        
        CoreTerm::Lam(param, body, _) => {
            // Lambda binds the parameter to a Lambda
            let mut new_bound = bound_vars.clone();
            new_bound.insert(param.clone(), VarInfo::Lambda);
            validate_term(body, &new_bound, registry)
        }
        
        CoreTerm::App(func, arg, _) => {
            // First validate both subterms
            // For func, use special validation that doesn't check arity to avoid partial application errors
            validate_term_no_arity(func, bound_vars, registry)?;
            validate_term(arg, bound_vars, registry)?;
            
            // C2: Check if function position can be applied
            if !can_be_function(func, bound_vars, registry) {
                let head_desc = format_term_for_error(func);
                return Err(ValidationError::new(
                    format!("E_APPLY_NON_FUNCTION: head={}", head_desc)
                ));
            }
            
            Ok(())
        }
        
        CoreTerm::Let(name, val, body, _) => {
            // First validate the value expression
            validate_term(val, bound_vars, registry)?;
            
            // Determine what kind of value we're binding to
            let var_info = match val.as_ref() {
                CoreTerm::Lam(_, _, _) => VarInfo::Lambda,
                CoreTerm::IntLit(_, _) | CoreTerm::BoolLit(_, _) | CoreTerm::UnitLit(_) | 
                CoreTerm::StrLit(_, _) | CoreTerm::Tuple(_, _) | CoreTerm::Ctor(_, _, _) => VarInfo::NonLambda,
                _ => VarInfo::Unknown, // Complex expressions
            };
            
            // Then validate body with the new binding
            let mut new_bound = bound_vars.clone();
            new_bound.insert(name.clone(), var_info);
            validate_term(body, &new_bound, registry)
        }
        
        CoreTerm::Tuple(elems, _) => {
            for elem in elems {
                validate_term(elem, bound_vars, registry)?;
            }
            Ok(())
        }

        CoreTerm::Proj(tuple_expr, _idx, _) => {
            validate_term(tuple_expr, bound_vars, registry)
        }

        CoreTerm::If(cond, then_branch, else_branch, _) => {
            validate_term(cond, bound_vars, registry)?;
            validate_term(then_branch, bound_vars, registry)?;
            validate_term(else_branch, bound_vars, registry)
        }

        CoreTerm::Match(scrutinee, _patterns, _) => {
            validate_term(scrutinee, bound_vars, registry)?;
            // For simplicity, we don't validate match patterns in Batch C
            Ok(())
        }
    }
}

/// Validate a term without checking arity (used for function position validation to avoid partial application errors)
fn validate_term_no_arity(term: &CoreTerm, bound_vars: &HashMap<String, VarInfo>, registry: &Registry) -> Result<(), ValidationError> {
    match term {
        CoreTerm::IntLit(_, _) | CoreTerm::BoolLit(_, _) | CoreTerm::UnitLit(_) | 
        CoreTerm::StrLit(_, _) => Ok(()),

        CoreTerm::Ctor(_, fields, _) => {
            for field in fields {
                validate_term_no_arity(field, bound_vars, registry)?;
            }
            Ok(())
        }
        
        CoreTerm::Tuple(elements, _) => {
            for elem in elements {
                validate_term_no_arity(elem, bound_vars, registry)?;
            }
            Ok(())
        }
        
        CoreTerm::Var(name, span) => {
            if !bound_vars.contains_key(name) && !validation_registry::is_known_function(registry, name) {
                let mut msg = format!("E_UNBOUND_VAR: {}", name);
                if let Some(s) = span {
                    msg.push_str(&format!("\n  at {}:{}:{}", s.file, s.line, s.column));
                }
                Err(ValidationError::new(msg))
            } else {
                Ok(())
            }
        }
        
        CoreTerm::Lam(param, body, _) => {
            let mut new_bound = bound_vars.clone();
            new_bound.insert(param.clone(), VarInfo::Lambda);
            validate_term_no_arity(body, &new_bound, registry)
        }
        
        CoreTerm::App(func, arg, _) => {
            // Recursively validate without arity checking
            validate_term_no_arity(func, bound_vars, registry)?;
            validate_term_no_arity(arg, bound_vars, registry)?;
            
            // Check if function position can be applied
            if !can_be_function(func, bound_vars, registry) {
                let head_desc = format_term_for_error(func);
                return Err(ValidationError::new(
                    format!("E_APPLY_NON_FUNCTION: head={}", head_desc)
                ));
            }
            
            // NO arity checking here - that's the whole point
            Ok(())
        }
        
        CoreTerm::Let(name, val, body, _) => {
            validate_term_no_arity(val, bound_vars, registry)?;
            let var_info = match val.as_ref() {
                CoreTerm::Lam(_, _, _) => VarInfo::Lambda,
                CoreTerm::IntLit(_, _) | CoreTerm::BoolLit(_, _) | CoreTerm::UnitLit(_) | 
                CoreTerm::StrLit(_, _) | CoreTerm::Tuple(_, _) | CoreTerm::Ctor(_, _, _) => VarInfo::NonLambda,
                _ => VarInfo::Unknown,
            };
            let mut new_bound = bound_vars.clone();
            new_bound.insert(name.clone(), var_info);
            validate_term_no_arity(body, &new_bound, registry)
        }
        
        CoreTerm::Proj(tuple_expr, _idx, _) => {
            validate_term_no_arity(tuple_expr, bound_vars, registry)
        }
        
        CoreTerm::If(cond, then_branch, else_branch, _) => {
            validate_term_no_arity(cond, bound_vars, registry)?;
            validate_term_no_arity(then_branch, bound_vars, registry)?;
            validate_term_no_arity(else_branch, bound_vars, registry)
        }
        
        CoreTerm::Match(scrutinee, _patterns, _) => {
            validate_term_no_arity(scrutinee, bound_vars, registry)?;
            Ok(())
        }
    }
}

/// Check if a term can be applied as a function
/// Returns true for:
/// - Lam (lambda expressions) 
/// - Var that is bound to a Lambda
/// - Var that refers to a known builtin/foreign function ( ONLY)
/// Returns false for:
/// - IntLit, BoolLit, UnitLit, StrLit, Tuple, etc.
/// - Var that is bound to a NonLambda
fn can_be_function(term: &CoreTerm, bound_vars: &HashMap<String, VarInfo>, registry: &Registry) -> bool {
    match term {
        CoreTerm::Lam(_, _, _) => true,
        CoreTerm::Var(name, _) => {
            match bound_vars.get(name) {
                Some(VarInfo::Lambda) => true,
                Some(VarInfo::NonLambda) => false,
                Some(VarInfo::Unknown) => true, // Be conservative for complex cases
                None => {
                    //  ONLY: Allow known builtins to be called
                    validation_registry::is_known_function(registry, name)
                }
            }
        }
        CoreTerm::IntLit(_, _) | CoreTerm::BoolLit(_, _) | CoreTerm::UnitLit(_) | 
        CoreTerm::StrLit(_, _) | CoreTerm::Tuple(_, _) | CoreTerm::Ctor(_, _, _) => false,
        
        // For more complex terms, be conservative and allow them
        // (they might evaluate to functions)
        CoreTerm::App(_, _, _) | CoreTerm::Let(_, _, _, _) | CoreTerm::If(_, _, _, _) | 
        CoreTerm::Proj(_, _, _) | CoreTerm::Match(_, _, _) => true,
    }
}

/// Format a term for error messages (showing only the top-level structure)
fn format_term_for_error(term: &CoreTerm) -> String {
    match term {
        CoreTerm::IntLit(n, _) => format!("IntLit({})", n),
        CoreTerm::BoolLit(b, _) => format!("BoolLit({})", b),
        CoreTerm::UnitLit(_) => "UnitLit".to_string(),
        CoreTerm::StrLit(s, _) => {
            let preview = if s.len() > 20 { &s[..20] } else { s };
            format!("StrLit(\"{}...\")", preview)
        }
        CoreTerm::Var(name, _) => format!("Var({})", name),
        CoreTerm::Ctor(name, _, _) => format!("Ctor({})", name),
        CoreTerm::Lam(param, _, _) => format!("Lam({}, <body>)", param),
        CoreTerm::App(_, _, _) => "App(<func>, <arg>)".to_string(),
        CoreTerm::Let(name, _, _, _) => format!("Let({}, <val>, <body>)", name),
        CoreTerm::Tuple(_, _) => "Tuple(<...>)".to_string(),
        CoreTerm::Proj(_, idx, _) => format!("Proj(<tuple>, {})", idx),
        CoreTerm::If(_, _, _, _) => "If(<cond>, <then>, <else>)".to_string(),
        CoreTerm::Match(_, _, _) => "Match(<scrutinee>, <arms>)".to_string(),
    }
}
#[allow(dead_code)]
// Reserved for richer call-shape validation
// Extract function name and arity from an App chain if it represents a function call
// Returns (function_name, arity) if it's a function call, None otherwise
fn extract_function_call(term: &CoreTerm) -> Option<(String, u32)> {
    // Count the arguments by traversing the App chain
    let mut current = term;
    let mut args = 0;
    
    // Walk up the App chain to count arguments
    while let CoreTerm::App(func, _arg, _) = current {
        args += 1;
        current = func;
    }
    
    // Check if the head is a Var (function name)
    if let CoreTerm::Var(name, _) = current {

        Some((name.clone(), args))
    } else {
        None
    }
}