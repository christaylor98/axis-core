use std::fs;
use std::rc::Rc;
use crate::runtime::CoreTerm;
use crate::trace;

#[allow(dead_code)]
// Loaded Core IR metadata; consumed by later pipeline stages
pub struct CoreProgram {
    pub strings: Vec<String>,
    pub root_term: CoreTerm,
    pub entrypoint_id: usize,
}

pub fn load_core_bundle(path: &str) -> Result<CoreProgram, String> {
    eprintln!("[TRACE] loading core bundle path={}", path);
    
    let bytes = fs::read(path)
        .map_err(|e| format!("Failed to read Core bundle: {}", e))?;
    
    eprintln!("[TRACE] core bundle loaded size={} bytes", bytes.len());
    trace::trace(&format!("Loading Core bundle: {} ({} bytes)", path, bytes.len()));
    
    deserialize_core_bundle(&bytes)
}

fn deserialize_core_bundle(bytes: &[u8]) -> Result<CoreProgram, String> {
    use capnp::message::ReaderOptions;
    use capnp::serialize;
    
    let reader = serialize::read_message(
        &mut &bytes[..],
        ReaderOptions::new()
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
    
    Ok(CoreProgram {
        strings,
        root_term,
        entrypoint_id,
    })
}

fn deserialize_core_term(reader: crate::axis_core_ir_0_1_capnp::core_term::Reader) -> Result<CoreTerm, String> {
    use crate::axis_core_ir_0_1_capnp::core_term::Which;
    
    match reader.which() {
        Ok(Which::CIntLit(lit)) => {
            let lit = lit.map_err(|e| format!("Failed to read CIntLit: {}", e))?;
            let value = lit.get_value();
            Ok(CoreTerm::IntLit(value, None))
        },
        Ok(Which::CBoolLit(lit)) => {
            let lit = lit.map_err(|e| format!("Failed to read CBoolLit: {}", e))?;
            let value = lit.get_value();
            Ok(CoreTerm::BoolLit(value, None))
        },
        Ok(Which::CUnitLit(_)) => {
            Ok(CoreTerm::UnitLit(None))
        },
        Ok(Which::CStrLit(lit)) => {
            let lit = lit.map_err(|e| format!("Failed to read CStrLit: {}", e))?;
            let value = lit.get_value()
                .map_err(|e| format!("Failed to get string value: {}", e))?;
            Ok(CoreTerm::StrLit(
                value.to_str().map_err(|e| format!("Invalid UTF-8 in string: {}", e))?.to_string(),
                None
            ))
        },
        Ok(Which::CVar(var)) => {
            let var = var.map_err(|e| format!("Failed to read CVar: {}", e))?;
            let name = var.get_name()
                .map_err(|e| format!("Failed to get var name: {}", e))?;
            Ok(CoreTerm::Var(
                name.to_str().map_err(|e| format!("Invalid UTF-8 in var name: {}", e))?.to_string(),
                None
            ))
        },
        Ok(Which::CLam(lam)) => {
            let lam = lam.map_err(|e| format!("Failed to read CLam: {}", e))?;
            let param = lam.get_param()
                .map_err(|e| format!("Failed to get param: {}", e))?;
            let body_reader = lam.get_body()
                .map_err(|e| format!("Failed to get body: {}", e))?;
            let body = deserialize_core_term(body_reader)?;
            Ok(CoreTerm::Lam(
                param.to_str().map_err(|e| format!("Invalid UTF-8 in param: {}", e))?.to_string(),
                Rc::new(body),
                None
            ))
        },
        Ok(Which::CApp(app)) => {
            let app = app.map_err(|e| format!("Failed to read CApp: {}", e))?;
            let func_reader = app.get_func()
                .map_err(|e| format!("Failed to get func: {}", e))?;
            let arg_reader = app.get_arg()
                .map_err(|e| format!("Failed to get arg: {}", e))?;
            let func = deserialize_core_term(func_reader)?;
            let arg = deserialize_core_term(arg_reader)?;
            Ok(CoreTerm::App(Rc::new(func), Rc::new(arg), None))
        },
        Ok(Which::CTuple(tup)) => {
            let tup = tup.map_err(|e| format!("Failed to read CTuple: {}", e))?;
            let elems_reader = tup.get_elems()
                .map_err(|e| format!("Failed to get elems: {}", e))?;
            let mut elems = Vec::new();
            for i in 0..elems_reader.len() {
                let elem = elems_reader.get(i);
                elems.push(deserialize_core_term(elem)?);
            }
            Ok(CoreTerm::Tuple(elems, None))
        },
        Ok(Which::CProj(proj)) => {
            let proj = proj.map_err(|e| format!("Failed to read CProj: {}", e))?;
            let expr_reader = proj.get_expr()
                .map_err(|e| format!("Failed to get expr: {}", e))?;
            let index = proj.get_index() as usize;
            let expr = deserialize_core_term(expr_reader)?;
            Ok(CoreTerm::Proj(Rc::new(expr), index, None))
        },
        Ok(Which::CLet(let_)) => {
            let let_ = let_.map_err(|e| format!("Failed to read CLet: {}", e))?;
            let name = let_.get_name()
                .map_err(|e| format!("Failed to get name: {}", e))?;
            let value_reader = let_.get_value()
                .map_err(|e| format!("Failed to get value: {}", e))?;
            let body_reader = let_.get_body()
                .map_err(|e| format!("Failed to get body: {}", e))?;
            let value = deserialize_core_term(value_reader)?;
            let body = deserialize_core_term(body_reader)?;
            Ok(CoreTerm::Let(
                name.to_str().map_err(|e| format!("Invalid UTF-8 in let name: {}", e))?.to_string(),
                Rc::new(value),
                Rc::new(body),
                None
            ))
        },
        Ok(Which::CIf(if_)) => {
            let if_ = if_.map_err(|e| format!("Failed to read CIf: {}", e))?;
            let cond_reader = if_.get_cond()
                .map_err(|e| format!("Failed to get cond: {}", e))?;
            let then_reader = if_.get_then_branch()
                .map_err(|e| format!("Failed to get then: {}", e))?;
            let else_reader = if_.get_else_branch()
                .map_err(|e| format!("Failed to get else: {}", e))?;
            let cond = deserialize_core_term(cond_reader)?;
            let then_branch = deserialize_core_term(then_reader)?;
            let else_branch = deserialize_core_term(else_reader)?;
            Ok(CoreTerm::If(Rc::new(cond), Rc::new(then_branch), Rc::new(else_branch), None))
        },
        Ok(Which::CCtor(ctor)) => {
            let ctor = ctor.map_err(|e| format!("Failed to read CCtor: {}", e))?;
            let name = ctor.get_name()
                .map_err(|e| format!("Failed to get name: {}", e))?;
            let fields_reader = ctor.get_fields()
                .map_err(|e| format!("Failed to get fields: {}", e))?;
            let mut fields = Vec::new();
            for i in 0..fields_reader.len() {
                let field = fields_reader.get(i);
                fields.push(deserialize_core_term(field)?);
            }
            Ok(CoreTerm::Ctor(
                name.to_str().map_err(|e| format!("Invalid UTF-8 in ctor name: {}", e))?.to_string(),
                fields,
                None
            ))
        },
        Ok(Which::CMatch(match_)) => {
            let match_ = match_.map_err(|e| format!("Failed to read CMatch: {}", e))?;
            let scrutinee_reader = match_.get_scrutinee()
                .map_err(|e| format!("Failed to get scrutinee: {}", e))?;
            let scrutinee = deserialize_core_term(scrutinee_reader)?;
            
            // Deserialize match arms
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
            
            Ok(CoreTerm::Match(Rc::new(scrutinee), arms, None))
        },
        Err(e) => Err(format!("Unknown CoreTerm variant: {:?}", e)),
    }
}

// Deserialize Pattern from Cap'n Proto
fn deserialize_pattern(reader: crate::axis_core_ir_0_1_capnp::pattern::Reader) -> Result<crate::runtime::Pattern, String> {
    use crate::axis_core_ir_0_1_capnp::pattern::Which;
    use crate::runtime::Pattern;
    
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

// Serialize CoreTerm to Cap'n Proto
pub fn serialize_core_term<'a>(
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
            
            // Serialize match arms
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

// Serialize Pattern to Cap'n Proto
fn serialize_pattern<'a>(
    pattern: &crate::runtime::Pattern,
    builder: crate::axis_core_ir_0_1_capnp::pattern::Builder<'a>
) {
    use crate::runtime::Pattern;
    
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

// Create a core bundle binary
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

