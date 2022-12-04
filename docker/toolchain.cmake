# default toolchain file for targets, see #1110
# required so CMAKE_CROSSCOMPILING_EMULATOR is set,
# as well for embedded systems and other targets.
#
# all embedded systems without an OS should set the system name to generic
# https://cmake.org/cmake/help/book/mastering-cmake/chapter/Cross%20Compiling%20With%20CMake.html

set(CMAKE_SYSTEM_NAME "$ENV{CROSS_CMAKE_SYSTEM_NAME}")
set(CMAKE_SYSTEM_PROCESSOR "$ENV{CROSS_CMAKE_SYSTEM_PROCESSOR}")
if(DEFINED ENV{CROSS_TARGET_RUNNER})
    set(runner "$ENV{CROSS_TARGET_RUNNER}")
    separate_arguments(runner)
    set(CMAKE_CROSSCOMPILING_EMULATOR ${runner})
endif()

# not all of these are standard, however, they're common enough
# that it's good practice to define them.
set(prefix "$ENV{CROSS_TOOLCHAIN_PREFIX}")
set(suffix "$ENV{CROSS_TOOLCHAIN_SUFFIX}")
set(CMAKE_C_COMPILER "${prefix}gcc${suffix}")
set(CMAKE_ASM_COMPILER "${prefix}gcc${suffix}")
set(CMAKE_CXX_COMPILER "${prefix}g++${suffix}")
set(CMAKE_AR "${prefix}ar")
set(CMAKE_LINKER "${prefix}ld")
set(CMAKE_NM "${prefix}nm")
set(CMAKE_OBJCOPY "${prefix}objcopy")
set(CMAKE_OBJDUMP "${prefix}objdump")
set(CMAKE_RANLIB "${prefix}ranlib")
set(CMAKE_STRIP "${prefix}strip")

# these are cached so any build system that compiled outside of the rust
# build system, such as a third-party cmake build and install of a shared
# library, will still work. however, cmake-rs can override these values
if(DEFINED ENV{CROSS_CMAKE_OBJECT_FLAGS})
    set(CMAKE_C_FLAGS "$ENV{CROSS_CMAKE_OBJECT_FLAGS}" CACHE STRING "C Compiler options")
    set(CMAKE_CXX_FLAGS "$ENV{CROSS_CMAKE_OBJECT_FLAGS}" CACHE STRING "C++ Compiler options")
    set(CMAKE_ASM_FLAGS "$ENV{CROSS_CMAKE_OBJECT_FLAGS}" CACHE STRING "ASM Compiler options")
endif()

# if cross-compiling, we need to disable where the root path
# is found and also provide our own sysroot
if(DEFINED ENV{CROSS_SYSROOT})
    set(CMAKE_FIND_ROOT_PATH "$ENV{CROSS_SYSROOT}" "${CMAKE_PREFIX_PATH}")
    set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
    set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
    set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
    set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
endif()

set(crt "$ENV{CROSS_CMAKE_CRT}")
if(crt STREQUAL "newlib")
    # cmake normally tries to test the C and C++ compilers by building and
    # running a binary, but this fails for bare-metal targets, since
    # they are missing start files and potentially other symbols.
    # choosing to make a static library causes cmake to skip the check.
    set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)
endif()
