// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, PreEscaped, html};
use serde::Deserialize;

pub const PRIVATE_CONTENT_TYPE: &str = "application/vnd.gilti.repolist+json";

#[derive(Deserialize)]
struct RepoList {
    page: String,
    title: String,
    root_desc: String,
    root_url: String,
    about_url: String,
    noheader: bool,
    search: String,
    current_url: String,
    root_readme: bool,
    owner_enabled: bool,
    links_enabled: bool,
    section_grouping: bool,
    shell: Shell,
    sort_urls: SortUrls,
    rows: Vec<Row>,
    pager: Vec<Pager>,
}

#[derive(Deserialize)]
struct Shell {
    embedded: bool,
    robots: String,
    css: Vec<String>,
    js: Vec<String>,
    favicon: String,
    head_include: Option<String>,
    header: Option<String>,
    footer_configured: bool,
    footer: Option<String>,
    logo: String,
    logo_link: String,
    cgit_version: String,
    git_version: String,
    generated_at: String,
}

#[derive(Deserialize)]
struct SortUrls {
    name: String,
    desc: String,
    owner: String,
    idle: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Row {
    Section(Section),
    Repo(Box<Repo>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
    section: Option<String>,
}

#[derive(Deserialize)]
struct Repo {
    name: String,
    section: Option<String>,
    url: String,
    description: Description,
    owner: String,
    owner_url: String,
    idle: Option<Age>,
    log_url: String,
    tree_url: String,
}

#[derive(Deserialize)]
struct Description {
    text: String,
    truncated: bool,
}

#[derive(Deserialize)]
struct Age {
    timestamp: i64,
    title: String,
    unit: String,
    amount: f64,
}

#[derive(Deserialize)]
struct Pager {
    url: String,
    current: bool,
}

pub fn render(body: &[u8]) -> Result<Markup, serde_json::Error> {
    let page: RepoList = serde_json::from_slice(body)?;
    if page.page != "repolist" {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unknown private page",
        )));
    }
    Ok(render_repolist(page))
}

fn include(content: &Option<String>) -> Markup {
    PreEscaped(content.clone().unwrap_or_default())
}

fn render_repolist(page: RepoList) -> Markup {
    let shell = &page.shell;
    html! {
        @if shell.embedded {
            (include(&shell.header))
        } @else {
            (maud::DOCTYPE)
            html lang="en" {
                head {
                    title { (&page.title) }
                    meta name="generator" content=(format!("cgit {}", shell.cgit_version));

                    @if !shell.robots.is_empty() {
                        meta name="robots" content=(&shell.robots);
                    }

                    @if shell.css.is_empty() {
                        link rel="stylesheet" type="text/css" href="/cgit.css";
                    }

                    @for css in &shell.css {
                        @if !css.is_empty() {
                            link rel="stylesheet" type="text/css" href=(css);
                        }
                    }

                    @if shell.js.is_empty() {
                        script type="text/javascript" src="/cgit.js" {}
                    }

                    @for js in &shell.js {
                        @if !js.is_empty() {
                            script type="text/javascript" src=(js) {}
                        }
                    }

                    @if !shell.favicon.is_empty() {
                        link rel="shortcut icon" href=(&shell.favicon);
                    }

                    (include(&shell.head_include))
                }
                body {
                    (include(&shell.header))
                    (page_content(&page, true))
                }
            }
        }
        @if shell.embedded {
            (page_content(&page, false))
            (footer(shell, true))
        }
    }
}

fn page_content(page: &RepoList, footer_inside: bool) -> Markup {
    let shell = &page.shell;
    let has_repos = page.rows.iter().any(|row| matches!(row, Row::Repo(_)));
    html! {
        div id="cgit" {
            @if !page.noheader {
                table id="header" {
                    tr {
                        @if !shell.logo.is_empty() {
                            td class="logo" rowspan="2" {
                                a href=(if shell.logo_link.is_empty() { &page.root_url } else { &shell.logo_link }) {
                                    img src=(&shell.logo) alt="cgit logo";
                                }
                            }
                        }
                        td class="main" { (&page.title) }
                    }
                    tr { td class="sub" { (&page.root_desc) } }
                }
            }
            table class="tabs" { tr {
                td {
                    a href=(&page.root_url) class="active" { "index" }
                    @if page.root_readme { a href=(&page.about_url) { "about" } }
                }
                td class="form" {
                    form method="get" action=(&page.current_url) {
                        input type="search" name="q" size="10" value=(&page.search);
                        input type="submit" value="search";
                    }
                }
            }}
            div class="content" {
                table summary="repository list" class="list nowrap" {
                    @if has_repos { tr class="nohover" {
                        th class="left" { a href=(&page.sort_urls.name) { "Name" } }
                        th class="left" { a href=(&page.sort_urls.desc) { "Description" } }
                        @if page.owner_enabled { th class="left" { a href=(&page.sort_urls.owner) { "Owner" } } }
                        th class="left" { a href=(&page.sort_urls.idle) { "Idle" } }
                        @if page.links_enabled { th class="left" { "Links" } }
                    }}
                    @for row in &page.rows {
                        @match row {
                            Row::Section(section) => tr class="nohover-highlight" {
                                td colspan=(3 + usize::from(page.owner_enabled) + usize::from(page.links_enabled)) class="reposection" { (section.section.as_deref().unwrap_or("")) }
                            },
                            Row::Repo(repo) => tr {
                                td class=(if page.section_grouping && repo.section.is_some() { "sublevel-repo" } else { "toplevel-repo" }) { a href=(&repo.url) { (&repo.name) } }
                                td { a href=(&repo.url) { (&repo.description.text) @if repo.description.truncated { "..." } } }
                                @if page.owner_enabled { td { a href=(&repo.owner_url) { (&repo.owner) } } }
                                td { @if let Some(age) = &repo.idle { span class=(if age.unit == "minutes" { "age-mins".to_owned() } else { format!("age-{}", age.unit) }) data-ut=(age.timestamp) title=(&age.title) { (format!("{:.0}", age.amount)) " " (if age.unit == "minutes" { "min." } else { &age.unit }) } } }
                                @if page.links_enabled { td { a class="button" href=(&repo.url) { "summary" } a class="button" href=(&repo.log_url) { "log" } a class="button" href=(&repo.tree_url) { "tree" } } }
                            },
                        }
                    }
                }
                @if !page.pager.is_empty() { ul class="pager" { @for (index, item) in page.pager.iter().enumerate() { li { a href=(&item.url) class=[item.current.then_some("current")] title=(format!("Page {}", index + 1)) { "[" (index + 1) "]" } } } } }
            }
            @if footer_inside { (footer(&page.shell, false)) }
        }
    }
}

fn footer(shell: &Shell, embedded: bool) -> Markup {
    if let Some(footer) = &shell.footer {
        return PreEscaped(footer.clone());
    }
    if embedded || shell.footer_configured {
        return Markup::default();
    }
    html! { div class="footer" { "generated by " a href="https://git.zx2c4.com/cgit/about/" { "cgit " (&shell.cgit_version) } " (" a href="https://git-scm.com/" { "git " (&shell.git_version) } ") at " (&shell.generated_at) } }
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_shell_and_escapes_data() {
        let markup = super::render(br#"{"page":"repolist","title":"Gilti","root_desc":"","root_url":"/","about_url":"/?p=about","noheader":false,"search":"<x>","current_url":"/","root_readme":true,"owner_enabled":true,"links_enabled":true,"section_grouping":true,"shell":{"embedded":false,"robots":"noindex","css":["/custom.css"],"js":["/custom.js"],"favicon":"/favicon.ico","head_include":"<meta name='x'>","header":"ignored","footer_configured":false,"footer":null,"logo":"/cgit.png","logo_link":"","cgit_version":"v1","git_version":"2","generated_at":"now"},"sort_urls":{"name":"/?s=name","desc":"/?s=desc","owner":"/?s=owner","idle":"/?s=idle"},"rows":[{"name":"<repo>","section":"group","url":"/?url=repo","description":{"text":"x&y","truncated":true},"owner":"o","owner_url":"/?q=o","idle":{"timestamp":1,"title":"date","unit":"years","amount":1},"log_url":"/?url=repo/log/&showmsg=1","tree_url":"/?url=repo/tree/"}],"pager":[]}"#).unwrap().into_string();
        assert!(markup.contains("&lt;repo&gt;"));
        assert!(markup.contains("x&amp;y..."));
        assert!(markup.contains("href=\"/?url=repo/log/&amp;showmsg=1\""));
        assert!(markup.contains("/custom.css"));
        assert!(markup.contains("cgit.png"));
        assert!(markup.contains("name='x'"));
    }

    #[test]
    fn empty_rows_have_no_column_header() {
        let markup = super::render(br#"{"page":"repolist","title":"Gilti","root_desc":"","root_url":"/","about_url":"/?p=about","noheader":true,"search":"","current_url":"/","root_readme":false,"owner_enabled":false,"links_enabled":false,"section_grouping":false,"shell":{"embedded":false,"robots":"","css":[],"js":[],"favicon":"","head_include":null,"header":null,"footer_configured":false,"footer":null,"logo":"","logo_link":"","cgit_version":"v1","git_version":"2","generated_at":"now"},"sort_urls":{"name":"/?s=name","desc":"/?s=desc","owner":"/?s=owner","idle":"/?s=idle"},"rows":[],"pager":[{"url":"/?ofs=0","current":false}]}"#).unwrap().into_string();
        assert!(!markup.contains("<tr class=\"nohover\">"));
    }

    #[test]
    fn footer_respects_embedded_and_unreadable_include_modes() {
        let mut shell = super::Shell {
            embedded: false,
            robots: String::new(),
            css: Vec::new(),
            js: Vec::new(),
            favicon: String::new(),
            head_include: None,
            header: None,
            footer_configured: false,
            footer: None,
            logo: String::new(),
            logo_link: String::new(),
            cgit_version: "v1".into(),
            git_version: "2".into(),
            generated_at: "now".into(),
        };
        assert!(super::footer(&shell, true).into_string().is_empty());
        shell.footer_configured = true;
        assert!(super::footer(&shell, false).into_string().is_empty());
        shell.footer = Some("<footer>configured</footer>".into());
        assert!(
            super::footer(&shell, true)
                .into_string()
                .contains("<footer>configured</footer>")
        );
    }

    #[test]
    fn rejects_invalid_model() {
        assert!(super::render(br#"{"page":"repolist"}"#).is_err());
    }
}
