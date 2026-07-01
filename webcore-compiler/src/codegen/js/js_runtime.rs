//! Runtime preamble emitters: State class and the v3 reactive bind functions.
//!
//! Each `emit_*` function returns a JS string fragment that is concatenated
//! by `generate_runtime_js_with_vars_and_exprs` to form the full runtime.
//!
//! The contract between the generated HTML (`data-webcore-*` attributes) and
//! these runtime functions is documented in `docs/runtime.md`.

use super::js_dom::RuntimeFeatures;

/// Emit the State class definition, `const S`, `const STORE`, optional `const refs`,
/// and the `$effect` primitive.
pub(super) fn emit_state_class(has_refs: bool) -> String {
    let mut js = String::new();
    // __wcfx: currently-running effect (null when not inside an effect).
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

/// Emit the reactive binding functions. Each reads the module-scoped `_e` map of
/// compiled read-expression closures via `_e[id]()` (closed over, not passed in).
pub(super) fn emit_bind_fns_v3(f: &RuntimeFeatures) -> String {
    let mut js = String::new();

    if f.has_if {
        if f.has_transition {
            js.push_str(
                "const bindIf=()=>{\n\
                 document.querySelectorAll('[data-webcore-if]').forEach(el=>{\n\
                   const id=el.dataset.webcoreIf,fn=_e[id],\n\
                         next=el.nextElementSibling,\n\
                         hasElse=next?.dataset.webcoreElse===id,\n\
                         upd=()=>{\n\
                           const v=fn?.(),show=!!v;\n\
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
                   const id=el.dataset.webcoreIf,fn=_e[id],\n\
                         next=el.nextElementSibling,\n\
                         hasElse=next?.dataset.webcoreElse===id,\n\
                         upd=()=>{\n\
                           const v=fn?.();\n\
                           el.style.display=v?'':'none';\n\
                           if(hasElse)next.style.display=v?'none':''\n\
                         };\n\
                   $effect(upd);\n\
                 })\n\
                 };\n",
            );
        }
    }
    // bindFor in v3 is unchanged — it uses S.get(itN) directly for iterables
    if f.has_for {
        js.push_str("const bindFor=(root=document)=>{root.querySelectorAll('template[data-webcore-for]').forEach(tmpl=>{if(tmpl._wc_b)return;tmpl._wc_b=1;const iN=tmpl.dataset.webcoreFor,rawItN=tmpl.dataset.webcoreIn,keyExpr=tmpl.dataset.webcoreForKey,idxN=tmpl.dataset.webcoreForIndex,rangeStr=tmpl.dataset.webcoreForRange,pCtx=tmpl._wc_ctx??{},cont=tmpl.nextElementSibling,getItems=()=>{if(rangeStr){const[f,t]=rangeStr.split('..').map(Number);return Array.from({length:t-f},(_,i)=>String(f+i));}for(const[n,v]of Object.entries(pCtx)){if(rawItN===n)return Array.isArray(v)?v:[];if(rawItN.startsWith(n+'.')){const r=rawItN.slice(n.length+1).split('.').reduce((o,k)=>o?.[k],v);return Array.isArray(r)?r:[];}}const isStore=rawItN.startsWith('$store.'),itN=isStore?rawItN.slice(7):rawItN;return(isStore?STORE:S).get(itN)??[];},evalKey=keyExpr?(val=>keyExpr.split('.').reduce((o,k)=>o?.[k],{[iN]:val})):null,fillItem=(el,val,i)=>{el.querySelectorAll('[data-webcore-interpolation]').forEach(s=>{const ie=s.dataset.webcoreInterpolation;if(ie===iN){s.textContent=String(val??'');return;}if(idxN&&ie===idxN){s.textContent=String(i);return;}if(ie.startsWith(iN+'.')){s.textContent=String(ie.slice(iN.length+1).split('.').reduce((o,k)=>o?.[k],val)??'');return;}for(const[n,v]of Object.entries(pCtx)){if(ie===n){s.textContent=String(v??'');return;}if(ie.startsWith(n+'.')){s.textContent=String(ie.slice(n.length+1).split('.').reduce((o,k)=>o?.[k],v)??'');return;}}});el.dataset.webcoreIdx=String(i);if(val&&typeof val==='object')Object.entries(val).forEach(([k,v])=>{if(typeof v!=='object')el.dataset[k]=String(v)});},render=()=>{if(!tmpl.isConnected)return;const items=getItems();if(evalKey){const newKeys=items.map(evalKey);const existing=new Map([...cont.children].map(c=>[c.dataset.webcoreKey,c]));const keep=new Set(newKeys);[...existing.keys()].filter(k=>!keep.has(k)).forEach(k=>existing.get(k).remove());const frag=document.createDocumentFragment();newKeys.forEach((key,i)=>{if(existing.has(key)){const el=existing.get(key);fillItem(el,items[i],i);frag.appendChild(el);}else{const cl=tmpl.content.cloneNode(true);const fe=cl.firstElementChild;if(fe){fe.dataset.webcoreKey=key;fillItem(fe,items[i],i);}cl.querySelectorAll('template[data-webcore-for]').forEach(t=>{t._wc_ctx={...pCtx,[iN]:items[i]};});frag.append(...Array.from(cl.children));}});cont.replaceChildren(frag);}else{const frag=document.createDocumentFragment();items.forEach((val,i)=>{const cl=tmpl.content.cloneNode(true);const firstEl=cl.firstElementChild;if(firstEl)fillItem(firstEl,val,i);cl.querySelectorAll('template[data-webcore-for]').forEach(t=>{t._wc_ctx={...pCtx,[iN]:val};});frag.appendChild(cl);});cont.replaceChildren(frag);}bindFor(cont);};$effect(render);});};\n");
    }
    if f.has_dynamic_attrs {
        js.push_str(
            "const bindAttrs=()=>{\n\
             document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\n\
               [...el.attributes]\n\
                 .filter(a=>a.name.startsWith('data-webcore-attr-'))\n\
                 .forEach(a=>{\n\
                   const name=a.name.slice(18),id=a.value,fn=_e[id],\n\
                         upd=()=>{\n\
                           const val=String(fn?.()??'');\n\
                           name in el?el[name]=val:el.setAttribute(name,val)\n\
                         };\n\
                   $effect(upd);\n\
                 });\n",
        );
        if f.has_style_binding {
            js.push_str("   for(const a of el.attributes){if(a.name.startsWith('data-webcore-style-')){const p=a.name.slice('data-webcore-style-'.length);const id=a.value;const fn=_e[id];const styleUpd=()=>el.style.setProperty(p,String(fn?.()??''));$effect(styleUpd);}}\n");
        }
        if f.has_spread {
            js.push_str("for(const a of el.attributes){if(a.name==='data-webcore-spread'){const id=a.value;const fn=_e[id];const upd=()=>{const obj=fn?.()??{};if(typeof obj==='object')Object.entries(obj).forEach(([k,v])=>{k in el?el[k]=v:el.setAttribute(k,String(v))})};$effect(upd);}}\n");
        }
        js.push_str("  })\n  };\n");
    } else if f.has_style_binding {
        js.push_str(
            "const bindAttrs=()=>{\
document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\
for(const a of Array.from(el.attributes)){\
if(a.name.startsWith('data-webcore-style-')){\
const p=a.name.slice('data-webcore-style-'.length);\
const id=a.value;const fn=_e[id];\
const styleUpd=()=>el.style.setProperty(p,String(fn?.()??''));\
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
                   const cls=attr.name.slice(19),id=attr.value,fn=_e[id],\n\
                         upd=()=>el.classList.toggle(cls,!!fn?.());\n\
                   $effect(upd);\n\
                 }\n\
               }\n\
             })\n\
             };\n",
        );
    }
    if f.has_defer {
        js.push_str(
            "const bindDefer=()=>{\
document.querySelectorAll('[data-webcore-defer]').forEach(el=>el.style.display='');\
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
