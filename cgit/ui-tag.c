/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* ui-tag.c: display a tag
 *
 * Copyright (C) 2006-2014 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#define USE_THE_REPOSITORY_VARIABLE

#include "cgit.h"
#include "ui-tag.h"
#include "html.h"
#include "ui-shared.h"

static void print_tag_content(char *buf)
{
	char *p;

	if (!buf)
		return;

	html("<div class='commit-subject'>");
	p = strchr(buf, '\n');
	if (p)
		*p = '\0';
	html_txt(buf);
	html("</div>");
	if (p) {
		html("<div class='commit-msg'>");
		html_txt(++p);
		html("</div>");
	}
}

static void print_download_links(char *revname)
{
	html("<tr><th>download</th><td class='oid'>");
	cgit_print_snapshot_links(ctx.repo, revname, "<br/>");
	html("</td></tr>");
}

void cgit_print_tag(char *revname)
{
	struct strbuf fullref = STRBUF_INIT;
	struct object_id oid;
	struct object *obj;
	const char *displayname;

	if (!revname)
		revname = ctx.qry.head;

	if (skip_prefix(revname, "refs/tags/", &displayname))
		strbuf_addstr(&fullref, revname);
	else {
		displayname = revname;
		strbuf_addf(&fullref, "refs/tags/%s", revname);
	}
	if (repo_get_oid(the_repository, fullref.buf, &oid)) {
		cgit_print_error_page(404, "Not found",
			"Bad tag reference: %s", displayname);
		goto cleanup;
	}
	obj = parse_object(the_repository, &oid);
	if (!obj) {
		cgit_print_error_page(500, "Internal server error",
			"Bad object id: %s", oid_to_hex(&oid));
		goto cleanup;
	}
	if (obj->type == OBJ_TAG) {
		struct tag *tag;
		struct taginfo *info;

		tag = lookup_tag(the_repository, &oid);
		if (!tag || parse_tag(the_repository, tag) || !(info = cgit_parse_tag(tag))) {
			cgit_print_error_page(500, "Internal server error",
				"Bad tag object: %s", displayname);
			goto cleanup;
		}
		cgit_print_layout_start();
		html("<table class='commit-info'>\n");
		html("<tr><td>tag name</td><td>");
		html_txt(displayname);
		htmlf(" (%s)</td></tr>\n", oid_to_hex(&oid));
		if (info->tagger_date > 0) {
			html("<tr><td>tag date</td><td>");
			html_txt(show_date(info->tagger_date, info->tagger_tz,
						cgit_date_mode(DATE_ISO8601)));
			html("</td></tr>\n");
		}
		if (info->tagger) {
			html("<tr><td>tagged by</td><td>");
			html_txt(info->tagger);
			if (info->tagger_email && !ctx.cfg.noplainemail) {
				html(" ");
				html_txt(info->tagger_email);
			}
			html("</td></tr>\n");
		}
		html("<tr><td>tagged object</td><td class='oid'>");
		cgit_object_link(tag->tagged);
		html("</td></tr>\n");
		if (ctx.repo->snapshots)
			print_download_links(revname);
		html("</table>\n");
		print_tag_content(info->msg);
		cgit_print_layout_end();
		cgit_free_taginfo(info);
	} else {
		cgit_print_layout_start();
		html("<table class='commit-info'>\n");
		html("<tr><td>tag name</td><td>");
		html_txt(displayname);
		html("</td></tr>\n");
		html("<tr><td>tagged object</td><td class='oid'>");
		cgit_object_link(obj);
		html("</td></tr>\n");
		if (ctx.repo->snapshots)
			print_download_links(revname);
		html("</table>\n");
		cgit_print_layout_end();
	}

cleanup:
	strbuf_release(&fullref);
}
