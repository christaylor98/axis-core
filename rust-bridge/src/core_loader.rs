// core_loader.rs removed: functionality moved to `core_ir.rs`.
// This file is intentionally left as a placeholder to avoid accidental
// use while code is transitioning. Use `core_ir::load_core_bundle` instead.

use crate::core_ir::CoreTerm;
use std::rc::Rc;
#[allow(dead_code)]
// Transitional helpers retained for alternate emission paths
fn deserialize_core_term(value: &serde_json::Value) -> Result<CoreTerm, String> {
    let obj = value.as_object().ok_or("CoreTerm must be an object")?;
    let tag = obj.get("tag")
        .and_then(|v| v.as_str())
        .ok_or("CoreTerm missing 'tag' field")?;

    match tag {
        "CIntLit" => {
            let n = obj.get("value")
                .and_then(|v| v.as_i64())
                .ok_or("CIntLit missing integer value")?;
            Ok(CoreTerm::IntLit(n, None))
        },
        "CBoolLit" => {
            let b = obj.get("value")
                .and_then(|v| v.as_bool())
                .ok_or("CBoolLit missing boolean value")?;
            Ok(CoreTerm::BoolLit(b, None))
        },
        "CUnitLit" => {
            Ok(CoreTerm::UnitLit(None))
        },
        "CStrLit" => {
            let s = obj.get("value")
                .and_then(|v| v.as_str())
                .ok_or("CStrLit missing string value")?;
            Ok(CoreTerm::StrLit(s.to_string(), None))
        },
        "CVar" => {
            let name = obj.get("name")
                .and_then(|v| v.as_str())
                .ok_or("CVar missing name")?;
            Ok(CoreTerm::Var(name.to_string(), None))
        },
        "CLam" => {
            let param = obj.get("param")
                .and_then(|v| v.as_str())
                .ok_or("CLam missing param")?;
            let body_val = obj.get("body")
                .ok_or("CLam missing body")?;
            let body = deserialize_core_term(body_val)?;
            Ok(CoreTerm::Lam(param.to_string(), Rc::new(body), None))
        },
        "CApp" => {
            let func_val = obj.get("func")
                .ok_or("CApp missing func")?;
            let arg_val = obj.get("arg")
                .ok_or("CApp missing arg")?;
            let func = deserialize_core_term(func_val)?;
            let arg = deserialize_core_term(arg_val)?;
            Ok(CoreTerm::App(Rc::new(func), Rc::new(arg), None))
        },
        "CTuple" => {
            let elems_val = obj.get("elems")
                .and_then(|v| v.as_array())
                .ok_or("CTuple missing elems array")?;
            let mut elems = Vec::new();
            for elem_val in elems_val {
                elems.push(deserialize_core_term(elem_val)?);
            }
            Ok(CoreTerm::Tuple(elems, None))
        },
        "CProj" => {
            let expr_val = obj.get("expr")
                .ok_or("CProj missing expr")?;
            let index = obj.get("index")
                .and_then(|v| v.as_i64())
                .ok_or("CProj missing index")?;
            let expr = deserialize_core_term(expr_val)?;
            Ok(CoreTerm::Proj(Rc::new(expr), index as usize, None))
        },
        "CLet" => {
            let name = obj.get("name")
                .and_then(|v| v.as_str())
                .ok_or("CLet missing name")?;
            let value_val = obj.get("value")
                .ok_or("CLet missing value")?;
            let body_val = obj.get("body")
                .ok_or("CLet missing body")?;
            let value = deserialize_core_term(value_val)?;
            let body = deserialize_core_term(body_val)?;
            Ok(CoreTerm::Let(name.to_string(), Rc::new(value), Rc::new(body), None))
        },
        "CIf" => {
            let cond_val = obj.get("cond")
                .ok_or("CIf missing cond")?;
            let then_val = obj.get("then")
                .ok_or("CIf missing then")?;
            let else_val = obj.get("else")
                .ok_or("CIf missing else")?;
            let cond = deserialize_core_term(cond_val)?;
            let then_branch = deserialize_core_term(then_val)?;
            let else_branch = deserialize_core_term(else_val)?;
            Ok(CoreTerm::If(Rc::new(cond), Rc::new(then_branch), Rc::new(else_branch), None))
        },
        "CCtor" => {
            let name = obj.get("name")
                .and_then(|v| v.as_str())
                .ok_or("CCtor missing name")?;
            let fields_val = obj.get("fields")
                .and_then(|v| v.as_array())
                .ok_or("CCtor missing fields array")?;
            let mut fields = Vec::new();
            for field in fields_val {
                fields.push(deserialize_core_term(field)?);
            }
            Ok(CoreTerm::Ctor(name.to_string(), fields, None))
        }
        _ => Err(format!("Unknown CoreTerm tag: {}", tag))
    }
}

// serialize functions are not required in the bridge; only deserialization is used
