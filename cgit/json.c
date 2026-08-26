/* SPDX-FileCopyrightText: 2026 Nikolay Govorov */
/* SPDX-License-Identifier: GPL-2.0-only */

#include "json.h"
#include "cgit.h"

void json(const char *raw) {
  if (write(STDOUT_FILENO, raw, strlen(raw)) != strlen(raw))
    die_errno("write error on json output");
}

void jsonf(const char *format, ...) {
  va_list args;
  struct strbuf buf = STRBUF_INIT;

  va_start(args, format);
  strbuf_vaddf(&buf, format, args);
  va_end(args);
  json(buf.buf);
  strbuf_release(&buf);
}

static int utf8_len(const unsigned char *p) {
  int len;

  if (p[0] < 0x80) {
    return 1;
  }

  if (p[0] >= 0xc2 && p[0] <= 0xdf) {
    len = 2;
  } else if (p[0] >= 0xe0 && p[0] <= 0xef) {
    len = 3;
  } else if (p[0] >= 0xf0 && p[0] <= 0xf4) {
    len = 4;
  } else {
    return 0;
  }

  if (!p[1] || (len > 2 && !p[2]) || (len > 3 && !p[3])) {
    return 0;
  }

  if ((p[1] & 0xc0) != 0x80 || (len > 2 && (p[2] & 0xc0) != 0x80) ||
      (len > 3 && (p[3] & 0xc0) != 0x80)) {
    return 0;
  }

  if ((p[0] == 0xe0 && p[1] < 0xa0) || (p[0] == 0xed && p[1] > 0x9f) ||
      (p[0] == 0xf0 && p[1] < 0x90) || (p[0] == 0xf4 && p[1] > 0x8f)) {
    return 0;
  }

  return len;
}

void json_value(const char *value) {
  const unsigned char *p = (const unsigned char *)(value ? value : "");

  json("\"");
  while (*p) {
    int len;
    switch (*p) {
    case '\"':
      json("\\\"");
      p++;
      break;
    case '\\':
      json("\\\\");
      p++;
      break;
    case '\b':
      json("\\b");
      p++;
      break;
    case '\f':
      json("\\f");
      p++;
      break;
    case '\n':
      json("\\n");
      p++;
      break;
    case '\r':
      json("\\r");
      p++;
      break;
    case '\t':
      json("\\t");
      p++;
      break;
    default:
      if (*p < 0x20) {
        jsonf("\\u%04x", *p++);
      } else if (*p < 0x80) {
        char ch[] = {*p++, '\0'};
        json(ch);
      } else if ((len = utf8_len(p))) {
        if (write(STDOUT_FILENO, p, len) != len)
          die_errno("write error on json output");
        p += len;
      } else {
        json("\xef\xbf\xbd");
        p++;
      }
    }
  }
  json("\"");
}

void json_key(const char *key) {
  json_value(key);
  json(":");
}

void json_int(intmax_t value) { jsonf("%" PRIdMAX, value); }

void json_bool(int value) { json(value ? "true" : "false"); }
