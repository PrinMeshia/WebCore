//! SSG (Static Site Generation) tests.

#[cfg(test)]
use super::*;

#[test]
fn golden_ssg_interpolation_prerendered() {
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 7 }
    view { p "Valeur : {count}" }
}
page "home" { Counter {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let html = generate_html(&doc, "home", &opts()).expect("codegen").html;
    let initial = crate::core::ssg::build_initial_state(&doc);
    let ssg = crate::core::ssg::apply_ssg_with_locales(&html, &initial, &HashMap::new(), "");
    assert!(
        ssg.contains(&format!("{}=\"count\">7</span>", attr_names::INTERPOLATION)),
        "interpolation span not pre-rendered:\n{}",
        ssg
    );
}

#[test]
fn golden_ssg_if_display_preset() {
    let src = r#"
layout MainLayout { main { slot content } }
component Widget {
    state { show: Number = 1 }
    view {
        @if show > 0 {
            p "Visible"
        } @else {
            p "Hidden"
        }
    }
}
page "home" { Widget {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let html = generate_html(&doc, "home", &opts()).expect("codegen").html;
    let initial = crate::core::ssg::build_initial_state(&doc);
    let ssg = crate::core::ssg::apply_ssg_with_locales(&html, &initial, &HashMap::new(), "");
    assert!(
        ssg.contains(&format!("{}=\"show &gt; 0\"", attr_names::IF)) && ssg.contains(r#"style="display:block""#),
        "@if branch not pre-rendered as visible:\n{}",
        ssg
    );
    assert!(
        ssg.contains(&format!("{}=\"show &gt; 0\"", attr_names::IF_ELSE)) && ssg.contains(r#"style="display:none""#),
        "@else branch not pre-rendered as hidden:\n{}",
        ssg
    );
}
