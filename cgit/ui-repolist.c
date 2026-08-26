/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* ui-repolist.c: functions for generating the repolist page
 *
 * Copyright (C) 2006-2014 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#include "cgit.h"
#include "ui-repolist.h"
#include "html.h"
#include "json.h"
#include "ui-shared.h"
#include "version.h"

static time_t read_agefile(const char *path)
{
	time_t result;
	size_t size;
	char *buf = NULL;
	struct strbuf date_buf = STRBUF_INIT;

	if (read_first_line(path, &buf, &size)) {
		free(buf);
		return 0;
	}

	if (parse_date(buf, &date_buf) == 0)
		result = strtoul(date_buf.buf, NULL, 10);
	else
		result = 0;
	free(buf);
	strbuf_release(&date_buf);
	return result;
}

static int get_repo_modtime(const struct cgit_repo *repo, time_t *mtime)
{
	struct strbuf path = STRBUF_INIT;
	struct stat s;
	struct cgit_repo *r = (struct cgit_repo *)repo;

	if (repo->mtime != -1) {
		*mtime = repo->mtime;
		return 1;
	}
	strbuf_addf(&path, "%s/%s", repo->path, ctx.cfg.agefile);
	if (stat(path.buf, &s) == 0) {
		*mtime = read_agefile(path.buf);
		if (*mtime) {
			r->mtime = *mtime;
			goto end;
		}
	}

	strbuf_reset(&path);
	strbuf_addf(&path, "%s/refs/heads/%s", repo->path,
		    repo->defbranch ? repo->defbranch : "master");
	if (stat(path.buf, &s) == 0) {
		*mtime = s.st_mtime;
		r->mtime = *mtime;
		goto end;
	}

	strbuf_reset(&path);
	strbuf_addf(&path, "%s/%s", repo->path, "packed-refs");
	if (stat(path.buf, &s) == 0) {
		*mtime = s.st_mtime;
		r->mtime = *mtime;
		goto end;
	}

	*mtime = 0;
	r->mtime = *mtime;
end:
	strbuf_release(&path);
	return (r->mtime != 0);
}

static int is_match(struct cgit_repo *repo)
{
	if (!ctx.qry.search)
		return 1;
	if (repo->url && strcasestr(repo->url, ctx.qry.search))
		return 1;
	if (repo->name && strcasestr(repo->name, ctx.qry.search))
		return 1;
	if (repo->desc && strcasestr(repo->desc, ctx.qry.search))
		return 1;
	if (repo->owner && strcasestr(repo->owner, ctx.qry.search))
		return 1;
	return 0;
}

static int is_in_url(struct cgit_repo *repo)
{
	if (!ctx.qry.url)
		return 1;
	if (repo->url && starts_with(repo->url, ctx.qry.url))
		return 1;
	return 0;
}

static int is_visible(struct cgit_repo *repo)
{
	if (repo->hide || repo->ignore)
		return 0;
	if (!(is_match(repo) && is_in_url(repo)))
		return 0;
	return 1;
}

static int any_repos_visible(void)
{
	int i;

	for (i = 0; i < cgit_repolist.count; i++) {
		if (is_visible(&cgit_repolist.repos[i]))
			return 1;
	}
	return 0;
}

static int cmp(const char *s1, const char *s2)
{
	if (s1 && s2) {
		if (ctx.cfg.case_sensitive_sort)
			return strcmp(s1, s2);
		else
			return strcasecmp(s1, s2);
	}
	if (s1 && !s2)
		return -1;
	if (s2 && !s1)
		return 1;
	return 0;
}

static int sort_name(const void *a, const void *b)
{
	const struct cgit_repo *r1 = a;
	const struct cgit_repo *r2 = b;

	return cmp(r1->name, r2->name);
}

static int sort_desc(const void *a, const void *b)
{
	const struct cgit_repo *r1 = a;
	const struct cgit_repo *r2 = b;

	return cmp(r1->desc, r2->desc);
}

static int sort_owner(const void *a, const void *b)
{
	const struct cgit_repo *r1 = a;
	const struct cgit_repo *r2 = b;

	return cmp(r1->owner, r2->owner);
}

static int sort_idle(const void *a, const void *b)
{
	const struct cgit_repo *r1 = a;
	const struct cgit_repo *r2 = b;
	time_t t1, t2;

	t1 = t2 = 0;
	get_repo_modtime(r1, &t1);
	get_repo_modtime(r2, &t2);
	return t2 - t1;
}

static int sort_section(const void *a, const void *b)
{
	const struct cgit_repo *r1 = a;
	const struct cgit_repo *r2 = b;
	int result;

	result = cmp(r1->section, r2->section);
	if (!result) {
		if (!strcmp(ctx.cfg.repository_sort, "age"))
			result = sort_idle(r1, r2);
		if (!result)
			result = cmp(r1->name, r2->name);
	}
	return result;
}

struct sortcolumn {
	const char *name;
	int (*fn)(const void *a, const void *b);
};

static const struct sortcolumn sortcolumn[] = {
	{"section", sort_section},
	{"name", sort_name},
	{"desc", sort_desc},
	{"owner", sort_owner},
	{"idle", sort_idle},
	{NULL, NULL}
};

static int sort_repolist(char *field)
{
	const struct sortcolumn *column;

	for (column = &sortcolumn[0]; column->name; column++) {
		if (strcmp(field, column->name))
			continue;
		qsort(cgit_repolist.repos, cgit_repolist.count,
			sizeof(struct cgit_repo), column->fn);
		return 1;
	}
	return 0;
}


static void json_include(const char *path)
{
	struct strbuf content = STRBUF_INIT;

	if (!path || strbuf_read_file(&content, path, 0) < 0)
		json("null");
	else
		json_value(content.buf);
	strbuf_release(&content);
}

static void json_string_list(const struct string_list *list)
{
	struct string_list_item *item;
	int first = 1;

	json("[");
	for_each_string_list_item(item, list) {
		if (!first)
			json(",");
		json_value(item->string);
		first = 0;
	}
	json("]");
}

static void url_arg(struct strbuf *url, const char *value)
{
	const unsigned char *p = (const unsigned char *)(value ? value : "");

	for (; *p; p++) {
		if (isalnum(*p) || strchr("!$()*,./:;@-[]_~", *p))
			strbuf_addch(url, *p);
		else if (*p == ' ')
			strbuf_addch(url, '+');
		else
			strbuf_addf(url, "%%%02x", *p);
	}
}

static void url_path(struct strbuf *url, const char *value)
{
	const unsigned char *p = (const unsigned char *)(value ? value : "");

	for (; *p; p++) {
		if (isalnum(*p) || strchr("!$()*,./:;@-[]_~+&", *p))
			strbuf_addch(url, *p);
		else
			strbuf_addf(url, "%%%02x", *p);
	}
}

static char *sort_url(const char *sort)
{
	char *currenturl = cgit_currenturl();
	struct strbuf url = STRBUF_INIT;

	strbuf_addf(&url, "%s?s=%s", currenturl, sort);
	free(currenturl);
	if (ctx.qry.search) {
		strbuf_addstr(&url, "&q=");
		url_arg(&url, ctx.qry.search);
	}
	return strbuf_detach(&url, NULL);
}

static char *pager_url(const char *search, const char *sort, int ofs)
{
	char *currenturl = cgit_currenturl();
	struct strbuf url = STRBUF_INIT;
	const char *delimiter = "?";

	strbuf_addstr(&url, currenturl);
	free(currenturl);
	if (search) {
		strbuf_addf(&url, "?q=%s", search);
		delimiter = "&";
	}
	if (sort) {
		strbuf_addf(&url, "%ss=%s", delimiter, sort);
		delimiter = "&";
	}
	if (ofs)
		strbuf_addf(&url, "%sofs=%d", delimiter, ofs);
	return strbuf_detach(&url, NULL);
}

static char *repo_url(const struct cgit_repo *repo, const char *page,
		      const char *query)
{
	struct strbuf url = STRBUF_INIT;
	const char *delimiter = "?";

	if (ctx.cfg.virtual_root) {
		url_path(&url, ctx.cfg.virtual_root);
		url_path(&url, repo->url);
		if (repo->url[strlen(repo->url) - 1] != '/')
			strbuf_addch(&url, '/');
		if (page) {
			url_path(&url, page);
			strbuf_addch(&url, '/');
		}
	} else {
		url_path(&url, ctx.cfg.script_name);
		strbuf_addstr(&url, "?url=");
		url_arg(&url, repo->url);
		if (repo->url[strlen(repo->url) - 1] != '/')
			strbuf_addch(&url, '/');
		if (page) {
			url_arg(&url, page);
			strbuf_addch(&url, '/');
		}
		delimiter = "&";
	}
	if (query)
		strbuf_addf(&url, "%s%s", delimiter, query);
	return strbuf_detach(&url, NULL);
}

static void json_description(const char *description)
{
	size_t len = description ? strlen(description) : 0;
	size_t shown = len;

	if (shown > ctx.cfg.max_repodesc_len)
		shown = ctx.cfg.max_repodesc_len;
	char *text = xmemdupz(description ? description : "", shown);
	json("{");
	json_key("text");
	json_value(text);
	free(text);
	json(",");
	json_key("truncated");
	json_bool(len > shown);
	json("}");
}

static void json_age(struct cgit_repo *repo)
{
	time_t t, now, seconds;
	const char *unit;
	double amount;

	if (!get_repo_modtime(repo, &t)) {
		json("null");
		return;
	}
	time(&now);
	seconds = now > t ? now - t : 0;
	if (seconds < TM_HOUR * 2) { unit = "minutes"; amount = seconds * 1.0 / TM_MIN; }
	else if (seconds < TM_DAY * 2) { unit = "hours"; amount = seconds * 1.0 / TM_HOUR; }
	else if (seconds < TM_WEEK * 2) { unit = "days"; amount = seconds * 1.0 / TM_DAY; }
	else if (seconds < TM_MONTH * 2) { unit = "weeks"; amount = seconds * 1.0 / TM_WEEK; }
	else if (seconds < TM_YEAR * 2) { unit = "months"; amount = seconds * 1.0 / TM_MONTH; }
	else { unit = "years"; amount = seconds * 1.0 / TM_YEAR; }
	json("{");
	json_key("timestamp"); json_int(t); json(",");
	json_key("title"); json_value(show_date(t, 0, cgit_date_mode(DATE_ISO8601))); json(",");
	json_key("unit"); json_value(unit); json(",");
	json_key("amount"); jsonf("%.0f", amount);
	json("}");
}

static void json_shell(void)
{
	json_key("shell"); json("{");
	json_key("embedded"); json_bool(ctx.cfg.embedded); json(",");
	json_key("robots"); json_value(ctx.cfg.robots); json(",");
	json_key("css"); json_string_list(&ctx.cfg.css); json(",");
	json_key("js"); json_string_list(&ctx.cfg.js); json(",");
	json_key("favicon"); json_value(ctx.cfg.favicon); json(",");
	json_key("head_include"); json_include(ctx.cfg.head_include); json(",");
	json_key("header"); json_include(ctx.cfg.header); json(",");
	json_key("footer_configured"); json_bool(ctx.cfg.footer != NULL); json(",");
	json_key("footer"); json_include(ctx.cfg.footer); json(",");
	json_key("logo"); json_value(ctx.cfg.logo); json(",");
	json_key("logo_link"); json_value(ctx.cfg.logo_link); json(",");
	json_key("cgit_version"); json_value(cgit_version); json(",");
	json_key("git_version"); json_value(git_version_string); json(",");
	json_key("generated_at"); json_value(show_date(time(NULL), 0, cgit_date_mode(DATE_ISO8601)));
	json("}");
}

void cgit_print_repolist(void)
{
	int i, hits = 0, sorted = 0, first = 1;
	char *last_section = NULL;

	if (!any_repos_visible()) {
		cgit_print_error_page(404, "Not found", "No repositories found");
		return;
	}
	ctx.page.title = ctx.cfg.root_title;
	ctx.page.mimetype = "application/vnd.gilti.repolist+json";
	ctx.page.charset = NULL;
	cgit_print_http_headers();

	if (ctx.qry.sort)
		sorted = sort_repolist(ctx.qry.sort);
	else if (ctx.cfg.section_sort)
		sort_repolist("section");

	json("{");
	json_key("page"); json_value("repolist"); json(",");
	json_key("title"); json_value(ctx.cfg.root_title); json(",");
	json_key("root_desc"); json_value(ctx.cfg.root_desc); json(",");
	json_key("root_url"); json_value(cgit_rooturl()); json(",");
	json_key("about_url"); { struct strbuf about = STRBUF_INIT; strbuf_addf(&about, "%s?p=about", cgit_rooturl()); json_value(about.buf); strbuf_release(&about); } json(",");
	json_key("noheader"); json_bool(ctx.cfg.noheader); json(",");
	json_key("search"); json_value(ctx.qry.search); json(",");
	json_key("current_url"); { char *url = cgit_currenturl(); json_value(url); free(url); } json(",");
	json_key("root_readme"); json_bool(ctx.cfg.root_readme != NULL); json(",");
	json_key("owner_enabled"); json_bool(ctx.cfg.enable_index_owner); json(",");
	json_key("links_enabled"); json_bool(ctx.cfg.enable_index_links); json(",");
	json_key("section_grouping"); json_bool(!sorted); json(",");
	json_shell(); json(",");
	json_key("sort_urls"); json("{");
	{ const char *names[] = {"name", "desc", "owner", "idle"}; for (i = 0; i < 4; i++) { char *url = sort_url(names[i]); if (i) json(","); json_key(names[i]); json_value(url); free(url); } }
	json("},");
	json_key("rows"); json("[");
	for (i = 0; i < cgit_repolist.count; i++) {
		struct cgit_repo *repo = &cgit_repolist.repos[i];
		char *section, *url;
		if (!is_visible(repo)) continue;
		hits++;
		if (hits <= ctx.qry.ofs || hits > ctx.qry.ofs + ctx.cfg.max_repo_count) continue;
		section = repo->section && *repo->section ? repo->section : NULL;
		if (!sorted &&
		    ((last_section == NULL && section != NULL) ||
		     (last_section != NULL && section == NULL) ||
		     (last_section != NULL && section != NULL &&
		      strcmp(section, last_section)))) {
			if (!first) json(",");
			json("{"); json_key("section");
			if (section) json_value(section); else json("null");
			json("}");
			first = 0;
			last_section = section;
		}
		if (!first) json(",");
		json("{");
		json_key("name"); json_value(repo->name); json(",");
		json_key("section"); if (section) json_value(section); else json("null"); json(",");
		json_key("url"); url = repo_url(repo, NULL, NULL); json_value(url); free(url); json(",");
		json_key("description"); json_description(repo->desc); json(",");
		json_key("owner"); json_value(repo->owner); json(",");
		json_key("owner_url"); { char *current = cgit_currenturl(); struct strbuf owner = STRBUF_INIT; strbuf_addf(&owner, "%s?q=", current); url_arg(&owner, repo->owner); json_value(owner.buf); strbuf_release(&owner); free(current); } json(",");
		json_key("idle"); json_age(repo); json(",");
		json_key("log_url"); url = repo_url(repo, "log", ctx.qry.showmsg ? "showmsg=1" : NULL); json_value(url); free(url); json(",");
		json_key("tree_url"); url = repo_url(repo, "tree", NULL); json_value(url); free(url);
		json("}"); first = 0;
	}
	json("],");
	json_key("pager"); json("[");
	if (hits > ctx.cfg.max_repo_count)
		for (i = 0; i * ctx.cfg.max_repo_count < hits; i++) { char *url = pager_url(ctx.qry.search, ctx.qry.sort, i * ctx.cfg.max_repo_count); if (i) json(","); json("{"); json_key("url"); json_value(url); free(url); json(","); json_key("current"); json_bool(ctx.qry.ofs == i * ctx.cfg.max_repo_count); json("}"); }
	json("]}");
}

void cgit_print_site_readme(void)
{
	cgit_print_layout_start();
	if (!ctx.cfg.root_readme)
		goto done;
	html_include(ctx.cfg.root_readme);
done:
	cgit_print_layout_end();
}
