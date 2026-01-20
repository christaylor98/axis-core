use std::fs;
use std::rc::Rc;

/// Lightweight program container returned by the bridge loader
pub struct CoreProgram {
    pub strings: Vec<String>,
    pub root_term: CoreTerm,
    pub entrypoint_id: usize,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

// In-memory CoreTerm shape used by the emitter logic
#[derive(Clone, Debug)]
pub enum CoreTerm {
    IntLit(i64, Option<Span>),
    BoolLit(bool, Option<Span>),
    UnitLit(Option<Span>),
    StrLit(String, Option<Span>),
    Var(String, Option<Span>),
    Lam(String, Rc<CoreTerm>, Option<Span>),
    App(Rc<CoreTerm>, Rc<CoreTerm>, Option<Span>),
    Tuple(Vec<CoreTerm>, Option<Span>),
    Proj(Rc<CoreTerm>, usize, Option<Span>),
    Let(String, Rc<CoreTerm>, Rc<CoreTerm>, Option<Span>),
    If(Rc<CoreTerm>, Rc<CoreTerm>, Rc<CoreTerm>, Option<Span>),
    Match(Rc<CoreTerm>, Vec<(Pattern, CoreTerm)>, Option<Span>),
    Ctor(String, Vec<CoreTerm>, Option<Span>),
}

#[derive(Clone, Debug)]
pub enum Pattern {
    PInt(i64),
    PBool(bool),
    PUnit,
    PVar(String),
    PTuple(Vec<Pattern>),
    PEnum(String, Vec<Pattern>),
}

// Stack-based iterative deserialization to handle deeply nested Core IR
enum StackFrame<'a> {
    // Leaf nodes - ready to convert
    IntLit(i64),
    BoolLit(bool),
    UnitLit,
    StrLit(String),
    Var(String),
    
    // Non-leaf nodes - waiting for children
    Lam {
        param: String,
        body_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        body_done: bool,
    },
    App {
        func_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        arg_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        func_done: bool,
        arg_done: bool,
    },
    Tuple {
        readers: Vec<crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>>,
        children: Vec<CoreTerm>,
        next_idx: usize,
    },
    Proj {
        expr_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        index: usize,
        expr_done: bool,
    },
    Let {
        name: String,
        value_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        body_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        value_done: bool,
        body_done: bool,
    },
    If {
        cond_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        then_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        else_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        cond_done: bool,
        then_done: bool,
        else_done: bool,
    },
    Ctor {
        name: String,
        readers: Vec<crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>>,
        children: Vec<CoreTerm>,
        next_idx: usize,
    },
    Match {
        scrutinee_reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>,
        scrutinee_done: bool,
        arms: Vec<(Pattern, CoreTerm)>,
    },
}

fn deserialize_core_term(reader: crate::axis_core_ir_0_1_capnp::core_term::Reader) -> Result<CoreTerm, String> {
    
    let mut work_stack: Vec<StackFrame> = Vec::new();
    let mut result_stack: Vec<CoreTerm> = Vec::new();
    let mut loop_counter: usize = 0;
    
    // Push initial reader as work
    work_stack.push(parse_reader_to_frame(reader)?);
    
    while let Some(frame) = work_stack.pop() {
        loop_counter += 1;
        if loop_counter % 10000 == 0 {
            eprintln!("[PROGRESS] phase=axis_rust_bridge loop=core_ir_deserialize count={}", loop_counter);
        }
        match frame {
            // Leaf nodes - directly push to result stack
            StackFrame::IntLit(v) => {
                result_stack.push(CoreTerm::IntLit(v, None));
            },
            StackFrame::BoolLit(v) => {
                result_stack.push(CoreTerm::BoolLit(v, None));
            },
            StackFrame::UnitLit => {
                result_stack.push(CoreTerm::UnitLit(None));
            },
            StackFrame::StrLit(s) => {
                result_stack.push(CoreTerm::StrLit(s, None));
            },
            StackFrame::Var(name) => {
                result_stack.push(CoreTerm::Var(name, None));
            },
            
            // Non-leaf nodes - process children
            StackFrame::Lam { param, body_reader, body_done } => {
                if !body_done {
                    // Re-push this frame with body_done=true
                    work_stack.push(StackFrame::Lam { param, body_reader, body_done: true });
                    // Push body work
                    work_stack.push(parse_reader_to_frame(body_reader)?);
                } else {
                    // Body is on result stack
                    let body = result_stack.pop().ok_or("Stack underflow: Lam body")?;
                    result_stack.push(CoreTerm::Lam(param, Rc::new(body), None));
                }
            },
            
            StackFrame::App { func_reader, arg_reader, func_done, arg_done } => {
                if !func_done {
                    work_stack.push(StackFrame::App { func_reader, arg_reader, func_done: true, arg_done: false });
                    work_stack.push(parse_reader_to_frame(func_reader)?);
                } else if !arg_done {
                    work_stack.push(StackFrame::App { func_reader, arg_reader, func_done: true, arg_done: true });
                    work_stack.push(parse_reader_to_frame(arg_reader)?);
                } else {
                    let arg = result_stack.pop().ok_or("Stack underflow: App arg")?;
                    let func = result_stack.pop().ok_or("Stack underflow: App func")?;
                    result_stack.push(CoreTerm::App(Rc::new(func), Rc::new(arg), None));
                }
            },
            
            StackFrame::Tuple { readers, mut children, next_idx } => {
                if next_idx < readers.len() {
                    let reader_to_process = readers[next_idx];
                    work_stack.push(StackFrame::Tuple { readers, children, next_idx: next_idx + 1 });
                    work_stack.push(parse_reader_to_frame(reader_to_process)?);
                } else {
                    // All children processed - collect from result stack
                    let count = readers.len();
                    for _ in 0..count {
                        children.push(result_stack.pop().ok_or("Stack underflow: Tuple")?);
                    }
                    children.reverse();
                    result_stack.push(CoreTerm::Tuple(children, None));
                }
            },
            
            StackFrame::Proj { expr_reader, index, expr_done } => {
                if !expr_done {
                    work_stack.push(StackFrame::Proj { expr_reader, index, expr_done: true });
                    work_stack.push(parse_reader_to_frame(expr_reader)?);
                } else {
                    let expr = result_stack.pop().ok_or("Stack underflow: Proj expr")?;
                    result_stack.push(CoreTerm::Proj(Rc::new(expr), index, None));
                }
            },
            
            StackFrame::Let { name, value_reader, body_reader, value_done, body_done } => {
                if !value_done {
                    work_stack.push(StackFrame::Let { name, value_reader, body_reader, value_done: true, body_done: false });
                    work_stack.push(parse_reader_to_frame(value_reader)?);
                } else if !body_done {
                    work_stack.push(StackFrame::Let { name, value_reader, body_reader, value_done: true, body_done: true });
                    work_stack.push(parse_reader_to_frame(body_reader)?);
                } else {
                    let body = result_stack.pop().ok_or("Stack underflow: Let body")?;
                    let value = result_stack.pop().ok_or("Stack underflow: Let value")?;
                    result_stack.push(CoreTerm::Let(name, Rc::new(value), Rc::new(body), None));
                }
            },
            
            StackFrame::If { cond_reader, then_reader, else_reader, cond_done, then_done, else_done } => {
                if !cond_done {
                    work_stack.push(StackFrame::If { cond_reader, then_reader, else_reader, cond_done: true, then_done: false, else_done: false });
                    work_stack.push(parse_reader_to_frame(cond_reader)?);
                } else if !then_done {
                    work_stack.push(StackFrame::If { cond_reader, then_reader, else_reader, cond_done: true, then_done: true, else_done: false });
                    work_stack.push(parse_reader_to_frame(then_reader)?);
                } else if !else_done {
                    work_stack.push(StackFrame::If { cond_reader, then_reader, else_reader, cond_done: true, then_done: true, else_done: true });
                    work_stack.push(parse_reader_to_frame(else_reader)?);
                } else {
                    let else_branch = result_stack.pop().ok_or("Stack underflow: If else")?;
                    let then_branch = result_stack.pop().ok_or("Stack underflow: If then")?;
                    let cond = result_stack.pop().ok_or("Stack underflow: If cond")?;
                    result_stack.push(CoreTerm::If(Rc::new(cond), Rc::new(then_branch), Rc::new(else_branch), None));
                }
            },
            
            StackFrame::Ctor { name, readers, mut children, next_idx } => {
                if next_idx < readers.len() {
                    let reader_to_process = readers[next_idx];
                    work_stack.push(StackFrame::Ctor { name, readers, children, next_idx: next_idx + 1 });
                    work_stack.push(parse_reader_to_frame(reader_to_process)?);
                } else {
                    // All children processed - collect from result stack
                    let count = readers.len();
                    for _ in 0..count {
                        children.push(result_stack.pop().ok_or("Stack underflow: Ctor field")?);
                    }
                    children.reverse();
                    result_stack.push(CoreTerm::Ctor(name, children, None));
                }
            },
            
            StackFrame::Match { scrutinee_reader, scrutinee_done, arms } => {
                if !scrutinee_done {
                    work_stack.push(StackFrame::Match { scrutinee_reader, scrutinee_done: true, arms });
                    work_stack.push(parse_reader_to_frame(scrutinee_reader)?);
                } else {
                    let scrutinee = result_stack.pop().ok_or("Stack underflow: Match scrutinee")?;
                    result_stack.push(CoreTerm::Match(Rc::new(scrutinee), arms, None));
                }
            },
        }
    }
    
    result_stack.pop().ok_or_else(|| "Empty result stack after deserialization".to_string())
}

fn parse_reader_to_frame<'a>(reader: crate::axis_core_ir_0_1_capnp::core_term::Reader<'a>) -> Result<StackFrame<'a>, String> {
    use crate::axis_core_ir_0_1_capnp::core_term::Which;
    
    match reader.which() {
        Ok(Which::CIntLit(lit)) => {
            let lit = lit.map_err(|e| format!("Failed to read CIntLit: {}", e))?;
            Ok(StackFrame::IntLit(lit.get_value()))
        },
        Ok(Which::CBoolLit(lit)) => {
            let lit = lit.map_err(|e| format!("Failed to read CBoolLit: {}", e))?;
            Ok(StackFrame::BoolLit(lit.get_value()))
        },
        Ok(Which::CUnitLit(_)) => {
            Ok(StackFrame::UnitLit)
        },
        Ok(Which::CStrLit(lit)) => {
            let lit = lit.map_err(|e| format!("Failed to read CStrLit: {}", e))?;
            let value = lit.get_value()
                .map_err(|e| format!("Failed to get string value: {}", e))?;
            Ok(StackFrame::StrLit(
                value.to_str().map_err(|e| format!("Invalid UTF-8 in string: {}", e))?.to_string()
            ))
        },
        Ok(Which::CVar(var)) => {
            let var = var.map_err(|e| format!("Failed to read CVar: {}", e))?;
            let name = var.get_name()
                .map_err(|e| format!("Failed to get var name: {}", e))?;
            Ok(StackFrame::Var(
                name.to_str().map_err(|e| format!("Invalid UTF-8 in var name: {}", e))?.to_string()
            ))
        },
        Ok(Which::CLam(lam)) => {
            let lam = lam.map_err(|e| format!("Failed to read CLam: {}", e))?;
            let param = lam.get_param()
                .map_err(|e| format!("Failed to get param: {}", e))?;
            let body_reader = lam.get_body()
                .map_err(|e| format!("Failed to get body: {}", e))?;
            Ok(StackFrame::Lam {
                param: param.to_str().map_err(|e| format!("Invalid UTF-8 in param: {}", e))?.to_string(),
                body_reader,
                body_done: false,
            })
        },
        Ok(Which::CApp(app)) => {
            let app = app.map_err(|e| format!("Failed to read CApp: {}", e))?;
            let func_reader = app.get_func()
                .map_err(|e| format!("Failed to get func: {}", e))?;
            let arg_reader = app.get_arg()
                .map_err(|e| format!("Failed to get arg: {}", e))?;
            Ok(StackFrame::App {
                func_reader,
                arg_reader,
                func_done: false,
                arg_done: false,
            })
        },
        Ok(Which::CTuple(tup)) => {
            let tup = tup.map_err(|e| format!("Failed to read CTuple: {}", e))?;
            let elems_reader = tup.get_elems()
                .map_err(|e| format!("Failed to get elems: {}", e))?;
            let mut readers = Vec::new();
            for i in 0..elems_reader.len() {
                readers.push(elems_reader.get(i));
            }
            Ok(StackFrame::Tuple {
                readers,
                children: Vec::new(),
                next_idx: 0,
            })
        },
        Ok(Which::CProj(proj)) => {
            let proj = proj.map_err(|e| format!("Failed to read CProj: {}", e))?;
            let expr_reader = proj.get_expr()
                .map_err(|e| format!("Failed to get expr: {}", e))?;
            let index = proj.get_index() as usize;
            Ok(StackFrame::Proj {
                expr_reader,
                index,
                expr_done: false,
            })
        },
        Ok(Which::CLet(let_)) => {
            let let_ = let_.map_err(|e| format!("Failed to read CLet: {}", e))?;
            let name = let_.get_name()
                .map_err(|e| format!("Failed to get name: {}", e))?;
            let value_reader = let_.get_value()
                .map_err(|e| format!("Failed to get value: {}", e))?;
            let body_reader = let_.get_body()
                .map_err(|e| format!("Failed to get body: {}", e))?;
            Ok(StackFrame::Let {
                name: name.to_str().map_err(|e| format!("Invalid UTF-8 in let name: {}", e))?.to_string(),
                value_reader,
                body_reader,
                value_done: false,
                body_done: false,
            })
        },
        Ok(Which::CIf(if_)) => {
            let if_ = if_.map_err(|e| format!("Failed to read CIf: {}", e))?;
            let cond_reader = if_.get_cond()
                .map_err(|e| format!("Failed to get cond: {}", e))?;
            let then_reader = if_.get_then_branch()
                .map_err(|e| format!("Failed to get then: {}", e))?;
            let else_reader = if_.get_else_branch()
                .map_err(|e| format!("Failed to get else: {}", e))?;
            Ok(StackFrame::If {
                cond_reader,
                then_reader,
                else_reader,
                cond_done: false,
                then_done: false,
                else_done: false,
            })
        },
        Ok(Which::CCtor(ctor)) => {
            let ctor = ctor.map_err(|e| format!("Failed to read CCtor: {}", e))?;
            let name = ctor.get_name()
                .map_err(|e| format!("Failed to get name: {}", e))?;
            let fields_reader = ctor.get_fields()
                .map_err(|e| format!("Failed to get fields: {}", e))?;
            let mut readers = Vec::new();
            for i in 0..fields_reader.len() {
                readers.push(fields_reader.get(i));
            }
            Ok(StackFrame::Ctor {
                name: name.to_str().map_err(|e| format!("Invalid UTF-8 in ctor name: {}", e))?.to_string(),
                readers,
                children: Vec::new(),
                next_idx: 0,
            })
        },
        Ok(Which::CMatch(match_)) => {
            let match_ = match_.map_err(|e| format!("Failed to read CMatch: {}", e))?;
            let scrutinee_reader = match_.get_scrutinee()
                .map_err(|e| format!("Failed to get scrutinee: {}", e))?;
            
            // Deserialize arms (patterns and bodies)
            let arms_reader = match_.get_arms()
                .map_err(|e| format!("Failed to get arms: {}", e))?;
            let mut arms = Vec::new();
            for arm_reader in arms_reader.iter() {
                let pattern_reader = arm_reader.get_pattern()
                    .map_err(|e| format!("Failed to get pattern: {}", e))?;
                let pattern = deserialize_pattern(pattern_reader)?;
                
                let body_reader = arm_reader.get_body()
                    .map_err(|e| format!("Failed to get body: {}", e))?;
                let body = deserialize_core_term(body_reader)?;
                
                arms.push((pattern, body));
            }
            
            Ok(StackFrame::Match {
                scrutinee_reader,
                scrutinee_done: false,
                arms,
            })
        },
        Err(e) => Err(format!("Unknown CoreTerm variant: {:?}", e)),
    }
}

// Deserialize Pattern from Cap'n Proto
fn deserialize_pattern(reader: crate::axis_core_ir_0_1_capnp::pattern::Reader) -> Result<Pattern, String> {
    use crate::axis_core_ir_0_1_capnp::pattern::Which;
    
    match reader.which() {
        Ok(Which::PInt(p)) => {
            let p = p.map_err(|e| format!("Failed to read PInt: {}", e))?;
            Ok(Pattern::PInt(p.get_value()))
        },
        Ok(Which::PBool(p)) => {
            let p = p.map_err(|e| format!("Failed to read PBool: {}", e))?;
            Ok(Pattern::PBool(p.get_value()))
        },
        Ok(Which::PUnit(_)) => {
            Ok(Pattern::PUnit)
        },
        Ok(Which::PVar(p)) => {
            let p = p.map_err(|e| format!("Failed to read PVar: {}", e))?;
            let name = p.get_name()
                .map_err(|e| format!("Failed to get var name: {}", e))?;
            Ok(Pattern::PVar(
                name.to_str().map_err(|e| format!("Invalid UTF-8 in var name: {}", e))?.to_string()
            ))
        },
        Ok(Which::PTuple(p)) => {
            let p = p.map_err(|e| format!("Failed to read PTuple: {}", e))?;
            let patterns_reader = p.get_patterns()
                .map_err(|e| format!("Failed to get tuple patterns: {}", e))?;
            let mut patterns = Vec::new();
            for pat_reader in patterns_reader.iter() {
                patterns.push(deserialize_pattern(pat_reader)?);
            }
            Ok(Pattern::PTuple(patterns))
        },
        Ok(Which::PEnum(p)) => {
            let p = p.map_err(|e| format!("Failed to read PEnum: {}", e))?;
            let name = p.get_name()
                .map_err(|e| format!("Failed to get enum name: {}", e))?;
            let patterns_reader = p.get_patterns()
                .map_err(|e| format!("Failed to get enum patterns: {}", e))?;
            let mut patterns = Vec::new();
            for pat_reader in patterns_reader.iter() {
                patterns.push(deserialize_pattern(pat_reader)?);
            }
            Ok(Pattern::PEnum(
                name.to_str().map_err(|e| format!("Invalid UTF-8 in enum name: {}", e))?.to_string(),
                patterns
            ))
        },
        Err(e) => Err(format!("Unknown Pattern variant: {:?}", e)),
    }
}

// ============================================================
// SERIALIZATION: Write Core IR to Cap'n Proto binary format
// ============================================================

/// Serialize a CoreTerm to Cap'n Proto format
fn serialize_core_term(term: &CoreTerm, builder: crate::axis_core_ir_0_1_capnp::core_term::Builder) {
    match term {
        CoreTerm::IntLit(n, _) => {
            let mut lit = builder.init_c_int_lit();
            lit.set_value(*n);
        },
        CoreTerm::BoolLit(b, _) => {
            let mut lit = builder.init_c_bool_lit();
            lit.set_value(*b);
        },
        CoreTerm::UnitLit(_) => {
            builder.init_c_unit_lit();
        },
        CoreTerm::StrLit(s, _) => {
            let mut lit = builder.init_c_str_lit();
            lit.set_value(s);
        },
        CoreTerm::Var(name, _) => {
            let mut var = builder.init_c_var();
            var.set_name(name);
        },
        CoreTerm::Lam(param, body, _) => {
            let mut lam = builder.init_c_lam();
            lam.set_param(param);
            let body_builder = lam.init_body();
            serialize_core_term(body, body_builder);
        },
        CoreTerm::App(func, arg, _) => {
            let mut app = builder.init_c_app();
            let func_builder = app.reborrow().init_func();
            serialize_core_term(func, func_builder);
            let arg_builder = app.init_arg();
            serialize_core_term(arg, arg_builder);
        },
        CoreTerm::Tuple(elems, _) => {
            let tup = builder.init_c_tuple();
            let mut elems_builder = tup.init_elems(elems.len() as u32);
            for (i, elem) in elems.iter().enumerate() {
                let elem_builder = elems_builder.reborrow().get(i as u32);
                serialize_core_term(elem, elem_builder);
            }
        },
        CoreTerm::Proj(expr, index, _) => {
            let mut proj = builder.init_c_proj();
            proj.set_index(*index as u32);
            let expr_builder = proj.init_expr();
            serialize_core_term(expr, expr_builder);
        },
        CoreTerm::Let(name, value, body, _) => {
            let mut let_node = builder.init_c_let();
            let_node.set_name(name);
            let value_builder = let_node.reborrow().init_value();
            serialize_core_term(value, value_builder);
            let body_builder = let_node.init_body();
            serialize_core_term(body, body_builder);
        },
        CoreTerm::If(cond, then_branch, else_branch, _) => {
            let mut if_node = builder.init_c_if();
            let cond_builder = if_node.reborrow().init_cond();
            serialize_core_term(cond, cond_builder);
            let then_builder = if_node.reborrow().init_then_branch();
            serialize_core_term(then_branch, then_builder);
            let else_builder = if_node.init_else_branch();
            serialize_core_term(else_branch, else_builder);
        },
        CoreTerm::Ctor(name, fields, _) => {
            let mut ctor = builder.init_c_ctor();
            ctor.set_name(name);
            let mut fields_builder = ctor.init_fields(fields.len() as u32);
            for (i, field) in fields.iter().enumerate() {
                let field_builder = fields_builder.reborrow().get(i as u32);
                serialize_core_term(field, field_builder);
            }
        },
        CoreTerm::Match(scrutinee, arms, _) => {
            let mut match_node = builder.init_c_match();
            let scrutinee_builder = match_node.reborrow().init_scrutinee();
            serialize_core_term(scrutinee, scrutinee_builder);
            let mut arms_builder = match_node.init_arms(arms.len() as u32);
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

/// Serialize a Pattern to Cap'n Proto format
fn serialize_pattern(pattern: &Pattern, builder: crate::axis_core_ir_0_1_capnp::pattern::Builder) {
    match pattern {
        Pattern::PInt(n) => {
            let mut p = builder.init_p_int();
            p.set_value(*n);
        },
        Pattern::PBool(b) => {
            let mut p = builder.init_p_bool();
            p.set_value(*b);
        },
        Pattern::PUnit => {
            builder.init_p_unit();
        },
        Pattern::PVar(name) => {
            let mut p = builder.init_p_var();
            p.set_name(name);
        },
        Pattern::PTuple(patterns) => {
            let p = builder.init_p_tuple();
            let mut patterns_builder = p.init_patterns(patterns.len() as u32);
            for (i, pat) in patterns.iter().enumerate() {
                let pat_builder = patterns_builder.reborrow().get(i as u32);
                serialize_pattern(pat, pat_builder);
            }
        },
        Pattern::PEnum(name, patterns) => {
            let mut p = builder.init_p_enum();
            p.set_name(name);
            let mut patterns_builder = p.init_patterns(patterns.len() as u32);
            for (i, pat) in patterns.iter().enumerate() {
                let pat_builder = patterns_builder.reborrow().get(i as u32);
                serialize_pattern(pat, pat_builder);
            }
        },
    }
}

/// Create a Core bundle binary from a CoreTerm
pub fn create_core_bundle(term: &CoreTerm, entrypoint_name: &str) -> Vec<u8> {
    use capnp::message::Builder;
    use capnp::serialize;
    
    let mut message = Builder::new_default();
    
    {
        let mut bundle = message.init_root::<crate::axis_core_ir_0_1_capnp::core_bundle::Builder>();
        bundle.set_version("0.1");
        bundle.set_entrypoint_name(entrypoint_name);
        bundle.set_entrypoint_id(0);
        bundle.reborrow().init_string_table(0);
        
        let core_term_builder = bundle.init_core_term();
        serialize_core_term(term, core_term_builder);
    }
    
    let mut buf = Vec::new();
    serialize::write_message(&mut buf, &message).unwrap();
    buf
}

/// Write a Core bundle to a file path
pub fn write_core_bundle_to_file(term: &CoreTerm, entrypoint_name: &str, path: &str) -> Result<(), String> {
    let bytes = create_core_bundle(term, entrypoint_name);
    fs::write(path, bytes).map_err(|e| format!("Failed to write Core bundle: {}", e))
}

/// Inspect a Core bundle file and return a summary
pub fn inspect_core_bundle(path: &str) -> Result<String, String> {
    let program = load_core_bundle(path)?;
    Ok(format!(
        "Core bundle: {}\n  Version: 0.1\n  Entrypoint ID: {}\n  String table entries: {}\n  Root term: {:?}",
        path,
        program.entrypoint_id,
        program.strings.len(),
        core_term_summary(&program.root_term)
    ))
}

/// Generate a brief summary of a CoreTerm (for inspection)
fn core_term_summary(term: &CoreTerm) -> String {
    match term {
        CoreTerm::IntLit(n, _) => format!("IntLit({})", n),
        CoreTerm::BoolLit(b, _) => format!("BoolLit({})", b),
        CoreTerm::UnitLit(_) => "UnitLit".to_string(),
        CoreTerm::StrLit(s, _) => format!("StrLit({:?})", s),
        CoreTerm::Var(name, _) => format!("Var({})", name),
        CoreTerm::Lam(param, _, _) => format!("Lam({}, ...)", param),
        CoreTerm::App(_, _, _) => "App(...)".to_string(),
        CoreTerm::Tuple(elems, _) => format!("Tuple({} elems)", elems.len()),
        CoreTerm::Proj(_, idx, _) => format!("Proj(..., {})", idx),
        CoreTerm::Let(name, _, _, _) => format!("Let({}, ...)", name),
        CoreTerm::If(_, _, _, _) => "If(...)".to_string(),
        CoreTerm::Match(_, arms, _) => format!("Match({} arms)", arms.len()),
        CoreTerm::Ctor(name, fields, _) => format!("Ctor({}, {} fields)", name, fields.len()),
    }
}

/// Load a core bundle binary file produced by `axis-compiler`
pub fn load_core_bundle(path: &str) -> Result<CoreProgram, String> {
    use capnp::message::ReaderOptions;
    use capnp::serialize;
    
    let bytes = fs::read(path)
        .map_err(|e| format!("Failed to read Core bundle: {}", e))?;

    let mut opts = ReaderOptions::new();

    // Allow very large compiler IRs (trusted input)
    opts.traversal_limit_in_words = Some(1024 * 1024 * 1024); // ~8GB logical traversal
    opts.nesting_limit = 1_000_000;                     // extremely deep trees

    let reader = serialize::read_message(
        &mut &bytes[..],
        opts
    ).map_err(|e| format!("Failed to read Cap'n Proto message: {}", e))?;
    
    let bundle = reader.get_root::<crate::axis_core_ir_0_1_capnp::core_bundle::Reader>()
        .map_err(|e| format!("Failed to get root: {}", e))?;
    
    let version = bundle.get_version()
        .map_err(|e| format!("Failed to get version: {}", e))?;
    
    if version.to_str().map_err(|e| format!("Invalid UTF-8 in version: {}", e))? != "0.1" {
        return Err(format!("Unsupported Core bundle version: {:?}", version));
    }
    
    let entrypoint_id = bundle.get_entrypoint_id() as usize;
    
    let string_table = bundle.get_string_table()
        .map_err(|e| format!("Failed to get string table: {}", e))?;
    
    let mut strings = Vec::new();
    for i in 0..string_table.len() {
        let s = string_table.get(i)
            .map_err(|e| format!("Failed to get string {}: {}", i, e))?;
        strings.push(s.to_str()
            .map_err(|e| format!("Invalid UTF-8 in string {}: {}", i, e))?
            .to_string());
    }
    
    let core_term_reader = bundle.get_core_term()
        .map_err(|e| format!("Failed to get core term: {}", e))?;
    
    let root_term = deserialize_core_term(core_term_reader)?;

    Ok(CoreProgram { strings, root_term, entrypoint_id })
}
