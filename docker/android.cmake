# toolchain file for android targets, see #1110

set(CMAKE_SYSTEM_NAME "$ENV{CROSS_CMAKE_SYSTEM_NAME}")
set(CMAKE_SYSTEM_PROCESSOR "$ENV{CROSS_CMAKE_SYSTEM_PROCESSOR}")
set(CMAKE_ANDROID_STANDALONE_TOOLCHAIN /android-ndk)
set(CMAKE_ANDROID_API "$ENV{CROSS_ANDROID_SDK}")
if(DEFINED ENV{CROSS_TARGET_RUNNER})
    set(runner "$ENV{CROSS_TARGET_RUNNER}")
    separate_arguments(runner)
    set(CMAKE_CROSSCOMPILING_EMULATOR ${runner})
endif()

# these are cached so any build system that compiled outside of the rust
# build system, such as a third-party cmake build and install of a shared
# library, will still work. however, cmake-rs can override these values
if(DEFINED ENV{CROSS_CMAKE_OBJECT_FLAGS})
    set(CMAKE_C_FLAGS "$ENV{CROSS_CMAKE_OBJECT_FLAGS}" CACHE STRING "C Compiler options")
    set(CMAKE_CXX_FLAGS "$ENV{CROSS_CMAKE_OBJECT_FLAGS}" CACHE STRING "C++ Compiler options")
    set(CMAKE_ASM_FLAGS "$ENV{CROSS_CMAKE_OBJECT_FLAGS}" CACHE STRING "ASM Compiler options")
endif()

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY BOTH)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE BOTH)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE BOTH)
