/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

#ifndef UI_LOG_H
#define UI_LOG_H

extern void cgit_print_log(const char *tip, int ofs, int cnt, char *grep,
			   char *pattern, const char *path, int pager,
			   int commit_graph, int commit_sort);
extern void show_commit_decorations(struct commit *commit);

#endif /* UI_LOG_H */
