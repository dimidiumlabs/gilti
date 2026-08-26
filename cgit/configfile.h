/* SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
 * SPDX-License-Identifier: GPL-2.0-only
 */

#ifndef CONFIGFILE_H
#define CONFIGFILE_H

#include "cgit.h"

typedef void (*configfile_value_fn)(const char *name, const char *value);

extern int parse_configfile(const char *filename, configfile_value_fn fn);

#endif /* CONFIGFILE_H */
