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

static char *request_optional_string(const char *name)
{
	const char *value = getenv(name);

	return value && *value ? xstrdup(value) : NULL;
}

static int request_optional_integer(const char *name)
{
	const char *value = getenv(name);
	char *end;
	long result;

	if (!value || !*value)
		return 0;
	errno = 0;
	result = strtol(value, &end, 10);
	if (errno || end == value || *end || result < INT_MIN || result > INT_MAX) {
		fprintf(stderr, "gilti-cgit: environment variable %s must be an integer\n",
			name);
		exit(1);
	}
	return result;
}

static void prepare_context(void)
{
	const char *value;

	memset(&ctx, 0, sizeof(ctx));
	ctx.cfg.clone_prefix = config_optional_string("CGIT_CLONE_PREFIX");
	ctx.cfg.clone_url = config_optional_string("CGIT_CLONE_URL");
	ctx.cfg.commit_sort = config_integer("CGIT_COMMIT_SORT");
	ctx.cfg.difftype = config_integer("CGIT_DIFFTYPE");
	ctx.cfg.embedded = config_integer("CGIT_EMBEDDED");
	ctx.cfg.enable_commit_graph = config_integer("CGIT_ENABLE_COMMIT_GRAPH");
	ctx.cfg.enable_follow_links = config_integer("CGIT_ENABLE_FOLLOW_LINKS");
	ctx.cfg.enable_log_filecount = config_integer("CGIT_ENABLE_LOG_FILECOUNT");
	ctx.cfg.enable_log_linecount = config_integer("CGIT_ENABLE_LOG_LINECOUNT");
	ctx.cfg.enable_remote_branches = config_integer("CGIT_ENABLE_REMOTE_BRANCHES");
	ctx.cfg.enable_subject_links = config_integer("CGIT_ENABLE_SUBJECT_LINKS");
	ctx.cfg.favicon = config_string("CGIT_FAVICON");
	ctx.cfg.footer = config_optional_string("CGIT_FOOTER");
	ctx.cfg.head_include = config_optional_string("CGIT_HEAD_INCLUDE");
	ctx.cfg.header = config_optional_string("CGIT_HEADER");
	ctx.cfg.local_time = config_integer("CGIT_LOCAL_TIME");
	ctx.cfg.logo = config_string("CGIT_LOGO");
	ctx.cfg.logo_link = config_optional_string("CGIT_LOGO_LINK");
	ctx.cfg.max_atom_items = config_integer("CGIT_MAX_ATOM_ITEMS");
	ctx.cfg.max_commit_count = config_integer("CGIT_MAX_COMMIT_COUNT");
	ctx.cfg.max_msg_len = config_integer("CGIT_MAX_MESSAGE_LENGTH");
	ctx.cfg.max_stats = config_integer("CGIT_MAX_STATS");
	ctx.cfg.mimetype_file = config_optional_string("CGIT_MIMETYPE_FILE");
	ctx.cfg.module_link = config_optional_string("CGIT_MODULE_LINK");
	ctx.cfg.noheader = config_integer("CGIT_NOHEADER");
	ctx.cfg.noplainemail = config_integer("CGIT_NOPLAINEMAIL");
	cgit_default_repo_desc = config_string("CGIT_REPO_DEFAULT_DESC");
	ctx.cfg.renamelimit = config_integer("CGIT_RENAMELIMIT");
	ctx.cfg.robots = config_string("CGIT_ROBOTS");
	ctx.cfg.root_desc = config_string("CGIT_ROOT_DESC");
	ctx.cfg.root_title = config_string("CGIT_ROOT_TITLE");
	ctx.cfg.script_name = config_string("SCRIPT_NAME");
	ctx.cfg.section = config_string("CGIT_SECTION");
	ctx.cfg.snapshots = config_integer("CGIT_SNAPSHOTS");
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
}

static void prepare_request(void)
{
	ctx.qry.repo = request_optional_string("GILTI_REPOSITORY");
	ctx.qry.page = config_string("GILTI_PAGE");
	ctx.qry.url = request_optional_string("GILTI_CURRENT_URL");
	ctx.qry.head = request_optional_string("GILTI_REVISION");
	if (ctx.qry.head) {
		ctx.qry.oid = xstrdup(ctx.qry.head);
		ctx.qry.has_oid = 1;
	}
	ctx.qry.oid2 = request_optional_string("GILTI_OLD_REVISION");
	ctx.qry.path = request_optional_string("GILTI_PATH");
	ctx.qry.format = request_optional_string("GILTI_FORMAT");
	ctx.qry.signature = request_optional_integer("GILTI_SIGNATURE");
	ctx.qry.search = request_optional_string("GILTI_QUERY_SEARCH");
	ctx.qry.grep = request_optional_string("GILTI_QUERY_GREP");
	ctx.qry.sort = request_optional_string("GILTI_QUERY_SORT");
	ctx.qry.period = request_optional_string("GILTI_QUERY_PERIOD");
	ctx.qry.ofs = request_optional_integer("GILTI_QUERY_OFFSET");
	ctx.qry.showmsg = request_optional_integer("GILTI_QUERY_SHOWMSG");
	ctx.qry.context = request_optional_integer("GILTI_QUERY_CONTEXT");
	ctx.qry.ignorews = request_optional_integer("GILTI_QUERY_IGNOREWS");
	ctx.qry.follow = request_optional_integer("GILTI_QUERY_FOLLOW");
	if (getenv("GILTI_QUERY_DIFFTYPE")) {
		ctx.qry.difftype = request_optional_integer("GILTI_QUERY_DIFFTYPE");
		ctx.qry.has_difftype = 1;
	}
}

static void prepare_repository(void)
{
	char *description = NULL;
	char *description_path;
	char *separator;
	struct passwd *owner;
	struct stat stat;
	size_t size;

	ctx.repo = cgit_add_repo(ctx.qry.repo);
	ctx.repo->path = config_string("GILTI_REPOSITORY_PATH");
	description_path = fmtalloc("%s/description", ctx.repo->path);
	if (!read_first_line(description_path, &description, &size))
		ctx.repo->desc = description;
	free(description_path);
	if (!lstat(ctx.repo->path, &stat) && (owner = getpwuid(stat.st_uid))) {
		ctx.repo->owner = strdup_first_line(owner->pw_gecos && *owner->pw_gecos ?
			owner->pw_gecos : owner->pw_name);
		separator = strchr(ctx.repo->owner, ',');
		if (separator)
			*separator = '\0';
	}
}

static char *guess_defbranch(void)
{
	const char *ref;
	struct object_id oid;

	ref = refs_resolve_ref_unsafe(get_main_ref_store(the_repository),
				     "HEAD", 0, &oid, NULL);
	if (!ref)
		return xstrdup("HEAD");
	return xstrdup(ref);
}

struct walk_tree_context {
	const char *match_path;
	unsigned int found_path:1;
};

static int find_path(const struct object_id *oid UNUSED, struct strbuf *base,
		     const char *pathname, unsigned mode, void *cbdata)
{
	struct walk_tree_context *walk_tree_ctx = cbdata;

	if (!S_ISREG(mode))
		return READ_TREE_RECURSIVE;
	if (strncmp(base->buf, walk_tree_ctx->match_path, base->len) ||
	    strcmp(walk_tree_ctx->match_path + base->len, pathname))
		return READ_TREE_RECURSIVE;
	walk_tree_ctx->found_path = 1;
	return 0;
}

static int ref_path_exists(const char *path, const char *ref)
{
	struct object_id oid;
	unsigned long size;
	struct pathspec_item path_item = {
		.match = xstrdup(path),
		.len = strlen(path)
	};
	struct pathspec paths = {
		.nr = 1,
		.items = &path_item
	};
	struct walk_tree_context walk_tree_ctx = {
		.match_path = path,
		.found_path = 0
	};

	if (!repo_get_oid(the_repository, ref, &oid) &&
	    odb_read_object_info(the_repository->objects, &oid, &size) == OBJ_COMMIT)
		read_tree(the_repository,
			  repo_get_commit_tree(the_repository,
				lookup_commit_reference(the_repository, &oid)),
			  &paths, find_path, &walk_tree_ctx);
	free(path_item.match);
	return walk_tree_ctx.found_path;
}

static void parse_readme(const char *readme, char **filename, char **ref)
{
	const char *colon;

	*filename = NULL;
	*ref = NULL;
	if (!readme || !*readme)
		return;
	colon = strchr(readme, ':');
	if (colon && colon[1]) {
		if (colon == readme)
			*ref = xstrdup(ctx.qry.head ? ctx.qry.head : ctx.repo->defbranch);
		else
			*ref = xstrndup(readme, colon - readme);
		readme = colon + 1;
	}
	*filename = *ref || readme[0] == '/' ? xstrdup(readme) :
		fmtalloc("%s/%s", ctx.repo->path, readme);
}

static void choose_readme(void)
{
	char *filename, *ref;
	struct string_list_item *entry;

	for_each_string_list_item(entry, &ctx.repo->readme) {
		parse_readme(entry->string, &filename, &ref);
		if ((ref && ref_path_exists(filename, ref)) ||
		    (!ref && !access(filename, R_OK))) {
			ctx.repo->readme.strdup_strings = 1;
			string_list_clear(&ctx.repo->readme, 0);
			ctx.repo->readme.strdup_strings = 0;
			string_list_append(&ctx.repo->readme, filename)->util = ref;
			return;
		}
		free(filename);
		free(ref);
	}
	ctx.repo->readme.strdup_strings = 1;
	string_list_clear(&ctx.repo->readme, 0);
	ctx.repo->readme.strdup_strings = 0;
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

	if (!ctx.qry.head)
		ctx.qry.head = xstrdup("HEAD");

	if (repo_get_oid(the_repository, ctx.qry.head, &oid)) {
		cgit_print_error_page(404, "Not found",
				"Invalid revision: %s", ctx.qry.head);
		return 1;
	}
	string_list_sort(&ctx.repo->submodules);
	cgit_prepare_repo_env(ctx.repo);
	choose_readme();
	return 0;
}

static void process_request(void)
{
	struct cgit_cmd *cmd;
	int nongit = 0;

	if (ctx.qry.repo && !ctx.repo) {
		cgit_print_error_page(404, "Not found", "Repository not found");
		return;
	}

	if (ctx.repo)
		prepare_repo_env(&nongit);

	cmd = cgit_get_cmd();
	if (!cmd) {
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
	set_die_routine(cgit_die_routine);

	prepare_context();
	prepare_request();
	if (ctx.qry.repo)
		prepare_repository();
	process_request();
	return 0;
}
