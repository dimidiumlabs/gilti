/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

/* scan-tree.c
 *
 * Copyright (C) 2006-2014 cgit Development Team <cgit@lists.zx2c4.com>
 *
 * Licensed under GNU General Public License v2
 *   (see COPYING for full license text)
 */

#include "cgit.h"
#include "scan-tree.h"
#include "html.h"

/* return 1 if path contains a objects/ directory and a HEAD file */
static int is_git_dir(const char *path)
{
	struct stat st;
	struct strbuf pathbuf = STRBUF_INIT;
	int result = 0;

	strbuf_addf(&pathbuf, "%s/objects", path);
	if (stat(pathbuf.buf, &st)) {
		if (errno != ENOENT)
			fprintf(stderr, "Error checking path %s: %s (%d)\n",
				path, strerror(errno), errno);
		goto out;
	}
	if (!S_ISDIR(st.st_mode))
		goto out;

	strbuf_reset(&pathbuf);
	strbuf_addf(&pathbuf, "%s/HEAD", path);
	if (stat(pathbuf.buf, &st)) {
		if (errno != ENOENT)
			fprintf(stderr, "Error checking path %s: %s (%d)\n",
				path, strerror(errno), errno);
		goto out;
	}
	if (!S_ISREG(st.st_mode))
		goto out;

	result = 1;
out:
	strbuf_release(&pathbuf);
	return result;
}

static char *xstrrchr(char *s, char *from, int c)
{
	while (from >= s && *from != c)
		from--;
	return from < s ? NULL : from;
}

static void add_repo(const char *base, struct strbuf *path)
{
	struct stat st;
	struct passwd *pwd;
	struct cgit_repo *repo;
	size_t pathlen;
	struct strbuf rel = STRBUF_INIT;
	char *p, *slash;
	int n;
	size_t size;

	if (stat(path->buf, &st)) {
		fprintf(stderr, "Error accessing %s: %s (%d)\n",
			path->buf, strerror(errno), errno);
		return;
	}

	strbuf_addch(path, '/');
	pathlen = path->len;

	if (ctx.cfg.strict_export) {
		strbuf_addstr(path, ctx.cfg.strict_export);
		if(stat(path->buf, &st))
			return;
		strbuf_setlen(path, pathlen);
	}

	strbuf_addstr(path, "noweb");
	if (!stat(path->buf, &st))
		return;
	strbuf_setlen(path, pathlen);

	if (!starts_with(path->buf, base))
		strbuf_addbuf(&rel, path);
	else
		strbuf_addstr(&rel, path->buf + strlen(base) + 1);

	if (!strcmp(rel.buf + rel.len - 5, "/.git"))
		strbuf_setlen(&rel, rel.len - 5);
	else if (rel.len && rel.buf[rel.len - 1] == '/')
		strbuf_setlen(&rel, rel.len - 1);

	repo = cgit_add_repo(rel.buf);

	if (ctx.cfg.remove_suffix) {
		size_t urllen;
		strip_suffix(repo->url, ".git", &urllen);
		strip_suffix_mem(repo->url, &urllen, "/");
		repo->url[urllen] = '\0';
	}
	repo->path = strdup_first_line(path->buf);
	while (!repo->owner) {
		if ((pwd = getpwuid(st.st_uid)) == NULL) {
			fprintf(stderr, "Error reading owner-info for %s: %s (%d)\n",
				path->buf, strerror(errno), errno);
			break;
		}
		if (pwd->pw_gecos)
			if ((p = strchr(pwd->pw_gecos, ',')))
				*p = '\0';
		repo->owner = strdup_first_line(pwd->pw_gecos ? pwd->pw_gecos : pwd->pw_name);
	}

	if (repo->desc == cgit_default_repo_desc || !repo->desc) {
		strbuf_addstr(path, "description");
		if (!stat(path->buf, &st))
			read_first_line(path->buf, &repo->desc, &size);
		strbuf_setlen(path, pathlen);
	}

	if (ctx.cfg.section_from_path) {
		n = ctx.cfg.section_from_path;
		if (n > 0) {
			slash = rel.buf - 1;
			while (slash && n && (slash = strchr(slash + 1, '/')))
				n--;
		} else {
			slash = rel.buf + rel.len;
			while (slash && n && (slash = xstrrchr(rel.buf, slash - 1, '/')))
				n++;
		}
		if (slash && !n) {
			*slash = '\0';
			repo->section = strdup_first_line(rel.buf);
			*slash = '/';
			if (starts_with(repo->name, repo->section)) {
				repo->name += strlen(repo->section);
				if (*repo->name == '/')
					repo->name++;
			}
		}
	}

	strbuf_release(&rel);
}

static void scan_path(const char *base, const char *path)
{
	DIR *dir = opendir(path);
	struct dirent *ent;
	struct strbuf pathbuf = STRBUF_INIT;
	size_t pathlen = strlen(path);
	struct stat st;

	if (!dir) {
		fprintf(stderr, "Error opening directory %s: %s (%d)\n",
			path, strerror(errno), errno);
		return;
	}

	strbuf_add(&pathbuf, path, strlen(path));
	if (is_git_dir(pathbuf.buf)) {
		add_repo(base, &pathbuf);
		goto end;
	}
	strbuf_addstr(&pathbuf, "/.git");
	if (is_git_dir(pathbuf.buf)) {
		add_repo(base, &pathbuf);
		goto end;
	}
	/*
	 * Add one because we don't want to lose the trailing '/' when we
	 * reset the length of pathbuf in the loop below.
	 */
	pathlen++;
	while ((ent = readdir(dir)) != NULL) {
		if (ent->d_name[0] == '.') {
			if (ent->d_name[1] == '\0')
				continue;
			if (ent->d_name[1] == '.' && ent->d_name[2] == '\0')
				continue;
			if (!ctx.cfg.scan_hidden_path)
				continue;
		}
		strbuf_setlen(&pathbuf, pathlen);
		strbuf_addstr(&pathbuf, ent->d_name);
		if (stat(pathbuf.buf, &st)) {
			fprintf(stderr, "Error checking path %s: %s (%d)\n",
				pathbuf.buf, strerror(errno), errno);
			continue;
		}
		if (S_ISDIR(st.st_mode))
			scan_path(base, pathbuf.buf);
	}
end:
	strbuf_release(&pathbuf);
	closedir(dir);
}

void scan_tree(const char *path)
{
	scan_path(path, path);
}
