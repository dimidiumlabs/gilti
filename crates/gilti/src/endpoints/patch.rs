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
    let repositories = std::sync::Arc::clone(&context.repositories);
    let name = route.repo.clone();
    let old = route.params.old_rev.clone();
    let new = route.params.new_rev.clone();
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::patch::Patch::load(repositories.as_path(), &name, &old, &new)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    let output = match gilti_git::commands::format_patch(
        &context.git,
        &model.repository_path,
        &model.old_oid,
        &model.new_oid,
        route.params.path.as_deref(),
    )
    .await
    {
        Ok(output) => output,
        Err(error) => return super::error(error),
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
