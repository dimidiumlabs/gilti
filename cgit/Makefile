# SPDX-FileCopyrightText: cgit Development Team <cgit@lists.zx2c4.com>
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: GPL-2.0-only

# Run from Git's source directory so cgit can reuse Git's build variables,
# compatibility layer, and static libraries.
include Makefile

CGIT_PREFIX = ../
CGIT_VERSION = v1.3.1
CGIT_SCRIPT_NAME = gilti-cgit
CGIT_CONFIG = /etc/cgitrc
CACHE_ROOT = /var/cache/cgit

CGIT_CFLAGS += -DCGIT_CONFIG='"$(CGIT_CONFIG)"'
CGIT_CFLAGS += -DCGIT_SCRIPT_NAME='"$(CGIT_SCRIPT_NAME)"'
CGIT_CFLAGS += -DCGIT_CACHE_ROOT='"$(CACHE_ROOT)"'
CGIT_CFLAGS += -DCGIT_VERSION='"$(CGIT_VERSION)"'
CGIT_CFLAGS += -DNO_LUA

ifeq ($(uname_S),Linux)
	CGIT_CFLAGS += -DHAVE_LINUX_SENDFILE
	CGIT_LIBS += -ldl
endif

CGIT_OBJS := $(patsubst %.c,%.o,$(wildcard $(CGIT_PREFIX)*.c))
cgit_dep_files := $(foreach f,$(CGIT_OBJS),$(dir $(f)).depend/$(notdir $(f)).d)
-include $(wildcard $(cgit_dep_files))

$(CGIT_PREFIX).depend:
	@mkdir -p $@

$(CGIT_OBJS): %.o: %.c GIT-CFLAGS $(CGIT_PREFIX)Makefile | $(CGIT_PREFIX).depend
	$(QUIET_CC)$(CC) -o $*.o -c $(dep_args) $(ALL_CFLAGS) $(EXTRA_CPPFLAGS) $(CGIT_CFLAGS) $<

$(CGIT_PREFIX)cgit: $(CGIT_OBJS) GIT-LDFLAGS $(GITLIBS)
	$(QUIET_LINK)$(CC) $(ALL_CFLAGS) -o $@ $(ALL_LDFLAGS) $(filter %.o,$^) $(LIBS) $(CGIT_LIBS)
