/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-FileCopyrightText: 2026 Nikolay Govorov
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* cmd.c: the cgit command dispatcher
 *
 * Copyright (C) 2006-2017 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#include "cgit.h"
#include "cmd.h"
#include "ui-shared.h"
#include "ui-atom.h"
#include "ui-commit.h"
#include "ui-diff.h"
#include "ui-log.h"
#include "ui-patch.h"
#include "ui-snapshot.h"
#include "ui-stats.h"

static void atom_fn(void)
{
	cgit_print_atom(ctx.qry.head, ctx.qry.path, ctx.cfg.max_atom_items);
}

static void revision_fn(void)
{
	cgit_print_commit(ctx.qry.oid, NULL);
}

static void diff_fn(void)
{
	cgit_print_diff(ctx.qry.oid, ctx.qry.oid2, ctx.qry.path, 1, 0);
}

static void rawdiff_fn(void)
{
	cgit_print_diff(ctx.qry.oid, ctx.qry.oid2, ctx.qry.path, 1, 1);
}

static void log_fn(void)
{
	cgit_print_log(ctx.qry.oid, ctx.qry.ofs, ctx.cfg.max_commit_count,
		       ctx.qry.grep, ctx.qry.search, ctx.qry.path, 1,
		       ctx.repo->enable_commit_graph,
		       ctx.repo->commit_sort);
}

static void patch_fn(void)
{
	cgit_print_patch(ctx.qry.oid, ctx.qry.oid2, ctx.qry.path);
}

static void snapshot_fn(void)
{
	char *filename;

	if (!ctx.qry.format) {
		cgit_print_error_page(400, "Bad request", "Archive format is required");
		return;
	}
	filename = fmtalloc("%s.%s%s", cgit_snapshot_prefix(ctx.repo),
			    ctx.qry.format, ctx.qry.signature ? ".asc" : "");
	cgit_print_snapshot(ctx.qry.head, ctx.qry.oid, filename, 0);
	free(filename);
}

static void stats_fn(void)
{
	cgit_show_stats();
}

#define def_cmd(name, want_repo, want_vpath) \
	{#name, name##_fn, want_repo, want_vpath}

struct cgit_cmd *cgit_get_cmd(void)
{
	static struct cgit_cmd cmds[] = {
		def_cmd(atom, 1, 0),
		def_cmd(diff, 1, 1),
		def_cmd(log, 1, 1),
		def_cmd(patch, 1, 1),
		def_cmd(rawdiff, 1, 1),
		def_cmd(revision, 1, 0),
		def_cmd(snapshot, 1, 0),
		def_cmd(stats, 1, 1),
	};
	int i;

	for (i = 0; i < sizeof(cmds)/sizeof(*cmds); i++)
		if (!strcmp(ctx.qry.page, cmds[i].name))
			return &cmds[i];
	return NULL;
}
