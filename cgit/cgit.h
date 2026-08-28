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

struct cgit_exec_filter {
	char *cmd;
	char **argv;
	int old_stdout;
	int pid;
};

struct cgit_repo {
	char *url;
	char *name;
	char *path;
	char *desc;
	char *extra_head_content;
	char *owner;
	char *homepage;
	char *defbranch;
	char *module_link;
	struct string_list readme;
	char *section;
	char *clone_url;
	char *logo;
	char *logo_link;
	char *snapshot_prefix;
	int snapshots;
	int enable_blame;
	int enable_commit_graph;
	int enable_follow_links;
	int enable_log_filecount;
	int enable_log_linecount;
	int enable_remote_branches;
	int enable_subject_links;
	int enable_html_serving;
	int max_stats;
	int branch_sort;
	int commit_sort;
	time_t mtime;
	struct string_list submodules;
	int hide;
	int ignore;
};

struct cgit_repolist {
	int length;
	int count;
	struct cgit_repo *repos;
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
	int has_difftype;
	char *repo;
	char *page;
	char *search;
	char *grep;
	char *head;
	char *oid;
	char *oid2;
	char *path;
	char *url;
	char *period;
	char *format;
	int signature;
	int   ofs;
	char *sort;
	int showmsg;
	diff_type difftype;
	int show_all;
	int context;
	int ignorews;
	int follow;
	char *vpath;
};

struct cgit_config {
	char *agefile;
	char *clone_prefix;
	char *clone_url;
	char *favicon;
	char *footer;
	char *head_include;
	char *header;
	char *logo;
	char *logo_link;
	char *mimetype_file;
	char *module_link;
	struct string_list readme;
	struct string_list css;
	char *robots;
	char *root_title;
	char *root_desc;
	char *root_readme;
	char *script_name;
	char *section;
	char *repository_sort;
	char *virtual_root;	/* Always ends with '/'. */
	char *strict_export;
	int case_sensitive_sort;
	int embedded;
	int enable_follow_links;
	int enable_http_clone;
	int enable_index_links;
	int enable_index_owner;
	int enable_blame;
	int enable_commit_graph;
	int enable_log_filecount;
	int enable_log_linecount;
	int enable_remote_branches;
	int enable_subject_links;
	int enable_html_serving;
	int enable_tree_linenumbers;
	int local_time;
	int max_atom_items;
	int max_repo_count;
	int max_commit_count;
	int max_msg_len;
	int max_repodesc_len;
	int max_blob_size;
	int max_stats;
	int noplainemail;
	int noheader;
	int renamelimit;
	int remove_suffix;
	int scan_hidden_path;
	int section_from_path;
	int snapshots;
	int section_sort;
	int summary_branches;
	int summary_log;
	int summary_tags;
	diff_type difftype;
	int branch_sort;
	int commit_sort;
	struct string_list mimetypes;
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

typedef int (*write_archive_fn_t)(const char *, const char *);

struct cgit_snapshot_format {
	const char *suffix;
	const char *mimetype;
	write_archive_fn_t write_func;
};

extern const char *cgit_version;

extern struct cgit_repolist cgit_repolist;
extern struct cgit_context ctx;
extern const struct cgit_snapshot_format cgit_snapshot_formats[];

extern char *cgit_default_repo_desc;
extern struct cgit_repo *cgit_add_repo(const char *url);
extern struct cgit_repo *cgit_get_repoinfo(const char *url);

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

extern int cgit_parse_snapshots_mask(const char *str);
extern const struct object_id *cgit_snapshot_get_sig(const char *ref,
						     const struct cgit_snapshot_format *f);
extern const unsigned cgit_snapshot_format_bit(const struct cgit_snapshot_format *f);

extern void cgit_exec_filter_init(struct cgit_exec_filter *filter, char *cmd, char **argv);
extern int cgit_open_exec_filter(struct cgit_exec_filter *filter);
extern int cgit_close_exec_filter(struct cgit_exec_filter *filter);

extern void cgit_prepare_repo_env(struct cgit_repo * repo);

extern int read_first_line(const char *path, char **buf, size_t *size);

extern char *strdup_first_line(const char *txt);

extern char *expand_macros(const char *txt);

extern char *get_mimetype_for_filename(const char *filename);

#endif /* CGIT_H */
