# this is a special makefile checking we handle nested
# conditionals properly, that removing sections won't
# cause unequal conditional blocks. it may still lead
# to missing definitions, but it won't fail due to
# unmatched if and endif directives.

LOCAL_PATH := $(call my-dir)

ifneq ($(ENV1),)

# -----------------------------------------------------------------------------
# Benchmarks.
# -----------------------------------------------------------------------------

test_tags := tests

benchmark_c_flags := \
	-Wall -Wextra \
	-Werror \
	-fno-builtin \

benchmark_src_files := benchmark_main.cc
ifneq ($(ENV2),)
	benchmark_src_files += bench1.cc
else
	benchmark_src_files += bench2.cc
endif

# Build benchmarks.
include $(CLEAR_VARS)
LOCAL_MODULE := benchmarks
LOCAL_MODULE_TAGS := tests
LOCAL_CFLAGS += $(benchmark_c_flags)
LOCAL_SHARED_LIBRARIES += libm libdl
LOCAL_SRC_FILES := $(benchmark_src_files)

endif

# Other section.
# =========================================================
include $(call all-makefiles-under,$(LOCAL_PATH))

# =============================================================================
# Unit tests.
# =============================================================================

test_c_flags := \
	-g \
	-Wall \
	-Werror

##################################
# test executable
LOCAL_MODULE := mod2
LOCAL_SRC_FILES := mod.c
LOCAL_SHARED_LIBRARIES := libcutils
LOCAL_CFLAGS := $(test_c_flags)
LOCAL_MODULE_RELATIVE_PATH := mod2-tests
