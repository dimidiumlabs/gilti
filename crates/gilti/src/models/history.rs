// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::BTreeMap, path::Path, process::Command};

pub const LOG_PAGE_SIZE: usize = 50;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Search {
    None,
    Grep(String),
    Author(String),
    Committer(String),
    Range(String),
}

pub struct Options {
    pub path: Option<String>,
    pub follow: bool,
    pub search: Search,
    pub offset: usize,
    pub limit: usize,
    pub graph: bool,
    pub ignore_whitespace: bool,
    pub include_statistics: bool,
}

pub struct History {
    pub repository: super::repository::Info,
    pub revision: String,
    pub entries: Vec<Entry>,
    pub has_next: bool,
    pub graph: bool,
}

pub struct Entry {
    pub oid: String,
    pub subject: String,
    pub body: String,
    pub author: super::commit::Identity,
    pub committer: super::commit::Identity,
    pub decorations: Vec<super::commit::Decoration>,
    pub notes: Option<String>,
    pub files: usize,
    pub additions: usize,
    pub deletions: usize,
    pub graph: String,
    pub graph_continuations: Vec<String>,
    pub path: Option<String>,
}

impl History {
    pub fn load(
        root: &Path,
        name: &str,
        revision: crate::router::Revision,
        options: Options,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let resolved = super::revision::commit(&repository, &revision)?;
        let revision_name = super::revision::selector(&revision);
        if let Search::Range(range) = &options.search {
            for selector in range
                .split_ascii_whitespace()
                .flat_map(|selector| selector.split(".."))
            {
                let selector = if selector == "HEAD" {
                    crate::router::Revision::Head
                } else if selector.starts_with("refs/") {
                    crate::router::Revision::Ref(selector.to_owned())
                } else {
                    crate::router::Revision::Commit(selector.to_owned())
                };
                super::revision::commit(&repository, &selector)?;
            }
        }
        let repository_path = super::repository::path(root, name)?;
        let follow = options.follow && options.path.is_some();
        let oids = select(
            &repository_path,
            resolved.id().to_string(),
            &options,
            follow,
        )?;
        let has_next = oids.len() > options.limit;
        let selected = &oids[..oids.len().min(options.limit)];
        let git_statistics = if !options.include_statistics {
            Some(BTreeMap::new())
        } else if follow {
            None
        } else {
            Some(statistics_from_git(
                &repository_path,
                selected,
                options.path.as_deref(),
                options.ignore_whitespace,
            )?)
        };
        let mut decorations = decorations(&repository)?;
        let mut entries = Vec::with_capacity(selected.len());
        let mut path = options.path.clone();
        for selected in selected {
            let oid = git2::Oid::from_str_ext(&selected.oid, repository.object_format())
                .map_err(|_| super::Error::NotFound)?;
            let commit = repository
                .find_commit(oid)
                .map_err(super::Error::from_git)?;
            let (files, additions, deletions) = match &git_statistics {
                Some(statistics) => statistics.get(&selected.oid).copied().unwrap_or_default(),
                None => statistics_from_git2(
                    &repository,
                    &commit,
                    path.as_deref(),
                    options.ignore_whitespace,
                )?,
            };
            let message = String::from_utf8_lossy(commit.message_bytes());
            let subject =
                String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default()).into_owned();
            let body = message
                .split_once('\n')
                .map_or("", |(_, body)| body.trim_start_matches('\n'))
                .to_owned();
            let notes = repository
                .find_note(None, oid)
                .ok()
                .map(|note| String::from_utf8_lossy(note.message_bytes()).into_owned());
            entries.push(Entry {
                oid: selected.oid.clone(),
                subject,
                body,
                author: identity(commit.author()),
                committer: identity(commit.committer()),
                decorations: decorations.remove(&oid).unwrap_or_default(),
                notes,
                files,
                additions,
                deletions,
                graph: selected.graph.clone(),
                graph_continuations: selected.graph_continuations.clone(),
                path: path.clone(),
            });
            if follow {
                path = previous_path(&repository, &commit, path)?;
            }
        }
        Ok(Self {
            repository: info,
            revision: revision_name,
            entries,
            has_next,
            graph: options.graph && !follow,
        })
    }
}

struct Selected {
    oid: String,
    graph: String,
    graph_continuations: Vec<String>,
}

fn git(path: &Path) -> Command {
    let mut command = Command::new(crate::GIT);
    command
        .env_clear()
        .env("HOME", "/var/empty")
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .arg("--no-replace-objects")
        .arg("--git-dir")
        .arg(path)
        .arg("-c")
        .arg("core.hooksPath=/dev/null")
        .arg("-c")
        .arg("color.ui=false")
        .arg("--no-pager");
    command
}

fn select(
    path: &Path,
    revision: String,
    options: &Options,
    follow: bool,
) -> Result<Vec<Selected>, super::Error> {
    let mut command = git(path);
    command
        .arg("log")
        .arg("--no-ext-diff")
        .arg("--no-textconv")
        .arg("--no-color")
        .arg("--format=format:%x1e%H%x1f")
        .arg(format!("--max-count={}", options.limit.saturating_add(1)))
        .arg(format!("--skip={}", options.offset));
    if options.graph && !follow {
        command.arg("--graph");
    }
    if options.ignore_whitespace {
        command.arg("--ignore-all-space");
    }
    if follow {
        command.arg("--follow");
    }
    match &options.search {
        Search::None => {
            command.arg(revision);
        }
        Search::Grep(value) => {
            command
                .arg("--regexp-ignore-case")
                .arg(format!("--grep={value}"))
                .arg(revision);
        }
        Search::Author(value) => {
            command
                .arg("--regexp-ignore-case")
                .arg(format!("--author={value}"))
                .arg(revision);
        }
        Search::Committer(value) => {
            command
                .arg("--regexp-ignore-case")
                .arg(format!("--committer={value}"))
                .arg(revision);
        }
        Search::Range(value) => {
            for selector in value.split_ascii_whitespace() {
                command.arg(selector);
            }
        }
    }
    if let Some(path) = &options.path {
        command.arg("--").arg(path);
    }
    let output = command
        .output()
        .map_err(|error| super::Error::Internal(error.to_string()))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(super::Error::Internal(format!("git log failed: {error}")));
    }
    parse(&output.stdout)
}

fn parse(bytes: &[u8]) -> Result<Vec<Selected>, super::Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| super::Error::Internal("git log emitted invalid UTF-8".to_owned()))?;
    let mut result = Vec::new();
    let mut prefix_start = 0;
    while let Some(relative) = text[prefix_start..].find('\x1e') {
        let marker = prefix_start + relative;
        let graph_lines = text[prefix_start..marker]
            .strip_prefix('\n')
            .unwrap_or(&text[prefix_start..marker])
            .split_terminator('\n')
            .collect::<Vec<_>>();
        let (graph, graph_continuations) =
            graph_lines
                .split_last()
                .map_or(("", Vec::new()), |(graph, continuations)| {
                    (
                        *graph,
                        continuations
                            .iter()
                            .map(|line| (*line).to_owned())
                            .collect(),
                    )
                });
        let remainder = &text[marker + 1..];
        let (oid, _) = remainder.split_once('\x1f').ok_or_else(|| {
            super::Error::Internal("git log emitted an invalid record".to_owned())
        })?;
        let oid = oid.trim();
        if !(matches!(oid.len(), 40 | 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit())) {
            return Err(super::Error::Internal(
                "git log emitted an invalid object id".to_owned(),
            ));
        }
        result.push(Selected {
            oid: oid.to_owned(),
            graph: graph.to_owned(),
            graph_continuations,
        });
        prefix_start = marker + 1 + oid.len() + 1;
    }
    Ok(result)
}

fn decorations(
    repository: &git2::Repository,
) -> Result<BTreeMap<git2::Oid, Vec<super::commit::Decoration>>, super::Error> {
    let mut result = BTreeMap::new();
    if let Ok(head) = repository.head()
        && let Ok(commit) = head.peel_to_commit()
    {
        result
            .entry(commit.id())
            .or_insert_with(Vec::new)
            .push(super::commit::Decoration {
                label: "HEAD".to_owned(),
                reference: None,
            });
    }
    let references = repository.references().map_err(super::Error::from_git)?;
    for reference in references.flatten() {
        let Ok(name) = reference.name() else {
            continue;
        };
        if !name.starts_with("refs/heads/") && !name.starts_with("refs/tags/") {
            continue;
        }
        let Ok(commit) = reference.peel_to_commit() else {
            continue;
        };
        let label = name
            .strip_prefix("refs/heads/")
            .or_else(|| name.strip_prefix("refs/tags/"))
            .unwrap_or(name);
        result
            .entry(commit.id())
            .or_insert_with(Vec::new)
            .push(super::commit::Decoration {
                label: label.to_owned(),
                reference: Some(name.to_owned()),
            });
    }
    Ok(result)
}

fn statistics_from_git(
    repository: &Path,
    selected: &[Selected],
    path: Option<&str>,
    ignore_whitespace: bool,
) -> Result<BTreeMap<String, (usize, usize, usize)>, super::Error> {
    if selected.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut command = git(repository);
    command
        .arg("log")
        .arg("--no-walk=unsorted")
        .arg("--root")
        .arg("--diff-merges=first-parent")
        .arg("--no-ext-diff")
        .arg("--no-textconv")
        .arg("--find-renames")
        .arg("--numstat")
        .arg("-z")
        .arg("--format=format:GILTI-STATS%x00%H%x00");
    if ignore_whitespace {
        command.arg("--ignore-all-space");
    }
    for entry in selected {
        command.arg(&entry.oid);
    }
    if let Some(path) = path {
        command.arg("--").arg(path);
    }
    let output = command
        .output()
        .map_err(|error| super::Error::Internal(error.to_string()))?;
    if !output.status.success() {
        return Err(super::Error::Internal(format!(
            "git log statistics failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    parse_statistics(&output.stdout)
}

fn parse_statistics(
    output: &[u8],
) -> Result<BTreeMap<String, (usize, usize, usize)>, super::Error> {
    const MARKER: &[u8] = b"GILTI-STATS\0";
    let mut result = BTreeMap::new();
    let mut cursor = 0;
    while let Some(relative) = output[cursor..]
        .windows(MARKER.len())
        .position(|value| value == MARKER)
    {
        let start = cursor + relative + MARKER.len();
        let oid_end = output[start..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|end| start + end)
            .ok_or_else(|| super::Error::Internal("git statistics omitted an OID".to_owned()))?;
        let oid = std::str::from_utf8(&output[start..oid_end]).map_err(|_| {
            super::Error::Internal("git statistics emitted invalid UTF-8".to_owned())
        })?;
        if !(matches!(oid.len(), 40 | 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit())) {
            return Err(super::Error::Internal(
                "git statistics emitted an invalid object ID".to_owned(),
            ));
        }
        let next = output[oid_end + 1..]
            .windows(MARKER.len())
            .position(|value| value == MARKER)
            .map_or(output.len(), |offset| oid_end + 1 + offset);
        let mut files = 0;
        let mut additions = 0;
        let mut deletions = 0;
        let mut fields = output[oid_end + 1..next].split(|byte| *byte == 0);
        while let Some(field) = fields.next() {
            let field = field.strip_prefix(b"\n").unwrap_or(field);
            let mut values = field.splitn(3, |byte| *byte == b'\t');
            let (Some(added), Some(deleted), Some(path)) =
                (values.next(), values.next(), values.next())
            else {
                continue;
            };
            let count = |value: &[u8]| {
                (value != b"-")
                    .then(|| std::str::from_utf8(value).ok()?.parse::<usize>().ok())
                    .flatten()
                    .unwrap_or(0)
            };
            if (added == b"-" || added.iter().all(u8::is_ascii_digit))
                && (deleted == b"-" || deleted.iter().all(u8::is_ascii_digit))
            {
                files += 1;
                additions += count(added);
                deletions += count(deleted);
                if path.is_empty() {
                    fields.next();
                    fields.next();
                }
            }
        }
        result.insert(oid.to_owned(), (files, additions, deletions));
        cursor = next;
    }
    Ok(result)
}

fn statistics_from_git2(
    repository: &git2::Repository,
    commit: &git2::Commit<'_>,
    path: Option<&str>,
    ignore_whitespace: bool,
) -> Result<(usize, usize, usize), super::Error> {
    let old_tree = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .map_err(super::Error::from_git)?
                .tree()
                .map_err(super::Error::from_git)?,
        )
    } else {
        None
    };
    let new_tree = commit.tree().map_err(super::Error::from_git)?;
    let mut options = git2::DiffOptions::new();
    if ignore_whitespace {
        options.ignore_whitespace(true);
    }
    let mut diff = repository
        .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut options))
        .map_err(super::Error::from_git)?;
    diff.find_similar(None).map_err(super::Error::from_git)?;
    let Some(path) = path else {
        let stats = diff.stats().map_err(super::Error::from_git)?;
        return Ok((stats.files_changed(), stats.insertions(), stats.deletions()));
    };
    let mut files = 0;
    let mut additions = 0;
    let mut deletions = 0;
    for (index, delta) in diff.deltas().enumerate() {
        if delta.old_file().path_bytes() != Some(path.as_bytes())
            && delta.new_file().path_bytes() != Some(path.as_bytes())
        {
            continue;
        }
        files += 1;
        if let Some(patch) = git2::Patch::from_diff(&diff, index).map_err(super::Error::from_git)? {
            let (_, added, deleted) = patch.line_stats().map_err(super::Error::from_git)?;
            additions += added;
            deletions += deleted;
        }
    }
    Ok((files, additions, deletions))
}

fn previous_path(
    repository: &git2::Repository,
    commit: &git2::Commit<'_>,
    path: Option<String>,
) -> Result<Option<String>, super::Error> {
    let Some(path) = path else { return Ok(None) };
    if commit.parent_count() == 0 {
        return Ok(Some(path));
    }
    let parent = commit.parent(0).map_err(super::Error::from_git)?;
    let old_tree = parent.tree().map_err(super::Error::from_git)?;
    let new_tree = commit.tree().map_err(super::Error::from_git)?;
    let mut diff = repository
        .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
        .map_err(super::Error::from_git)?;
    diff.find_similar(None).map_err(super::Error::from_git)?;
    for delta in diff.deltas() {
        if delta.status() == git2::Delta::Renamed
            && delta.new_file().path_bytes() == Some(path.as_bytes())
        {
            return Ok(delta
                .old_file()
                .path_bytes()
                .map(|path| String::from_utf8_lossy(path).into_owned()));
        }
    }
    Ok(Some(path))
}

fn identity(signature: git2::Signature<'_>) -> super::commit::Identity {
    super::commit::Identity {
        name: signature
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned()),
        email: signature
            .email()
            .map(str::to_owned)
            .unwrap_or_else(|_| String::from_utf8_lossy(signature.email_bytes()).into_owned()),
        timestamp: signature.when().seconds(),
        offset_minutes: signature.when().offset_minutes(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_control_delimited_graph_output() {
        let rows = super::parse(b"*   \x1e0123456789abcdef0123456789abcdef01234567\x1f\n|\\  \n| * \x1e89abcdef0123456789abcdef0123456789abcdef\x1f\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].graph, "*   ");
        assert_eq!(rows[1].graph, "| * ");
        assert_eq!(rows[1].graph_continuations, ["|\\  "]);
    }

    #[test]
    fn parses_git_numstat_records_and_binary_files() {
        let oid = "0123456789abcdef0123456789abcdef01234567";
        let other = "89abcdef0123456789abcdef0123456789abcdef";
        let output = format!(
            "GILTI-STATS\0{oid}\0\0\n1\t2\tfile.txt\0-\t-\tbinary\0GILTI-STATS\0{other}\0\0\n0\t0\t\0old\0new\0"
        );
        let statistics = super::parse_statistics(output.as_bytes()).unwrap();
        assert_eq!(statistics[oid], (2, 1, 2));
        assert_eq!(statistics[other], (1, 0, 0));
    }
}
