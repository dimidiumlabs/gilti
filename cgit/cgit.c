/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-FileCopyrightText: 2026 Nikolay Govorov
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* cgit.c: cgi for the git scm
 *
 * Copyright (C) 2006-2014 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#define USE_THE_REPOSITORY_VARIABLE

#include "cgit.h"
#include "cmd.h"
#include "html.h"
#include "ui-shared.h"
#include "ui-blob.h"
#include "ui-summary.h"
#include "scan-tree.h"

const char *cgit_version = CGIT_VERSION;

static const char *config_value(const char *name)
{
	const char *value = getenv(name);

	if (!value) {
		fprintf(stderr, "gilti-cgit: missing required environment variable %s\n",
			name);
		exit(1);
	}
	return value;
}

static char *config_string(const char *name)
{
	return xstrdup(config_value(name));
}

static char *config_optional_string(const char *name)
{
	const char *value = config_value(name);

	return *value ? xstrdup(value) : NULL;
}

static int config_integer(const char *name)
{
	const char *value = config_value(name);
	char *end;
	long result;

	errno = 0;
	result = strtol(value, &end, 10);
	if (errno || end == value || *end || result < INT_MIN || result > INT_MAX) {
		fprintf(stderr, "gilti-cgit: environment variable %s must be an integer\n",
			name);
		exit(1);
	}
	return result;
}

static void querystring_cb(const char *name, const char *value)
{
	if (!value)
		value = "";

	if (!strcmp(name,"r")) {
		ctx.qry.repo = xstrdup(value);
		ctx.repo = cgit_get_repoinfo(value);
	} else if (!strcmp(name, "p")) {
		ctx.qry.page = xstrdup(value);
	} else if (!strcmp(name, "url")) {
		if (*value == '/')
			value++;
		ctx.qry.url = xstrdup(value);
		cgit_parse_url(value);
	} else if (!strcmp(name, "qt")) {
		ctx.qry.grep = xstrdup(value);
	} else if (!strcmp(name, "q")) {
		ctx.qry.search = xstrdup(value);
	} else if (!strcmp(name, "h")) {
		ctx.qry.head = xstrdup(value);
	} else if (!strcmp(name, "id")) {
		ctx.qry.oid = xstrdup(value);
		ctx.qry.has_oid = 1;
	} else if (!strcmp(name, "id2")) {
		ctx.qry.oid2 = xstrdup(value);
		ctx.qry.has_oid = 1;
	} else if (!strcmp(name, "ofs")) {
		ctx.qry.ofs = atoi(value);
	} else if (!strcmp(name, "path")) {
		ctx.qry.path = trim_end(value, '/');
	} else if (!strcmp(name, "name")) {
		ctx.qry.name = xstrdup(value);
	} else if (!strcmp(name, "s")) {
		ctx.qry.sort = xstrdup(value);
	} else if (!strcmp(name, "showmsg")) {
		ctx.qry.showmsg = atoi(value);
	} else if (!strcmp(name, "period")) {
		ctx.qry.period = xstrdup(value);
	} else if (!strcmp(name, "dt")) {
		ctx.qry.difftype = atoi(value);
		ctx.qry.has_difftype = 1;
	} else if (!strcmp(name, "ss")) {
		/* No longer generated, but there may be links out there. */
		ctx.qry.difftype = atoi(value) ? DIFF_SSDIFF : DIFF_UNIFIED;
		ctx.qry.has_difftype = 1;
	} else if (!strcmp(name, "all")) {
		ctx.qry.show_all = atoi(value);
	} else if (!strcmp(name, "context")) {
		ctx.qry.context = atoi(value);
	} else if (!strcmp(name, "ignorews")) {
		ctx.qry.ignorews = atoi(value);
	} else if (!strcmp(name, "follow")) {
		ctx.qry.follow = atoi(value);
	}
}

static void prepare_context(void)
{
	const char *value;

	memset(&ctx, 0, sizeof(ctx));
	ctx.cfg.agefile = config_string("CGIT_AGEFILE");
	ctx.cfg.branch_sort = config_integer("CGIT_BRANCH_SORT");
	ctx.cfg.case_sensitive_sort = config_integer("CGIT_CASE_SENSITIVE_SORT");
	ctx.cfg.clone_prefix = config_optional_string("CGIT_CLONE_PREFIX");
	ctx.cfg.clone_url = config_optional_string("CGIT_CLONE_URL");
	ctx.cfg.commit_sort = config_integer("CGIT_COMMIT_SORT");
	ctx.cfg.difftype = config_integer("CGIT_DIFFTYPE");
	ctx.cfg.embedded = config_integer("CGIT_EMBEDDED");
	ctx.cfg.enable_blame = config_integer("CGIT_ENABLE_BLAME");
	ctx.cfg.enable_commit_graph = config_integer("CGIT_ENABLE_COMMIT_GRAPH");
	ctx.cfg.enable_follow_links = config_integer("CGIT_ENABLE_FOLLOW_LINKS");
	ctx.cfg.enable_html_serving = config_integer("CGIT_ENABLE_HTML_SERVING");
	ctx.cfg.enable_http_clone = config_integer("CGIT_ENABLE_HTTP_CLONE");
	ctx.cfg.enable_index_links = config_integer("CGIT_ENABLE_INDEX_LINKS");
	ctx.cfg.enable_index_owner = config_integer("CGIT_ENABLE_INDEX_OWNER");
	ctx.cfg.enable_log_filecount = config_integer("CGIT_ENABLE_LOG_FILECOUNT");
	ctx.cfg.enable_log_linecount = config_integer("CGIT_ENABLE_LOG_LINECOUNT");
	ctx.cfg.enable_remote_branches = config_integer("CGIT_ENABLE_REMOTE_BRANCHES");
	ctx.cfg.enable_subject_links = config_integer("CGIT_ENABLE_SUBJECT_LINKS");
	ctx.cfg.enable_tree_linenumbers = config_integer("CGIT_ENABLE_TREE_LINENUMBERS");
	ctx.cfg.favicon = config_string("CGIT_FAVICON");
	ctx.cfg.footer = config_optional_string("CGIT_FOOTER");
	ctx.cfg.head_include = config_optional_string("CGIT_HEAD_INCLUDE");
	ctx.cfg.header = config_optional_string("CGIT_HEADER");
	ctx.cfg.local_time = config_integer("CGIT_LOCAL_TIME");
	ctx.cfg.logo = config_string("CGIT_LOGO");
	ctx.cfg.logo_link = config_optional_string("CGIT_LOGO_LINK");
	ctx.cfg.max_atom_items = config_integer("CGIT_MAX_ATOM_ITEMS");
	ctx.cfg.max_blob_size = config_integer("CGIT_MAX_BLOB_SIZE");
	ctx.cfg.max_commit_count = config_integer("CGIT_MAX_COMMIT_COUNT");
	ctx.cfg.max_msg_len = config_integer("CGIT_MAX_MESSAGE_LENGTH");
	ctx.cfg.max_repo_count = config_integer("CGIT_MAX_REPO_COUNT");
	ctx.cfg.max_repodesc_len = config_integer("CGIT_MAX_REPODESC_LENGTH");
	ctx.cfg.max_stats = config_integer("CGIT_MAX_STATS");
	ctx.cfg.mimetype_file = config_optional_string("CGIT_MIMETYPE_FILE");
	ctx.cfg.module_link = config_optional_string("CGIT_MODULE_LINK");
	ctx.cfg.noheader = config_integer("CGIT_NOHEADER");
	ctx.cfg.noplainemail = config_integer("CGIT_NOPLAINEMAIL");
	ctx.cfg.remove_suffix = config_integer("CGIT_REMOVE_SUFFIX");
	cgit_default_repo_desc = config_string("CGIT_REPO_DEFAULT_DESC");
	ctx.cfg.renamelimit = config_integer("CGIT_RENAMELIMIT");
	ctx.cfg.repository_sort = config_string("CGIT_REPOSITORY_SORT");
	ctx.cfg.robots = config_string("CGIT_ROBOTS");
	ctx.cfg.root_desc = config_string("CGIT_ROOT_DESC");
	ctx.cfg.root_readme = config_optional_string("CGIT_ROOT_README");
	ctx.cfg.root_title = config_string("CGIT_ROOT_TITLE");
	ctx.cfg.scan_hidden_path = config_integer("CGIT_SCAN_HIDDEN_PATH");
	ctx.cfg.script_name = config_string("SCRIPT_NAME");
	ctx.cfg.section = config_string("CGIT_SECTION");
	ctx.cfg.section_from_path = config_integer("CGIT_SECTION_FROM_PATH");
	ctx.cfg.section_sort = config_integer("CGIT_SECTION_SORT");
	ctx.cfg.snapshots = config_integer("CGIT_SNAPSHOTS");
	ctx.cfg.strict_export = config_optional_string("CGIT_STRICT_EXPORT");
	ctx.cfg.summary_branches = config_integer("CGIT_SUMMARY_BRANCHES");
	ctx.cfg.summary_log = config_integer("CGIT_SUMMARY_LOG");
	ctx.cfg.summary_tags = config_integer("CGIT_SUMMARY_TAGS");
	ctx.cfg.virtual_root = ensure_end(config_value("CGIT_VIRTUAL_ROOT"), '/');
	string_list_init_dup(&ctx.cfg.css);
	string_list_init_dup(&ctx.cfg.js);
	string_list_init_dup(&ctx.cfg.mimetypes);
	string_list_init_dup(&ctx.cfg.readme);
	value = config_value("CGIT_CSS");
	if (*value)
		string_list_append(&ctx.cfg.css, value);
	value = config_value("CGIT_JS");
	if (*value)
		string_list_append(&ctx.cfg.js, value);
	value = config_value("CGIT_README_0");
	if (*value)
		string_list_append(&ctx.cfg.readme, value);
	value = config_value("CGIT_README_1");
	if (*value)
		string_list_append(&ctx.cfg.readme, value);

	ctx.env.http_host = getenv("HTTP_HOST");
	ctx.env.https = getenv("HTTPS");
	ctx.env.path_info = getenv("PATH_INFO");
	ctx.env.query_string = getenv("QUERY_STRING");
	ctx.env.request_method = getenv("REQUEST_METHOD");
	ctx.env.server_name = getenv("SERVER_NAME");
	ctx.env.server_port = getenv("SERVER_PORT");
	ctx.page.mimetype = "text/html";
	ctx.page.charset = PAGE_ENCODING;
	ctx.page.modified = time(NULL);
	if (ctx.env.query_string)
		ctx.qry.raw = xstrdup(ctx.env.query_string);
}

struct refmatch {
	char *req_ref;
	char *first_ref;
	int match;
};

static int find_current_ref(const struct reference *ref, void *cb_data)
{
	struct refmatch *info;

	info = (struct refmatch *)cb_data;
	if (!strcmp(ref->name, info->req_ref))
		info->match = 1;
	if (!info->first_ref)
		info->first_ref = xstrdup(ref->name);
	return info->match;
}

static void free_refmatch_inner(struct refmatch *info)
{
	if (info->first_ref)
		free(info->first_ref);
}

static char *find_default_branch(struct cgit_repo *repo)
{
	struct refmatch info;
	char *ref;

	info.req_ref = repo->defbranch;
	info.first_ref = NULL;
	info.match = 0;
	refs_for_each_branch_ref(get_main_ref_store(the_repository),
				 find_current_ref, &info);
	if (info.match)
		ref = info.req_ref;
	else
		ref = info.first_ref;
	if (ref)
		ref = xstrdup(ref);
	free_refmatch_inner(&info);

	return ref;
}

static char *guess_defbranch(void)
{
	const char *ref, *refname;
	struct object_id oid;

	ref = refs_resolve_ref_unsafe(get_main_ref_store(the_repository),
				     "HEAD", 0, &oid, NULL);
	if (!ref || !skip_prefix(ref, "refs/heads/", &refname))
		return "master";
	return xstrdup(refname);
}

/* The caller must free filename and ref after calling this. */
static inline void parse_readme(const char *readme, char **filename, char **ref, struct cgit_repo *repo)
{
	const char *colon;

	*filename = NULL;
	*ref = NULL;

	if (!readme || !readme[0])
		return;

	/* Check if the readme is tracked in the git repo. */
	colon = strchr(readme, ':');
	if (colon && strlen(colon) > 1) {
		/* If it starts with a colon, we want to use head given
		 * from query or the default branch */
		if (colon == readme && ctx.qry.head)
			*ref = xstrdup(ctx.qry.head);
		else if (colon == readme && repo->defbranch)
			*ref = xstrdup(repo->defbranch);
		else
			*ref = xstrndup(readme, colon - readme);
		readme = colon + 1;
	}

	/* Prepend repo path to relative readme path unless tracked. */
	if (!(*ref) && readme[0] != '/')
		*filename = fmtalloc("%s/%s", repo->path, readme);
	else
		*filename = xstrdup(readme);
}
static void choose_readme(struct cgit_repo *repo)
{
	int found;
	char *filename, *ref;
	struct string_list_item *entry;

	if (!repo->readme.nr)
		return;

	found = 0;
	for_each_string_list_item(entry, &repo->readme) {
		parse_readme(entry->string, &filename, &ref, repo);
		if (!filename) {
			free(filename);
			free(ref);
			continue;
		}
		if (ref) {
			if (cgit_ref_path_exists(filename, ref, 1)) {
				found = 1;
				break;
			}
		}
		else if (!access(filename, R_OK)) {
			found = 1;
			break;
		}
		free(filename);
		free(ref);
	}
	repo->readme.strdup_strings = 1;
	string_list_clear(&repo->readme, 0);
	repo->readme.strdup_strings = 0;
	if (found)
		string_list_append(&repo->readme, filename)->util = ref;
}

static void print_no_repo_clone_urls(const char *url)
{
        html("<tr><td><a rel='vcs-git' href='");
        html_url_path(url);
        html("' title='");
        html_attr(ctx.repo->name);
        html(" Git repository'>");
        html_txt(url);
        html("</a></td></tr>\n");
}

static void prepare_repo_env(int *nongit)
{
	/* The path to the git repository. */
	setenv("GIT_DIR", ctx.repo->path, 1);

	/* Setup the git directory and initialize the notes system. Both of these
	 * load local configuration from the git repository, so we do them both while
	 * the HOME variables are unset. */
	setup_git_directory_gently(nongit);
	load_display_notes(NULL);
}

static int prepare_repo_cmd(int nongit)
{
	struct object_id oid;
	int rc;

	if (nongit) {
		const char *name = ctx.repo->name;
		rc = errno;
		ctx.page.title = fmtalloc("%s - %s", ctx.cfg.root_title,
						"config error");
		ctx.repo = NULL;
		cgit_print_http_headers();
		cgit_print_docstart();
		cgit_print_pageheader();
		cgit_print_error("Failed to open %s: %s", name,
				 rc ? strerror(rc) : "Not a valid git repository");
		cgit_print_docend();
		return 1;
	}
	ctx.page.title = fmtalloc("%s - %s", ctx.repo->name, ctx.repo->desc);

	if (!ctx.repo->defbranch)
		ctx.repo->defbranch = guess_defbranch();

	if (!ctx.qry.head) {
		ctx.qry.nohead = 1;
		ctx.qry.head = find_default_branch(ctx.repo);
	}

	if (!ctx.qry.head) {
		cgit_print_http_headers();
		cgit_print_docstart();
		cgit_print_pageheader();
		cgit_print_error("Repository seems to be empty");
		if (!strcmp(ctx.qry.page, "summary")) {
			html("<table class='list'><tr class='nohover'><td>&nbsp;</td></tr><tr class='nohover'><th class='left'>Clone</th></tr>\n");
			cgit_prepare_repo_env(ctx.repo);
			cgit_add_clone_urls(print_no_repo_clone_urls);
			html("</table>\n");
		}
		cgit_print_docend();
		return 1;
	}

	if (repo_get_oid(the_repository, ctx.qry.head, &oid)) {
		char *old_head = ctx.qry.head;
		ctx.qry.head = xstrdup(ctx.repo->defbranch);
		cgit_print_error_page(404, "Not found",
				"Invalid branch: %s", old_head);
		free(old_head);
		return 1;
	}
	string_list_sort(&ctx.repo->submodules);
	cgit_prepare_repo_env(ctx.repo);
	choose_readme(ctx.repo);
	return 0;
}

static void process_request(void)
{
	struct cgit_cmd *cmd;
	int nongit = 0;

	if (ctx.repo)
		prepare_repo_env(&nongit);

	cmd = cgit_get_cmd();
	if (!cmd) {
		ctx.page.title = "cgit error";
		cgit_print_error_page(404, "Not found", "Invalid request");
		return;
	}

	if (!ctx.cfg.enable_http_clone && cmd->is_clone) {
		ctx.page.title = "cgit error";
		cgit_print_error_page(404, "Not found", "Invalid request");
		return;
	}

	if (cmd->want_repo && !ctx.repo) {
		cgit_print_error_page(400, "Bad request",
				"No repository selected");
		return;
	}

	/* If cmd->want_vpath is set, assume ctx.qry.path contains a "virtual"
	 * in-project path limit to be made available at ctx.qry.vpath.
	 * Otherwise, no path limit is in effect (ctx.qry.vpath = NULL).
	 */
	ctx.qry.vpath = cmd->want_vpath ? ctx.qry.path : NULL;

	if (ctx.repo && prepare_repo_cmd(nongit))
		return;

	cmd->fn();
}

static NORETURN void cgit_die_routine(const char *msg, va_list params)
{
	cgit_vprint_error_page(400, "Bad request", msg, params);
	exit(0);
}

int cmd_main(int argc UNUSED, const char **argv UNUSED)
{
	const char *path;

	set_die_routine(cgit_die_routine);

	prepare_context();
	cgit_repolist.length = 0;
	cgit_repolist.count = 0;
	cgit_repolist.repos = NULL;

	scan_tree(config_value("CGIT_SCAN_PATH"));
	ctx.repo = NULL;
	http_parse_querystring(ctx.qry.raw, querystring_cb);

	/* If no url parameter is specified on the querystring, use PATH_INFO
	 * as url. This allows cgit to work with virtual urls without the need
	 * for rewriterules in the webserver.
	 */
	path = ctx.env.path_info;
	if (!ctx.qry.url && path) {
		if (path[0] == '/')
			path++;
		ctx.qry.url = xstrdup(path);
		if (ctx.qry.raw) {
			char *newqry = fmtalloc("%s?%s", path, ctx.qry.raw);
			free(ctx.qry.raw);
			ctx.qry.raw = newqry;
		} else
			ctx.qry.raw = xstrdup(ctx.qry.url);
		cgit_parse_url(ctx.qry.url);
	}

	process_request();
	return 0;
}
