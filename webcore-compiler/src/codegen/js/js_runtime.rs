//! Runtime preamble emitters: State class, VARS array, evalCond, and bind functions.
//!
//! Each `emit_*` function returns a JS string fragment that is concatenated
//! by `generate_runtime_js_with_vars` to form the full runtime.
//!
//! The contract between the generated HTML (`data-webcore-*` attributes) and
//! these runtime functions is documented in `docs/runtime.md`.

use super::js_dom::RuntimeFeatures;
use std::collections::HashSet;
use std::fmt::Write as _;

/// Emit the State class definition, `const S`, `const STORE`, optional `const refs`,
/// and the `$effect` primitive.
pub(super) fn emit_state_class(has_refs: bool) -> String {
    let mut js = String::new();
    // __wcfx: currently-running effect (null when not inside an effect).
    // var (not let/const) so it hoists to window and is visible inside new Function() bodies.
    // setQ: silent setter — updates value without notifying listeners (used by computed)
    js.push_str("var __wcfx=null;\n");
    js.push_str("class State{#d=new Map();#l=new Map();#s=new Map();\n");
    js.push_str("get(k){if(__wcfx){if(!this.#s.has(k))this.#s.set(k,new Set());this.#s.get(k).add(__wcfx);}return this.#d.get(k);}\n");
    js.push_str("set(k,v){if(Object.is(this.#d.get(k),v))return;this.#d.set(k,v);this.#l.get(k)?.forEach(f=>f(v));const e=[...(this.#s.get(k)??[])];this.#s.get(k)?.clear();e.forEach(f=>f());}\n");
    js.push_str("setQ(k,v){this.#d.set(k,v)}\n");
    js.push_str("on(k,f){(this.#l.get(k)??this.#l.set(k,[]).get(k)).push(f)}}\n");
    js.push_str("const S=new State();\n");
    js.push_str("const STORE=new State();\n");
    if has_refs {
        js.push_str("const refs={};\n");
    }
    // $effect: fine-grained reactive primitive — runs fn immediately, tracks deps via __wcfx,
    // re-runs automatically when any accessed state key changes.
    js.push_str("function $effect(fn){const r=()=>{const p=__wcfx;__wcfx=r;try{fn();}finally{__wcfx=p;}};r();}\n");
    js
}

/// Emit `const VARS=[...]` and `const STORE_VARS=[...]` arrays.
pub(super) fn emit_vars_array(
    state_vars: &HashSet<String>,
    store_vars: &HashSet<String>,
) -> String {
    let mut js = String::new();
    let mut sorted_vars: Vec<_> = state_vars.iter().collect();
    sorted_vars.sort();
    let vars_list = sorted_vars
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(js, "const VARS=[{vars_list}];").expect("write! to String is infallible");

    let mut sorted_store: Vec<_> = store_vars.iter().collect();
    sorted_store.sort();
    let store_list = sorted_store
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(js, "const STORE_VARS=[{store_list}];").expect("write! to String is infallible");
    js
}

/// Emit the `evalCond` function, tree-shaken from the runtime when not needed.
pub(super) fn emit_evalcond(f: &RuntimeFeatures, has_locales: bool) -> String {
    // Fast-path lookups for simple identifiers avoid new Function entirely, which
    // is important because (a) new Function runs in global scope so block-scoped
    // S/STORE/U would not be visible if the fast path were skipped, and (b) it
    // avoids the catch fallback returning false for numeric-zero values.
    let route_fast_path = if f.has_param_routes {
        "const rp=_c.match(/^\\$route\\.([a-zA-Z_]\\w*)$/);if(rp)return ROUTE_PARAMS[rp[1]];"
    } else {
        ""
    };
    let query_fast_path = if f.has_query_params {
        "const qp=_c.match(/^\\$query\\.([a-zA-Z_]\\w*)$/);if(qp)return QUERY_PARAMS[qp[1]];"
    } else {
        ""
    };
    let route_replace = if f.has_param_routes {
        "e=e.replace(/\\$route\\.([a-zA-Z_]\\w*)/g,\"ROUTE_PARAMS['$1']\");"
    } else {
        ""
    };
    let query_replace = if f.has_query_params {
        "e=e.replace(/\\$query\\.([a-zA-Z_]\\w*)/g,\"QUERY_PARAMS['$1']\");"
    } else {
        ""
    };
    // S, STORE, U (and t/setLocale when i18n is active) are block-scoped — pass
    // them explicitly as Function parameters so that the dynamically-created
    // function body can resolve them even though Function() runs in global scope.
    let has_query = f.has_query_params;
    let fn_call = match (f.has_param_routes, has_locales, has_query) {
        (true,  true,  true)  => "new Function('S','STORE','U','ROUTE_PARAMS','QUERY_PARAMS','t','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS,QUERY_PARAMS,t)",
        (true,  true,  false) => "new Function('S','STORE','U','ROUTE_PARAMS','t','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS,t)",
        (true,  false, true)  => "new Function('S','STORE','U','ROUTE_PARAMS','QUERY_PARAMS','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS,QUERY_PARAMS)",
        (true,  false, false) => "new Function('S','STORE','U','ROUTE_PARAMS','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS)",
        (false, true,  true)  => "new Function('S','STORE','U','QUERY_PARAMS','t','\"use strict\";return('+e+')')(S,STORE,U,QUERY_PARAMS,t)",
        (false, true,  false) => "new Function('S','STORE','U','t','\"use strict\";return('+e+')')(S,STORE,U,t)",
        (false, false, true)  => "new Function('S','STORE','U','QUERY_PARAMS','\"use strict\";return('+e+')')(S,STORE,U,QUERY_PARAMS)",
        (false, false, false) => "new Function('S','STORE','U','\"use strict\";return('+e+')')(S,STORE,U)",
    };
    // Pre-compile per-variable regexes once (at page load) instead of inside evalCond.
    // _VR: array of [RegExp, replacement] pairs, longest-var-first to avoid partial matches.
    // VARS_SET: Set<string> for O(1) fast-path lookup instead of Array.indexOf O(n).
    // Fast path: simple state-var → S.get(name); $store.x → STORE.get(x);
    // optional $route.x → ROUTE_PARAMS[x]; optional $query.x → QUERY_PARAMS[x].
    // Complex expressions fall through to new Function.  On error return undefined
    // (not false) so interpolation spans show '' rather than the string "false".
    // Note: local var named _c (not t) to avoid shadowing the i18n t() function.
    format!(
        "const _VR=[...VARS].sort((a,b)=>b.length-a.length).map(v=>[new RegExp('\\\\b'+v+'\\\\b','g'),\"S.get('\"+v+\"')\"]);\n\
         const VARS_SET=new Set(VARS);\n\
         const evalCond=c=>{{const _c=c.trim();if(VARS_SET.has(_c))return S.get(_c);const sm=_c.match(/^\\$store\\.([a-zA-Z_]\\w*)$/);if(sm)return STORE.get(sm[1]);{route_fast_path}{query_fast_path}let e=_c;e=e.replace(/\\$store\\.([a-zA-Z_]\\w*)/g,\"STORE.get('$1')\");{route_replace}{query_replace}_VR.forEach(([re,r])=>{{e=e.replace(re,r)}});try{{return {fn_call}}}catch(_){{return undefined}}}};\n"
    )
}

/// Emit the reactive binding functions (`bindIf`, `bindFor`, `bindAttrs`,
/// `bindClassBindings`, `validateField`, `bindValidation`), tree-shaken
/// from the output when the corresponding features are absent.
pub(super) fn emit_bind_fns(f: &RuntimeFeatures) -> String {
    let mut js = String::new();

    if f.has_if {
        // bindIf with optional webc:transition support
        if f.has_transition {
            js.push_str(
                "const bindIf=()=>{\n\
                 document.querySelectorAll('[data-webcore-if]').forEach(el=>{\n\
                   const cond=el.dataset.webcoreIf,\n\
                         next=el.nextElementSibling,\n\
                         hasElse=next?.dataset.webcoreElse===cond,\n\
                         upd=()=>{\n\
                           const v=evalCond(cond),show=!!v;\n\
                           const _tr=el.dataset.webcoreTransition;\n\
                           if(_tr){\n\
                             if(show){\n\
                               el.style.display='';\n\
                               el.classList.add('webc-'+_tr+'-enter');\n\
                               requestAnimationFrame(()=>el.classList.replace('webc-'+_tr+'-enter','webc-'+_tr+'-enter-to'));\n\
                             } else {\n\
                               el.classList.add('webc-'+_tr+'-leave');\n\
                               requestAnimationFrame(()=>{\n\
                                 el.classList.replace('webc-'+_tr+'-leave','webc-'+_tr+'-leave-to');\n\
                                 el.addEventListener('transitionend',()=>{el.style.display='none';el.classList.remove('webc-'+_tr+'-leave-to');},{once:true});\n\
                               });\n\
                             }\n\
                           } else {\n\
                             el.style.display=show?'':'none';\n\
                           }\n\
                           if(hasElse)next.style.display=show?'none':''\n\
                         };\n\
                   $effect(upd);\n\
                 })\n\
                 };\n"
            );
        } else {
            js.push_str(
                "const bindIf=()=>{\n\
                 document.querySelectorAll('[data-webcore-if]').forEach(el=>{\n\
                   const cond=el.dataset.webcoreIf,\n\
                         next=el.nextElementSibling,\n\
                         hasElse=next?.dataset.webcoreElse===cond,\n\
                         upd=()=>{\n\
                           const v=evalCond(cond);\n\
                           el.style.display=v?'':'none';\n\
                           if(hasElse)next.style.display=v?'none':''\n\
                         };\n\
                   $effect(upd);\n\
                 })\n\
                 };\n",
            );
        }
    }
    if f.has_for {
        // bindFor — renders @for loops; supports optional key-based DOM diffing when
        // data-webcore-for-key is present on the template (avoids full re-render).
        // fillItem(el, val, i): sets text for interpolation spans (supports "item.prop" paths),
        // writes data-webcore-idx, and mirrors object properties as data-* attributes for CSS.
        // Keyed diffing: webcoreKey stored on firstElementChild (no extra wrapper div).
        // bindFor supports nested @for loops: outer loop variables are accessible inside
        // inner loops via the _wc_ctx context map propagated to inner templates.
        // - root parameter: defaults to document; inner calls use the container element.
        // - _wc_b flag: prevents double-binding when bindFor(cont) is called recursively.
        // - pCtx (parent context): map of {varName: value} from all enclosing @for loops.
        //   fillItem falls back to pCtx for interpolations not matching the local var.
        // - getItems(): resolves the iterable — from pCtx property access (e.g. "post.comments"),
        //   $store, or top-level state.
        // - isConnected guard: stale $effect callbacks from removed templates exit early.
        js.push_str("const bindFor=(root=document)=>{root.querySelectorAll('template[data-webcore-for]').forEach(tmpl=>{if(tmpl._wc_b)return;tmpl._wc_b=1;const iN=tmpl.dataset.webcoreFor,rawItN=tmpl.dataset.webcoreIn,keyExpr=tmpl.dataset.webcoreForKey,idxN=tmpl.dataset.webcoreForIndex,rangeStr=tmpl.dataset.webcoreForRange,pCtx=tmpl._wc_ctx??{},cont=tmpl.nextElementSibling,getItems=()=>{if(rangeStr){const[f,t]=rangeStr.split('..').map(Number);return Array.from({length:t-f},(_,i)=>String(f+i));}for(const[n,v]of Object.entries(pCtx)){if(rawItN===n)return Array.isArray(v)?v:[];if(rawItN.startsWith(n+'.')){const r=rawItN.slice(n.length+1).split('.').reduce((o,k)=>o?.[k],v);return Array.isArray(r)?r:[];}}const isStore=rawItN.startsWith('$store.'),itN=isStore?rawItN.slice(7):rawItN;return(isStore?STORE:S).get(itN)??[];},evalKey=keyExpr?(val=>keyExpr.split('.').reduce((o,k)=>o?.[k],{[iN]:val})):null,fillItem=(el,val,i)=>{el.querySelectorAll('[data-webcore-interpolation]').forEach(s=>{const ie=s.dataset.webcoreInterpolation;if(ie===iN){s.textContent=String(val??'');return;}if(idxN&&ie===idxN){s.textContent=String(i);return;}if(ie.startsWith(iN+'.')){s.textContent=String(ie.slice(iN.length+1).split('.').reduce((o,k)=>o?.[k],val)??'');return;}for(const[n,v]of Object.entries(pCtx)){if(ie===n){s.textContent=String(v??'');return;}if(ie.startsWith(n+'.')){s.textContent=String(ie.slice(n.length+1).split('.').reduce((o,k)=>o?.[k],v)??'');return;}}});el.dataset.webcoreIdx=String(i);if(val&&typeof val==='object')Object.entries(val).forEach(([k,v])=>{if(typeof v!=='object')el.dataset[k]=String(v)});},render=()=>{if(!tmpl.isConnected)return;const items=getItems();if(evalKey){const newKeys=items.map(evalKey);const existing=new Map([...cont.children].map(c=>[c.dataset.webcoreKey,c]));const keep=new Set(newKeys);[...existing.keys()].filter(k=>!keep.has(k)).forEach(k=>existing.get(k).remove());const frag=document.createDocumentFragment();newKeys.forEach((key,i)=>{if(existing.has(key)){const el=existing.get(key);fillItem(el,items[i],i);frag.appendChild(el);}else{const cl=tmpl.content.cloneNode(true);const fe=cl.firstElementChild;if(fe){fe.dataset.webcoreKey=key;fillItem(fe,items[i],i);}cl.querySelectorAll('template[data-webcore-for]').forEach(t=>{t._wc_ctx={...pCtx,[iN]:items[i]};});frag.append(...Array.from(cl.children));}});cont.replaceChildren(frag);}else{const frag=document.createDocumentFragment();items.forEach((val,i)=>{const cl=tmpl.content.cloneNode(true);const firstEl=cl.firstElementChild;if(firstEl)fillItem(firstEl,val,i);cl.querySelectorAll('template[data-webcore-for]').forEach(t=>{t._wc_ctx={...pCtx,[iN]:val};});frag.appendChild(cl);});cont.replaceChildren(frag);}bindFor(cont);};$effect(render);});};\n");
    }
    if f.has_dynamic_attrs {
        if f.has_style_binding {
            js.push_str(
                "const bindAttrs=()=>{\n\
                 document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\n\
                   [...el.attributes]\n\
                     .filter(a=>a.name.startsWith('data-webcore-attr-'))\n\
                     .forEach(a=>{\n\
                       const name=a.name.slice(18),expr=a.value,\n\
                             upd=()=>{\n\
                               const val=String(evalCond(expr)??'');\n\
                               name in el?el[name]=val:el.setAttribute(name,val)\n\
                             };\n\
                       $effect(upd);\n\
                     });\n\
                   for(const a of el.attributes){if(a.name.startsWith('data-webcore-style-')){const p=a.name.slice('data-webcore-style-'.length);const styleUpd=()=>el.style.setProperty(p,String(evalCond(a.value)??''));$effect(styleUpd);}}\n\
                 })\n\
                 };\n"
            );
        } else {
            js.push_str(
                "const bindAttrs=()=>{\n\
                 document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\n\
                   [...el.attributes]\n\
                     .filter(a=>a.name.startsWith('data-webcore-attr-'))\n\
                     .forEach(a=>{\n\
                       const name=a.name.slice(18),expr=a.value,\n\
                             upd=()=>{\n\
                               const val=String(evalCond(expr)??'');\n\
                               name in el?el[name]=val:el.setAttribute(name,val)\n\
                             };\n\
                       $effect(upd);\n\
                     })\n\
                 })\n\
                 };\n",
            );
        }
    } else if f.has_style_binding {
        // Only style bindings, no regular dynamic attrs — emit a simpler bindAttrs
        js.push_str(
            "const bindAttrs=()=>{\
document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\
for(const a of Array.from(el.attributes)){\
if(a.name.startsWith('data-webcore-style-')){\
const p=a.name.slice('data-webcore-style-'.length);\
const styleUpd=()=>el.style.setProperty(p,String(evalCond(a.value)??''));\
$effect(styleUpd);\
}\
}\
})\
};\n",
        );
    }
    if f.has_class_binding {
        js.push_str(
            "const bindClassBindings=()=>{\n\
             document.querySelectorAll('[data-webcore-class-bound]').forEach(el=>{\n\
               for(const attr of Array.from(el.attributes)){\n\
                 if(attr.name.startsWith('data-webcore-class-')&&attr.name!=='data-webcore-class-bound'){\n\
                   const cls=attr.name.slice(19),expr=attr.value,\n\
                         upd=()=>el.classList.toggle(cls,!!evalCond(expr));\n\
                   $effect(upd);\n\
                 }\n\
               }\n\
             })\n\
             };\n",
        );
    }
    if f.has_validation {
        js.push_str(
            "const validateField=input=>{\n\
               const val=input.value??'';\n\
               if('webcoreValidateRequired'in input.dataset&&!val.trim())\n\
                 return input.dataset.webcoreValidateRequired||'Champ requis';\n\
               const ml=input.dataset.webcoreValidateMinlength;\n\
               if(ml&&val.length<+ml)\n\
                 return input.dataset.webcoreValidateMinlengthMsg||`Minimum ${ml} caractères`;\n\
               const xl=input.dataset.webcoreValidateMaxlength;\n\
               if(xl&&val.length>+xl)\n\
                 return input.dataset.webcoreValidateMaxlengthMsg||`Maximum ${xl} caractères`;\n\
               if('webcoreValidateEmail'in input.dataset&&\n\
                  !/^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(val))\n\
                 return input.dataset.webcoreValidateEmail||'Email invalide';\n\
               const pat=input.dataset.webcoreValidatePattern;\n\
               if(pat){try{if(!new RegExp(pat).test(val))\n\
                 return input.dataset.webcoreValidatePatternMsg||'Format invalide'}catch(_){}}\n\
               return''\n\
             };\n",
        );
        js.push_str(
            "const bindValidation=()=>{\n\
             document.querySelectorAll('form').forEach(form=>{\n\
               const check=input=>{\n\
                 const field=input.dataset.webcoreField,\n\
                       err=validateField(input),\n\
                       el=field&&form.querySelector(`[data-webcore-error=\"${field}\"]`);\n\
                 if(el){(el.firstElementChild||el).textContent=err;el.style.display=err?'':'none'}\n\
                 return!err\n\
               };\n\
               form.querySelectorAll('[data-webcore-field]').forEach(input=>{\n\
                 input.addEventListener('blur',()=>{input.dataset.webcoreTouched='1';check(input)});\n\
                 input.addEventListener('input',()=>{if(input.dataset.webcoreTouched)check(input)})\n\
               });\n\
               form.addEventListener('submit',e=>{\n\
                 let ok=true;\n\
                 form.querySelectorAll('[data-webcore-field]').forEach(input=>{\n\
                   if(!check(input))ok=false\n\
                 });\n\
                 if(!ok){e.preventDefault();e.stopImmediatePropagation()}\n\
               },true)\n\
             })\n\
             };\n"
        );
    }
    js
}
