//! WebCore built-in function registry (4.1, epic #57).
//!
//! Single source of truth for namespaced built-ins (`math.round`, `str.slugify`,
//! …) used by:
//! - the JS codegen — [`rewrite`] turns `<ns>.<fn>(` source calls into runtime
//!   helper identifiers, and [`emit_used`] emits `const <helper>=<js>;` only for
//!   the helpers that actually appear in the generated runtime (tree-shaking);
//! - the SSG evaluator — deterministic numeric builtins carry an `ssg` folder so
//!   `math.round(2.4)` is pre-computed at build time (zero runtime cost).
//!
//! Namespaces (`math`, `str`, `fmt`, `arr`) are reserved prefixes.

/// Build-time folding strategy for a deterministic builtin (SSG). `None` marks
/// runtime-only builtins (non-deterministic, or not worth pre-computing).
pub(crate) enum Ssg {
    /// Not foldable at build time — emitted for the runtime only. Used by
    /// locale-dependent (`fmt.*`) and array (`arr.*`) namespaces.
    None,
    /// Numeric fold: every argument resolves to a number.
    Num(fn(&[f64]) -> Option<f64>),
    /// String fold: every argument resolves to a string (numbers included, as
    /// their text form — the folder parses what it needs).
    Str(fn(&[String]) -> Option<String>),
}

/// A namespaced built-in function.
pub(crate) struct Builtin {
    /// Source syntax, e.g. `"math.round"`.
    pub source: &'static str,
    /// Runtime helper identifier the source call is rewritten to.
    pub runtime: &'static str,
    /// JS expression assigned to `runtime` when the builtin is used.
    pub js: &'static str,
    /// Build-time folding strategy.
    pub ssg: Ssg,
}

/// The registry. New namespaces (str/fmt/arr) are appended here.
pub(crate) const BUILTINS: &[Builtin] = &[
    // ── math.* (deterministic → SSG-foldable) ─────────────────────────────────
    Builtin {
        source: "math.round",
        runtime: "_bmround",
        js: "(x)=>Math.round(x)",
        ssg: Ssg::Num(|a| a.first().map(|x| x.round())),
    },
    Builtin {
        source: "math.floor",
        runtime: "_bmfloor",
        js: "(x)=>Math.floor(x)",
        ssg: Ssg::Num(|a| a.first().map(|x| x.floor())),
    },
    Builtin {
        source: "math.ceil",
        runtime: "_bmceil",
        js: "(x)=>Math.ceil(x)",
        ssg: Ssg::Num(|a| a.first().map(|x| x.ceil())),
    },
    Builtin {
        source: "math.clamp",
        runtime: "_bmclamp",
        js: "(x,a,b)=>Math.min(Math.max(x,a),b)",
        ssg: Ssg::Num(|a| (a.len() == 3).then(|| a[0].max(a[1]).min(a[2]))),
    },
    Builtin {
        source: "math.pow",
        runtime: "_bmpow",
        js: "(x,y)=>Math.pow(x,y)",
        ssg: Ssg::Num(|a| (a.len() == 2).then(|| a[0].powf(a[1]))),
    },
    Builtin {
        source: "math.sqrt",
        runtime: "_bmsqrt",
        js: "(x)=>Math.sqrt(x)",
        ssg: Ssg::Num(|a| a.first().map(|x| x.sqrt())),
    },
    Builtin {
        source: "math.sign",
        runtime: "_bmsign",
        // Matches Math.sign semantics (0 for zero, not signum's 1).
        js: "(x)=>Math.sign(x)",
        ssg: Ssg::Num(|a| {
            a.first().map(|&x| {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            })
        }),
    },
    Builtin {
        source: "math.hypot",
        runtime: "_bmhypot",
        js: "(...a)=>Math.hypot(...a)",
        ssg: Ssg::Num(|a| Some(a.iter().map(|x| x * x).sum::<f64>().sqrt())),
    },
    // ── str.* (deterministic text transforms → SSG-foldable) ──────────────────
    Builtin {
        source: "str.upper",
        runtime: "_bsupper",
        js: "(s)=>String(s).toUpperCase()",
        ssg: Ssg::Str(|a| a.first().map(|s| s.to_uppercase())),
    },
    Builtin {
        source: "str.lower",
        runtime: "_bslower",
        js: "(s)=>String(s).toLowerCase()",
        ssg: Ssg::Str(|a| a.first().map(|s| s.to_lowercase())),
    },
    Builtin {
        source: "str.capitalize",
        runtime: "_bscap",
        js: "(s)=>{s=String(s);return s?s[0].toUpperCase()+s.slice(1):s}",
        ssg: Ssg::Str(|a| {
            a.first().map(|s| {
                let mut c = s.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
        }),
    },
    Builtin {
        source: "str.trim",
        runtime: "_bstrim",
        js: "(s)=>String(s).trim()",
        ssg: Ssg::Str(|a| a.first().map(|s| s.trim().to_string())),
    },
    Builtin {
        source: "str.slugify",
        runtime: "_bsslug",
        js: "(s)=>String(s).toLowerCase().trim().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'')",
        ssg: Ssg::Str(|a| a.first().map(|s| slugify(s))),
    },
    Builtin {
        source: "str.truncate",
        runtime: "_bstrunc",
        // str.truncate(s, n) → first n chars, ellipsis if cut.
        js: "(s,n)=>{s=String(s);return s.length>n?s.slice(0,n)+'\\u2026':s}",
        ssg: Ssg::Str(|a| {
            let s = a.first()?;
            let n = a.get(1)?.trim().parse::<usize>().ok()?;
            let chars: Vec<char> = s.chars().collect();
            if chars.len() > n {
                Some(chars[..n].iter().collect::<String>() + "\u{2026}")
            } else {
                Some(s.clone())
            }
        }),
    },
    Builtin {
        source: "str.repeat",
        runtime: "_bsrep",
        js: "(s,n)=>String(s).repeat(Math.max(0,n|0))",
        ssg: Ssg::Str(|a| {
            let s = a.first()?;
            let n = a.get(1)?.trim().parse::<f64>().ok()?;
            Some(s.repeat(n.max(0.0) as usize))
        }),
    },
    // ── fmt.* (locale-aware via Intl → runtime-only, never SSG-folded) ─────────
    // These read the runtime `LOCALE` when i18n is configured and fall back to
    // the environment default otherwise, so build-time folding (which has no
    // reliable locale) is deliberately skipped — hence `Ssg::None`.
    Builtin {
        source: "fmt.number",
        runtime: "_bfnum",
        js: "(n)=>new Intl.NumberFormat(typeof LOCALE!=='undefined'?LOCALE:undefined).format(n)",
        ssg: Ssg::None,
    },
    Builtin {
        source: "fmt.currency",
        runtime: "_bfcur",
        js: "(n,c)=>new Intl.NumberFormat(typeof LOCALE!=='undefined'?LOCALE:undefined,{style:'currency',currency:c}).format(n)",
        ssg: Ssg::None,
    },
    Builtin {
        source: "fmt.percent",
        runtime: "_bfpct",
        js: "(n)=>new Intl.NumberFormat(typeof LOCALE!=='undefined'?LOCALE:undefined,{style:'percent'}).format(n)",
        ssg: Ssg::None,
    },
    Builtin {
        source: "fmt.date",
        runtime: "_bfdate",
        js: "(d)=>new Intl.DateTimeFormat(typeof LOCALE!=='undefined'?LOCALE:undefined,{dateStyle:'medium'}).format(new Date(d))",
        ssg: Ssg::None,
    },
    // ── arr.* (array helpers → runtime-only, arrays aren't SSG state) ──────────
    Builtin {
        source: "arr.sum",
        runtime: "_barsum",
        js: "(a)=>(a||[]).reduce((s,x)=>s+(+x||0),0)",
        ssg: Ssg::None,
    },
    Builtin {
        source: "arr.first",
        runtime: "_barfirst",
        js: "(a)=>(a||[])[0]",
        ssg: Ssg::None,
    },
    Builtin {
        source: "arr.last",
        runtime: "_barlast",
        js: "(a)=>{a=a||[];return a[a.length-1]}",
        ssg: Ssg::None,
    },
    Builtin {
        source: "arr.unique",
        runtime: "_baruniq",
        js: "(a)=>[...new Set(a||[])]",
        ssg: Ssg::None,
    },
    Builtin {
        source: "arr.sort",
        runtime: "_barsort",
        // Numeric-aware where both sides are numbers, lexicographic otherwise.
        js: "(a)=>[...(a||[])].sort((x,y)=>x>y?1:x<y?-1:0)",
        ssg: Ssg::None,
    },
    Builtin {
        source: "arr.reverse",
        runtime: "_barrev",
        js: "(a)=>[...(a||[])].reverse()",
        ssg: Ssg::None,
    },
    Builtin {
        source: "arr.join",
        runtime: "_barjoin",
        js: "(a,s)=>(a||[]).join(s)",
        ssg: Ssg::None,
    },
];

/// Lowercase, trim, collapse non-alphanumerics to single hyphens, strip edges.
/// Kept in sync with the `str.slugify` runtime JS above.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.trim().chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Rewrite `<ns>.<fn>(` source calls to their runtime helper names.
pub(crate) fn rewrite(expr: &str) -> String {
    let mut r = expr.to_string();
    for b in BUILTINS {
        let from = format!("{}(", b.source);
        if r.contains(&from) {
            r = r.replace(&from, &format!("{}(", b.runtime));
        }
    }
    r
}

/// Emit `const <runtime>=<js>;` for every builtin whose helper appears in `js`.
pub(crate) fn emit_used(js: &str) -> String {
    let mut out = String::new();
    for b in BUILTINS {
        if js.contains(&format!("{}(", b.runtime)) {
            out.push_str(&format!("const {}={};\n", b.runtime, b.js));
        }
    }
    out
}

/// Split a comma-separated argument list at the top level (ignoring commas
/// nested inside `()` / `[]`).
pub(crate) fn split_args(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut quote = 0u8; // 0 = outside a string, else the open quote byte
    let bytes = inner.as_bytes();
    for (i, &c) in bytes.iter().enumerate() {
        if quote != 0 {
            // Inside a string literal: only its matching, unescaped quote ends it.
            if c == quote && bytes[i - 1] != b'\\' {
                quote = 0;
            }
            continue;
        }
        match c {
            b'"' | b'\'' | b'`' => quote = c,
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => {
                args.push(inner[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= inner.len() {
        args.push(inner[start..].to_string());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_maps_source_to_runtime() {
        assert_eq!(rewrite("math.round(count)"), "_bmround(count)");
        assert_eq!(rewrite("math.clamp(x, 0, 10)"), "_bmclamp(x, 0, 10)");
        // Unknown members are left untouched.
        assert_eq!(rewrite("math.tau"), "math.tau");
    }

    #[test]
    fn emit_used_is_tree_shaken() {
        assert!(emit_used("y=_bmround(3)").contains("const _bmround="));
        assert!(!emit_used("y=_bmround(3)").contains("_bmclamp"));
        assert_eq!(emit_used("no builtins here"), "");
    }

    #[test]
    fn split_args_respects_nesting() {
        assert_eq!(split_args("a, b, c"), vec!["a", " b", " c"]);
        assert_eq!(split_args("f(a, b), c"), vec!["f(a, b)", " c"]);
        // Commas inside string literals are not argument separators.
        assert_eq!(
            split_args("\"Hello, World!\", 3"),
            vec!["\"Hello, World!\"", " 3"]
        );
    }
}
