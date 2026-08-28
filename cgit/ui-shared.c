/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-FileCopyrightText: 2026 Nikolay Govorov
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* ui-shared.c: common web output functions
 *
 * Copyright (C) 2006-2017 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#define USE_THE_REPOSITORY_VARIABLE

#include "cgit.h"
#include "ui-shared.h"
#include "cmd.h"
#include "html.h"
#include "version.h"

static const char cgit_doctype[] =
"<!DOCTYPE html>\n";

static char *http_date(time_t t)
{
	static char day[][4] =
		{"Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"};
	static char month[][4] =
		{"Jan", "Feb", "Mar", "Apr", "May", "Jun",
		 "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};
	struct tm tm;
	gmtime_r(&t, &tm);
	return fmt("%s, %02d %s %04d %02d:%02d:%02d GMT", day[tm.tm_wday],
		   tm.tm_mday, month[tm.tm_mon], 1900 + tm.tm_year,
		   tm.tm_hour, tm.tm_min, tm.tm_sec);
}

void cgit_print_error(const char *fmt, ...)
{
	va_list ap;
	va_start(ap, fmt);
	cgit_vprint_error(fmt, ap);
	va_end(ap);
}

void cgit_vprint_error(const char *fmt, va_list ap)
{
	va_list cp;
	html("<div class='error'>");
	va_copy(cp, ap);
	html_vtxtf(fmt, cp);
	va_end(cp);
	html("</div>\n");
}

const char *cgit_httpscheme(void)
{
	if (ctx.env.https && !strcmp(ctx.env.https, "on"))
		return "https://";
	else
		return "http://";
}

char *cgit_hosturl(void)
{
	if (ctx.env.http_host)
		return xstrdup(ctx.env.http_host);
	if (!ctx.env.server_name)
		return NULL;
	if (!ctx.env.server_port || atoi(ctx.env.server_port) == 80)
		return xstrdup(ctx.env.server_name);
	return fmtalloc("%s:%s", ctx.env.server_name, ctx.env.server_port);
}

char *cgit_currenturl(void)
{
	return xstrdup(ctx.qry.url ? ctx.qry.url : cgit_rooturl());
}

char *cgit_currentfullurl(void)
{
	const char *url = ctx.qry.url ? ctx.qry.url : cgit_rooturl();
	const char *query = ctx.env.query_string;

	return query && *query ? fmtalloc("%s?%s", url, query) : xstrdup(url);
}

const char *cgit_rooturl(void)
{
	if (ctx.cfg.virtual_root)
		return ctx.cfg.virtual_root;
	else
		return ctx.cfg.script_name;
}

static void add_url_path(struct strbuf *url, const char *value)
{
	const unsigned char *p = (const unsigned char *)(value ? value : "");

	for (; *p; p++) {
		if (isalnum(*p) || *p == '/' || *p == '_')
			strbuf_addch(url, *p);
		else
			strbuf_addf(url, "%%%02x", *p);
	}
}

static void add_repo_url(struct strbuf *url, const char *reponame)
{
	strbuf_addch(url, '/');
	add_url_path(url, reponame);
}

static const char *link_revision(const char *head, const char *rev)
{
	return rev ? rev : head ? head : "HEAD";
}

static void add_revision(struct strbuf *url, const char *revision)
{
	struct object_id oid;

	if (!strcmp(revision, "HEAD") || starts_with(revision, "refs/") ||
	    !get_oid_hex(revision, &oid))
		add_url_path(url, revision);
	else {
		strbuf_addstr(url, "refs/heads/");
		add_url_path(url, revision);
	}
}

static char *repo_view_url(const char *reponame, const char *page,
			   const char *revision, const char *path)
{
	struct strbuf url = STRBUF_INIT;

	add_repo_url(&url, reponame);
	if (!page || !strcmp(page, "summary"))
		return strbuf_detach(&url, NULL);
	strbuf_addstr(&url, "/+/");
	if (!strcmp(page, "about") || !strcmp(page, "stats") ||
	    !strcmp(page, "refs")) {
		strbuf_addstr(&url, page);
		return strbuf_detach(&url, NULL);
	}
	add_revision(&url, revision ? revision : "HEAD");
	if (strcmp(page, "commit")) {
		strbuf_addstr(&url, "/+/");
		strbuf_addstr(&url, !strcmp(page, "plain") ? "tree" : page);
		if (path) {
			strbuf_addch(&url, '/');
			add_url_path(&url, path);
		}
	}
	return strbuf_detach(&url, NULL);
}

char *cgit_repourl(const char *reponame)
{
	return repo_view_url(reponame, NULL, NULL, NULL);
}

char *cgit_fileurl(const char *reponame, const char *pagename,
		   const char *filename, const char *query)
{
	char *url = repo_view_url(reponame,
				  !strcmp(pagename, "atom") ? "feed/atom" : pagename,
				  ctx.qry.head, filename);
	char *result;

	if (!query)
		return url;
	result = fmtalloc("%s?%s", url, query);
	free(url);
	return result;
}

char *cgit_pageurl(const char *reponame, const char *pagename,
		   const char *query)
{
	return cgit_fileurl(reponame, pagename, NULL, query);
}

char *cgit_revurl(const char *reponame, const char *revision)
{
	return repo_view_url(reponame, "commit", revision, NULL);
}

char *cgit_treeurl(const char *reponame, const char *revision,
		   const char *path, const char *fragment)
{
	char *url = repo_view_url(reponame, "tree", revision, path);
	char *result;

	if (!fragment)
		return url;
	result = fmtalloc("%s#%s", url, fragment);
	free(url);
	return result;
}

const char *cgit_repobasename(const char *reponame)
{
	/* I assume we don't need to store more than one repo basename */
	static char rvbuf[1024];
	int p;
	const char *rv;
	size_t len;

	len = strlcpy(rvbuf, reponame, sizeof(rvbuf));
	if (len >= sizeof(rvbuf))
		die("cgit_repobasename: truncated repository name '%s'", reponame);
	p = len - 1;
	/* strip trailing slashes */
	while (p && rvbuf[p] == '/')
		rvbuf[p--] = '\0';
	/* strip trailing .git */
	if (p >= 3 && starts_with(&rvbuf[p-3], ".git")) {
		p -= 3;
		rvbuf[p--] = '\0';
	}
	/* strip more trailing slashes if any */
	while (p && rvbuf[p] == '/')
		rvbuf[p--] = '\0';
	/* find last slash in the remaining string */
	rv = strrchr(rvbuf, '/');
	if (rv)
		return ++rv;
	return rvbuf;
}

const char *cgit_snapshot_prefix(const struct cgit_repo *repo)
{
	if (repo->snapshot_prefix)
		return repo->snapshot_prefix;

	return cgit_repobasename(repo->url);
}

static void site_url(const char *page, const char *search, const char *sort, int ofs, int always_root)
{
	char *delim = "?";

	if (page)
		htmlf("/-/%s", page);
	else if (always_root)
		html_attr(cgit_rooturl());
	else {
		char *currenturl = cgit_currenturl();
		html_attr(currenturl);
		free(currenturl);
	}

	if (search) {
		html(delim);
		html("q=");
		html_attr(search);
		delim = "&amp;";
	}
	if (sort) {
		html(delim);
		html("s=");
		html_attr(sort);
		delim = "&amp;";
	}
	if (ofs) {
		html(delim);
		htmlf("ofs=%d", ofs);
	}
}

static void site_link(const char *page, const char *name, const char *title,
		      const char *class, const char *search, const char *sort, int ofs, int always_root)
{
	html("<a");
	if (title) {
		html(" title='");
		html_attr(title);
		html("'");
	}
	if (class) {
		html(" class='");
		html_attr(class);
		html("'");
	}
	html(" href='");
	site_url(page, search, sort, ofs, always_root);
	html("'>");
	html_txt(name);
	html("</a>");
}

void cgit_index_link(const char *name, const char *title, const char *class,
		     const char *pattern, const char *sort, int ofs, int always_root)
{
	site_link(NULL, name, title, class, pattern, sort, ofs, always_root);
}

static void link_start(const char *url, const char *title, const char *class)
{
	html("<a");
	if (title) {
		html(" title='");
		html_attr(title);
		html("'");
	}
	if (class) {
		html(" class='");
		html_attr(class);
		html("'");
	}
	html(" href='");
	html_attr(url);
}

static char *repolink(const char *title, const char *class, const char *page,
		      const char *head, const char *path)
{
	char *url = repo_view_url(ctx.repo->url, page, head, path);

	link_start(url, title, class);
	free(url);
	return "?";
}

static void reporevlink(const char *page, const char *name, const char *title,
			const char *class, const char *head, const char *rev,
			const char *path)
{
	char *url = repo_view_url(ctx.repo->url, page,
				  link_revision(head, rev), path);

	if (!strcmp(page ? page : "", "plain")) {
		char *raw = fmtalloc("%s?format=raw", url);
		free(url);
		url = raw;
	}
	link_start(url, title, class);
	free(url);
	html("'>");
	html_txt(name);
	html("</a>");
}

void cgit_summary_link(const char *name, const char *title, const char *class,
		       const char *head)
{
	reporevlink(NULL, name, title, class, head, NULL, NULL);
}

void cgit_tag_link(const char *name, const char *title, const char *class,
		   const char *tag)
{
	char *ref = starts_with(tag, "refs/tags/") ? xstrdup(tag) :
		fmtalloc("refs/tags/%s", tag);

	reporevlink("commit", name, title, class, ref, NULL, NULL);
	free(ref);
}

void cgit_tree_link(const char *name, const char *title, const char *class,
		    const char *head, const char *rev, const char *path)
{
	reporevlink("tree", name, title, class, head, rev, path);
}

void cgit_plain_link(const char *name, const char *title, const char *class,
		     const char *head, const char *rev, const char *path)
{
	reporevlink("plain", name, title, class, head, rev, path);
}

void cgit_blame_link(const char *name, const char *title, const char *class,
		     const char *head, const char *rev, const char *path)
{
	reporevlink("blame", name, title, class, head, rev, path);
}

void cgit_log_link(const char *name, const char *title, const char *class,
		   const char *head, const char *rev, const char *path,
		   int ofs, const char *grep, const char *pattern, int showmsg,
		   int follow)
{
	char *delim;

	delim = repolink(title, class, "log", link_revision(head, rev), path);
	if (grep && pattern) {
		html(delim);
		html("qt=");
		html_url_arg(grep);
		delim = "&amp;";
		html(delim);
		html("q=");
		html_url_arg(pattern);
	}
	if (ofs > 0) {
		html(delim);
		html("ofs=");
		htmlf("%d", ofs);
		delim = "&amp;";
	}
	if (showmsg) {
		html(delim);
		html("showmsg=1");
		delim = "&amp;";
	}
	if (follow) {
		html(delim);
		html("follow=1");
	}
	html("'>");
	html_txt(name);
	html("</a>");
}

void cgit_commit_link(const char *name, const char *title, const char *class,
		      const char *head, const char *rev, const char *path)
{
	char *delim;

	delim = repolink(title, class, "commit", link_revision(head, rev), NULL);
	if (ctx.qry.difftype) {
		html(delim);
		htmlf("dt=%d", ctx.qry.difftype);
		delim = "&amp;";
	}
	if (ctx.qry.context > 0 && ctx.qry.context != 3) {
		html(delim);
		html("context=");
		htmlf("%d", ctx.qry.context);
		delim = "&amp;";
	}
	if (ctx.qry.ignorews) {
		html(delim);
		html("ignorews=1");
		delim = "&amp;";
	}
	if (ctx.qry.follow) {
		html(delim);
		html("follow=1");
	}
	html("'>");
	if (name[0] != '\0') {
		if (strlen(name) > ctx.cfg.max_msg_len && ctx.cfg.max_msg_len >= 15) {
			html_ntxt(name, ctx.cfg.max_msg_len - 3);
			html("...");
		} else
			html_txt(name);
	} else
		html_txt("(no commit message)");
	html("</a>");
}

void cgit_refs_link(const char *name, const char *title, const char *class,
		    const char *head, const char *rev, const char *path)
{
	reporevlink("refs", name, title, class, head, rev, path);
}

void cgit_snapshot_link(const char *name, const char *title, const char *class,
			const char *head, const char *rev,
			const char *archivename)
{
	const char *format = NULL;
	const struct cgit_snapshot_format *f;
	char *url, *full;
	int signature = ends_with(archivename, ".asc");

	for (f = cgit_snapshot_formats; f->suffix; f++)
		if (strstr(archivename, f->suffix)) {
			format = f->suffix + 1;
			break;
		}
	if (!format)
		return;
	url = repo_view_url(ctx.repo->url,
			    signature ? "archive-signature" : "archive",
			    link_revision(head, rev), NULL);
	full = fmtalloc("%s?format=%s", url, format);
	free(url);
	link_start(full, title, class);
	free(full);
	html("'>");
	html_txt(name);
	html("</a>");
}

void cgit_diff_link(const char *name, const char *title, const char *class,
		    const char *head, const char *new_rev, const char *old_rev,
		    const char *path)
{
	struct strbuf url = STRBUF_INIT;
	char *delim = "?";

	add_repo_url(&url, ctx.repo->url);
	strbuf_addstr(&url, "/+/diff/");
	add_revision(&url, old_rev ? old_rev : "HEAD");
	strbuf_addstr(&url, "..");
	add_revision(&url, link_revision(head, new_rev));
	if (path) {
		strbuf_addstr(&url, "/+/");
		add_url_path(&url, path);
	}
	link_start(url.buf, title, class);
	strbuf_release(&url);
	if (ctx.qry.difftype) {
		html(delim);
		htmlf("dt=%d", ctx.qry.difftype);
		delim = "&amp;";
	}
	if (ctx.qry.context > 0 && ctx.qry.context != 3) {
		html(delim);
		html("context=");
		htmlf("%d", ctx.qry.context);
		delim = "&amp;";
	}
	if (ctx.qry.ignorews) {
		html(delim);
		html("ignorews=1");
		delim = "&amp;";
	}
	if (ctx.qry.follow) {
		html(delim);
		html("follow=1");
	}
	html("'>");
	html_txt(name);
	html("</a>");
}

void cgit_patch_link(const char *name, const char *title, const char *class,
		     const char *head, const char *rev, const char *path)
{
	struct strbuf url = STRBUF_INIT;
	struct object_id oid;
	struct commit *commit;
	const char *new_rev = link_revision(head, rev);
	char *old_rev = xstrdup("HEAD");

	if (!repo_get_oid(the_repository, new_rev, &oid) &&
	    (commit = lookup_commit_reference(the_repository, &oid)) &&
	    commit->parents) {
		free(old_rev);
		old_rev = xstrdup(oid_to_hex(&commit->parents->item->object.oid));
	}
	add_repo_url(&url, ctx.repo->url);
	strbuf_addstr(&url, "/+/patch/");
	add_revision(&url, old_rev);
	strbuf_addstr(&url, "..");
	add_revision(&url, new_rev);
	if (path) {
		strbuf_addstr(&url, "/+/");
		add_url_path(&url, path);
	}
	link_start(url.buf, title, class);
	strbuf_release(&url);
	free(old_rev);
	html("'>");
	html_txt(name);
	html("</a>");
}

void cgit_stats_link(const char *name, const char *title, const char *class,
		     const char *head, const char *path)
{
	reporevlink("stats", name, title, class, head, NULL, path);
}

static void cgit_self_link(char *name, const char *title, const char *class)
{
	if (!strcmp(ctx.qry.page, "repolist"))
		cgit_index_link(name, title, class, ctx.qry.search, ctx.qry.sort,
				ctx.qry.ofs, 1);
	else if (!strcmp(ctx.qry.page, "summary"))
		cgit_summary_link(name, title, class, ctx.qry.head);
	else if (!strcmp(ctx.qry.page, "tag"))
		cgit_tag_link(name, title, class, ctx.qry.has_oid ?
			       ctx.qry.oid : ctx.qry.head);
	else if (!strcmp(ctx.qry.page, "tree"))
		cgit_tree_link(name, title, class, ctx.qry.head,
			       ctx.qry.has_oid ? ctx.qry.oid : NULL,
			       ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "plain"))
		cgit_plain_link(name, title, class, ctx.qry.head,
				ctx.qry.has_oid ? ctx.qry.oid : NULL,
				ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "blame"))
		cgit_blame_link(name, title, class, ctx.qry.head,
				ctx.qry.has_oid ? ctx.qry.oid : NULL,
				ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "log"))
		cgit_log_link(name, title, class, ctx.qry.head,
			      ctx.qry.has_oid ? ctx.qry.oid : NULL,
			      ctx.qry.path, ctx.qry.ofs,
			      ctx.qry.grep, ctx.qry.search,
			      ctx.qry.showmsg, ctx.qry.follow);
	else if (!strcmp(ctx.qry.page, "commit") || !strcmp(ctx.qry.page, "revision"))
		cgit_commit_link(name, title, class, ctx.qry.head,
				 ctx.qry.has_oid ? ctx.qry.oid : NULL,
				 ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "patch"))
		cgit_patch_link(name, title, class, ctx.qry.head,
				ctx.qry.has_oid ? ctx.qry.oid : NULL,
				ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "refs"))
		cgit_refs_link(name, title, class, ctx.qry.head,
			       ctx.qry.has_oid ? ctx.qry.oid : NULL,
			       ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "snapshot"))
		cgit_snapshot_link(name, title, class, ctx.qry.head,
				   ctx.qry.has_oid ? ctx.qry.oid : NULL,
				   ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "diff"))
		cgit_diff_link(name, title, class, ctx.qry.head,
			       ctx.qry.oid, ctx.qry.oid2,
			       ctx.qry.path);
	else if (!strcmp(ctx.qry.page, "stats"))
		cgit_stats_link(name, title, class, ctx.qry.head,
				ctx.qry.path);
	else {
		/* Don't known how to make link for this page */
		repolink(title, class, ctx.qry.page, ctx.qry.head, ctx.qry.path);
		html("><!-- cgit_self_link() doesn't know how to make link for page '");
		html_txt(ctx.qry.page);
		html("' -->");
		html_txt(name);
		html("</a>");
	}
}

void cgit_object_link(struct object *obj)
{
	char *page, *shortrev, *fullrev, *name;

	fullrev = oid_to_hex(&obj->oid);
	shortrev = xstrdup(fullrev);
	shortrev[10] = '\0';
	if (obj->type == OBJ_COMMIT) {
		cgit_commit_link(fmt("commit %s...", shortrev), NULL, NULL,
				 ctx.qry.head, fullrev, NULL);
		return;
	} else if (obj->type == OBJ_TREE)
		page = "tree";
	else if (obj->type == OBJ_TAG)
		page = "tag";
	else
		page = "blob";
	name = fmt("%s %s...", type_name(obj->type), shortrev);
	reporevlink(page, name, NULL, NULL, ctx.qry.head, fullrev, NULL);
}

static struct string_list_item *lookup_path(struct string_list *list,
					    const char *path)
{
	struct string_list_item *item;

	while (path && path[0]) {
		if ((item = string_list_lookup(list, path)))
			return item;
		if (!(path = strchr(path, '/')))
			break;
		path++;
	}
	return NULL;
}

void cgit_submodule_link(const char *class, char *path, const char *rev)
{
	struct string_list *list;
	struct string_list_item *item;
	char tail, *dir;
	size_t len;

	len = 0;
	tail = 0;
	list = &ctx.repo->submodules;
	item = lookup_path(list, path);
	if (!item) {
		len = strlen(path);
		tail = path[len - 1];
		if (tail == '/') {
			path[len - 1] = 0;
			item = lookup_path(list, path);
		}
	}
	if (item || ctx.repo->module_link) {
		html("<a ");
		if (class)
			htmlf("class='%s' ", class);
		html("href='");
		if (item) {
			html_attrf(item->util, rev);
		} else {
			dir = strrchr(path, '/');
			if (dir)
				dir++;
			else
				dir = path;
			html_attrf(ctx.repo->module_link, dir, rev);
		}
		html("'>");
		html_txt(path);
		html("</a>");
	} else {
		html("<span");
		if (class)
			htmlf(" class='%s'", class);
		html(">");
		html_txt(path);
		html("</span>");
	}
	html_txtf(" @ %.7s", rev);
	if (item && tail)
		path[len - 1] = tail;
}

const struct date_mode cgit_date_mode(enum date_mode_type type)
{
	static struct date_mode mode;
	mode.type = type;
	mode.local = ctx.cfg.local_time;
	return mode;
}

static void print_rel_date(time_t t, int tz, double value,
	const char *class, const char *suffix)
{
	htmlf("<span class='%s' data-ut='%" PRIu64 "' title='", class, (uint64_t)t);
	html_attr(show_date(t, tz, cgit_date_mode(DATE_ISO8601)));
	htmlf("'>%.0f %s</span>", value, suffix);
}

void cgit_print_age(time_t t, int tz, time_t max_relative)
{
	time_t now, secs;

	if (!t)
		return;
	time(&now);
	secs = now - t;
	if (secs < 0)
		secs = 0;

	if (secs > max_relative && max_relative >= 0) {
		html("<span title='");
		html_attr(show_date(t, tz, cgit_date_mode(DATE_ISO8601)));
		html("'>");
		html_txt(show_date(t, tz, cgit_date_mode(DATE_SHORT)));
		html("</span>");
		return;
	}

	if (secs < TM_HOUR * 2) {
		print_rel_date(t, tz, secs * 1.0 / TM_MIN, "age-mins", "min.");
		return;
	}
	if (secs < TM_DAY * 2) {
		print_rel_date(t, tz, secs * 1.0 / TM_HOUR, "age-hours", "hours");
		return;
	}
	if (secs < TM_WEEK * 2) {
		print_rel_date(t, tz, secs * 1.0 / TM_DAY, "age-days", "days");
		return;
	}
	if (secs < TM_MONTH * 2) {
		print_rel_date(t, tz, secs * 1.0 / TM_WEEK, "age-weeks", "weeks");
		return;
	}
	if (secs < TM_YEAR * 2) {
		print_rel_date(t, tz, secs * 1.0 / TM_MONTH, "age-months", "months");
		return;
	}
	print_rel_date(t, tz, secs * 1.0 / TM_YEAR, "age-years", "years");
}

void cgit_print_http_headers(void)
{
	if (ctx.page.status)
		htmlf("Status: %d %s\n", ctx.page.status, ctx.page.statusmsg);
	if (ctx.page.mimetype && ctx.page.charset)
		htmlf("Content-Type: %s; charset=%s\n", ctx.page.mimetype,
		      ctx.page.charset);
	else if (ctx.page.mimetype)
		htmlf("Content-Type: %s\n", ctx.page.mimetype);
	if (ctx.page.size)
		htmlf("Content-Length: %zd\n", ctx.page.size);
	if (ctx.page.filename) {
		html("Content-Disposition: inline; filename=\"");
		html_header_arg_in_quotes(ctx.page.filename);
		html("\"\n");
	}
	htmlf("Last-Modified: %s\n", http_date(ctx.page.modified));
	if (ctx.page.etag)
		htmlf("ETag: \"%s\"\n", ctx.page.etag);
	html("\n");
	if (ctx.env.request_method && !strcmp(ctx.env.request_method, "HEAD"))
		exit(0);
}

void cgit_redirect(const char *url, bool permanent)
{
	htmlf("Status: %d %s\n", permanent ? 301 : 302, permanent ? "Moved" : "Found");
	html("Location: ");
	html_url_path(url);
	html("\n\n");
}

static void print_rel_vcs_link(const char *url)
{
	html("<link rel='vcs-git' href='");
	html_attr(url);
	html("' title='");
	html_attr(ctx.repo->name);
	html(" Git repository'/>\n");
}

static int emit_css_link(struct string_list_item *s, void *arg)
{
	/* Do not emit anything if css= is specified. */
	if (s && *s->string == '\0')
		return 0;

	html("<link rel='stylesheet' type='text/css' href='");
	if (s)
		html_attr(s->string);
	else
		html_attr((const char *)arg);
	html("'/>\n");

	return 0;
}

static int emit_js_link(struct string_list_item *s, void *arg)
{
	/* Do not emit anything if js= is specified. */
	if (s && *s->string == '\0')
		return 0;

	html("<script type='text/javascript' src='");
	if (s)
		html_attr(s->string);
	else
		html_attr((const char *)arg);
	html("'></script>\n");

	return 0;
}

void cgit_print_docstart(void)
{
	char *host = cgit_hosturl();

	if (ctx.cfg.embedded) {
		if (ctx.cfg.header)
			html_include(ctx.cfg.header);
		return;
	}

	html(cgit_doctype);
	html("<html lang='en'>\n");
	html("<head>\n");
	html("<title>");
	html_txt(ctx.page.title);
	html("</title>\n");
	htmlf("<meta name='generator' content='cgit %s'/>\n", cgit_version);
	if (ctx.cfg.robots && *ctx.cfg.robots)
		htmlf("<meta name='robots' content='%s'/>\n", ctx.cfg.robots);

	if (ctx.cfg.css.items)
		for_each_string_list(&ctx.cfg.css, emit_css_link, NULL);
	else
		emit_css_link(NULL, "/-/assets/cgit.css");

	if (ctx.cfg.js.items)
		for_each_string_list(&ctx.cfg.js, emit_js_link, NULL);
	else
		emit_js_link(NULL, "/-/assets/cgit.js");

	if (ctx.cfg.favicon && *ctx.cfg.favicon) {
		html("<link rel='shortcut icon' href='");
		html_attr(ctx.cfg.favicon);
		html("'/>\n");
	}
	if (host && ctx.repo && ctx.qry.head && starts_with(ctx.qry.head, "refs/")) {
		char *fileurl;

		html("<link rel='alternate' title='Atom feed' href='");
		html(cgit_httpscheme());
		html_attr(host);
		fileurl = cgit_fileurl(ctx.repo->url, "atom", ctx.qry.vpath, NULL);
		html_attr(fileurl);
		html("' type='application/atom+xml'/>\n");
		free(fileurl);
	}
	if (ctx.repo)
		cgit_add_clone_urls(print_rel_vcs_link);
	if (ctx.cfg.head_include)
		html_include(ctx.cfg.head_include);
	if (ctx.repo && ctx.repo->extra_head_content)
		html(ctx.repo->extra_head_content);
	html("</head>\n");
	html("<body>\n");
	if (ctx.cfg.header)
		html_include(ctx.cfg.header);
	free(host);
}

void cgit_print_docend(void)
{
	html("</div> <!-- class=content -->\n");
	if (ctx.cfg.embedded) {
		html("</div> <!-- id=cgit -->\n");
		if (ctx.cfg.footer)
			html_include(ctx.cfg.footer);
		return;
	}
	if (ctx.cfg.footer)
		html_include(ctx.cfg.footer);
	else {
		htmlf("<div class='footer'>generated by <a href='https://git.zx2c4.com/cgit/about/'>cgit %s</a> "
			"(<a href='https://git-scm.com/'>git %s</a>) at ", cgit_version, git_version_string);
		html_txt(show_date(time(NULL), 0, cgit_date_mode(DATE_ISO8601)));
		html("</div>\n");
	}
	html("</div> <!-- id=cgit -->\n");
	html("</body>\n</html>\n");
}

void cgit_print_error_page(int code, const char *msg, const char *fmt, ...)
{
	va_list ap;
	va_start(ap, fmt);
	cgit_vprint_error_page(code, msg, fmt, ap);
	va_end(ap);
}

void cgit_vprint_error_page(int code, const char *msg, const char *fmt, va_list ap)
{
	ctx.page.status = code;
	ctx.page.statusmsg = msg;
	cgit_print_layout_start();
	cgit_vprint_error(fmt, ap);
	cgit_print_layout_end();
}

void cgit_print_layout_start(void)
{
	cgit_print_http_headers();
	cgit_print_docstart();
	cgit_print_pageheader();
}

void cgit_print_layout_end(void)
{
	cgit_print_docend();
}

static void add_clone_urls(void (*fn)(const char *), char *txt, char *suffix)
{
	struct strbuf **url_list = strbuf_split_str(txt, ' ', 0);
	int i;

	for (i = 0; url_list[i]; i++) {
		strbuf_rtrim(url_list[i]);
		if (url_list[i]->len == 0)
			continue;
		if (suffix && *suffix)
			strbuf_addf(url_list[i], "/%s", suffix);
		fn(url_list[i]->buf);
	}

	strbuf_list_free(url_list);
}

void cgit_add_clone_urls(void (*fn)(const char *))
{
	if (ctx.repo->clone_url)
		add_clone_urls(fn, expand_macros(ctx.repo->clone_url), NULL);
	else if (ctx.cfg.clone_prefix) {
		char *suffix = fmtalloc("%s.git", ctx.repo->url);
		add_clone_urls(fn, ctx.cfg.clone_prefix, suffix);
		free(suffix);
	} else {
		char *host = cgit_hosturl();
		struct strbuf url = STRBUF_INIT;

		if (!host)
			return;
		strbuf_addf(&url, "%s%s", cgit_httpscheme(), host);
		add_repo_url(&url, ctx.repo->url);
		strbuf_addstr(&url, ".git");
		fn(url.buf);
		strbuf_release(&url);
		free(host);
	}
}

static int print_branch_option(const struct reference *ref, void *cb_data UNUSED)
{
	char *url = repo_view_url(ctx.repo->url, "tree", ref->name,
				  ctx.qry.vpath);

	html_option(url, ref->name,
		    ctx.qry.head && !strcmp(ref->name, ctx.qry.head) ? url : NULL);
	free(url);
	return 0;
}

void cgit_add_hidden_formfields(int incl_head UNUSED, int incl_search,
				const char *page UNUSED)
{
	if (ctx.qry.showmsg)
		html_hidden("showmsg", "1");

	if (incl_search) {
		if (ctx.qry.grep)
			html_hidden("qt", ctx.qry.grep);
		if (ctx.qry.search)
			html_hidden("q", ctx.qry.search);
	}
}

static const char *hc(const char *page)
{
	if (!ctx.qry.page)
		return NULL;

	return strcmp(ctx.qry.page, page) ? NULL : "active";
}

static void cgit_print_path_crumbs(char *path)
{
	char *old_path = ctx.qry.path;
	char *p = path, *q, *end = path + strlen(path);
	int blame = !strcmp(ctx.qry.page, "blame");
	int levels = 0;

	ctx.qry.path = NULL;
	if (blame)
		cgit_tree_link("root", NULL, NULL, ctx.qry.head,
			       ctx.qry.oid, NULL);
	else
		cgit_self_link("root", NULL, NULL);
	ctx.qry.path = p = path;
	while (p < end) {
		if (!(q = strchr(p, '/')) || levels > 15)
			q = end;
		*q = '\0';
		html_txt("/");
		if (blame && q < end)
			cgit_tree_link(p, NULL, NULL, ctx.qry.head,
				       ctx.qry.oid, ctx.qry.path);
		else
			cgit_self_link(p, NULL, NULL);
		if (q < end)
			*q = '/';
		p = q + 1;
		++levels;
	}
	ctx.qry.path = old_path;
}

static void print_header(void)
{
	char *logo = NULL, *logo_link = NULL;

	html("<table id='header'>\n");
	html("<tr>\n");

	if (ctx.repo && ctx.repo->logo && *ctx.repo->logo)
		logo = ctx.repo->logo;
	else
		logo = ctx.cfg.logo;
	if (ctx.repo && ctx.repo->logo_link && *ctx.repo->logo_link)
		logo_link = ctx.repo->logo_link;
	else
		logo_link = ctx.cfg.logo_link;
	if (logo && *logo) {
		html("<td class='logo' rowspan='2'><a href='");
		if (logo_link && *logo_link)
			html_attr(logo_link);
		else
			html_attr(cgit_rooturl());
		html("'><img src='");
		html_attr(logo);
		html("' alt='cgit logo'/></a></td>\n");
	}

	html("<td class='main'>");
	if (ctx.repo) {
		cgit_index_link("index", NULL, NULL, NULL, NULL, 0, 1);
		html(" : ");
		cgit_summary_link(ctx.repo->name, NULL, NULL, NULL);
		html("</td><td class='form'>");
		html("<select onchange='window.location.href=this.value'>\n");
		refs_for_each_branch_ref(get_main_ref_store(the_repository),
					 print_branch_option, ctx.qry.head);
		if (ctx.repo->enable_remote_branches)
			refs_for_each_remote_ref(get_main_ref_store(the_repository),
						 print_branch_option, ctx.qry.head);
		html("</select>");
	} else
		html_txt(ctx.cfg.root_title);
	html("</td></tr>\n");

	html("<tr><td class='sub'>");
	if (ctx.repo) {
		html_txt(ctx.repo->desc);
		html("</td><td class='sub right'>");
		html_txt(ctx.repo->owner);
	} else {
		if (ctx.cfg.root_desc)
			html_txt(ctx.cfg.root_desc);
	}
	html("</td></tr></table>\n");
}

void cgit_print_pageheader(void)
{
	html("<div id='cgit'>");
	if (!ctx.cfg.noheader)
		print_header();

	html("<table class='tabs'><tr><td>\n");
	if (ctx.repo) {
		if (ctx.repo->readme.nr)
			reporevlink("about", "about", NULL,
				    hc("about"), ctx.qry.head, NULL,
				    NULL);
		cgit_summary_link("summary", NULL, hc("summary"),
				  ctx.qry.head);
		cgit_refs_link("refs", NULL, hc("refs"), ctx.qry.head,
			       ctx.qry.oid, NULL);
		cgit_log_link("log", NULL, hc("log"), ctx.qry.head,
			      NULL, ctx.qry.vpath, 0, NULL, NULL,
			      ctx.qry.showmsg, ctx.qry.follow);
		if (ctx.qry.page && !strcmp(ctx.qry.page, "blame"))
			cgit_blame_link("blame", NULL, hc("blame"), ctx.qry.head,
				        ctx.qry.oid, ctx.qry.vpath);
		else
			cgit_tree_link("tree", NULL, hc("tree"), ctx.qry.head,
				       ctx.qry.oid, ctx.qry.vpath);
		cgit_commit_link("commit", NULL,
				 ctx.qry.page && !strcmp(ctx.qry.page, "revision") ? "active" : hc("commit"),
				 ctx.qry.head, ctx.qry.oid, ctx.qry.vpath);
		cgit_diff_link("diff", NULL, hc("diff"), ctx.qry.head,
			       ctx.qry.oid, ctx.qry.oid2, ctx.qry.vpath);
		if (ctx.repo->max_stats)
			cgit_stats_link("stats", NULL, hc("stats"),
					ctx.qry.head, ctx.qry.vpath);
		if (ctx.repo->homepage) {
			html("<a href='");
			html_attr(ctx.repo->homepage);
			html("'>homepage</a>");
		}
		html("</td><td class='form'>");
		html("<form class='right' method='get' action='");
		if (ctx.cfg.virtual_root) {
			char *fileurl = cgit_fileurl(ctx.qry.repo, "log",
						   ctx.qry.vpath, NULL);
			html_attr(fileurl);
			free(fileurl);
		}
		html("'>\n");
		cgit_add_hidden_formfields(1, 0, "log");
		html("<select name='qt'>\n");
		html_option("grep", "log msg", ctx.qry.grep);
		html_option("author", "author", ctx.qry.grep);
		html_option("committer", "committer", ctx.qry.grep);
		html_option("range", "range", ctx.qry.grep);
		html("</select>\n");
		html("<input class='txt' type='search' size='10' name='q' value='");
		html_attr(ctx.qry.search);
		html("'/>\n");
		html("<input type='submit' value='search'/>\n");
		html("</form>\n");
	} else {
		char *currenturl = cgit_currenturl();
		site_link(NULL, "index", NULL, hc("repolist"), NULL, NULL, 0, 1);
		if (ctx.cfg.root_readme)
			site_link("about", "about", NULL, hc("about"),
				  NULL, NULL, 0, 1);
		html("</td><td class='form'>");
		html("<form method='get' action='");
		html_attr(currenturl);
		html("'>\n");
		html("<input type='search' name='q' size='10' value='");
		html_attr(ctx.qry.search);
		html("'/>\n");
		html("<input type='submit' value='search'/>\n");
		html("</form>");
		free(currenturl);
	}
	html("</td></tr></table>\n");
	if (ctx.repo && ctx.qry.vpath) {
		html("<div class='path'>");
		html("path: ");
		cgit_print_path_crumbs(ctx.qry.vpath);
		if (ctx.repo->enable_follow_links && !strcmp(ctx.qry.page, "log")) {
			html(" (");
			ctx.qry.follow = !ctx.qry.follow;
			cgit_self_link(ctx.qry.follow ? "follow" : "unfollow",
					NULL, NULL);
			ctx.qry.follow = !ctx.qry.follow;
			html(")");
		}
		html("</div>");
	}
	html("<div class='content'>");
}

void cgit_print_filemode(unsigned short mode)
{
	if (S_ISDIR(mode))
		html("d");
	else if (S_ISLNK(mode))
		html("l");
	else if (S_ISGITLINK(mode))
		html("m");
	else
		html("-");
	html_fileperm(mode >> 6);
	html_fileperm(mode >> 3);
	html_fileperm(mode);
}

void cgit_compose_snapshot_prefix(struct strbuf *filename, const char *base,
				  const char *ref)
{
	struct object_id oid;

	/*
	 * Prettify snapshot names by stripping leading "v" or "V" if the tag
	 * name starts with {v,V}[0-9] and the prettify mapping is injective,
	 * i.e. each stripped tag can be inverted without ambiguities.
	 */
	if (repo_get_oid(the_repository, fmt("refs/tags/%s", ref), &oid) == 0 &&
	    (ref[0] == 'v' || ref[0] == 'V') && isdigit(ref[1]) &&
	    ((repo_get_oid(the_repository, fmt("refs/tags/%s", ref + 1), &oid) == 0) +
	     (repo_get_oid(the_repository, fmt("refs/tags/v%s", ref + 1), &oid) == 0) +
	     (repo_get_oid(the_repository, fmt("refs/tags/V%s", ref + 1), &oid) == 0) == 1))
		ref++;

	strbuf_addf(filename, "%s-%s", base, ref);
}

void cgit_print_snapshot_links(const struct cgit_repo *repo, const char *ref,
			       const char *separator)
{
	const struct cgit_snapshot_format *f;
	struct strbuf filename = STRBUF_INIT;
	const char *basename;
	size_t prefixlen;

	basename = cgit_snapshot_prefix(repo);
	if (starts_with(ref, basename))
		strbuf_addstr(&filename, ref);
	else
		cgit_compose_snapshot_prefix(&filename, basename, ref);

	prefixlen = filename.len;
	for (f = cgit_snapshot_formats; f->suffix; f++) {
		if (!(repo->snapshots & cgit_snapshot_format_bit(f)))
			continue;
		strbuf_setlen(&filename, prefixlen);
		strbuf_addstr(&filename, f->suffix);
		cgit_snapshot_link(filename.buf, NULL, NULL, NULL, NULL,
				   filename.buf);
		if (cgit_snapshot_get_sig(ref, f)) {
			strbuf_addstr(&filename, ".asc");
			html(" (");
			cgit_snapshot_link("sig", NULL, NULL, NULL, NULL,
					   filename.buf);
			html(")");
		} else if (starts_with(f->suffix, ".tar") && cgit_snapshot_get_sig(ref, &cgit_snapshot_formats[0])) {
			strbuf_setlen(&filename, strlen(filename.buf) - strlen(f->suffix));
			strbuf_addstr(&filename, ".tar.asc");
			html(" (");
			cgit_snapshot_link("sig", NULL, NULL, NULL, NULL,
					   filename.buf);
			html(")");
		}
		html(separator);
	}
	strbuf_release(&filename);
}

void cgit_set_title_from_path(const char *path)
{
	struct strbuf sb = STRBUF_INIT;
	const char *slash, *last_slash;

	if (!path)
		return;

	for (last_slash = path + strlen(path); (slash = memrchr(path, '/', last_slash - path)) != NULL; last_slash = slash) {
		strbuf_add(&sb, slash + 1, last_slash - slash - 1);
		strbuf_addstr(&sb, " \xc2\xab ");
	}
	strbuf_add(&sb, path, last_slash - path);
	strbuf_addf(&sb, " - %s", ctx.page.title);
	ctx.page.title = strbuf_detach(&sb, NULL);
}
