/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-FileCopyrightText: 2026 Nikolay Govorov
 * SPDX-License-Identifier: GPL-2.0-only
 */

#ifndef CGIT_H
#define CGIT_H

#include <stdbool.h>

#include <git-compat-util.h>

#include <archive.h>
#include <commit.h>
#include <diffcore.h>
#include <diff.h>
#include <environment.h>
#include <graph.h>
#include <grep.h>
#include <hex.h>
#include <log-tree.h>
#include <notes.h>
#include <object.h>
#include <object-name.h>
#include <odb.h>
#include <path.h>
#include <refs.h>
#include <revision.h>
#include <setup.h>
#include <string-list.h>
#include <strvec.h>
#include <tag.h>
#include <tree.h>
#include <utf8.h>
#include <wrapper.h>
#include <xdiff-interface.h>
#include <xdiff/xdiff.h>

/* Add isgraph(x) to Git's sane ctype support (see git-compat-util.h) */
#undef isgraph
#define isgraph(x) (isprint((x)) && !isspace((x)))


/*
 * Limits used for relative dates
 */
#define TM_MIN    60
#define TM_HOUR  (TM_MIN * 60)
#define TM_DAY   (TM_HOUR * 24)
#define TM_WEEK  (TM_DAY * 7)
#define TM_YEAR  (TM_DAY * 365)
#define TM_MONTH (TM_YEAR / 12.0)


/*
 * Default encoding
 */
#define PAGE_ENCODING "UTF-8"

#define BIT(x)	(1U << (x))

typedef void (*filepair_fn)(struct diff_filepair *pair);
typedef void (*linediff_fn)(char *line, int len);

typedef enum {
	DIFF_UNIFIED, DIFF_SSDIFF, DIFF_STATONLY
} diff_type;

struct cgit_repo {
	char *url;
	char *name;
	char *path;
	char *desc;
	char *extra_head_content;
	char *owner;
	char *homepage;
	char *defbranch;
	struct string_list readme;
	char *section;
	char *clone_url;
	char *logo;
	char *logo_link;
	int enable_commit_graph;
	int enable_follow_links;
	int enable_log_filecount;
	int enable_log_linecount;
	int enable_remote_branches;
	int commit_sort;
	time_t mtime;
	int hide;
	int ignore;
};

struct commitinfo {
	struct commit *commit;
	char *author;
	char *author_email;
	unsigned long author_date;
	int author_tz;
	char *committer;
	char *committer_email;
	unsigned long committer_date;
	int committer_tz;
	char *subject;
	char *msg;
	char *msg_encoding;
};

struct taginfo {
	char *tagger;
	char *tagger_email;
	unsigned long tagger_date;
	int tagger_tz;
	char *msg;
};

struct refinfo {
	const char *refname;
	struct object *object;
	union {
		struct taginfo *tag;
		struct commitinfo *commit;
	};
};

struct reflist {
	struct refinfo **refs;
	int alloc;
	int count;
};

struct cgit_query {
	int has_oid;
	char *repo;
	char *page;
	char *search;
	char *grep;
	char *head;
	char *oid;
	char *path;
	char *url;
	int   ofs;
	int showmsg;
	diff_type difftype;
	int show_all;
	int context;
	int ignorews;
	int follow;
	char *vpath;
};

struct cgit_config {
	char *clone_prefix;
	char *clone_url;
	char *favicon;
	char *footer;
	char *head_include;
	char *header;
	char *logo;
	char *logo_link;
	struct string_list readme;
	struct string_list css;
	char *robots;
	char *root_title;
	char *root_desc;
	char *root_readme;
	char *script_name;
	char *section;
	char *virtual_root;	/* Always ends with '/'. */
	int embedded;
	int enable_follow_links;
	int enable_commit_graph;
	int enable_log_filecount;
	int enable_log_linecount;
	int enable_remote_branches;
	int local_time;
	int max_atom_items;
	int max_commit_count;
	int max_msg_len;
	int noplainemail;
	int noheader;
	int renamelimit;
	diff_type difftype;
	int commit_sort;
	struct string_list js;
};

struct cgit_page {
	time_t modified;
	size_t size;
	const char *mimetype;
	const char *charset;
	const char *filename;
	const char *etag;
	const char *title;
	int status;
	const char *statusmsg;
};

struct cgit_environment {
	const char *http_host;
	const char *https;
	const char *path_info;
	const char *query_string;
	const char *request_method;
	const char *server_name;
	const char *server_port;
};

struct cgit_context {
	struct cgit_environment env;
	struct cgit_query qry;
	struct cgit_config cfg;
	struct cgit_repo *repo;
	struct cgit_page page;
};

extern const char *cgit_version;

extern struct cgit_context ctx;

extern char *cgit_default_repo_desc;
extern struct cgit_repo *cgit_add_repo(const char *url);

extern int chk_zero(int result, char *msg);
extern int chk_positive(int result, char *msg);
extern int chk_non_negative(int result, char *msg);

extern char *trim_end(const char *str, char c);
extern char *ensure_end(const char *str, char c);

extern void strbuf_ensure_end(struct strbuf *sb, char c);

extern void cgit_add_ref(struct reflist *list, struct refinfo *ref);
extern void cgit_free_reflist_inner(struct reflist *list);
extern int cgit_refs_cb(const struct reference *ref, void *cb_data);

extern void cgit_free_commitinfo(struct commitinfo *info);
extern void cgit_free_taginfo(struct taginfo *info);

void cgit_diff_tree_cb(struct diff_queue_struct *q,
		       struct diff_options *options, void *data);

extern int cgit_diff_files(const struct object_id *old_oid,
			   const struct object_id *new_oid,
			   unsigned long *old_size, unsigned long *new_size,
			   int *binary, int context, int ignorews,
			   linediff_fn fn);

extern void cgit_diff_tree(const struct object_id *old_oid,
			   const struct object_id *new_oid,
			   filepair_fn fn, const char *prefix, int ignorews);

extern void cgit_diff_commit(struct commit *commit, filepair_fn fn,
			     const char *prefix);

__attribute__((format (printf,1,2)))
extern char *fmt(const char *format,...);

__attribute__((format (printf,1,2)))
extern char *fmtalloc(const char *format,...);

extern struct commitinfo *cgit_parse_commit(struct commit *commit);
extern struct taginfo *cgit_parse_tag(struct tag *tag);
extern const char *cgit_repobasename(const char *reponame);

extern void cgit_prepare_repo_env(struct cgit_repo * repo);

extern int read_first_line(const char *path, char **buf, size_t *size);

extern char *strdup_first_line(const char *txt);

extern char *expand_macros(const char *txt);

#endif /* CGIT_H */
