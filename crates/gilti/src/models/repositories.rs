// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct Repositories {
    pub rows: Vec<Repository>,
    pub offset: usize,
    pub has_next: bool,
}

pub struct Repository {
    pub name: String,
    pub description: String,
    pub timestamp: Option<i64>,
    pub populated: bool,
}

#[derive(Clone, Copy)]
pub enum Sort {
    Name,
    Description,
    Owner,
    Idle,
}

pub struct Filter {
    pub search: Option<String>,
    pub sort: Sort,
    pub offset: usize,
    pub limit: usize,
}

impl Repositories {
    pub fn load(root: &Path, filter: Filter) -> Result<Self, super::Error> {
        let mut rows = Vec::new();
        scan(root, root, &mut rows)?;
        if let Some(search) = filter.search.as_deref() {
            let search = search.to_lowercase();
            rows.retain(|repository| {
                repository.name.to_lowercase().contains(&search)
                    || repository.description.to_lowercase().contains(&search)
            });
        }
        if rows.is_empty() {
            return Err(super::Error::NotFound);
        }
        match filter.sort {
            Sort::Name | Sort::Owner => rows.sort_by(|left, right| left.name.cmp(&right.name)),
            Sort::Description => rows.sort_by(|left, right| {
                left.description
                    .cmp(&right.description)
                    .then_with(|| left.name.cmp(&right.name))
            }),
            Sort::Idle => rows.sort_by(|left, right| {
                right
                    .timestamp
                    .cmp(&left.timestamp)
                    .then_with(|| left.name.cmp(&right.name))
            }),
        }
        let has_next = rows.len() > filter.offset.saturating_add(filter.limit);
        let rows = rows
            .into_iter()
            .skip(filter.offset)
            .take(filter.limit)
            .collect();
        Ok(Self {
            rows,
            offset: filter.offset,
            has_next,
        })
    }
}

fn scan(root: &Path, directory: &Path, rows: &mut Vec<Repository>) -> Result<(), super::Error> {
    let entries =
        std::fs::read_dir(directory).map_err(|error| super::Error::Internal(error.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|error| super::Error::Internal(error.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| super::Error::Internal(error.to_string()))?;
        if !file_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if let Some(name) = repository_name(root, &path) {
            if let Some(repository) = load(&path, name) {
                rows.push(repository);
            }
        } else {
            scan(root, &path, rows)?;
        }
    }
    Ok(())
}

fn repository_name<'a>(root: &Path, path: &'a Path) -> Option<&'a str> {
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    relative.strip_suffix(".git")
}

fn load(path: &Path, name: &str) -> Option<Repository> {
    let repository = git2::Repository::open_bare(path).ok()?;
    let description = std::fs::read_to_string(path.join("description"))
        .ok()
        .map(|description| description.trim().to_owned())
        .filter(|description| !description.is_empty())
        .unwrap_or_else(|| "[no description]".to_owned());
    let commit = repository
        .head()
        .and_then(|head| head.peel_to_commit())
        .ok();
    Some(Repository {
        name: name.to_owned(),
        description,
        timestamp: commit.as_ref().map(|commit| commit.time().seconds()),
        populated: commit.is_some(),
    })
}

#[cfg(test)]
mod tests {
    fn filter(search: Option<&str>) -> super::Filter {
        super::Filter {
            search: search.map(str::to_owned),
            sort: super::Sort::Name,
            offset: 0,
            limit: 50,
        }
    }

    #[test]
    fn scans_nested_and_terminal_dot_git_names_and_filters() {
        let root =
            std::env::temp_dir().join(format!("gilti-repositories-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("group")).unwrap();
        git2::Repository::init_bare(root.join("group/project.git")).unwrap();
        git2::Repository::init_bare(root.join("group/dotted.git.git")).unwrap();
        git2::Repository::init_bare(root.join("empty.git")).unwrap();
        std::fs::write(
            root.join("group/project.git/description"),
            "Project description\n",
        )
        .unwrap();

        let repositories = super::Repositories::load(&root, filter(None)).unwrap();
        assert_eq!(
            repositories
                .rows
                .iter()
                .map(|repository| repository.name.as_str())
                .collect::<Vec<_>>(),
            ["empty", "group/dotted.git", "group/project"]
        );
        assert!(
            repositories
                .rows
                .iter()
                .all(|repository| !repository.populated)
        );
        let repositories =
            super::Repositories::load(&root, filter(Some("Project description"))).unwrap();
        assert_eq!(repositories.rows.len(), 1);
        assert_eq!(repositories.rows[0].name, "group/project");
        assert!(matches!(
            super::Repositories::load(&root, filter(Some("missing"))),
            Err(crate::models::Error::NotFound)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
