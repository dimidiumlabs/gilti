/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

#ifndef UI_BLOB_H
#define UI_BLOB_H

extern int cgit_ref_path_exists(const char *path, const char *ref, int file_only);
extern int cgit_print_file(char *path, const char *head, int file_only);
extern void cgit_print_blob(const char *hex, char *path, const char *head, int file_only);

#endif /* UI_BLOB_H */
