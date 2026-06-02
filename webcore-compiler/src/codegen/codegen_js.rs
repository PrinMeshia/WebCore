//! JavaScript Code Generator for WebCore Runtime

use crate::ast::*;
use crate::codegen::codegen_html::HandlerMapping;
use regex::Regex;
use std::collections::HashSet;

/// Collect all local state variable names from a document (component-level)
pub fn collect_state_variables(document: &WebCoreDocument) -> HashSet<String> {
    let mut vars = HashSet::new();
    for component in document.components.values() {
        for state_var in &component.state {
            vars.insert(state_var.name.clone());
        }
    }
    vars
}

/// Collect global store variable names
pub fn collect_store_variables(document: &WebCoreDocument) -> HashSet<String> {
    document.store.iter().map(|v| v.name.clone()).collect()
}

/// Replace `$store.varname` (sentinel: `__STORE_varname__`) then local vars, then restore.
/// The sentinel approach prevents local-var replacement from touching store references.
fn replace_store_and_local(expr: &str, state_vars: &HashSet<String>) -> String {
    let re_store = Regex::new(r"\$store\.([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    // Step 1: sentinel so local-var replacement can't touch store refs
    let sentineled = re_store.replace_all(expr, "__STORE_$1__").to_string();
    // Step 2: replace local state vars  (_ is a word char → \bcount\b won't match inside __STORE_count__)
    let with_local = replace_vars_short(&sentineled, state_vars);
    // Step 3: restore sentinels → STORE.get('...')
    let re_sent = Regex::new(r"__STORE_([a-zA-Z_][a-zA-Z0-9_]*)__").unwrap();
    re_sent
        .replace_all(&with_local, "STORE.get('$1')")
        .to_string()
}

/// Parse `$store.identifier op= value`
fn parse_store_compound_assign(expr: &str, op: &str) -> Option<(String, String)> {
    let trimmed = expr.trim_start();
    if !trimmed.starts_with("$store.") {
        return None;
    }
    let rest = trimmed["$store.".len()..].trim_start();
    let pos = rest.find(op)?;
    let var_name = rest[..pos].trim().to_string();
    let value = rest[pos + op.len()..].trim().to_string();
    if !var_name.is_empty()
        && var_name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !value.is_empty()
    {
        Some((var_name, value))
    } else {
        None
    }
}

/// Parse `$store.identifier = value`
fn parse_store_simple_assign(expr: &str) -> Option<(String, String)> {
    let trimmed = expr.trim_start();
    if !trimmed.starts_with("$store.") {
        return None;
    }
    let rest = trimmed["$store.".len()..].trim_start();
    let (var_name, value) = parse_simple_assign(rest)?;
    if !var_name.contains('.') && !var_name.contains('$') {
        Some((var_name, value))
    } else {
        None
    }
}

/// Full expression compiler: handles both `$store.var` and local state vars.
fn compile_expression_full(expr: &str, state_vars: &HashSet<String>) -> String {
    let compiled = expr.trim();

    // Store compound assigns: $store.count += 1
    if let Some((var, val)) = parse_store_compound_assign(compiled, "+=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')+{})", var, var, rhs);
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "-=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')-{})", var, var, rhs);
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "*=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')*{})", var, var, rhs);
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "/=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')/{})", var, var, rhs);
    }
    // Store simple assign: $store.count = value
    if let Some((var, val)) = parse_store_simple_assign(compiled) {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',{})", var, rhs);
    }

    // Local state compound assigns: count += 1
    if let Some((var, val)) = parse_compound_assign(compiled, "+=") {
        return format!(
            "S.set('{}',S.get('{}')+{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "-=") {
        return format!(
            "S.set('{}',S.get('{}')-{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "*=") {
        return format!(
            "S.set('{}',S.get('{}')*{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "/=") {
        return format!(
            "S.set('{}',S.get('{}')/{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    // Local state simple assign: count = max(0, count - 1)
    if let Some((var, val)) = parse_simple_assign(compiled) {
        if state_vars.contains(&var) {
            let replaced = replace_utils_short(&replace_store_and_local(&val, state_vars));
            return format!("S.set('{}',{})", var, replaced);
        }
    }
    // Navigation
    if compiled.contains("webcore_navigate(") {
        return compile_navigate_call(compiled);
    }
    // Default: replace all variables and utils
    let result = replace_store_and_local(compiled, state_vars);
    replace_utils_short(&result)
}

/// Parse compound assignment like "count += 1"
fn parse_compound_assign(expr: &str, operator: &str) -> Option<(String, String)> {
    if let Some(pos) = expr.find(operator) {
        let var_name = expr[..pos].trim().to_string();
        let value = expr[pos + operator.len()..].trim().to_string();
        // Validate variable name (simple identifier)
        if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') && !var_name.is_empty() {
            return Some((var_name, value));
        }
    }
    None
}

/// Parse simple assignment like "count = value" (but not ==, !=, <=, >=)
fn parse_simple_assign(expr: &str) -> Option<(String, String)> {
    // Find = that is not part of ==, !=, <=, >=, +=, -=, *=, /=
    let chars: Vec<char> = expr.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '=' {
            // Check previous char
            if i > 0 {
                let prev = chars[i - 1];
                if prev == '='
                    || prev == '!'
                    || prev == '<'
                    || prev == '>'
                    || prev == '+'
                    || prev == '-'
                    || prev == '*'
                    || prev == '/'
                {
                    continue;
                }
            }
            // Check next char
            if i + 1 < chars.len() && chars[i + 1] == '=' {
                continue;
            }

            let var_name = expr[..i].trim().to_string();
            let value = expr[i + 1..].trim().to_string();

            // Validate variable name
            if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') && !var_name.is_empty() {
                return Some((var_name, value));
            }
        }
    }
    None
}

/// Replace variable references with short getter (S.get)
fn replace_vars_short(expr: &str, state_vars: &HashSet<String>) -> String {
    let mut result = expr.to_string();
    let mut vars: Vec<_> = state_vars.iter().collect();
    vars.sort_by_key(|v| std::cmp::Reverse(v.len()));

    for var in vars {
        let pattern = format!(r"\b{}\b", regex::escape(var));
        if let Ok(re) = Regex::new(&pattern) {
            result = re
                .replace_all(&result, format!("S.get('{}')", var).as_str())
                .to_string();
        }
    }
    result
}

/// Replace utility functions with short aliases (U.max, etc.)
fn replace_utils_short(expr: &str) -> String {
    let mut result = expr.to_string();
    for (old, new) in [("max(", "U.max("), ("min(", "U.min("), ("abs(", "U.abs(")] {
        if !result.contains(&format!("U.{}", old)) {
            result = result.replace(old, new);
        }
    }
    result
}

/// Compile webcore_navigate calls to short nav() function
fn compile_navigate_call(expr: &str) -> String {
    if let Some(start) = expr.find("webcore_navigate(") {
        if let Some(end) = expr[start..].find(')') {
            let path = expr[start + 17..start + end].trim();
            if path == "root" {
                return "nav('/')".to_string();
            }
            if path.starts_with('"') {
                return format!("nav({})", path);
            }
            if path.starts_with('/') {
                return format!("nav('{}')", path);
            }
            return format!("nav('/{}')", path);
        }
    }
    expr.to_string()
}

pub fn generate_runtime_js(handlers: &[HandlerMapping], document: &WebCoreDocument) -> String {
    let state_vars = collect_state_variables(document);
    let store_vars = collect_store_variables(document);
    generate_runtime_js_with_vars(handlers, &state_vars, &store_vars, document)
}

pub fn generate_runtime_js_with_vars(
    handlers: &[HandlerMapping],
    state_vars: &HashSet<String>,
    store_vars: &HashSet<String>,
    document: &WebCoreDocument,
) -> String {
    let mut unique_handlers: std::collections::HashMap<String, &HandlerMapping> =
        std::collections::HashMap::new();
    for handler in handlers {
        unique_handlers.insert(handler.id.clone(), handler);
    }

    let mut js = String::new();

    js.push_str("// WebCore Runtime (ES2024+)\n");
    js.push_str("{\n");

    // Shared State class for both local state and global store
    js.push_str("class State{#d=new Map();#l=new Map();\n");
    js.push_str("set(k,v){this.#d.set(k,v);this.#l.get(k)?.forEach(f=>f(v))}\n");
    js.push_str("get(k){return this.#d.get(k)}\n");
    js.push_str("on(k,f){(this.#l.get(k)??this.#l.set(k,[]).get(k)).push(f)}}\n");
    js.push_str("const S=new State();\n");
    js.push_str("const STORE=new State();\n\n");

    // Initialize local component state
    for component in document.components.values() {
        for state_var in &component.state {
            let default_value = state_var.default_value.as_deref().unwrap_or("null");
            let value = if state_var.type_ == "String"
                && !default_value.starts_with('"')
                && default_value != "null"
            {
                format!("\"{}\"", default_value)
            } else {
                default_value.to_string()
            };
            js.push_str(&format!("S.set('{}',{});\n", state_var.name, value));
        }
    }

    // Initialize global store
    for store_var in &document.store {
        let default_value = store_var.default_value.as_deref().unwrap_or("null");
        let value = if store_var.type_ == "String"
            && !default_value.starts_with('"')
            && default_value != "null"
        {
            format!("\"{}\"", default_value)
        } else {
            default_value.to_string()
        };
        js.push_str(&format!("STORE.set('{}',{});\n", store_var.name, value));
    }

    // Variable name lists for reactive bindings
    let mut sorted_vars: Vec<_> = state_vars.iter().collect();
    sorted_vars.sort();
    let vars_list = sorted_vars
        .iter()
        .map(|v| format!("'{}'", v))
        .collect::<Vec<_>>()
        .join(",");
    js.push_str(&format!("const VARS=[{}];\n", vars_list));

    let mut sorted_store: Vec<_> = store_vars.iter().collect();
    sorted_store.sort();
    let store_list = sorted_store
        .iter()
        .map(|v| format!("'{}'", v))
        .collect::<Vec<_>>()
        .join(",");
    js.push_str(&format!("const STORE_VARS=[{}];\n", store_list));

    js.push_str("const{max,min,abs}=Math,U={max,min,abs};\n\n");

    // i18n runtime
    if !document.locales.is_empty() {
        let mut locale_entries: Vec<String> = document
            .locales
            .iter()
            .map(|(code, messages)| {
                let mut msg_entries: Vec<String> = messages
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", escape_js_str(k), escape_js_str(v)))
                    .collect();
                msg_entries.sort();
                format!("\"{}\":{{{}}}", escape_js_str(code), msg_entries.join(","))
            })
            .collect();
        locale_entries.sort();
        js.push_str(&format!(
            "const LOCALES={{{}}};\n",
            locale_entries.join(",")
        ));
        js.push_str(&format!(
            "let LOCALE=\"{}\";\n",
            escape_js_str(&document.default_locale)
        ));
        js.push_str("const t=k=>LOCALES[LOCALE]?.[k]??k;\n");
        js.push_str("const setLocale=l=>{if(LOCALES[l]){LOCALE=l;bind();bindIf();bindFor();bindAttrs()}};\n\n");
    }

    // evalCond: replaces $store.var → STORE.get('var') before local vars → S.get('var')
    js.push_str("const evalCond=c=>{let e=c;e=e.replace(/\\$store\\.([a-zA-Z_]\\w*)/g,\"STORE.get('$1')\");VARS.forEach(v=>{e=e.replace(new RegExp('\\\\b'+v+'\\\\b','g'),\"S.get('\"+v+\"')\")});try{return Function('\"use strict\";return('+e+')')()}catch(_){return false}};\n");

    // bindIf: reacts to both local state and store changes
    js.push_str("const bindIf=()=>{document.querySelectorAll('[data-webcore-if]').forEach(el=>{const cond=el.dataset.webcoreIf,next=el.nextElementSibling,hasElse=next?.dataset.webcoreElse===cond,upd=()=>{const v=evalCond(cond);el.style.display=v?'':'none';if(hasElse)next.style.display=v?'none':''};upd();VARS.forEach(v=>S.on(v,upd));STORE_VARS.forEach(v=>STORE.on(v,upd))});};\n");

    // bindFor: supports $store.listName as iterable
    js.push_str("const bindFor=()=>{document.querySelectorAll('template[data-webcore-for]').forEach(tmpl=>{const iN=tmpl.dataset.webcoreFor,rawItN=tmpl.dataset.webcoreIn,isStore=rawItN.startsWith('$store.'),itN=isStore?rawItN.slice(7):rawItN,state=isStore?STORE:S,cont=tmpl.nextElementSibling,render=()=>{const items=state.get(itN)??[];cont.innerHTML='';items.forEach(val=>{const cl=tmpl.content.cloneNode(true);cl.querySelectorAll('[data-webcore-interpolation]').forEach(s=>{if(s.dataset.webcoreInterpolation===iN)s.textContent=val});cont.appendChild(cl)})};render();state.on(itN,render);STORE_VARS.forEach(v=>STORE.on(v,render))});};\n");

    // bindAttrs: reacts to both local state and store
    js.push_str("const bindAttrs=()=>{document.querySelectorAll('[data-webcore-bound]').forEach(el=>{[...el.attributes].filter(a=>a.name.startsWith('data-webcore-attr-')).forEach(a=>{const name=a.name.slice(18),expr=a.value,upd=()=>el.setAttribute(name,String(evalCond(expr)??''));upd();VARS.forEach(v=>S.on(v,upd));STORE_VARS.forEach(v=>STORE.on(v,upd))})});};\n\n");

    // Compile handlers — use store-aware compiler
    js.push_str("const H={\n");
    let mut sorted_handlers: Vec<_> = unique_handlers.values().collect();
    sorted_handlers.sort_by(|a, b| a.id.cmp(&b.id));
    for handler in sorted_handlers {
        let compiled = compile_expression_full(&handler.expression, state_vars);
        js.push_str(&format!("{}(){{{}}},\n", handler.id, compiled));
    }
    js.push_str("};\n\n");

    // SPA Navigation
    js.push_str("const toFile=p=>p==='/'?'index.html':`${p.slice(1)}.html`;\n");
    // bind: reacts to both local state and store interpolations
    js.push_str("const bind=()=>document.querySelectorAll('[data-webcore-interpolation]').forEach(el=>{const e=el.dataset.webcoreInterpolation,u=()=>{el.textContent=evalCond(e)??''};u();VARS.forEach(v=>S.on(v,u));STORE_VARS.forEach(v=>STORE.on(v,u))});\n\n");

    js.push_str("const nav=async p=>{\n");
    js.push_str("const file=toFile(p);\n");
    js.push_str("try{const html=await(await fetch(file)).text();\n");
    js.push_str("const doc=new DOMParser().parseFromString(html,'text/html');\n");
    js.push_str("const main=doc.querySelector('main');\n");
    js.push_str("if(main)document.querySelector('main').replaceWith(main);\n");
    js.push_str("history.pushState({},'',p);bind();bindIf();bindFor();bindAttrs();bindValidation()}catch(e){location.href=file}};\n\n");

    js.push_str("addEventListener('popstate',()=>nav(location.pathname));\n\n");

    let i18n_export = if !document.locales.is_empty() {
        ",setLocale"
    } else {
        ""
    };
    js.push_str(&format!(
        "Object.assign(globalThis,{{webcore_navigate:nav,webcore_handle_event:(t,id)=>H[id]?.(){},\n",
        i18n_export
    ));
    js.push_str("...Object.fromEntries(['click','submit','change','input'].map(e=>[`webcore_handle_${e}`,id=>H[id]?.()]))});\n\n");

    // Form validation helpers
    js.push_str("const validateField=input=>{const val=input.value??'';if('webcoreValidateRequired'in input.dataset&&!val.trim())return input.dataset.webcoreValidateRequired||'Champ requis';const ml=input.dataset.webcoreValidateMinlength;if(ml&&val.length<+ml)return input.dataset.webcoreValidateMinlengthMsg||`Minimum ${ml} caractères`;const xl=input.dataset.webcoreValidateMaxlength;if(xl&&val.length>+xl)return input.dataset.webcoreValidateMaxlengthMsg||`Maximum ${xl} caractères`;if('webcoreValidateEmail'in input.dataset&&!/^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(val))return input.dataset.webcoreValidateEmail||'Email invalide';const pat=input.dataset.webcoreValidatePattern;if(pat){try{if(!new RegExp(pat).test(val))return input.dataset.webcoreValidatePatternMsg||'Format invalide'}catch(_){}}return''};\n");
    js.push_str("const bindValidation=()=>{document.querySelectorAll('form').forEach(form=>{const check=input=>{const field=input.dataset.webcoreField,err=validateField(input),el=field&&form.querySelector(`[data-webcore-error=\"${field}\"]`);if(el){el.textContent=err;el.style.display=err?'':'none'}return!err};form.querySelectorAll('[data-webcore-field]').forEach(input=>{input.addEventListener('blur',()=>{input.dataset.webcoreTouched='1';check(input)});input.addEventListener('input',()=>{if(input.dataset.webcoreTouched)check(input)})});form.addEventListener('submit',e=>{let ok=true;form.querySelectorAll('[data-webcore-field]').forEach(input=>{if(!check(input))ok=false});if(!ok)e.preventDefault()})});};\n\n");

    js.push_str("document.addEventListener('DOMContentLoaded',()=>{bind();bindIf();bindFor();bindAttrs();bindValidation()});\n");

    // WASM async loader — injected only when a wasm/ module is present.
    // The WASM object is shared by reference so Object.assign fills it in-place
    // once the module loads, making wasm.fn() calls reactive on next bind().
    if let Some(module) = &document.wasm_module {
        // JS: const WASM={};globalThis.wasm=WASM;
        //     (async()=>{try{...}catch(e){...}})();
        let wasm_loader = format!(
            "const WASM={{}};globalThis.wasm=WASM;\
(async()=>{{try{{const m=await import('./wasm/{m}.js');\
await m.default();Object.assign(WASM,m);\
bind();bindIf();bindFor();bindAttrs();\
}}catch(e){{console.warn('[WebCore WASM]',e);}}}})();\n",
            m = escape_js_str(module),
        );
        js.push_str(&wasm_loader);
    }

    js.push_str("}\n");

    js
}

/// Escape a string for safe embedding in a JS double-quoted string literal.
fn escape_js_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Strip line comments and collapse whitespace — safe for generated JS (no multiline strings).
pub fn minify_js(js: &str) -> String {
    js.lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .map(str::trim)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_increment() {
        let mut vars = HashSet::new();
        vars.insert("count".to_string());

        let result = compile_expression_full("count += 1", &vars);
        assert_eq!(result, "S.set('count',S.get('count')+1)");
    }

    #[test]
    fn test_compile_decrement() {
        let mut vars = HashSet::new();
        vars.insert("count".to_string());

        let result = compile_expression_full("count -= 1", &vars);
        assert_eq!(result, "S.set('count',S.get('count')-1)");
    }

    #[test]
    fn test_compile_assignment_with_max() {
        let mut vars = HashSet::new();
        vars.insert("count".to_string());

        let result = compile_expression_full("count = max(0, count - 1)", &vars);
        assert!(result.starts_with("S.set('count',"));
        assert!(result.contains("U.max"));
        assert!(result.contains("S.get('count')"));
    }

    #[test]
    fn test_compile_navigate() {
        let vars = HashSet::new();

        let result = compile_expression_full("webcore_navigate(/about)", &vars);
        assert_eq!(result, "nav('/about')");
    }

    #[test]
    fn test_multiple_variables() {
        let mut vars = HashSet::new();
        vars.insert("x".to_string());
        vars.insert("y".to_string());
        vars.insert("total".to_string());

        let result = compile_expression_full("total = x + y", &vars);
        assert!(result.starts_with("S.set('total',"));
        assert!(result.contains("S.get('x')"));
        assert!(result.contains("S.get('y')"));
    }
}
