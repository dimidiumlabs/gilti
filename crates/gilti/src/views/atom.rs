// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RefPath>,
    host: Option<&axum::http::HeaderValue>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo;
    let reference = route.params.reference;
    let path = route.params.path;
    let selection_path = path.clone();
    let model = tokio::task::spawn_blocking(move || {
        crate::models::history::History::load(
            std::path::Path::new(repositories),
            &name,
            crate::router::Revision::Ref(reference),
            crate::models::history::Options {
                path: selection_path,
                follow: false,
                search: crate::models::history::Search::None,
                offset: 0,
                limit: 10,
                graph: false,
                ignore_whitespace: false,
                include_statistics: false,
            },
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(crate::models::Error::Internal(error.to_string())),
    };
    let host = host
        .and_then(|host| host.to_str().ok())
        .filter(|host| !host.is_empty());
    let body = document(&model, path.as_deref(), host);
    super::bytes_response(
        "text/xml; charset=UTF-8",
        None,
        None,
        body.into_bytes(),
        &method,
    )
}

fn document(
    model: &crate::models::history::History,
    path: Option<&str>,
    host: Option<&str>,
) -> String {
    let repository = super::shared::repository_url(&model.repository.name);
    let reference = &model.revision;
    let mut feed = String::from("<feed xmlns='http://www.w3.org/2005/Atom'>\n<title>");
    text(&mut feed, &model.repository.name);
    if let Some(path) = path {
        feed.push('/');
        text(&mut feed, path);
    }
    feed.push_str(", branch ");
    text(&mut feed, reference);
    feed.push_str("</title>\n<subtitle>");
    text(&mut feed, &model.repository.description);
    feed.push_str("</subtitle>\n");
    let self_path = path.map_or_else(
        || {
            format!(
                "{repository}/+/{}/+/feed/atom",
                super::shared::encode_path(&model.revision)
            )
        },
        |path| {
            format!(
                "{repository}/+/{}/+/feed/atom/{}",
                super::shared::encode_path(&model.revision),
                super::shared::encode_path(path)
            )
        },
    );
    if let Some(host) = host {
        let absolute = |path: &str| format!("http://{host}{path}");
        element(&mut feed, "id", &absolute(&self_path));
        attribute(
            &mut feed,
            "link",
            &[('r', "self"), ('h', &absolute(&self_path))],
        );
        attribute(
            &mut feed,
            "link",
            &[
                ('r', "alternate"),
                ('t', "text/html"),
                ('h', &absolute(&repository)),
            ],
        );
    }
    if let Some(entry) = model.entries.first() {
        element(&mut feed, "updated", &timestamp(&entry.committer));
    }
    for entry in &model.entries {
        feed.push_str("<entry>\n<title>");
        text(&mut feed, &entry.subject);
        feed.push_str("</title>\n");
        element(&mut feed, "updated", &timestamp(&entry.committer));
        feed.push_str("<author>\n");
        if !entry.author.name.is_empty() {
            element(&mut feed, "name", &entry.author.name);
        }
        if !entry.author.email.is_empty() {
            element(&mut feed, "email", &entry.author.email);
        }
        feed.push_str("</author>\n");
        element(&mut feed, "published", &timestamp(&entry.author));
        if let Some(host) = host {
            attribute(
                &mut feed,
                "link",
                &[
                    ('r', "alternate"),
                    ('t', "text/html"),
                    ('h', &format!("http://{host}{repository}/+/{}", entry.oid)),
                ],
            );
        }
        element(&mut feed, "id", &urn(&entry.oid));
        feed.push_str("<content type='text'>\n");
        text(&mut feed, &entry.body);
        feed.push_str("</content>\n</entry>\n");
    }
    feed.push_str("</feed>\n");
    feed
}

fn element(output: &mut String, name: &str, value: &str) {
    output.push('<');
    output.push_str(name);
    output.push('>');
    text(output, value);
    output.push_str("</");
    output.push_str(name);
    output.push_str(">\n");
}

fn attribute(output: &mut String, name: &str, attributes: &[(char, &str)]) {
    output.push('<');
    output.push_str(name);
    for (key, value) in attributes {
        let key = match key {
            'r' => "rel",
            't' => "type",
            'h' => "href",
            _ => unreachable!(),
        };
        output.push(' ');
        output.push_str(key);
        output.push_str("='");
        attribute_value(output, value);
        output.push('\'');
    }
    output.push_str("/>\n");
}

fn text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            character if xml_character(character) => output.push(character),
            _ => output.push('\u{fffd}'),
        }
    }
}
fn attribute_value(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '\'' => output.push_str("&apos;"),
            '"' => output.push_str("&quot;"),
            character if xml_character(character) => output.push(character),
            _ => output.push('\u{fffd}'),
        }
    }
}

fn xml_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{a}' | '\u{d}')
        || ('\u{20}'..='\u{d7ff}').contains(&character)
        || ('\u{e000}'..='\u{fffd}').contains(&character)
        || character >= '\u{10000}'
}

fn urn(oid: &str) -> String {
    let algorithm = if oid.len() == 64 { "sha256" } else { "sha1" };
    format!("urn:{algorithm}:{oid}")
}

fn timestamp(identity: &crate::models::commit::Identity) -> String {
    let Some(value) = crate::models::time::utc(identity.timestamp) else {
        return "1970-01-01T00:00:00Z".to_owned();
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        value.tm_year + 1900,
        value.tm_mon + 1,
        value.tm_mday,
        value.tm_hour,
        value.tm_min,
        value.tm_sec
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_xml_and_identifies_sha256() {
        let mut output = String::new();
        super::text(&mut output, "<&>\0\u{1}\t");
        assert_eq!(output, "&lt;&amp;&gt;��\t");
        assert_eq!(
            super::urn(&"a".repeat(40)),
            format!("urn:sha1:{}", "a".repeat(40))
        );
        assert_eq!(
            super::urn(&"b".repeat(64)),
            format!("urn:sha256:{}", "b".repeat(64))
        );
        assert_eq!(
            super::timestamp(&crate::models::commit::Identity {
                name: String::new(),
                email: String::new(),
                timestamp: 0,
                offset_minutes: 330
            }),
            "1970-01-01T00:00:00Z"
        );
    }
}
