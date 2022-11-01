# this is a special makefile without any blocks

LOCAL_PATH := $(call my-dir)

test_tags := tests

benchmark_c_flags := \
	-Wall -Wextra \
	-Werror \
	-fno-builtin \

benchmark_src_files := \
	benchmark_main.cc \
	bench.cc

# Build benchmarks.
include $(CLEAR_VARS)
LOCAL_MODULE := benchmarks
LOCAL_MODULE_TAGS := tests
LOCAL_CFLAGS += $(benchmark_c_flags)
LOCAL_SHARED_LIBRARIES += libm libdl
LOCAL_SRC_FILES := $(benchmark_src_files)
