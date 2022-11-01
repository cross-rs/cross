LOCAL_PATH := $(call my-dir)

include $(CLEAR_VARS)

LOCAL_SRC_FILES := config.c
LOCAL_MODULE := config
LOCAL_SHARED_LIBRARIES := libcutils
LOCAL_CFLAGS := -Werror

include $(BUILD_HOST_EXECUTABLE)

LOCAL_PATH := $(call my-dir)

# -----------------------------------------------------------------------------
# Benchmarks.
# -----------------------------------------------------------------------------

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

# -----------------------------------------------------------------------------
# Unit tests.
# -----------------------------------------------------------------------------

test_c_flags := \
	-g \
	-Wall \
	-Werror

##################################
# test executable
LOCAL_MODULE := module
LOCAL_SRC_FILES := src.c
LOCAL_SHARED_LIBRARIES := libcutils
LOCAL_CFLAGS := $(test_c_flags)
LOCAL_MODULE_RELATIVE_PATH := config-tests

# Unit tests.
# =========================================================

include $(CLEAR_VARS)
LOCAL_MODULE := init_tests
LOCAL_SRC_FILES := \
	init_parser_test.cc \
	property_service_test.cc \
	service_test.cc \
	util_test.cc \

##################################
# test executable
LOCAL_MODULE := module
LOCAL_SRC_FILES := src.c
LOCAL_SHARED_LIBRARIES := libcutils
LOCAL_CFLAGS := $(test_c_flags)
LOCAL_MODULE_RELATIVE_PATH := config-tests
LOCAL_SHARED_LIBRARIES += \
	libcutils \
	libbase \

LOCAL_STATIC_LIBRARIES := libinit
LOCAL_SANITIZE := integer
LOCAL_CLANG := true
LOCAL_CPPFLAGS := -Wall -Wextra -Werror
include $(BUILD_NATIVE_TEST)

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
