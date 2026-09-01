// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

#[allow(dead_code)]
pub mod classes {
    include!(concat!(env!("OUT_DIR"), "/css_modules.rs"));
}

pub const STYLESHEET: &str = include_str!(concat!(env!("OUT_DIR"), "/stylesheet.css"));

#[cfg(test)]
mod tests {
    fn component_styles() -> [&'static str; 16] {
        [
            include_str!("components/code_block.module.css"),
            include_str!("components/diff.module.css"),
            include_str!("components/key_value.module.css"),
            include_str!("components/layout.module.css"),
            include_str!("components/log_table.module.css"),
            include_str!("components/refs_table.module.css"),
            include_str!("components/relative_time.module.css"),
            include_str!("components/table.module.css"),
            include_str!("endpoints/blame.module.css"),
            include_str!("endpoints/log.module.css"),
            include_str!("endpoints/overview.module.css"),
            include_str!("endpoints/repositories.module.css"),
            include_str!("endpoints/revision.module.css"),
            include_str!("endpoints/stats.module.css"),
            include_str!("endpoints/tag.module.css"),
            include_str!("endpoints/tree.module.css"),
        ]
    }

    #[test]
    fn application_styles_are_component_scoped_without_global_escapes() {
        let layout = include_str!("components/layout.module.css");
        assert!(layout.contains("max-width: 900px"));
        assert!(layout.contains("background: var(--color-neutral-0)"));
        assert!(layout.contains("grid-template-areas"));
        assert!(layout.contains(".navigation-link[data-active=true]"));
        assert!(!layout.contains("body"));
        for module in component_styles() {
            assert!(!module.contains(":global("));
        }
    }

    #[test]
    fn generated_stylesheet_separates_palette_and_scoped_components() {
        assert!(!super::STYLESHEET.contains("body{"));
        assert!(super::STYLESHEET.starts_with("@layer global,components;"));
        assert!(super::STYLESHEET.contains("@layer global{:root{--color-"));
        assert!(super::STYLESHEET.contains("@layer components{"));
        assert!(super::STYLESHEET.contains("_root"));
    }

    #[test]
    fn component_colors_come_only_from_the_global_palette() {
        let palette = include_str!("styles/palette.css");
        assert!(palette.contains(":root"));
        assert!(palette.contains("--color-neutral-0: #fff"));

        for module in component_styles() {
            for literal in [
                "#",
                "rgb(",
                "rgba(",
                "hsl(",
                "hsla(",
                "oklab(",
                "oklch(",
                "lab(",
                "lch(",
                "color-mix(",
            ] {
                assert!(!module.contains(literal), "found {literal} in {module}");
            }
            for token in module.split(|character: char| {
                !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            }) {
                assert!(
                    ![
                        "aqua", "black", "blue", "brown", "cyan", "fuchsia", "gray", "green",
                        "grey", "lime", "magenta", "maroon", "navy", "olive", "orange", "pink",
                        "purple", "red", "silver", "teal", "white", "yellow",
                    ]
                    .contains(&token),
                    "found named color {token} in {module}"
                );
            }

            let mut remaining = module;
            while let Some(offset) = remaining.find("--color-") {
                let reference = &remaining[offset..];
                let end = reference
                    .find(|character: char| {
                        !(character.is_ascii_alphanumeric() || character == '-')
                    })
                    .unwrap_or(reference.len());
                let name = &reference[..end];
                assert!(
                    palette.contains(&format!("{name}:")),
                    "undefined palette variable {name}"
                );
                remaining = &reference[end..];
            }
        }
    }

    #[test]
    fn layout_preserves_shared_typographic_rhythm() {
        let layout = include_str!("components/layout.module.css");
        assert!(!layout.contains(".content *"));
        assert!(!layout.contains("font-size: 11pt"));
    }

    #[test]
    fn sidebar_layout_is_shared_and_endpoint_styles_do_not_reimplement_it() {
        let layout = include_str!("components/layout.module.css");
        assert!(layout.contains(".sidebar"));
        assert!(layout.contains(".sidebar form"));
        assert!(layout.contains("display: grid"));

        for module in [
            include_str!("components/diff.module.css"),
            include_str!("endpoints/stats.module.css"),
        ] {
            assert!(!module.contains(".panel"));
            assert!(!module.contains(".label"));
            assert!(!module.contains(".ctrl"));
        }
    }

    #[test]
    fn navigation_search_stays_in_one_flexible_row() {
        let css = include_str!("components/layout.module.css");
        assert!(css.contains(".navigation-search form"));
        assert!(css.contains("display: flex"));
        assert!(css.contains("input[type=\"search\"]"));
        assert!(css.contains("flex: 0 1 20em"));
        assert!(css.contains("width: auto"));
    }

    #[test]
    fn relative_time_styles_are_scoped_by_root_and_unit() {
        let css = include_str!("components/relative_time.module.css");
        assert!(css.contains(".root[data-unit=\"minutes\"]"));
        assert!(css.contains(".root[data-unit=\"years\"]"));
        for color in [
            "--color-green-550",
            "--color-green-900",
            "--color-neutral-700",
            "--color-neutral-500",
            "--color-neutral-300",
        ] {
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
