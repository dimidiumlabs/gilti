// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

#[allow(dead_code)]
pub mod classes {
    include!(concat!(env!("OUT_DIR"), "/css_modules.rs"));
}

pub const STYLESHEET: &str = include_str!(concat!(env!("OUT_DIR"), "/stylesheet.css"));

#[cfg(test)]
mod tests {
    #[test]
    fn styles_use_a_single_global_foundation_without_global_module_escapes() {
        let foundation = include_str!("styles/foundation.css");
        assert!(foundation.contains("body"));
        for module in [
            include_str!("components/document.module.css"),
            include_str!("components/header.module.css"),
            include_str!("components/layout.module.css"),
            include_str!("components/relative_time.module.css"),
            include_str!("components/table.module.css"),
            include_str!("components/tabs.module.css"),
            include_str!("endpoints/blame.module.css"),
            include_str!("endpoints/diff.module.css"),
            include_str!("endpoints/log.module.css"),
            include_str!("endpoints/overview.module.css"),
            include_str!("endpoints/refs.module.css"),
            include_str!("endpoints/repositories.module.css"),
            include_str!("endpoints/revision.module.css"),
            include_str!("endpoints/stats.module.css"),
            include_str!("endpoints/tag.module.css"),
            include_str!("endpoints/tree.module.css"),
        ] {
            assert!(!module.contains(":global("));
        }
    }

    #[test]
    fn generated_stylesheet_contains_foundation_and_scoped_selectors() {
        assert!(super::STYLESHEET.contains("body"));
        assert!(super::STYLESHEET.contains("_root"));
    }

    #[test]
    fn relative_time_styles_are_scoped_by_root_and_unit() {
        let css = include_str!("components/relative_time.module.css");
        assert!(css.contains(".root[data-unit=\"minutes\"]"));
        assert!(css.contains(".root[data-unit=\"years\"]"));
        for color in ["#080", "#040", "#444", "#888", "#bbb"] {
            assert!(css.contains(color));
        }
        assert!(!css.contains(":global("));
    }

    #[test]
    fn list_row_css_bindings_do_not_leak_to_endpoints() {
        let table = include_str!("components/table.rs");
        assert!(!table.contains("static_row()"));
        for endpoint in [
            include_str!("endpoints/log.rs"),
            include_str!("endpoints/tree.rs"),
            include_str!("endpoints/overview.rs"),
            include_str!("endpoints/repositories.rs"),
            include_str!("endpoints/refs.rs"),
        ] {
            assert!(!endpoint.contains("DataTable::static_row"));
            assert!(!endpoint.contains("log::LOGHEADER"));
            assert!(!endpoint.contains("log::NOHOVER_HIGHLIGHT"));
        }
    }
}
