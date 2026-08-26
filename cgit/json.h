/* SPDX-FileCopyrightText: 2026 Nikolay Govorov */
/* SPDX-License-Identifier: GPL-2.0-only */

#ifndef JSON_H
#define JSON_H

#include "cgit.h"

extern void json(const char *raw);
extern void jsonf(const char *format, ...);
extern void json_key(const char *key);
extern void json_value(const char *value);
extern void json_int(intmax_t value);
extern void json_bool(int value);

#endif
