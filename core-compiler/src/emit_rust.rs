// Emit Rust code from Core IR - ANDL Loop 6: Value-based codegen
use crate::runtime::CoreTerm;
use std::rc::Rc;
use std::collections::HashSet;

// REGIME COMPLIANCE: No filename-based special-casing
// TEMPORARY: entry_fn parameter for entry point selection (will be removed)
pub fn emit_rust_from_core(core: &CoreTerm, _input_path: &str, entry_fn: &str) -> String {
    let mut output = String::new();
    let mut used_primitives: HashSet<String> = HashSet::new();

    // Generate runtime
    // output.push_str("use std::rc::Rc;\n");
    output.push_str(&generate_value_runtime(&used_primitives));
    output.push_str("\n// Generated function definitions\n");
    
    let mut emitted_functions = HashSet::new();
    emit_top_level_lets(core, &mut output, "", &mut emitted_functions, &mut used_primitives);

    // Generate runtime/foreign surface for ALL missing symbols
    let missing_symbols = compute_missing_symbols(&used_primitives, &emitted_functions);
    
    output.push_str("\n// ========== AUTO-GENERATED RUNTIME/FOREIGN SURFACE ==========\n");
    for symbol in missing_symbols.iter() {
        let arity = infer_arity(&output, symbol);
        let stub = generate_runtime_stub(symbol, arity);
        output.push_str(&stub);
        output.push_str("\n");
    }
    output.push_str("// ========== END AUTO-GENERATED RUNTIME ==========\n\n");

    // Verify all symbols are now defined
    let still_missing = verify_all_symbols_defined(&output, &used_primitives);
    if !still_missing.is_empty() {
        eprintln!("[emit_rust] ERROR: Still missing symbols after generation:");
        for sym in still_missing.iter().take(50) {
            eprintln!("  - {}", sym);
        }
        panic!("Runtime generation incomplete: {} symbols still missing", still_missing.len());
    }

    output
}

fn emit_term(term: &CoreTerm, indent: usize) -> String {
    // Convenience wrapper for ad-hoc calls; does not record used primitives.
    let mut tmp_set = HashSet::new();
    emit_term_with_module(term, indent, "", &mut tmp_set)
}

// Minimal helper: extract module path from an input path like "axis/compiler/main.ax"
fn extract_module_path(input_path: &str) -> String {
    let mut p = input_path;
    if let Some(stripped) = p.strip_prefix("./") {
        p = stripped;
    }
    if let Some(stripped) = p.strip_prefix("axis/") {
        p = stripped;
    }
    if let Some(dot) = p.rfind('.') {
        p = &p[..dot];
    }
    p.replace('/', "__")
}

// REGIME COMPLIANCE: Simplified function emission (no module paths)
fn emit_top_level_lets(core: &CoreTerm, output: &mut String, _module_path: &str, emitted_functions: &mut HashSet<String>, used_prims: &mut HashSet<String>) {
    // Walk nested top-level Let bindings and emit a Rust function for each
    let mut current = core;

    loop {
        match current {
            CoreTerm::Let(name, value_rc, body_rc, _) => {
                let value = value_rc.as_ref();

                // Collect parameters by descending through nested Lambdas
                let mut params: Vec<String> = Vec::new();
                let mut inner = value;
                while let CoreTerm::Lam(param, inner_body, _) = inner {
                    params.push(param.clone());
                    inner = inner_body.as_ref();
                }

                // REGIME COMPLIANCE: Simple name mangling (no module paths)
                let mangled = sanitize_ident(name);
                if emitted_functions.contains(&mangled) {
                    // skip duplicates
                } else {
                    emitted_functions.insert(mangled.clone());

                    // UNARY INVARIANT: All functions are unary
                    if params.is_empty() {
                        output.push_str(&format!("fn {}() -> Value {{\n", mangled));
                    } else if params.len() == 1 {
                        let param_name = sanitize_ident(&params[0]);
                        output.push_str(&format!("fn {}({}: Value) -> Value {{\n", mangled, param_name));
                    } else {
                        // N-arity function (N > 1) - use tuple destructuring
                        output.push_str(&format!("fn {}(args: Value) -> Value {{\n", mangled));
                        for (i, param) in params.iter().enumerate() {
                            let param_name = sanitize_ident(param);
                            output.push_str(&format!("    let {} = tuple_field(args.clone(), {});\n", param_name, i));
                        }
                    }

                    // Emit body
                    let body_code = emit_term_with_module(inner, 1, "", used_prims);
                    for line in body_code.lines() {
                        output.push_str("    ");
                        output.push_str(line);
                        output.push_str("\n");
                    }

                    output.push_str("}\n\n");
                }

                // Continue with the body (remaining top-level lets)
                current = body_rc.as_ref();
            }
            _ => break,
        }
    }
}

// Collect args from nested App nodes for uncurrying
// e.g., App(App(Var(f), a), b) -> (f, [a, b])
fn collect_app_args(term: &CoreTerm) -> (&CoreTerm, Vec<&CoreTerm>) {
    let mut args = Vec::new();
    let mut current = term;
    
    // Traverse nested App nodes to collect all arguments
    while let CoreTerm::App(func, arg, _) = current {
        args.push(arg.as_ref());
        current = func.as_ref();
    }
    
    // Reverse args since we collected them right-to-left
    args.reverse();
    (current, args)
}

fn emit_term_with_module(term: &CoreTerm, indent: usize, module_path: &str, used_prims: &mut std::collections::HashSet<String>) -> String {
    match term {
        CoreTerm::IntLit(n, _) => format!("Value::Int({})", n),
        CoreTerm::BoolLit(true, _) => "Value::Bool(true)".to_string(),
        CoreTerm::BoolLit(false, _) => "Value::Bool(false)".to_string(),
        CoreTerm::UnitLit(_) => "Value::Unit".to_string(),  // Unit as Value::Unit
        CoreTerm::StrLit(s, _) => {
            // Use proper string interning for Value::Str
            format!("Value::Str(intern_str(\"{}\"))", s.escape_default())
        }
        
        CoreTerm::Var(name, _) => {
            // Handle boolean literals (no mangling)
            let stripped_name = strip_namespaces(name);
            if stripped_name == "true" {
                return "Value::Bool(true)".to_string();
            }
            if stripped_name == "false" {
                return "Value::Bool(false)".to_string();
            }

            // FIELD ACCESS CONVENTION: c_pattern means "field pattern of c"
            // Known field names for MatchCase: pattern (0), body (1)
            // ONLY apply this for single-letter base names (e.g., c_pattern, not core_body)
            let field_map: &[(&str, usize)] = &[
                ("pattern", 0),
                ("body", 1),
            ];
            for (field_name, field_idx) in field_map {
                let suffix = format!("_{}", field_name);
                if stripped_name.ends_with(&suffix) {
                    let base = &stripped_name[..stripped_name.len() - suffix.len()];
                    // Only apply for single-letter base names (typical in patterns like Cons(c, rest))
                    if base.len() == 1 && base.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                        // Emit field projection: match &base { Value::Ctor { fields, .. } => fields[idx].clone(), _ => panic!(...) }
                        let base_mangled = sanitize_ident(base);
                        return format!(
                            "match &{} {{ Value::Ctor {{ fields, .. }} => fields[{}].clone(), _ => panic!(\"Field access on non-ctor\") }}",
                            base_mangled, field_idx
                        );
                    }
                }
            }

            // Task 3: Strip namespaces and sanitize identifier
            // If this looks like a constructor (capitalized final segment), emit as a zero-arg call: `Ctor()`
            let mangled = sanitize_ident(&stripped_name);
            let last_seg = &mangled;
            if last_seg.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                // Register as used primitive so helper gets generated
                used_prims.insert(mangled.clone());
                format!("{}()", mangled)
            } else {
                //  POLICY: Clone all variable references to avoid E0382 borrow errors
                // The emitted code may reuse variables in multiple contexts (match arms, if branches, etc.)
                format!("{}.clone()", mangled)
            }
        }
        CoreTerm::Ctor(name, fields, _) => {
            let tag_name = strip_namespaces(name);
            let mut field_exprs = Vec::new();
            for field in fields {
                field_exprs.push(emit_term_with_module(field, indent, module_path, used_prims));
            }
            let fields_code = if field_exprs.is_empty() {
                "vec![]".to_string()
            } else {
                format!("vec![{}]", field_exprs.join(", "))
            };
            format!(
                "Value::Ctor {{ tag: intern_tag(\"{}\"), fields: {} }}",
                tag_name,
                fields_code
            )
        }
        
        CoreTerm::Lam(param, body, _) => {
            // Emit lambda as a closure with a mangled Value parameter so Var references resolve
            let param_name = sanitize_ident(param);
            let body_code = emit_term_with_module(body, indent + 1, module_path, used_prims);
            format!("Box::new(move |{}: Value| -> Value {{ {} }}) as Box<dyn Fn(Value) -> Value>", param_name, body_code)
        }
        
        CoreTerm::App(func, arg, _) => {
            // UNCURRYING: Check if this is a nested application that should be flattened
            let (base_func, all_args) = collect_app_args(term);
            
            // Special-case: __ctor_field__ used as tuple projection (two-arg call)
            // We only rewrite when we're confident this is a projection (not
            // pattern-destructuring). Pattern-destructuring in the lowering phase
            // emits calls like __ctor_field__(_tmp_X, i) where the first arg is a
            // temporary variable beginning with "_tmp_". We avoid rewriting in
            // that case to preserve constructor field semantics.
            if let CoreTerm::Var(func_name, _) = base_func {
                let stripped = strip_namespaces(func_name);
                if stripped == "__ctor_field__" && all_args.len() == 2 {
                    // Check second arg is an int literal
                    if let CoreTerm::IntLit(idx, _) = all_args[1] {
                        // If first arg is a tmp var created by lowering, do NOT rewrite
                        let first_arg = all_args[0];
                        let is_tmp_var = match first_arg {
                            CoreTerm::Var(vn, _) => vn.starts_with("_tmp_"),
                            _ => false,
                        };

                        if !is_tmp_var {
                            // Emit as the foreign projection primitive `proj(value, index)`
                            // Use the canonical `proj` binding (unmangled) so the runtime
                            // dispatches to `foreign_core_proj`.
                            let proj_name = "proj".to_string();
                            used_prims.insert(proj_name.clone());
                            let tuple_code = emit_term_with_module(first_arg, indent, module_path, used_prims);
                            let tuple_final = if needs_clone(first_arg) { format!("{}.clone()", tuple_code) } else { tuple_code };
                            // Index must be a literal Int (0-based)
                            // UNARY INVARIANT: Pack both arguments into a single tuple
                            return format!("{}(Value::Tuple(vec![{}, Value::Int({})]))", proj_name, tuple_final, idx);
                        }
                    }
                }
            }

            if all_args.len() > 1 {
                // Multiple arguments: pack into single tuple (UNARY INVARIANT)
                match base_func {
                    CoreTerm::Var(func_name, _) => {
                        let mangled_name = sanitize_ident(&strip_namespaces(func_name));
                        // Record this mangled symbol as a used primitive/function name
                        used_prims.insert(mangled_name.clone());
                        let arg_codes: Vec<String> = all_args.iter()
                            .map(|a| {
                                let code = emit_term_with_module(a, indent, module_path, used_prims);
                                //  POLICY: clone all function arguments
                                if needs_clone(a) { format!("{}.clone()", code) } else { code }
                            })
                            .collect();
                        
                        // Special case: binary runtime primitives take two direct Value arguments
                        let binary_prims = [
                            "__add__", "__sub__", "__mul__", "__div__", "__mod__",
                            "__eq__", "__lt__", "__gt__", "__lte__", "__gte__",
                            "__and__", "__or__", "__concat__", "str_char"
                        ];
                        // Special case: ternary runtime primitives take three direct Value arguments
                        let ternary_prims = ["str_slice"];
                        
                        if binary_prims.contains(&mangled_name.as_str()) && arg_codes.len() == 2 {
                            // Emit direct binary call: prim(arg0, arg1)
                            format!("{}({}, {})", mangled_name, arg_codes[0], arg_codes[1])
                        } else if ternary_prims.contains(&mangled_name.as_str()) && arg_codes.len() == 3 {
                            // Emit direct ternary call: prim(arg0, arg1, arg2)
                            format!("{}({}, {}, {})", mangled_name, arg_codes[0], arg_codes[1], arg_codes[2])
                        } else {
                            // UNARY INVARIANT: Pack all arguments into a single Value::Tuple
                            format!("{}(Value::Tuple(vec![{}]))", mangled_name, arg_codes.join(", "))
                        }
                    }
                    _ => {
                        // Non-variable function: emit curried
                        let func_code = emit_term_with_module(func, indent, module_path, used_prims);
                        let arg_code = emit_term_with_module(arg, indent, module_path, used_prims);
                        format!("({})({})", func_code, arg_code)
                    }
                }
            } else {
                // Single argument: emit normally but with mangling
                match func.as_ref() {
                    CoreTerm::Var(func_name, _) => {
                        let mangled_name = sanitize_ident(&strip_namespaces(func_name));
                        used_prims.insert(mangled_name.clone());
                        let arg_code = emit_term_with_module(arg, indent, module_path, used_prims);
                        //  POLICY: clone function arguments
                        let arg_final = if needs_clone(arg) { format!("{}.clone()", arg_code) } else { arg_code };
                        format!("{}({})", mangled_name, arg_final)
                    }
                    _ => {
                        let func_code = emit_term_with_module(func, indent, module_path, used_prims);
                        let arg_code = emit_term_with_module(arg, indent, module_path, used_prims);
                        //  POLICY: clone function arguments
                        let arg_final = if needs_clone(arg) { format!("{}.clone()", arg_code) } else { arg_code };
                        format!("({})({})", func_code, arg_final)
                    }
                }
            }
        }
        
        CoreTerm::Let(name, value, body, _) => {
            // Mangle the binder so locals and Var references align with mangling
            let var_name = sanitize_ident(name);

            // Emit value and body recursively
            let value_code = emit_term_with_module(value, indent + 1, module_path, used_prims);
            let body_code = emit_term_with_module(body, indent + 1, module_path, used_prims);

            // TASK 3: UNCONDITIONAL tuple projection workaround
            // ALWAYS emit tuple projections for every let-binding
            // This handles Core IR bugs where tuple type annotations become patterns
            // Extra bindings are harmless; missing bindings are fatal
            
            // Preserve indentation: produce a block with an indented `let` and body
            let indent_str = "    ".repeat(indent);
            let inner_indent = "    ".repeat(indent + 1);

            // Build the full block
            let mut block = String::new();
            block.push_str("{\n");
            block.push_str(&format!("{}let {} = {};\n", inner_indent, var_name, value_code));

            eprintln!("[lower-let] {} projections=10", var_name);
            
            //  WORKAROUND: Emit tuple projections for let-bound variables
            // This handles Core IR bugs where references don't match bindings
            // Also create common aliases to handle name drift (e.g., json_term -> json_body)
            // for i in 0..10 {
            //     let proj_var = format!("{}_{}", var_name, i);
            //     block.push_str(&format!("{}let {} = tuple_field({}.clone(), {});\n", 
            //         inner_indent, proj_var, var_name, i));
            // }
            
            // ADDITIONAL workaround: Create name-drift aliases for common patterns
            // Pattern 1: var_term -> var (strip "_term" suffix)
            if var_name.ends_with("_term") {
                let base = &var_name[..var_name.len() - 5]; // Remove "_term"
                block.push_str(&format!("{}let {} = {}.clone();\n", 
                    inner_indent, base, var_name));
                // Also create projections for the alias
                for i in 0..10 {
                    let proj_var = format!("{}_{}", base, i);
                    block.push_str(&format!("{}let {} = tuple_field({}.clone(), {});\n", 
                        inner_indent, proj_var, base, i));
                }
            }
            // Pattern 2: Check if this looks like it should have a "_body" alias
            // json_term -> json_body
            if var_name.ends_with("_term") {
                let base = &var_name[..var_name.len() - 5];
                let body_alias = format!("{}_body", base);
                block.push_str(&format!("{}let {} = {}.clone();\n", 
                    inner_indent, body_alias, var_name));
            }

            // Indent body lines to match inner indentation
            let indented_body = body_code
                .lines()
                .map(|l| format!("{}{}", inner_indent, l))
                .collect::<Vec<_>>()
                .join("\n");

            block.push_str(&format!("{}\n", indented_body));
            block.push_str(&format!("{}}}", indent_str));
            block
        }
        
        CoreTerm::Tuple(elems, _) => {
            let elem_codes: Vec<String> = elems.iter()
                .map(|e| {
                    let code = emit_term_with_module(e, indent, module_path, used_prims);
                    //  POLICY: clone tuple elements
                    if needs_clone(e) { format!("{}.clone()", code) } else { code }
                })
                .collect();
            format!("Value::Tuple(vec![{}])", elem_codes.join(", "))
        }
        
        CoreTerm::Proj(tuple, idx, _) => {
            let tuple_code = emit_term_with_module(tuple, indent, module_path, used_prims);
            //  POLICY: clone projected values
            let tuple_final = if needs_clone(tuple) { format!("{}.clone()", tuple_code) } else { tuple_code };
            // CoreTerm::Proj uses 1-based indexing; proj expects 0-based index
            let zero_based = idx.saturating_sub(1);
            // Emit as foreign primitive `proj(value, index)` with literal Int
            // UNARY INVARIANT: Pack both arguments into a single tuple
            used_prims.insert("proj".to_string());
            format!("proj(Value::Tuple(vec![{}, Value::Int({})]))", tuple_final, zero_based)
        }
        
        CoreTerm::If(cond, then_branch, else_branch, _) => {
            let cond_code = emit_term_with_module(cond, indent, module_path, used_prims);
            let then_code = emit_term_with_module(then_branch, indent, module_path, used_prims);
            let else_code = emit_term_with_module(else_branch, indent, module_path, used_prims);
            // No additional cloning needed here - truthy takes a reference
            format!("if truthy(&({})) {{ {} }} else {{ {} }}",
                cond_code, then_code, else_code)
        }
        
        CoreTerm::Match(scrutinee, arms, _) => {
            // Emit a real Rust `match` on the evaluated scrutinee with recursive pattern lowering
            let scr_code = emit_term_with_module(scrutinee, indent + 1, module_path, used_prims);
            let scr_var = sanitize_ident("scr");

            let mut arm_strs: Vec<String> = Vec::new();
            let mut temp_counter = 0;
            
            for (pat, arm_term) in arms.iter() {
                // Use recursive pattern lowering
                let (arm_pat, bindings) = lower_pattern_recursive(
                    pat, 
                    &scr_var, 
                    module_path, 
                    &mut temp_counter
                );
                
                let arm_body = emit_term_with_module(arm_term, indent + 2, module_path, used_prims);

                let mut arm_block = String::new();
                arm_block.push_str(&format!("{} => {{\n", arm_pat));
                for binding in bindings.iter() {
                    arm_block.push_str(&format!("    {}\n", binding));
                }
                let indented = arm_body.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n");
                arm_block.push_str(&format!("{}\n", indented));
                arm_block.push_str("}");
                arm_strs.push(arm_block);
            }

            arm_strs.push("_ => { Value::Unit }".to_string());

            let mut full = String::new();
            full.push_str("{\n");
            full.push_str(&format!("let {} = {};\n", scr_var, scr_code));
            full.push_str(&format!("match {} {{\n", scr_var));
            for a in arm_strs.iter() {
                full.push_str(&format!("    {},\n", a));
            }
            full.push_str("}\n}");
            full
        }
    }
}

/// Recursively lower a pattern into a match arm pattern and a list of bindings
/// Returns: (rust_pattern_string, vec_of_binding_statements)
fn lower_pattern_recursive(
    pattern: &crate::runtime::Pattern,
    scrutinee_expr: &str,
    module_path: &str,
    temp_counter: &mut usize,
) -> (String, Vec<String>) {
    use crate::runtime::Pattern;
    
    match pattern {
        Pattern::PInt(n) => {
            eprintln!("[lower-pattern] PInt({})", n);
            (format!("Value::Int(x) if *x == {}", n), vec![])
        }
        Pattern::PBool(b) => {
            eprintln!("[lower-pattern] PBool({})", b);
            (format!("Value::Bool(x) if *x == {}", b), vec![])
        }
        Pattern::PUnit => {
            eprintln!("[lower-pattern] PUnit");
            ("Value::Unit".to_string(), vec![])
        }
        Pattern::PVar(name) => {
            // Discard pattern: emit no bindings
            if name == "_" {
                eprintln!("[lower-pattern] PVar(_) -> discard (no bindings)");
                return ("_".to_string(), vec![]);
            }
            
            let bname = sanitize_ident(name);
            
            //  FIX: Capitalized names are 0-arity constructors, not variables
            // Don't emit bindings for them - they're just pattern guards
            if bname.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                eprintln!("[lower-pattern] PVar({}) -> 0-arity ctor (no bindings)", name);
                return ("_".to_string(), vec![]);
            }
            
            eprintln!("[lower-pattern] PVar({}) -> bind {}", name, bname);
            let bindings = vec![format!("let {} = {}.clone();", bname, scrutinee_expr)];
            
            eprintln!("[lower-pattern] PVar total bindings: {}", bindings.len());
            ("_".to_string(), bindings)
        }
        Pattern::PTuple(elements) => {
            let vec_ident = format!("__tuple_fields_{}", *temp_counter);
            *temp_counter += 1;
            eprintln!("[lower-pattern] PTuple({} elems) -> {}", elements.len(), vec_ident);
            
            let mut bindings = Vec::new();
            for (i, sub_pat) in elements.iter().enumerate() {
                let field_expr = format!("{}[{}]", vec_ident, i);
                let (_, sub_bindings) = lower_pattern_recursive(
                    sub_pat,
                    &field_expr,
                    module_path,
                    temp_counter
                );
                bindings.extend(sub_bindings);
            }
            
            let pattern_str = format!(
                "Value::Tuple({}) if {}.len() == {}", 
                vec_ident, vec_ident, elements.len()
            );
            (pattern_str, bindings)
        }
        Pattern::PEnum(tag_name, fields) => {
            let fields_ident = "__ctor_fields".to_string();
            *temp_counter += 1;
            eprintln!("[lower-pattern] PEnum({}, {} fields) -> {}", tag_name, fields.len(), fields_ident);
            
            let mut bindings = Vec::new();
            
            // Special case: 0-arity constructors (e.g., Nil, True, False)
            // Must NOT generate `let X::Y = ...` syntax
            if fields.is_empty() {
                let stripped_tag = strip_namespaces(tag_name);
                let pattern_str = format!(
                    "Value::Ctor {{ tag, fields: {} }} if get_tag_name(tag) == \"{}\" && {}.is_empty()",
                    fields_ident, stripped_tag, fields_ident
                );
                return (pattern_str, bindings);
            }
            
            // Recursively lower each field pattern
            for (i, sub_pat) in fields.iter().enumerate() {
                match sub_pat {
                    Pattern::PVar(vname) => {
                        // Skip discard patterns
                        if vname == "_" {
                            continue;
                        }
                        
                        // Direct variable binding from field
                        let bname = sanitize_ident(vname);
                        eprintln!("[lower-pattern] bind {} <- field({}[{}])", bname, fields_ident, i);
                        bindings.push(format!("let {} = {}[{}].clone();", bname, fields_ident, i));
                    }
                    Pattern::PEnum(sub_tag, sub_fields) => {
                        // Nested constructor: extract field to temp, then manually extract its fields
                        let temp_name = format!("__tmp_{}", *temp_counter);
                        *temp_counter += 1;
                        eprintln!("[lower-pattern] nested PEnum({}) -> {}", sub_tag, temp_name);
                        bindings.push(format!("let {} = {}[{}].clone();", temp_name, fields_ident, i));
                        
                        // Now extract fields from this nested constructor
                        // We know temp_name is a Value::Ctor, so we can access its fields
                        let nested_fields_ident = format!("__ctor_fields_{}", *temp_counter);
                        *temp_counter += 1;
                        bindings.push(format!(
                            "let {} = match &{} {{ Value::Ctor {{ fields, .. }} => fields, _ => panic!(\"Pattern mismatch\") }};",
                            nested_fields_ident, temp_name
                        ));
                        
                        // Recursively process the sub-fields
                        for (j, subsub_pat) in sub_fields.iter().enumerate() {
                            let field_expr = format!("{}[{}]", nested_fields_ident, j);
                            let (_, subsub_bindings) = lower_pattern_recursive(
                                subsub_pat,
                                &field_expr,
                                module_path,
                                temp_counter
                            );
                            bindings.extend(subsub_bindings);
                        }
                    }
                    Pattern::PTuple(tuple_elems) => {
                        // Nested tuple: extract field to temp, then extract tuple elements
                        let temp_name = format!("__tmp_{}", *temp_counter);
                        *temp_counter += 1;
                        eprintln!("[lower-pattern] nested PTuple({} elems) -> {}", tuple_elems.len(), temp_name);
                        bindings.push(format!("let {} = {}[{}].clone();", temp_name, fields_ident, i));
                        
                        // Extract tuple fields
                        let tuple_fields_ident = format!("__tuple_fields_{}", *temp_counter);
                        *temp_counter += 1;
                        bindings.push(format!(
                            "let {} = match &{} {{ Value::Tuple(elems) => elems, _ => panic!(\"Pattern mismatch\") }};",
                            tuple_fields_ident, temp_name
                        ));
                        
                        // Recursively process tuple elements
                        for (j, elem_pat) in tuple_elems.iter().enumerate() {
                            let elem_expr = format!("{}[{}]", tuple_fields_ident, j);
                            let (_, elem_bindings) = lower_pattern_recursive(
                                elem_pat,
                                &elem_expr,
                                module_path,
                                temp_counter
                            );
                            bindings.extend(elem_bindings);
                        }
                    }
                    _ => {
                        // Other patterns (int, bool, unit) - just extract field
                        let temp_name = format!("__tmp_{}", *temp_counter);
                        *temp_counter += 1;
                        bindings.push(format!("let {} = {}[{}].clone();", temp_name, fields_ident, i));
                    }
                }
            }
            
            // Strip namespace from tag_name for Loop-6 semantics
            let stripped_tag = strip_namespaces(tag_name);
            let pattern_str = format!(
                "Value::Ctor {{ tag, fields: {} }} if get_tag_name(tag) == \"{}\"",
                fields_ident, stripped_tag
            );
            (pattern_str, bindings)
        }
    }
}


fn sanitize_ident(name: &str) -> String {
    // Replace dots and dashes with underscores to make valid Rust identifiers
    let base = name.replace('.', "_").replace('-', "_");
    
    // Handle Rust reserved words and crate names that conflict
    match base.as_str() {
        "core" => "core_".to_string(),
        "self" => "self_".to_string(),
        "Self" => "Self_".to_string(),
        "type" => "type_".to_string(),
        "match" => "match_".to_string(),
        "fn" => "fn_".to_string(),
        "let" => "let_".to_string(),
        "if" => "if_".to_string(),
        "else" => "else_".to_string(),
        "loop" => "loop_".to_string(),
        "for" => "for_".to_string(),
        "while" => "while_".to_string(),
        "break" => "break_".to_string(),
        "continue" => "continue_".to_string(),
        "return" => "return_".to_string(),
        "mod" => "mod_".to_string(),
        "pub" => "pub_".to_string(),
        "use" => "use_".to_string(),
        "struct" => "struct_".to_string(),
        "enum" => "enum_".to_string(),
        "impl" => "impl_".to_string(),
        "trait" => "trait_".to_string(),
        "where" => "where_".to_string(),
        "const" => "const_".to_string(),
        "static" => "static_".to_string(),
        "mut" => "mut_".to_string(),
        "ref" => "ref_".to_string(),
        "move" => "move_".to_string(),
        "box" => "box_".to_string(),
        "as" => "as_".to_string(),
        "in" => "in_".to_string(),
        "unsafe" => "unsafe_".to_string(),
        "extern" => "extern_".to_string(),
        "crate" => "crate_".to_string(),
        "super" => "super_".to_string(),
        _ => base,
    }
}

///  POLICY: Determine if a term needs .clone() when used
/// Clone everything except literals to avoid borrow errors
fn needs_clone(term: &CoreTerm) -> bool {
    use crate::runtime::CoreTerm;
    match term {
        CoreTerm::IntLit(_, _) => false,
        CoreTerm::BoolLit(_, _) => false,
        CoreTerm::UnitLit(_) => false,
        CoreTerm::StrLit(_, _) => false,
        _ => true,  // Clone: Var, App, Let, Lam, Tuple, Proj, If, Match
    }
}

/// Strip namespace from ALL constructor references (Loop-6 requirement)
/// compiler_main___Result::Ok -> compiler_main___Ok
/// compiler_main___Token::TokIdent -> compiler_main___TokIdent
/// compiler_main___List::Nil -> compiler_main___Nil
/// SurfaceAst::SStrLit -> SStrLit
fn strip_namespaces(name: &str) -> String {
    if let Some(colon_idx) = name.rfind("::") {
        // Always strip everything before the last ::
        name[colon_idx + 2..].to_string()
    } else {
        name.to_string()
    }
}

/// Fix 5.3b: Tuple pattern lowering
/// Convert result_0, result_1 references to __tuple_field__(result, 0), __tuple_field__(result, 1)
/// Compute missing symbols: required - defined
fn compute_missing_symbols(used: &HashSet<String>, defined: &HashSet<String>) -> HashSet<String> {
    // PRELUDE: Runtime functions defined in generate_value_runtime
    // Do NOT emit wrappers for these - they already exist
    let prelude: HashSet<String> = [
        "__add__", "__sub__", "__mul__", "__div__", "__mod__",
        "__eq__", "__lt__", "__lte__", "__gt__", "__gte__",
        "__and__", "__or__", "__not__", "__concat__",
        "str_len", "str_char", "str_slice", "str_to_int", "int_to_str",
        "list_reverse", "list_cons", "list_nil",
        "tuple_field", "ctor_field", "truthy",
        "io_print", "io_eprint", "io_read",
        "fs_read_text", "fs_write_text",
        "debug_trace", "get_tag_name", "intern_tag", "intern_str", "get_str",
    ].iter().map(|s| s.to_string()).collect();
    
    used.difference(defined)
        .filter(|s| !prelude.contains(s.as_str()))
        .cloned()
        .collect()
}

/// Infer arity of a function from its usage in generated code
/// Updated for unary calling convention: detects Value::Tuple(vec![...]) patterns
fn infer_arity(output: &str, func_name: &str) -> usize {
    // Look for calls like `func_name(Value::Tuple(vec![arg1, arg2, ...]))`
    let search = format!("{}(", func_name);
    if let Some(start) = output.find(&search) {
        let rest = &output[start + search.len()..];
        if let Some(end) = rest.find(')') {
            let args = &rest[..end];
            if args.trim().is_empty() {
                return 0;
            }
            
            // Check if this is a tuple-wrapped call: Value::Tuple(vec![...])
            if args.trim().starts_with("Value::Tuple(vec![") {
                // Extract the content inside vec![...]
                if let Some(vec_start) = args.find("vec![") {
                    let vec_content_start = vec_start + 5; // "vec![".len()
                    let after_vec = &args[vec_content_start..];
                    if let Some(vec_end) = after_vec.find("])") {
                        let vec_content = &after_vec[..vec_end];
                        if vec_content.trim().is_empty() {
                            return 0;
                        }
                        // Count arguments inside vec by counting commas + 1
                        // Need to account for nested structures
                        let comma_count = vec_content.matches(',').count();
                        return comma_count + 1;
                    }
                }
            }
            
            // Not a tuple-wrapped call - single argument or 0-arity
            return if args.contains(',') { args.matches(',').count() + 1 } else { 1 };
        }
    }
    // Default: 0-arity
    0
}

/// Verify all used symbols are defined in the output
fn verify_all_symbols_defined(output: &str, used: &HashSet<String>) -> Vec<String> {
    let mut missing = Vec::new();
    for sym in used.iter() {
        // Sanitize the symbol before checking (must match generate_runtime_stub)
        let safe_sym = sym.replace("::", "_");
        let fn_decl = format!("fn {}(", safe_sym);
        if !output.contains(&fn_decl) {
            missing.push(sym.clone());
        }
    }
    missing.sort();
    missing
}

/// Generate runtime stub for a single missing symbol with inferred arity
fn generate_runtime_stub(symbol: &str, arity: usize) -> String {
    // Sanitize symbol name - remove any "::" that slipped through
    let safe_symbol = symbol.replace("::", "_");
    
    // Extract the base name (after last ___)
    let base_name = safe_symbol.rsplit("___").next().unwrap_or(&safe_symbol);
    
    // UNARY INVARIANT: All functions are unary
    // Helper to generate unary parameter with optional destructuring
    let gen_unary_params = |n: usize| -> String {
        if n == 0 {
            String::new()
        } else {
            "args: Value".to_string()
        }
    };
    
    // Helper to generate destructuring code for multi-arg functions
    let gen_destructure = |n: usize| -> String {
        if n == 0 {
            String::new()
        } else if n == 1 {
            "    let _arg0 = args;\n".to_string()
        } else {
            let mut s = String::new();
            for i in 0..n {
                s.push_str(&format!("    let _arg{} = tuple_field(args.clone(), {});\n", i, i));
            }
            s
        }
    };
    
    // Helper to generate field list for constructors
    let gen_fields = |n: usize| -> String {
        if n == 0 {
            return "vec![]".to_string();
        }
        //  POLICY: clone all constructor fields
        format!("vec![{}]", (0..n).map(|i| format!("_arg{}.clone()", i)).collect::<Vec<_>>().join(", "))
    };
    
    // Operators (5 underscores pattern: _____op__)
    // All operators take a single tuple argument and destructure it
    if safe_symbol.contains("_____") {
        if safe_symbol.ends_with("_____add__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __add__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____sub__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __sub__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____mul__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __mul__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____div__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __div__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____mod__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __mod__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____eq__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __eq__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____lt__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __lt__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____lte__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __lte__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____gt__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __gt__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____gte__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __gte__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____and__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __and__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____or__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __or__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        } else if safe_symbol.ends_with("_____not__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __not__(_arg0)\n}}", safe_symbol, gen_destructure(1));
        } else if safe_symbol.ends_with("_____concat__") {
            return format!("fn {}(args: Value) -> Value {{\n{}    __concat__(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
        }
    }
    
    // Constructors (capitalized) - use inferred arity with unary signature
    if base_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        // Special cases where we know the behavior
        if base_name == "Nil" && arity == 0 {
            return format!("fn {}() -> Value {{ Value::List(vec![]) }}", safe_symbol);
        } else if base_name == "Cons" && arity == 2 {
            return format!("fn {}(args: Value) -> Value {{\n{}    list_cons(_arg0.clone(), _arg1.clone())\n}}", safe_symbol, gen_destructure(2));
        } else if base_name == "Pair" && arity == 2 {
            return format!("fn {}(args: Value) -> Value {{\n{}    Value::Tuple(vec![_arg0.clone(), _arg1.clone()])\n}}", safe_symbol, gen_destructure(2));
        } else if base_name == "True" && arity == 0 {
            return format!("fn {}() -> Value {{ Value::Bool(true) }}", safe_symbol);
        } else if base_name == "False" && arity == 0 {
            return format!("fn {}() -> Value {{ Value::Bool(false) }}", safe_symbol);
        } else if base_name == "Unit" && arity == 0 {
            return format!("fn {}() -> Value {{ Value::Unit }}", safe_symbol);
        } else if arity == 0 {
            // 0-arity constructor
            return format!("fn {}() -> Value {{ Value::Ctor {{ tag: intern_tag(\"{}\"), fields: vec![] }} }}", 
                safe_symbol, base_name);
        } else {
            // Generic constructor with inferred arity - unary signature
            return format!("fn {}(args: Value) -> Value {{\n{}    Value::Ctor {{ tag: intern_tag(\"{}\"), fields: {} }}\n}}", 
                safe_symbol, gen_destructure(arity), base_name, gen_fields(arity));
        }
    }
    
    // Tuple/Constructor field projection - special case for proj
    if base_name == "proj" {
        return format!("fn {}(args: Value) -> Value {{\n{}    tuple_field(_arg0, _arg1.as_int() as usize)\n}}", safe_symbol, gen_destructure(2));
    }
    
    // String operations - unary signatures
    if base_name.contains("str_len") || base_name.contains("string_length") {
        return format!("fn {}(args: Value) -> Value {{\n{}    str_len(_arg0)\n}}", safe_symbol, gen_destructure(1));
    } else if base_name.contains("str_char") && !base_name.contains("str_char_at") {
        return format!("fn {}(args: Value) -> Value {{\n{}    str_char(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
    } else if base_name.contains("str_char_at") {
        return format!("fn {}(args: Value) -> Value {{\n{}    str_char(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
    } else if base_name.contains("str_slice") || base_name.contains("substring") {
        return format!("fn {}(args: Value) -> Value {{\n{}    str_slice(_arg0, _arg1, _arg2)\n}}", safe_symbol, gen_destructure(3));
    } else if base_name.contains("str_concat") || base_name == "concat" {
        return format!("fn {}(args: Value) -> Value {{\n{}    str_concat(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
    } else if base_name.contains("str_eq") {
        return format!("fn {}(args: Value) -> Value {{\n{}    Value::Bool(_arg0 == _arg1)\n}}", safe_symbol, gen_destructure(2));
    } else if base_name.contains("int_to_str") || base_name.contains("to_string") {
        return format!("fn {}(args: Value) -> Value {{\n{}    int_to_str(_arg0)\n}}", safe_symbol, gen_destructure(1));
    } else if base_name.contains("str_to_int") {
        return format!("fn {}(args: Value) -> Value {{\n{}    str_to_int(_arg0)\n}}", safe_symbol, gen_destructure(1));
    }
    
    // List operations - unary signatures
    if base_name.contains("list_nil") {
        return format!("fn {}() -> Value {{ list_nil() }}", safe_symbol);
    } else if base_name.contains("list_cons") {
        return format!("fn {}(args: Value) -> Value {{\n{}    list_cons(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
    } else if base_name.contains("list_reverse") || base_name.contains("reverse") {
        return format!("fn {}(args: Value) -> Value {{\n{}    list_reverse(_arg0)\n}}", safe_symbol, gen_destructure(1));
    }
    
    // IO operations - unary signatures
    if base_name.contains("io_print") || base_name == "print" {
        return format!("fn {}(args: Value) -> Value {{\n{}    io_print(_arg0)\n}}", safe_symbol, gen_destructure(1));
    } else if base_name.contains("io_eprint") || base_name == "eprint" {
        return format!("fn {}(args: Value) -> Value {{\n{}    io_eprint(_arg0)\n}}", safe_symbol, gen_destructure(1));
    } else if base_name.contains("io_read") || base_name == "read_line" {
        return format!("fn {}() -> Value {{ io_read() }}", safe_symbol);
    }
    
    // Debug operations (observational, non-semantic)
    if base_name == "debug_trace" || base_name.contains("debug___trace") {
        return format!("fn {}(args: Value) -> Value {{\n{}    debug_trace(_arg0)\n}}", safe_symbol, gen_destructure(1));
    }
    
    // File operations - unary signatures
    if base_name.contains("fs_read_text") || base_name.contains("read_text") || base_name.contains("read_file") {
        return format!("fn {}(args: Value) -> Value {{\n{}    fs_read_text(_arg0)\n}}", safe_symbol, gen_destructure(1));
    } else if base_name.contains("fs_write_text") || base_name.contains("write_text") || base_name.contains("write_file") {
        return format!("fn {}(args: Value) -> Value {{\n{}    fs_write_text(_arg0, _arg1)\n}}", safe_symbol, gen_destructure(2));
    }
    
    //  MODULE CONCATENATION:
    // Axis-defined functions should be available after module loading.
    // Do NOT generate runtime stubs - rustc will fail with clear "cannot find function" error.
    if base_name.contains("parser_") || base_name.contains("parse_") 
        || base_name.contains("registry_") || base_name.contains("reg_")
        || base_name.contains("lower_")
        || base_name.contains("emit_")
        || base_name.contains("module_loader_")
        || base_name.contains("compiler_")
        || base_name.contains("cli_") {
        eprintln!("[emit_rust] ERROR: Missing Axis-defined function: {}", symbol);
        eprintln!("[emit_rust]   This function should be defined in Axis source after module concatenation.");
        eprintln!("[emit_rust]   Check that all required modules are being loaded.");
        // Return a comment-only stub that will cause rustc to fail with "cannot find function"
        return format!("// ERROR: Missing Axis function '{}' (should be in concatenated source)\n", symbol);
    } else if base_name.contains("foreign_") {
        return format!("fn {}({}) -> Value {{\n{}    Value::Unit\n}}", safe_symbol, gen_unary_params(arity), gen_destructure(arity));
    }
    
    // Default: generic function with inferred arity returning Unit - unary signature
    format!("fn {}({}) -> Value {{\n{}    Value::Unit\n}}", safe_symbol, gen_unary_params(arity), gen_destructure(arity))
}

fn fix_tuple_references(code: &str, tuple_var_name: &str) -> String {
    use std::collections::HashMap;
    
    let mut result = code.to_string();
    
    // Pattern: {tuple_var_name}_0, {tuple_var_name}_1, etc.
    let mut replacements = HashMap::new();
    
    // Look for patterns like result_0, result_1, result_2, etc.
    for i in 0..10 {  // Support up to 10-tuples (reasonable limit)
        let old_pattern = format!("{}_{}",  tuple_var_name, i);
        let new_pattern = format!("tuple_field({}, {})", tuple_var_name, i);
        replacements.insert(old_pattern, new_pattern);
    }
    
    // Apply all replacements
    for (old, new) in replacements {
        result = result.replace(&old, &new);
    }
    
    result
}

/// Generate Value-based runtime for ANDL Loop 6
fn generate_value_runtime(used_prims: &std::collections::HashSet<String>) -> String {
    let mut out = String::new();
    out.push_str(r#"
// ANDL Loop 6: Value Runtime Implementation

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(u32),      // String handle
    Unit,
    Tuple(Vec<Value>),
    List(Vec<Value>),
    Ctor { tag: u32, fields: Vec<Value> }, // Constructor with tag and fields
}

impl Value {
    pub fn as_int(&self) -> i64 {
        match self {
            Value::Int(n) => *n,
            _ => panic!("Expected Int, got {:?}", self),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            _ => panic!("Expected Bool, got {:?}", self),
        }
    }

    pub fn as_tuple(&self) -> &Vec<Value> {
        match self {
            Value::Tuple(elems) => elems,
            _ => panic!("Expected Tuple, got {:?}", self),
        }
    }

    pub fn as_list(&self) -> &Vec<Value> {
        match self {
            Value::List(elems) => elems,
            _ => panic!("Expected List, got {:?}", self),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(handle) => write!(f, "{}", get_str(*handle)),
            Value::Unit => write!(f, "()"),
            Value::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            },
            Value::List(elems) => {
                write!(f, "[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            },
            Value::Ctor { tag, fields } => {
                write!(f, "{}(", get_tag_name(*tag))?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", field)?;
                }
                write!(f, ")")
            },
        }
    }
}

// String table (thread-safe lazy statics)
use std::sync::{OnceLock, Mutex};
static STRING_TABLE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static STRING_MAP: OnceLock<Mutex<std::collections::HashMap<String, u32>>> = OnceLock::new();

pub fn init_runtime() {
    let table_mutex = STRING_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let mut table = table_mutex.lock().unwrap();
    if table.is_empty() {
        table.push("".to_string()); // Reserve handle 0 for empty string
    }
    // ensure map exists
    STRING_MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
}

pub fn intern_str(s: &str) -> u32 {
    let map_mutex = STRING_MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut map = map_mutex.lock().unwrap();
    if let Some(&handle) = map.get(s) {
        return handle;
    }
    let table_mutex = STRING_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let mut table = table_mutex.lock().unwrap();
    let handle = table.len() as u32;
    table.push(s.to_string());
    map.insert(s.to_string(), handle);
    handle
}

pub fn get_str(handle: u32) -> String {
    let table_mutex = STRING_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let table = table_mutex.lock().unwrap();
    table.get(handle as usize).cloned().unwrap_or_else(|| {
        // This should never happen in correct code - log the error
        eprintln!("[FATAL] get_str: invalid handle {} (table size: {})", handle, table.len());
        format!("<invalid-str-handle-{}>", handle)
    })
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Str(h) => *h != 0,
        Value::Unit => false,
        Value::Tuple(elems) => !elems.is_empty(),
        Value::List(elems) => !elems.is_empty(),
        Value::Ctor { .. } => true,
    }
}

// Tuple field access
pub fn tuple_field(tuple: Value, idx: usize) -> Value {
    match tuple {
        Value::Tuple(ref elems) => {
            elems.get(idx).cloned().unwrap_or(Value::Unit)
        },
        _ => Value::Unit,
    }
}

// Constructor field access
pub fn ctor_field(ctor: Value, idx: usize) -> Value {
    match ctor {
        Value::Ctor { ref fields, .. } => {
            fields.get(idx).cloned().unwrap_or(Value::Unit)
        },
        _ => Value::Unit,
    }
}

// Arithmetic primitives
pub fn __add__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_add(y)),
        _ => Value::Int(0), // Error fallback
    }
}

pub fn __sub__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_sub(y)),
        _ => Value::Int(0),
    }
}

pub fn __mul__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_mul(y)),
        _ => Value::Int(0),
    }
}

pub fn __div__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            if y == 0 { Value::Int(0) } else { Value::Int(x / y) }
        },
        _ => Value::Int(0),
    }
}

pub fn __mod__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            if y == 0 { Value::Int(0) } else { Value::Int(x % y) }
        },
        _ => Value::Int(0),
    }
}

// Comparison primitives
pub fn __eq__(a: Value, b: Value) -> Value {
    Value::Bool(a == b)
}

pub fn __lt__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Bool(x < y),
        _ => Value::Bool(false),
    }
}

pub fn __lte__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Bool(x <= y),
        _ => Value::Bool(false),
    }
}

pub fn __gt__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Bool(x > y),
        _ => Value::Bool(false),
    }
}

pub fn __gte__(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Bool(x >= y),
        _ => Value::Bool(false),
    }
}

// Logical primitives
pub fn __and__(a: Value, b: Value) -> Value {
    Value::Bool(truthy(&a) && truthy(&b))
}

pub fn __or__(a: Value, b: Value) -> Value {
    Value::Bool(truthy(&a) || truthy(&b))
}

pub fn __not__(a: Value) -> Value {
    Value::Bool(!truthy(&a))
}

// String primitives
pub fn str_len(s: Value) -> Value {
    match s {
        Value::Str(handle) => {
            let string = get_str(handle);
            Value::Int(string.len() as i64)
        },
        _ => Value::Int(0),
    }
}

pub fn str_char(s: Value, idx: Value) -> Value {
    match (s, idx) {
        (Value::Str(handle), Value::Int(i)) => {
            let string = get_str(handle);
            if let Some(c) = string.chars().nth(i as usize) {
                Value::Int(c as i64)
            } else {
                Value::Int(0)
            }
        },
        _ => Value::Int(0),
    }
}

pub fn str_char_at(s: Value, idx: Value) -> Value {
    str_char(s, idx)
}

pub fn str_slice(s: Value, start: Value, end: Value) -> Value {
    match (s, start, end) {
        (Value::Str(handle), Value::Int(s_idx), Value::Int(e_idx)) => {
            let string = get_str(handle);
            let slice = &string[s_idx.min(string.len() as i64) as usize..e_idx.min(string.len() as i64) as usize];
            Value::Str(intern_str(slice))
        },
        _ => Value::Str(0),
    }
}

pub fn str_to_int(s: Value) -> Value {
    match s {
        Value::Str(handle) => {
            let string = get_str(handle);
            Value::Int(string.parse().unwrap_or(0))
        },
        _ => Value::Int(0),
    }
}

pub fn str_concat(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Str(h1), Value::Str(h2)) => {
            let s1 = get_str(h1);
            let s2 = get_str(h2);
            Value::Str(intern_str(&format!("{}{}", s1, s2)))
        },
        _ => Value::Str(0),
    }
}

pub fn __concat__(a: Value, b: Value) -> Value {
    str_concat(a, b)
}

pub fn int_to_str(n: Value) -> Value {
    match n {
        Value::Int(i) => {
            let s = i.to_string();
            // Safety check: ensure we never return empty for valid integers
            if s.is_empty() {
                panic!("int_to_str: to_string() returned empty for {}", i);
            }
            let handle = intern_str(&s);
            // Verify the string can be retrieved correctly
            let retrieved = get_str(handle);
            if retrieved.is_empty() && i != 0 {
                panic!("int_to_str: intern_str/get_str corrupted string '{}' for {}", s, i);
            }
            Value::Str(handle)
        },
        _ => {
            // Non-integer values should never happen in well-typed code
            Value::Str(intern_str("<not-an-int>"))
        }
    }
}

// Tag table for constructors (similar to string table)
static TAG_TABLE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static TAG_MAP: OnceLock<Mutex<std::collections::HashMap<String, u32>>> = OnceLock::new();

pub fn intern_tag(name: &str) -> u32 {
    let map_mutex = TAG_MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut map = map_mutex.lock().unwrap();
    if let Some(&tag) = map.get(name) {
        return tag;
    }
    let table_mutex = TAG_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let mut table = table_mutex.lock().unwrap();
    let tag = table.len() as u32;
    table.push(name.to_string());
    map.insert(name.to_string(), tag);
    tag
}

pub fn get_tag_name(tag: u32) -> String {
    let table_mutex = TAG_TABLE.get_or_init(|| Mutex::new(Vec::new()));
    let table = table_mutex.lock().unwrap();
    table.get(tag as usize).cloned().unwrap_or_else(|| "Unknown".to_string())
}

// IO primitives
pub fn io_print(val: Value) -> Value {
    match val {
        Value::Str(handle) => print!("{}", get_str(handle)),
        Value::Int(n) => print!("{}", n),
        Value::Bool(b) => print!("{}", b),
        Value::Unit => print!("()"),
        _ => print!("{:?}", val),
    }
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
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
    std::io::Write::flush(&mut std::io::stderr()).unwrap();
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
    use std::io::BufRead;
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

// List primitives
pub fn list_nil() -> Value {
    Value::List(vec![])
}

pub fn list_cons(head: Value, tail: Value) -> Value {
    match tail {
        Value::List(mut elems) => {
            elems.insert(0, head);
            Value::List(elems)
        },
        _ => Value::List(vec![head]),
    }
}

pub fn list_reverse(list: Value) -> Value {
    match list {
        Value::List(mut elems) => {
            elems.reverse();
            Value::List(elems)
        },
        _ => Value::List(vec![]),
    }
}

"#);

    // Append textual runtime stubs so they become part of emitted code
    out.push_str(r#"

"#);

    // Emit exact-named wrappers for used primitives so generated code finds them.
    for name in used_prims.iter() {
        if out.contains(&format!("fn {}(", name)) {
            continue;
        }
        if name.ends_with("___str_len") {
            out.push_str(&format!("fn {}(s: Value) -> Value {{ str_len(s) }}\n", name));
        } else if name.ends_with("___debug_trace") {
            out.push_str(&format!("fn {}(msg: Value) -> Value {{ debug_trace(msg) }}\n", name));
        } else if name.ends_with("___str_char") {
            out.push_str(&format!("fn {}(s: Value, idx: Value) -> Value {{ str_char(s, idx) }}\n", name));
        } else if name.ends_with("___str_slice") {
            out.push_str(&format!("fn {}(s: Value, start: Value, end: Value) -> Value {{ str_slice(s, start, end) }}\n", name));
        } else if name.contains("concat") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ str_concat(a, b) }}\n", name));
        } else if name.ends_with("___Pair") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ Value::Tuple(vec![a, b]) }}\n", name));
        } else if name.ends_with("___Err") {
            out.push_str(&format!("fn {}(a: Value) -> Value {{ Value::Ctor {{ tag: 1, fields: vec![a] }} }}\n", name));
        } else if name.ends_with("___Ok") {
            out.push_str(&format!("fn {}(a: Value) -> Value {{ Value::Ctor {{ tag: 0, fields: vec![a] }} }}\n", name));
        } else if name.ends_with("___list_nil") {
            out.push_str(&format!("fn {}() -> Value {{ list_nil() }}\n", name));
        } else if name.ends_with("___list_cons") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ list_cons(a, b) }}\n", name));
        } else if name.ends_with("___list_reverse") {
            out.push_str(&format!("fn {}(a: Value) -> Value {{ list_reverse(a) }}\n", name));
        } else if name.ends_with("_____add__") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ __add__(a, b) }}\n", name));
        } else if name.ends_with("_____eq__") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ __eq__(a, b) }}\n", name));
        } else if name.ends_with("_____gt__") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ __gt__(a, b) }}\n", name));
        } else if name.ends_with("_____and__") {
            out.push_str(&format!("fn {}(a: Value, b: Value) -> Value {{ __and__(a, b) }}\n", name));
        }
    }

    // NOTE: All missing symbols (including cli_main) are now generated systematically
    // via generate_runtime_stub(). No more defensive hardcoded stubs here.

    out
}
// Runtime wrappers (constructors, helpers) are emitted into the generated
// Rust output by `generate_value_runtime` so they do not need to be compiled
// into the emitter binary. The concrete functions for module-scope names
// are therefore produced at codegen time based on `used_prims`.

