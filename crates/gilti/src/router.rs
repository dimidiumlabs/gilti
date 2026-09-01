// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

/*
URL scheme

The URL space is divided into disjoint Gilti-specific and repository-specific
namespaces. The reserved top-level segment "-" belongs to Gilti and cannot be
used as the first segment of a repository name.

Git objects are content-addressed and immutable: a full object ID always
identifies the same object contents. Refs and repository metadata are mutable,
and unreachable objects may eventually be removed.

Routes marked I use only immutable selectors. Routes marked M depend on the
current repository state. A composite selector inherits mutability from its
components: a tree path under a full commit OID is immutable, while the same
path under a branch or tag ref is mutable. A comparison is immutable only when
both of its revision selectors are immutable.

Repository names, ref names, and tree paths may all contain "/". Their
boundaries are expressed using literal "/+/" segments and are never inferred
from repository state. Structural delimiters are recognized before percent
decoding. A literal data segment equal to "+" must be percent-encoded as
"%2B".

Route parameters whose names end in "*" may contain "/". They are terminated
by the next structural "/+/" segment, a route-specific terminal marker, or the
end of the route. The terminal ".git" suffix separates a repository name from
Git HTTP transport paths.

  {repo*}           canonical repository name
  {ref*}            ref name relative to "refs/", for example "heads/main"
  {path*}           path within a Git tree
  {commit_id}       full OID of a commit; abbreviated IDs are not accepted
  {object_id}       full OID of any Git object; abbreviated IDs are not accepted
  {rev*}            "{commit_id}", "refs/{ref*}", or "HEAD"
  {asset_path*}     path below the Gilti static asset root
  {old_rev*}        revision selector
  {new_rev*}        revision selector
  {dumb_http_path*} path below the Git dumb HTTP "objects/" endpoint
  {lfs_path*}       path below the Git LFS endpoint

Archive, diff, blob, and object formats are representation options, not route
parameters. They are selected by the "format" query parameter, then by the
Accept header, and finally by the server default. The query parameter takes
precedence when both are present. Archive signature routes use the same query
parameter to identify the signed archive format. Raw blob contents, raw Git
object contents, and raw unified diffs are representations of their respective
views rather than separate views.

Rendered documents are an HTML view mode selected with the "view" query
parameter. Supported values are "source" and "rendered". Renderable documents
may default to "rendered"; other blobs default to "source".

Gilti views and operational endpoints:

/-/about                 <- Gilti about page
/-/terms                 <- Gilti terms of use
/-/assets/{asset_path*}  <- Static asset
/-/health                <- Health check returning {"status":"ok"}
*/

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Route {
    // Repository views:
    /// M: Repository list
    /// /
    Repositories,

    /// M: Repository overview
    /// /{repo*}
    Overview(RepoRoute<()>),

    /// M: Repository documentation
    /// /{repo*}/+/about
    About(RepoRoute<()>),

    /// M: Repository activity statistics
    /// /{repo*}/+/stats
    Stats(RepoRoute<()>),

    /// M: Recent tags, commits, and summary data
    /// /{repo*}/+/summary
    Summary(RepoRoute<()>),

    // Immutable object views:
    /// I: Git object
    /// /{repo*}/+/object/{object_id}
    Object(RepoRoute<String>),

    // Refs and revisions:
    /// M: Ref list
    /// /{repo*}/+/refs
    Refs(RepoRoute<()>),

    /// I/M: Revision overview and resolved target
    /// /{repo*}/+/{rev*}
    Revision(RepoRoute<Revision>),

    /// I/M: Commit log, optionally restricted to a path
    /// /{repo*}/+/{rev*}/+/log
    /// /{repo*}/+/{rev*}/+/log/{path*}
    Log(RepoRoute<RevisionPath>),

    /// I/M: Tree, blob, document, or submodule
    /// /{repo*}/+/{rev*}/+/tree
    /// /{repo*}/+/{rev*}/+/tree/{path*}
    Tree(RepoRoute<RevisionPath>),

    /// I/M: File blame
    /// /{repo*}/+/{rev*}/+/blame/{path*}
    Blame(RepoRoute<RevisionFile>),

    /// I/M: Archive of the root tree or a subtree
    /// /{repo*}/+/{rev*}/+/archive
    /// /{repo*}/+/{rev*}/+/archive/{path*}
    Archive(RepoRoute<RevisionPath>),

    /// I/M: Detached archive signature
    /// /{repo*}/+/{rev*}/+/archive-signature
    ArchiveSignature(RepoRoute<Revision>),

    // Feed & updates:
    /// M: Atom commit feed, optionally restricted to a path
    /// /{repo*}/+/refs/{ref*}/+/feed/atom
    /// /{repo*}/+/refs/{ref*}/+/feed/atom/{path*}
    AtomFeed(RepoRoute<RefPath>),

    // Comparisons and patches:
    /// I/M: Diff between two revisions, optionally restricted to a path
    /// /{repo*}/+/diff/{old_rev*}..{new_rev*}
    /// /{repo*}/+/diff/{old_rev*}..{new_rev*}/+/{path*}
    Diff(RepoRoute<Comparison>),

    /// I/M: Mail-formatted patch or patch series, optionally restricted to a path
    /// /{repo*}/+/patch/{old_rev*}..{new_rev*}
    /// /{repo*}/+/patch/{old_rev*}..{new_rev*}/+/{path*}
    Patch(RepoRoute<Comparison>),

    // Git HTTP transport endpoints:
    /// M: Public clone URL
    /// /{repo*}.git
    GitClone(RepoRoute<()>),

    /// M: Service discovery and ref advertisement
    /// /{repo*}.git/info/refs
    GitInfoRefs(RepoRoute<()>),

    /// M: Fetch protocol endpoint
    /// /{repo*}.git/git-upload-pack
    GitUploadPack(RepoRoute<()>),

    /// M: Push protocol endpoint, when enabled
    /// /{repo*}.git/git-receive-pack
    GitReceivePack(RepoRoute<()>),

    /// M: Dumb HTTP HEAD advertisement, when enabled
    /// /{repo*}.git/HEAD
    GitHead(RepoRoute<()>),

    /// Git dumb HTTP object endpoint, when enabled
    /// /{repo*}.git/objects/{dumb_http_path*}
    GitObjects(RepoRoute<String>),

    /// Git LFS endpoint, when enabled
    /// /{repo*}.git/info/lfs/{lfs_path*}
    GitLfs(RepoRoute<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoRoute<T> {
    pub repo: String,
    pub params: T,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionPath {
    pub rev: Revision,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionFile {
    pub rev: Revision,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefPath {
    pub reference: String,
    pub path: Option<String>,
}

/// A revision range separated by `..` and an optional tree path separated by `/+/`.
/// It is immutable only when both revision selectors are full commit OIDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comparison {
    pub old_rev: Revision,
    pub new_rev: Revision,
    pub path: Option<String>,
}

/// Git revision selector shared with the framework-independent repository library.
pub use gilti_git::Revision;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError;

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid repository route")
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Text,
    Slash,
    Boundary,
    Range,
    GitSuffix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Token<'a> {
    kind: TokenKind,
    raw: &'a str,
}

/// Parses a raw, percent-encoded URI path. Query parameters are handled separately.
pub fn parse(path: &str) -> Result<Route, ParseError> {
    let tokens = lex(path)?;
    Parser {
        tokens: &tokens,
        cursor: 0,
    }
    .parse()
}

fn lex(path: &str) -> Result<Vec<Token<'_>>, ParseError> {
    let bytes = path.as_bytes();
    let mut tokens = Vec::new();
    let (mut cursor, mut start) = (0, 0);
    while cursor < bytes.len() {
        if bytes[cursor] == b'%' {
            if cursor + 2 >= bytes.len()
                || !bytes[cursor + 1].is_ascii_hexdigit()
                || !bytes[cursor + 2].is_ascii_hexdigit()
            {
                return Err(ParseError);
            }
            cursor += 3;
            continue;
        }
        if matches!(bytes[cursor], b'?' | b'#') {
            return Err(ParseError);
        }
        let delimiter = if bytes[cursor..].starts_with(b"/+/") {
            Some((TokenKind::Boundary, 3))
        } else if bytes[cursor] == b'/' {
            Some((TokenKind::Slash, 1))
        } else if bytes[cursor..].starts_with(b"..") {
            Some((TokenKind::Range, 2))
        } else if bytes[cursor..].starts_with(b".git")
            && (cursor + 4 == bytes.len() || bytes[cursor + 4] == b'/')
        {
            Some((TokenKind::GitSuffix, 4))
        } else {
            None
        };
        let Some((kind, width)) = delimiter else {
            cursor += 1;
            continue;
        };
        if start < cursor {
            tokens.push(Token {
                kind: TokenKind::Text,
                raw: &path[start..cursor],
            });
        }
        tokens.push(Token {
            kind,
            raw: &path[cursor..cursor + width],
        });
        cursor += width;
        start = cursor;
    }
    if start < cursor {
        tokens.push(Token {
            kind: TokenKind::Text,
            raw: &path[start..cursor],
        });
    }
    Ok(tokens)
}

struct Parser<'tokens, 'input> {
    tokens: &'tokens [Token<'input>],
    cursor: usize,
}

impl<'tokens, 'input> Parser<'tokens, 'input> {
    fn parse(mut self) -> Result<Route, ParseError> {
        self.expect(TokenKind::Slash)?;
        if self.end() {
            return Ok(Route::Repositories);
        }
        let repo = decode_repo(self.take_until(&[TokenKind::Boundary, TokenKind::GitSuffix]))?;
        match self.peek() {
            None => Ok(Route::Overview(route(repo, ()))),
            Some(TokenKind::Boundary) => {
                self.cursor += 1;
                self.view(repo)
            }
            Some(TokenKind::GitSuffix) => {
                self.cursor += 1;
                self.git(repo)
            }
            _ => Err(ParseError),
        }
    }

    fn view(&mut self, repo: String) -> Result<Route, ParseError> {
        Ok(match self.peek_text() {
            Some("about") => {
                self.cursor += 1;
                self.finish()?;
                Route::About(route(repo, ()))
            }
            Some("stats") => {
                self.cursor += 1;
                self.finish()?;
                Route::Stats(route(repo, ()))
            }
            Some("summary") => {
                self.cursor += 1;
                self.finish()?;
                Route::Summary(route(repo, ()))
            }
            Some("refs") if self.cursor + 1 == self.tokens.len() => {
                self.cursor += 1;
                Route::Refs(route(repo, ()))
            }
            Some("object") => {
                self.cursor += 1;
                self.expect(TokenKind::Slash)?;
                let object_id = parse_oid(self.take_text()?)?;
                self.finish()?;
                Route::Object(route(repo, object_id))
            }
            Some("diff") | Some("patch") => {
                let patch = self.take_text()? == "patch";
                self.expect(TokenKind::Slash)?;
                return self.comparison(repo, patch);
            }
            _ => {
                let rev = self.revision()?;
                if self.end() {
                    Route::Revision(route(repo, rev))
                } else {
                    self.expect(TokenKind::Boundary)?;
                    return self.revision_view(repo, rev);
                }
            }
        })
    }

    fn revision_view(&mut self, repo: String, rev: Revision) -> Result<Route, ParseError> {
        Ok(match self.take_text()? {
            "log" => Route::Log(route(
                repo,
                RevisionPath {
                    rev,
                    path: self.optional_path(TokenKind::Slash)?,
                },
            )),
            "tree" => Route::Tree(route(
                repo,
                RevisionPath {
                    rev,
                    path: self.optional_path(TokenKind::Slash)?,
                },
            )),
            "blame" => {
                self.expect(TokenKind::Slash)?;
                Route::Blame(route(
                    repo,
                    RevisionFile {
                        rev,
                        path: decode_path(self.rest())?,
                    },
                ))
            }
            "archive" => Route::Archive(route(
                repo,
                RevisionPath {
                    rev,
                    path: self.optional_path(TokenKind::Slash)?,
                },
            )),
            "archive-signature" => {
                self.finish()?;
                Route::ArchiveSignature(route(repo, rev))
            }
            "feed" => {
                self.expect(TokenKind::Slash)?;
                self.expect_text("atom")?;
                Route::AtomFeed(route(
                    repo,
                    RefPath {
                        reference: reference(rev)?,
                        path: self.optional_path(TokenKind::Slash)?,
                    },
                ))
            }
            _ => return Err(ParseError),
        })
    }

    fn comparison(&mut self, repo: String, patch: bool) -> Result<Route, ParseError> {
        let old_rev = self.revision()?;
        self.expect(TokenKind::Range)?;
        let new_rev = self.revision()?;
        let comparison = route(
            repo,
            Comparison {
                old_rev,
                new_rev,
                path: self.optional_path(TokenKind::Boundary)?,
            },
        );
        Ok(if patch {
            Route::Patch(comparison)
        } else {
            Route::Diff(comparison)
        })
    }

    fn git(&mut self, repo: String) -> Result<Route, ParseError> {
        if self.end() {
            return Ok(Route::GitClone(route(repo, ())));
        }
        self.expect(TokenKind::Slash)?;
        Ok(match self.take_text()? {
            "info" => {
                self.expect(TokenKind::Slash)?;
                match self.take_text()? {
                    "refs" => {
                        self.finish()?;
                        Route::GitInfoRefs(route(repo, ()))
                    }
                    "lfs" => {
                        self.expect(TokenKind::Slash)?;
                        Route::GitLfs(route(repo, decode_path(self.rest())?))
                    }
                    _ => return Err(ParseError),
                }
            }
            "git-upload-pack" => {
                self.finish()?;
                Route::GitUploadPack(route(repo, ()))
            }
            "git-receive-pack" => {
                self.finish()?;
                Route::GitReceivePack(route(repo, ()))
            }
            "HEAD" => {
                self.finish()?;
                Route::GitHead(route(repo, ()))
            }
            "objects" => {
                self.expect(TokenKind::Slash)?;
                Route::GitObjects(route(repo, decode_path(self.rest())?))
            }
            _ => return Err(ParseError),
        })
    }

    fn revision(&mut self) -> Result<Revision, ParseError> {
        parse_revision(self.take_until(&[TokenKind::Boundary, TokenKind::Range]))
    }

    fn optional_path(&mut self, separator: TokenKind) -> Result<Option<String>, ParseError> {
        if self.end() {
            Ok(None)
        } else {
            self.expect(separator)?;
            decode_path(self.rest()).map(Some)
        }
    }

    fn take_until(&mut self, delimiters: &[TokenKind]) -> &'tokens [Token<'input>] {
        let start = self.cursor;
        while self.peek().is_some_and(|kind| !delimiters.contains(&kind)) {
            self.cursor += 1;
        }
        &self.tokens[start..self.cursor]
    }

    fn rest(&mut self) -> &'tokens [Token<'input>] {
        let rest = &self.tokens[self.cursor..];
        self.cursor = self.tokens.len();
        rest
    }

    fn peek(&self) -> Option<TokenKind> {
        self.tokens.get(self.cursor).map(|token| token.kind)
    }

    fn peek_text(&self) -> Option<&'input str> {
        self.tokens
            .get(self.cursor)
            .filter(|token| token.kind == TokenKind::Text)
            .map(|token| token.raw)
    }

    fn take_text(&mut self) -> Result<&'input str, ParseError> {
        let text = self.peek_text().ok_or(ParseError)?;
        self.cursor += 1;
        Ok(text)
    }

    fn expect_text(&mut self, expected: &str) -> Result<(), ParseError> {
        (self.take_text()? == expected)
            .then_some(())
            .ok_or(ParseError)
    }

    fn expect(&mut self, expected: TokenKind) -> Result<(), ParseError> {
        (self.peek() == Some(expected))
            .then(|| self.cursor += 1)
            .ok_or(ParseError)
    }

    fn finish(&self) -> Result<(), ParseError> {
        self.end().then_some(()).ok_or(ParseError)
    }

    fn end(&self) -> bool {
        self.cursor == self.tokens.len()
    }
}

fn parse_revision(value: &[Token<'_>]) -> Result<Revision, ParseError> {
    if value.len() == 1 && value[0].kind == TokenKind::Text {
        return match value[0].raw {
            "HEAD" => Ok(Revision::Head),
            oid => parse_oid(oid).map(Revision::Commit),
        };
    }
    if value.len() >= 2 && value[0].raw == "refs" && value[1].kind == TokenKind::Slash {
        let reference = format!("refs/{}", decode_path(&value[2..])?);
        return valid_ref(&reference)
            .then_some(Revision::Ref(reference))
            .ok_or(ParseError);
    }
    Err(ParseError)
}

fn parse_oid(value: &str) -> Result<String, ParseError> {
    (matches!(value.len(), 40 | 64)
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)))
    .then(|| value.to_owned())
    .ok_or(ParseError)
}

fn valid_ref(value: &str) -> bool {
    !value.ends_with('.')
        && !value.contains("..")
        && !value.contains("@{")
        && value.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !part.ends_with(".lock")
                && !part
                    .bytes()
                    .any(|byte| byte < b' ' || byte == 0x7f || b" ~^:?*[\\".contains(&byte))
        })
}

fn decode_repo(value: &[Token<'_>]) -> Result<String, ParseError> {
    decode_name(value, true)
}

fn decode_path(value: &[Token<'_>]) -> Result<String, ParseError> {
    decode_name(value, false)
}

fn decode_name(value: &[Token<'_>], repo: bool) -> Result<String, ParseError> {
    if value.is_empty() {
        return Err(ParseError);
    }
    for (index, component) in value
        .split(|token| token.kind == TokenKind::Slash)
        .enumerate()
    {
        let literal = |expected| component.len() == 1 && component[0].raw == expected;
        if component.is_empty()
            || component.iter().any(|token| {
                !matches!(
                    token.kind,
                    TokenKind::Text | TokenKind::Range | TokenKind::GitSuffix
                )
            })
            || literal(".")
            || literal("..")
            || literal("+")
            || repo
                && (index == 0 && literal("-")
                    || component
                        .last()
                        .is_some_and(|token| token.kind == TokenKind::GitSuffix))
        {
            return Err(ParseError);
        }
    }
    let mut decoded = String::new();
    for token in value {
        decoded.push_str(
            &percent_encoding::percent_decode_str(token.raw)
                .decode_utf8()
                .map_err(|_| ParseError)?,
        );
    }
    (!decoded.contains('\0'))
        .then_some(decoded)
        .ok_or(ParseError)
}

fn reference(rev: Revision) -> Result<String, ParseError> {
    match rev {
        Revision::Ref(reference) => Ok(reference),
        _ => Err(ParseError),
    }
}

fn route<T>(repo: String, params: T) -> RepoRoute<T> {
    RepoRoute { repo, params }
}

#[cfg(test)]
mod tests {
    use super::{
        Comparison, RefPath, RepoRoute, Revision, RevisionFile, RevisionPath, Route, TokenKind,
        lex, parse,
    };

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER: &str = "89abcdef0123456789abcdef0123456789abcdef";

    fn repository<T>(params: T) -> RepoRoute<T> {
        repo("group/проект", params)
    }

    fn repo<T>(name: &str, params: T) -> RepoRoute<T> {
        RepoRoute {
            repo: name.to_owned(),
            params,
        }
    }

    #[test]
    fn lexer_preserves_raw_tokens() {
        use TokenKind::{Boundary, GitSuffix, Range, Slash, Text};

        let tokens = lex("/repo%2Egit.git/diff/HEAD..refs/heads/main/+/src").unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|token| (token.kind, token.raw))
                .collect::<Vec<_>>(),
            [
                (Slash, "/"),
                (Text, "repo%2Egit"),
                (GitSuffix, ".git"),
                (Slash, "/"),
                (Text, "diff"),
                (Slash, "/"),
                (Text, "HEAD"),
                (Range, ".."),
                (Text, "refs"),
                (Slash, "/"),
                (Text, "heads"),
                (Slash, "/"),
                (Text, "main"),
                (Boundary, "/+/"),
                (Text, "src"),
            ]
        );
    }

    #[test]
    fn parses_repository_views() {
        assert_eq!(parse("/").unwrap(), Route::Repositories);
        assert_eq!(
            parse("/group/%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82").unwrap(),
            Route::Overview(repository(()))
        );
        assert_eq!(
            parse("/foo%2Egit/+/summary").unwrap(),
            Route::Summary(repo("foo.git", ()))
        );
        assert_eq!(
            parse("/odd..repo").unwrap(),
            Route::Overview(repo("odd..repo", ()))
        );
        for (path, expected) in [
            ("about", Route::About(repository(()))),
            ("stats", Route::Stats(repository(()))),
            ("summary", Route::Summary(repository(()))),
            ("refs", Route::Refs(repository(()))),
        ] {
            assert_eq!(parse(&format!("/group/проект/+/{path}")).unwrap(), expected);
        }
        assert_eq!(
            parse(&format!("/group/проект/+/object/{COMMIT}")).unwrap(),
            Route::Object(repository(COMMIT.to_owned()))
        );
    }

    #[test]
    fn parses_revision_views_and_escaped_paths() {
        let reference = Revision::Ref("refs/heads/feature/+".to_owned());
        assert_eq!(
            parse("/group/проект/+/refs/heads/feature/%2B").unwrap(),
            Route::Revision(repository(reference.clone()))
        );
        assert_eq!(
            parse("/group/проект/+/refs/heads/feature/%2B/+/tree").unwrap(),
            Route::Tree(repository(RevisionPath {
                rev: reference.clone(),
                path: None,
            }))
        );
        assert_eq!(
            parse("/group/проект/+/refs/heads/feature/%2B/+/tree/src/%2B/lib.rs").unwrap(),
            Route::Tree(repository(RevisionPath {
                rev: reference.clone(),
                path: Some("src/+/lib.rs".to_owned()),
            }))
        );
        assert_eq!(
            parse("/group/проект/+/HEAD/+/tree/src/a..b/foo.git").unwrap(),
            Route::Tree(repository(RevisionPath {
                rev: Revision::Head,
                path: Some("src/a..b/foo.git".to_owned()),
            }))
        );
        assert_eq!(
            parse(&format!("/group/проект/+/{COMMIT}/+/log/src/lib.rs")).unwrap(),
            Route::Log(repository(RevisionPath {
                rev: Revision::Commit(COMMIT.to_owned()),
                path: Some("src/lib.rs".to_owned()),
            }))
        );
        assert_eq!(
            parse("/group/проект/+/HEAD/+/archive").unwrap(),
            Route::Archive(repository(RevisionPath {
                rev: Revision::Head,
                path: None,
            }))
        );
        assert_eq!(
            parse("/group/проект/+/refs/heads/main/+/blame/src/lib.rs").unwrap(),
            Route::Blame(repository(RevisionFile {
                rev: Revision::Ref("refs/heads/main".to_owned()),
                path: "src/lib.rs".to_owned(),
            }))
        );
    }

    #[test]
    fn parses_feeds_comparisons_and_patches() {
        assert_eq!(
            parse("/group/проект/+/refs/heads/main/+/feed/atom/src/lib.rs").unwrap(),
            Route::AtomFeed(repository(RefPath {
                reference: "refs/heads/main".to_owned(),
                path: Some("src/lib.rs".to_owned()),
            }))
        );
        assert_eq!(
            parse(&format!(
                "/group/проект/+/diff/refs/heads/main..{COMMIT}/+/src/lib.rs"
            ))
            .unwrap(),
            Route::Diff(repository(Comparison {
                old_rev: Revision::Ref("refs/heads/main".to_owned()),
                new_rev: Revision::Commit(COMMIT.to_owned()),
                path: Some("src/lib.rs".to_owned()),
            }))
        );
        assert_eq!(
            parse(&format!("/group/проект/+/patch/{COMMIT}..{OTHER}")).unwrap(),
            Route::Patch(repository(Comparison {
                old_rev: Revision::Commit(COMMIT.to_owned()),
                new_rev: Revision::Commit(OTHER.to_owned()),
                path: None,
            }))
        );
    }

    #[test]
    fn parses_git_transport_routes() {
        for (path, expected) in [
            ("/group/проект.git", Route::GitClone(repository(()))),
            ("/foo%2Egit.git", Route::GitClone(repo("foo.git", ()))),
            (
                "/group/проект.git/info/refs",
                Route::GitInfoRefs(repository(())),
            ),
            (
                "/group/проект.git/git-upload-pack",
                Route::GitUploadPack(repository(())),
            ),
            (
                "/group/проект.git/git-receive-pack",
                Route::GitReceivePack(repository(())),
            ),
            ("/group/проект.git/HEAD", Route::GitHead(repository(()))),
            (
                "/group/проект.git/objects/ab/cdef",
                Route::GitObjects(repository("ab/cdef".to_owned())),
            ),
            (
                "/group/проект.git/info/lfs/objects/batch",
                Route::GitLfs(repository("objects/batch".to_owned())),
            ),
        ] {
            assert_eq!(parse(path).unwrap(), expected);
        }
    }

    #[test]
    fn rejects_ambiguous_or_noncanonical_routes() {
        for path in [
            "",
            "/-/about",
            "/repo/",
            "/repo/+/",
            "/repo/+/main",
            "/repo/+/deadbeef",
            "/repo/+/refs/heads/bad..ref",
            "/repo/+/HEAD/+/blame",
            "/repo/+/HEAD/+/feed/atom",
            "/repo/+/diff/HEAD...HEAD",
            "/repo/+/diff/HEAD..",
            "/repo/+/tree",
            "/repo.git/unknown",
            "/repo/%zz",
            "/repo?format=raw",
        ] {
            assert!(parse(path).is_err(), "accepted {path}");
        }
    }
}
