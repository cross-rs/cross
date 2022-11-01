# this is a special makefile checking support for multiline comments

LOCAL_PATH := $(call my-dir)

ifneq ($(ENV1),)

###########################################################
# new rules
# $(1): rule 1
# $(2): rule 2
###########################################################

include $(call all-makefiles-under,$(LOCAL_PATH))

endif
