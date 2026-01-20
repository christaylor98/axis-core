// Lower surface syntax to Core
use crate::surface_parser::{SurfaceExpr, SurfaceStmt, FnDef, Module};
use crate::runtime::Value;

// REGIME COMPLIANCE: No modules, no use statements, no main auto-calling
pub fn lower_module(module: Module) -> Value {
    let mut core = Value::Unit;
    
    // Lower all top-level functions (in reverse order for proper let-binding nesting)
    for func in module.functions.iter().rev() {
        let lambda = lower_function(func, &[]);
        core = Value::Let(func.name.clone(), Box::new(lambda), Box::new(core));
    }
    
    core
}

// REGIME COMPLIANCE: No module paths, simplified function lowering
fn lower_function(func: &FnDef, _module_path: &[String]) -> Value {
    let body = lower_expr(&func.body);
    build_lambda(&func.params, body)
}

fn build_lambda(params: &[String], body: Value) -> Value {
    if params.is_empty() {
        Value::Lam("_unit".to_string(), Box::new(body))
    } else {
        let mut result = body;
        for param in params.iter().rev() {
            result = Value::Lam(param.clone(), Box::new(result));
        }
        result
    }
}

fn lower_expr(expr: &SurfaceExpr) -> Value {
    match expr {
        SurfaceExpr::IntLit(n) => Value::Int(*n),
        SurfaceExpr::BoolLit(b) => Value::Bool(*b),
        SurfaceExpr::StringLit(s) => Value::Str(crate::intern_string(s.clone())),
        SurfaceExpr::UnitLit => Value::Unit,
        SurfaceExpr::Ident(name) => Value::Var(name.clone()),
        SurfaceExpr::Call(name, args) => {
            // Handle struct literal syntax: TypeName { field: value, ... }
            // Parser represents this as Call("__struct_lit__", [TypeName, "field", value, ...])
            if name == "__struct_lit__" && !args.is_empty() {
                // First arg is the type name, rest are field-value pairs
                let type_name = if let SurfaceExpr::Ident(tn) = &args[0] {
                    tn.clone()
                } else {
                    panic!("Invalid struct literal: expected type name");
                };
                
                // Apply constructor to field values in order
                let mut app = Value::Var(type_name);
                for i in (1..args.len()).step_by(2) {
                    // Skip field names (odd indices), lower values (even indices)
                    if i + 1 < args.len() {
                        let field_value = lower_expr(&args[i + 1]);
                        app = Value::App(Box::new(app), Box::new(field_value));
                    }
                }
                return app;
            }
            
            let mut app = Value::Var(name.clone());
            let is_ctor = is_constructor_name(name);

            if args.is_empty() {
                if is_ctor {
                    // Constructors with no fields lower as bare identifiers
                    return Value::Var(name.clone());
                }
                app = Value::App(Box::new(app), Box::new(Value::Unit));
            } else {
                for arg in args {
                    let lowered_arg = lower_expr(arg);
                    app = Value::App(Box::new(app), Box::new(lowered_arg));
                }
            }

            app
        }
        SurfaceExpr::Proj(obj, idx) => {
            // Lower explicit projection to CField enum so surface_to_core
            // will convert it to a Core `Proj` node during final conversion.
            let obj_val = lower_expr(obj);
            let idx_val = Value::Int(*idx);
            Value::Enum("CField".to_string(), vec![obj_val, idx_val])
        }
        SurfaceExpr::Block(stmts) => lower_block(stmts),
        SurfaceExpr::Match(scrutinee, arms) => {
            // L4.3: Lower to Core decision structure
            let scrut_val = lower_expr(scrutinee);
            
            if arms.is_empty() {
                panic!("Match with no arms");
            }
            
            // Build match arms with pattern strings and body values
            let core_arms: Vec<(String, Value)> = arms
                .iter()
                .map(|arm| {
                    let body_val = lower_expr(&arm.expr);
                    (arm.pattern.clone(), body_val)
                })
                .collect();
            
            Value::Match(Box::new(scrut_val), core_arms)
        }
        SurfaceExpr::If { cond, then_branch, else_branch } => {
            // L4.2: Lower to real Core conditional
            let cond_val = lower_expr(cond);
            let then_val = lower_expr(then_branch);
            let else_val = lower_expr(else_branch);
            Value::If(
                Box::new(cond_val),
                Box::new(then_val),
                Box::new(else_val),
            )
        }
    }
}

fn lower_block(stmts: &[SurfaceStmt]) -> Value {
    // L4.1: Block lowers to real Core sequence
    if stmts.is_empty() {
        // Empty block yields Unit
        return Value::Unit;
    }
    
    if stmts.len() == 1 {
        // Single statement - return its value
        return lower_stmt(&stmts[0]);
    }
    
    // Multiple statements - process in order
    match &stmts[0] {
        SurfaceStmt::Let(name, expr) => {
            // Let binding - bind and continue
            let value = lower_expr(expr);
            let rest = lower_block(&stmts[1..]);
            Value::Let(name.clone(), Box::new(value), Box::new(rest))
        }
        SurfaceStmt::LetPattern(ctor_name, field_vars, expr) => {
            //  Pattern let - desugar to field extraction
            // let Pair(x, y) = rhs  =>  let _tmp = rhs in let x = __ctor_field__(_tmp, 0) in let y = __ctor_field__(_tmp, 1) in <rest>
            let rhs_value = lower_expr(expr);
            let tmp_var = format!("_tmp_{}_{}", ctor_name, field_vars.len());
            
            // Build nested lets for each field variable
            let rest = lower_block(&stmts[1..]);
            let mut body = rest;
            for (i, field_var) in field_vars.iter().enumerate().rev() {
                // Build: __ctor_field__(_tmp, i)
                let extract_call = Value::App(
                    Box::new(Value::App(
                        Box::new(Value::Var("__ctor_field__".to_string())),
                        Box::new(Value::Var(tmp_var.clone())),
                    )),
                    Box::new(Value::Int(i as i64)),
                );
                body = Value::Let(field_var.clone(), Box::new(extract_call), Box::new(body));
            }
            
            // Bind the temporary variable to the RHS value
            Value::Let(tmp_var, Box::new(rhs_value), Box::new(body))
        }
        SurfaceStmt::Expr(expr) => {
            // Expression statement - evaluate for side effects, discard value
            let value = lower_expr(expr);
            let rest = lower_block(&stmts[1..]);
            // Bind to dummy variable to sequence evaluation
            Value::Let("_discard".to_string(), Box::new(value), Box::new(rest))
        }
    }
}

fn lower_stmt(stmt: &SurfaceStmt) -> Value {
    match stmt {
        SurfaceStmt::Let(name, expr) => {
            Value::Let(name.clone(), Box::new(lower_expr(expr)), Box::new(Value::Unit))
        }
        SurfaceStmt::LetPattern(ctor_name, field_vars, expr) => {
            //  Pattern let in single-statement context
            let rhs_value = lower_expr(expr);
            let tmp_var = format!("_tmp_{}_{}", ctor_name, field_vars.len());
            
            // Build nested lets for each field variable, ending with Unit
            let mut body = Value::Unit;
            for (i, field_var) in field_vars.iter().enumerate().rev() {
                let extract_call = Value::App(
                    Box::new(Value::App(
                        Box::new(Value::Var("__ctor_field__".to_string())),
                        Box::new(Value::Var(tmp_var.clone())),
                    )),
                    Box::new(Value::Int(i as i64)),
                );
                body = Value::Let(field_var.clone(), Box::new(extract_call), Box::new(body));
            }
            
            Value::Let(tmp_var, Box::new(rhs_value), Box::new(body))
        }
        SurfaceStmt::Expr(expr) => lower_expr(expr),
    }
}

fn is_constructor_name(name: &str) -> bool {
    let last = if let Some(idx) = name.rfind("::") {
        &name[idx + 2..]
    } else if let Some(idx) = name.rfind('.') {
        &name[idx + 1..]
    } else {
        name
    };
    last.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}
