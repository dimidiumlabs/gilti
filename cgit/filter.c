/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* filter.c: executable filter used by snapshot compression
 *
 * Copyright (C) 2006-2014 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#include "cgit.h"

void cgit_exec_filter_init(struct cgit_exec_filter *filter, char *cmd, char **argv)
{
	memset(filter, 0, sizeof(*filter));
	filter->cmd = cmd;
	filter->argv = argv;
}

int cgit_open_exec_filter(struct cgit_exec_filter *filter)
{
	int pipe_fh[2];

	filter->old_stdout = chk_positive(dup(STDOUT_FILENO),
		"Unable to duplicate STDOUT");
	chk_zero(pipe(pipe_fh), "Unable to create pipe to subprocess");
	filter->pid = chk_non_negative(fork(), "Unable to create subprocess");
	if (filter->pid == 0) {
		close(pipe_fh[1]);
		chk_non_negative(dup2(pipe_fh[0], STDIN_FILENO),
			"Unable to use pipe as STDIN");
		execvp(filter->cmd, filter->argv);
		die_errno("Unable to exec subprocess %s", filter->cmd);
	}
	close(pipe_fh[0]);
	chk_non_negative(dup2(pipe_fh[1], STDOUT_FILENO),
		"Unable to use pipe as STDOUT");
	close(pipe_fh[1]);
	return 0;
}

int cgit_close_exec_filter(struct cgit_exec_filter *filter)
{
	int exit_status = 0;

	chk_non_negative(dup2(filter->old_stdout, STDOUT_FILENO),
		"Unable to restore STDOUT");
	close(filter->old_stdout);
	if (filter->pid < 0)
		return WEXITSTATUS(exit_status);
	waitpid(filter->pid, &exit_status, 0);
	if (!WIFEXITED(exit_status))
		die("Subprocess %s exited abnormally", filter->cmd);
	return WEXITSTATUS(exit_status);
}
