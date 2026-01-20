// Core IR emission runtime — linked from generated Axis code
// This module provides `axis_emit_core_bundle_to_file` as a runtime function.

use crate::core_ir::{CoreTerm, Pattern};
use crate::runtime::value::Value;
use crate::runtime::value::{get_str, get_tag_name};
use std::fs;
use std::rc::Rc;

/// Runtime entry point called from generated Axis code
/// Signature: axis_emit_core_bundle_to_file(bundle: CoreBundle, path: Str) -> Result[Unit]
/// UNARY CONTRACT: Accepts Value::Tuple containing [bundle, path]
pub fn axis_emit_core_bundle_to_file(args: Value) -> Value {
    // Unpack arguments from tuple
    let (bundle, path) = match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            (elems[0].clone(), elems[1].clone())
        },
        _ => {
            let err_tag = crate::runtime::value::intern_tag("Err");
            let msg = crate::runtime::value::intern_str("axis_emit_core_bundle_to_file: expected tuple with 2 elements");
            return Value::Ctor { tag: err_tag, fields: vec![Value::Str(msg)] };
        }
    };
    match emit_core_bundle_impl(bundle, path) {
        Ok(_) => {
            // Ok(Unit) constructor
            let ok_tag = crate::runtime::value::intern_tag("Ok");
            Value::Ctor {
                tag: ok_tag,
                fields: vec![Value::Unit],
            }
        }
        Err(e) => {
            // Err(msg) constructor
            let err_tag = crate::runtime::value::intern_tag("Err");
            let msg_str = crate::runtime::value::intern_str(&e);
            Value::Ctor {
                tag: err_tag,
                fields: vec![Value::Str(msg_str)],
            }
        }
    }
}

fn emit_core_bundle_impl(bundle: Value, path: Value) -> Result<(), String> {
    // Extract path string
    let path_str = match path {
        Value::Str(handle) => get_str(handle).to_string(),
        _ => return Err(format!("Expected Str for path, got {:?}", path)),
    };

    // Decode CoreBundle: CoreBundle(StringTable, List[CoreTerm])
    let (string_table_val, core_terms_val) = match &bundle {
        Value::Ctor { tag, fields } if get_tag_name(*tag) == "CoreBundle" && fields.len() == 2 => {
            (&fields[0], &fields[1])
        }
        _ => return Err(format!("Expected CoreBundle constructor, got {:?}", bundle)),
    };

    // Decode StringTable: StringTable(List[Str], Int)
    let strings = match string_table_val {
        Value::Ctor { tag, fields } if get_tag_name(*tag) == "StringTable" && fields.len() == 2 => {
            match &fields[0] {
                Value::List(strs) => {
                    let mut result = Vec::new();
                    for s_val in strs {
                        match s_val {
                            Value::Str(handle) => result.push(get_str(*handle).to_string()),
                            _ => return Err(format!("Expected Str in string table, got {:?}", s_val)),
                        }
                    }
                    result
                }
                _ => return Err(format!("Expected List[Str] in StringTable, got {:?}", fields[0])),
            }
        }
        _ => return Err(format!("Expected StringTable constructor, got {:?}", string_table_val)),
    };

    // Decode List[CoreTerm]
    let core_terms_list = match core_terms_val {
        Value::List(terms) => terms,
        _ => return Err(format!("Expected List[CoreTerm], got {:?}", core_terms_val)),
    };

    // Convert Value CoreTerms to Rust CoreTerm
    let mut rust_terms = Vec::new();
    for term_val in core_terms_list {
        rust_terms.push(value_to_core_term(term_val)?);
    }

    // For now, emit a single root term (first term in list, or Unit if empty)
    let root_term = if rust_terms.is_empty() {
        CoreTerm::UnitLit(None)
    } else if rust_terms.len() == 1 {
        rust_terms.into_iter().next().unwrap()
    } else {
        // Multiple terms - wrap in a Let chain or Tuple
        // For simplicity, emit the last term (typical pattern in Axis)
        rust_terms.into_iter().last().unwrap()
    };

    // Serialize to Cap'n Proto using compiler's approach
    let bytes = create_core_bundle(&root_term, "main", &strings)?;

    // Write to file
    fs::write(&path_str, bytes)
        .map_err(|e| format!("Failed to write Core bundle to {}: {}", path_str, e))?;

    Ok(())
}

/// Convert runtime Value representation of CoreTerm to Rust CoreTerm
fn value_to_core_term(val: &Value) -> Result<CoreTerm, String> {
    match val {
        Value::Ctor { tag, fields } => {
            let tag_name = get_tag_name(*tag);
            
            match tag_name.as_str() {
                "CIntLit" if fields.len() == 1 => {
                    Ok(CoreTerm::IntLit(fields[0].as_int(), None))
                }
                "CBoolLit" if fields.len() == 1 => {
                    Ok(CoreTerm::BoolLit(fields[0].as_bool(), None))
                }
                "CUnitLit" => {
                    Ok(CoreTerm::UnitLit(None))
                }
                "CStrLit" if fields.len() == 1 => {
                    match &fields[0] {
                        Value::Str(handle) => Ok(CoreTerm::StrLit(get_str(*handle).to_string(), None)),
                        _ => Err(format!("Expected Str in CStrLit, got {:?}", fields[0])),
                    }
                }
                "CVar" if fields.len() == 1 => {
                    match &fields[0] {
                        Value::Str(handle) => Ok(CoreTerm::Var(get_str(*handle).to_string(), None)),
                        _ => Err(format!("Expected Str in CVar, got {:?}", fields[0])),
                    }
                }
                "CLam" if fields.len() == 2 => {
                    let param = match &fields[0] {
                        Value::Str(handle) => get_str(*handle).to_string(),
                        _ => return Err(format!("Expected Str param in CLam, got {:?}", fields[0])),
                    };
                    let body = value_to_core_term(&fields[1])?;
                    Ok(CoreTerm::Lam(param, Rc::new(body), None))
                }
                "CApp" if fields.len() == 2 => {
                    let func = value_to_core_term(&fields[0])?;
                    let arg = value_to_core_term(&fields[1])?;
                    Ok(CoreTerm::App(Rc::new(func), Rc::new(arg), None))
                }
                "CTuple" if fields.len() == 1 => {
                    match &fields[0] {
                        Value::List(elems) => {
                            let mut terms = Vec::new();
                            for elem in elems {
                                terms.push(value_to_core_term(elem)?);
                            }
                            Ok(CoreTerm::Tuple(terms, None))
                        }
                        _ => Err(format!("Expected List in CTuple, got {:?}", fields[0])),
                    }
                }
                "CProj" if fields.len() == 2 => {
                    let expr = value_to_core_term(&fields[0])?;
                    let idx = fields[1].as_int() as usize;
                    Ok(CoreTerm::Proj(Rc::new(expr), idx, None))
                }
                "CLet" if fields.len() == 3 => {
                    let name = match &fields[0] {
                        Value::Str(handle) => get_str(*handle).to_string(),
                        _ => return Err(format!("Expected Str name in CLet, got {:?}", fields[0])),
                    };
                    let value = value_to_core_term(&fields[1])?;
                    let body = value_to_core_term(&fields[2])?;
                    Ok(CoreTerm::Let(name, Rc::new(value), Rc::new(body), None))
                }
                "CIf" if fields.len() == 3 => {
                    let cond = value_to_core_term(&fields[0])?;
                    let then_br = value_to_core_term(&fields[1])?;
                    let else_br = value_to_core_term(&fields[2])?;
                    Ok(CoreTerm::If(Rc::new(cond), Rc::new(then_br), Rc::new(else_br), None))
                }
                "CCtor" if fields.len() == 2 => {
                    let name = match &fields[0] {
                        Value::Str(handle) => get_str(*handle).to_string(),
                        _ => return Err(format!("Expected Str name in CCtor, got {:?}", fields[0])),
                    };
                    let field_list = match &fields[1] {
                        Value::List(fs) => {
                            let mut terms = Vec::new();
                            for f in fs {
                                terms.push(value_to_core_term(f)?);
                            }
                            terms
                        }
                        _ => return Err(format!("Expected List in CCtor fields, got {:?}", fields[1])),
                    };
                    Ok(CoreTerm::Ctor(name, field_list, None))
                }
                "CMatch" if fields.len() == 2 => {
                    let scrutinee = value_to_core_term(&fields[0])?;
                    let arms_list = match &fields[1] {
                        Value::List(arms) => {
                            let mut result = Vec::new();
                            for arm in arms {
                                // Each arm is a MatchCase constructor: MatchCase(pattern, body)
                                match arm {
                                    Value::Ctor { tag: arm_tag, fields: arm_fields } 
                                        if get_tag_name(*arm_tag) == "MatchCase" && arm_fields.len() == 2 => {
                                        let pattern = value_to_pattern(&arm_fields[0])?;
                                        let body = value_to_core_term(&arm_fields[1])?;
                                        result.push((pattern, body));
                                    }
                                    _ => return Err(format!("Expected MatchCase in CMatch arms, got {:?}", arm)),
                                }
                            }
                            result
                        }
                        _ => return Err(format!("Expected List in CMatch arms, got {:?}", fields[1])),
                    };
                    Ok(CoreTerm::Match(Rc::new(scrutinee), arms_list, None))
                }
                _ => Err(format!("Unknown CoreTerm constructor: {}", tag_name)),
            }
        }
        _ => Err(format!("Expected Ctor for CoreTerm, got {:?}", val)),
    }
}

/// Convert runtime Value representation of Pattern to Rust Pattern
fn value_to_pattern(val: &Value) -> Result<Pattern, String> {
    match val {
        Value::Ctor { tag, fields } => {
            let tag_name = get_tag_name(*tag);
            
            match tag_name.as_str() {
                "PInt" if fields.len() == 1 => {
                    Ok(Pattern::PInt(fields[0].as_int()))
                }
                "PBool" if fields.len() == 1 => {
                    Ok(Pattern::PBool(fields[0].as_bool()))
                }
                "PUnit" => {
                    Ok(Pattern::PUnit)
                }
                "PVar" if fields.len() == 1 => {
                    match &fields[0] {
                        Value::Str(handle) => Ok(Pattern::PVar(get_str(*handle).to_string())),
                        _ => Err(format!("Expected Str in PVar, got {:?}", fields[0])),
                    }
                }
                "PTuple" if fields.len() == 1 => {
                    match &fields[0] {
                        Value::List(pats) => {
                            let mut result = Vec::new();
                            for p in pats {
                                result.push(value_to_pattern(p)?);
                            }
                            Ok(Pattern::PTuple(result))
                        }
                        _ => Err(format!("Expected List in PTuple, got {:?}", fields[0])),
                    }
                }
                "PEnum" if fields.len() == 2 => {
                    let name = match &fields[0] {
                        Value::Str(handle) => get_str(*handle).to_string(),
                        _ => return Err(format!("Expected Str name in PEnum, got {:?}", fields[0])),
                    };
                    let pats = match &fields[1] {
                        Value::List(ps) => {
                            let mut result = Vec::new();
                            for p in ps {
                                result.push(value_to_pattern(p)?);
                            }
                            result
                        }
                        _ => return Err(format!("Expected List in PEnum patterns, got {:?}", fields[1])),
                    };
                    Ok(Pattern::PEnum(name, pats))
                }
                _ => Err(format!("Unknown Pattern constructor: {}", tag_name)),
            }
        }
        _ => Err(format!("Expected Ctor for Pattern, got {:?}", val)),
    }
}

/// Create a core bundle binary (based on compiler's create_core_bundle)
fn create_core_bundle(term: &CoreTerm, entrypoint_name: &str, strings: &[String]) -> Result<Vec<u8>, String> {
    use capnp::message::Builder;
    use capnp::serialize;
    
    let mut message = Builder::new_default();
    
    {
        let mut bundle = message.init_root::<crate::axis_core_ir_0_1_capnp::core_bundle::Builder>();
        bundle.set_version("0.1");
        bundle.set_entrypoint_name(entrypoint_name);
        bundle.set_entrypoint_id(0);
        
        // Set string table
        let mut string_table_builder = bundle.reborrow().init_string_table(strings.len() as u32);
        for (i, s) in strings.iter().enumerate() {
            string_table_builder.set(i as u32, s);
        }
        
        let core_term_builder = bundle.init_core_term();
        serialize_core_term(term, core_term_builder);
    }
    
    let mut buf = Vec::new();
    serialize::write_message(&mut buf, &message)
        .map_err(|e| format!("Failed to serialize Cap'n Proto message: {}", e))?;
    Ok(buf)
}

/// Serialize CoreTerm to Cap'n Proto (reuse from core_ir.rs serialization helpers)
fn serialize_core_term<'a>(
    term: &CoreTerm,
    builder: crate::axis_core_ir_0_1_capnp::core_term::Builder<'a>
) {
    match term {
        CoreTerm::IntLit(n, _) => {
            let mut lit_builder = builder.init_c_int_lit();
            lit_builder.set_value(*n);
        },
        CoreTerm::BoolLit(b, _) => {
            let mut lit_builder = builder.init_c_bool_lit();
            lit_builder.set_value(*b);
        },
        CoreTerm::UnitLit(_) => {
            builder.init_c_unit_lit();
        },
        CoreTerm::StrLit(s, _) => {
            let mut lit_builder = builder.init_c_str_lit();
            lit_builder.set_value(s);
        },
        CoreTerm::Var(name, _) => {
            let mut var_builder = builder.init_c_var();
            var_builder.set_name(name);
        },
        CoreTerm::Lam(param, body, _) => {
            let mut lam_builder = builder.init_c_lam();
            lam_builder.set_param(param);
            let body_builder = lam_builder.init_body();
            serialize_core_term(body, body_builder);
        },
        CoreTerm::App(func, arg, _) => {
            let mut app_builder = builder.init_c_app();
            let func_builder = app_builder.reborrow().init_func();
            serialize_core_term(func, func_builder);
            let arg_builder = app_builder.init_arg();
            serialize_core_term(arg, arg_builder);
        },
        CoreTerm::Tuple(elems, _) => {
            let tup_builder = builder.init_c_tuple();
            let mut elems_builder = tup_builder.init_elems(elems.len() as u32);
            for (i, elem) in elems.iter().enumerate() {
                let elem_builder = elems_builder.reborrow().get(i as u32);
                serialize_core_term(elem, elem_builder);
            }
        },
        CoreTerm::Proj(expr, index, _) => {
            let mut proj_builder = builder.init_c_proj();
            proj_builder.set_index(*index as u32);
            let expr_builder = proj_builder.init_expr();
            serialize_core_term(expr, expr_builder);
        },
        CoreTerm::Let(name, value, body, _) => {
            let mut let_builder = builder.init_c_let();
            let_builder.set_name(name);
            let value_builder = let_builder.reborrow().init_value();
            serialize_core_term(value, value_builder);
            let body_builder = let_builder.init_body();
            serialize_core_term(body, body_builder);
        },
        CoreTerm::If(cond, then_branch, else_branch, _) => {
            let mut if_builder = builder.init_c_if();
            let cond_builder = if_builder.reborrow().init_cond();
            serialize_core_term(cond, cond_builder);
            let then_builder = if_builder.reborrow().init_then_branch();
            serialize_core_term(then_branch, then_builder);
            let else_builder = if_builder.init_else_branch();
            serialize_core_term(else_branch, else_builder);
        },
        CoreTerm::Ctor(name, fields, _) => {
            let mut ctor_builder = builder.init_c_ctor();
            ctor_builder.set_name(name);
            let mut fields_builder = ctor_builder.init_fields(fields.len() as u32);
            for (i, field) in fields.iter().enumerate() {
                let field_builder = fields_builder.reborrow().get(i as u32);
                serialize_core_term(field, field_builder);
            }
        },
        CoreTerm::Match(scrutinee, arms, _) => {
            let mut match_builder = builder.init_c_match();
            let scrutinee_builder = match_builder.reborrow().init_scrutinee();
            serialize_core_term(scrutinee, scrutinee_builder);
            
            let mut arms_builder = match_builder.init_arms(arms.len() as u32);
            for (i, (pattern, body)) in arms.iter().enumerate() {
                let mut arm_builder = arms_builder.reborrow().get(i as u32);
                let pattern_builder = arm_builder.reborrow().init_pattern();
                serialize_pattern(pattern, pattern_builder);
                let body_builder = arm_builder.init_body();
                serialize_core_term(body, body_builder);
            }
        },
    }
}

/// Serialize Pattern to Cap'n Proto
fn serialize_pattern<'a>(
    pattern: &Pattern,
    builder: crate::axis_core_ir_0_1_capnp::pattern::Builder<'a>
) {
    match pattern {
        Pattern::PInt(n) => {
            let mut p_builder = builder.init_p_int();
            p_builder.set_value(*n);
        },
        Pattern::PBool(b) => {
            let mut p_builder = builder.init_p_bool();
            p_builder.set_value(*b);
        },
        Pattern::PUnit => {
            builder.init_p_unit();
        },
        Pattern::PVar(name) => {
            let mut p_builder = builder.init_p_var();
            p_builder.set_name(name);
        },
        Pattern::PTuple(patterns) => {
            let p_builder = builder.init_p_tuple();
            let mut patterns_builder = p_builder.init_patterns(patterns.len() as u32);
            for (i, pat) in patterns.iter().enumerate() {
                let pat_builder = patterns_builder.reborrow().get(i as u32);
                serialize_pattern(pat, pat_builder);
            }
        },
        Pattern::PEnum(name, patterns) => {
            let mut p_builder = builder.init_p_enum();
            p_builder.set_name(name);
            let mut patterns_builder = p_builder.init_patterns(patterns.len() as u32);
            for (i, pat) in patterns.iter().enumerate() {
                let pat_builder = patterns_builder.reborrow().get(i as u32);
                serialize_pattern(pat, pat_builder);
            }
        },
    }
}
