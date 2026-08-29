// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Comparison>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo.clone();
    let old = route.params.old_rev.clone();
    let new = route.params.new_rev.clone();
    let model = tokio::task::spawn_blocking(move || {
        crate::models::patch::Patch::load(std::path::Path::new(repositories), &name, &old, &new)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(crate::models::Error::Internal(error.to_string())),
    };
    let mut command = tokio::process::Command::new(crate::GIT);
    command
        .arg("--git-dir")
        .arg(&model.repository_path)
        .arg("-c")
        .arg("color.ui=false")
        .arg("format-patch")
        .arg("--stdout")
        .arg("--keep-subject")
        .arg("--no-renames")
        .arg("--signature=Gilti")
        .arg(format!("{}..{}", model.old_oid, model.new_oid))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(path) = &route.params.path {
        command.arg("--").arg(format!(":(literal){path}"));
    }
    let output = match command.output().await {
        Ok(output) if output.status.success() => output.stdout,
        Ok(output) => {
            eprintln!(
                "gilti: git format-patch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return super::error(crate::models::Error::Internal(
                "git format-patch failed".to_owned(),
            ));
        }
        Err(error) => return super::error(crate::models::Error::Internal(error.to_string())),
    };
    let filename = format!("{}..{}.patch", model.old_oid, model.new_oid);
    super::bytes_response(
        "text/plain; charset=UTF-8",
        Some(format!("inline; filename=\"{filename}\"")),
        None,
        output,
        &method,
    )
}
