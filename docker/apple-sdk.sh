#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    install_packages curl unzip xz-utils

    local td
    td="$(mktemp -d)"

    pushd "${td}"

	# Adapted from https://gist.github.com/1Conan/4347fd5f604cfe6116f7acb0237ef155
	# and https://github.com/ProcursusTeam/Procursus/blob/master/Makefile

    curl --retry 3 -sSfL "https://github.com/phracker/MacOSX-SDKs/releases/download/10.15/MacOSX10.15.sdk.tar.xz" -o macOS.sdk.tar.xz
	curl --retry 3 -sSfL "https://github.com/okanon/iPhoneOS.sdk/releases/download/v0.0.1/iPhoneOS13.2.sdk.tar.gz" -o iOS.sdk.tar.gz
	curl --retry 3 -sSfL "https://cdn.discordapp.com/attachments/688121419980341282/725234834024431686/c.zip" -o cpp.zip
    
	mkdir -p /opt/{iPhoneOS,MacOSX}.sdk
	tar --strip-components=1 -xaf macOS.sdk.tar.xz -C /opt/MacOSX.sdk
	tar --strip-components=1 -xaf iOS.sdk.tar.gz -C /opt/iPhoneOS.sdk
	unzip -o cpp.zip -d /opt/iPhoneOS.sdk/usr/include

	# Copy headers from MacOSX.sdk
	mkdir -p /opt/iPhoneOS.sdk/usr/include/IOKit
	cp -af /opt/MacOSX.sdk/usr/include/{arpa,net,xpc} /opt/iPhoneOS.sdk/usr/include
	cp -af /opt/MacOSX.sdk/usr/include/objc/objc-runtime.h /opt/iPhoneOS.sdk/usr/include/objc
	cp -af /opt/MacOSX.sdk/usr/include/libkern/OSTypes.h /opt/iPhoneOS.sdk/usr/include/libkern
	cp -af /opt/MacOSX.sdk/usr/include/sys/{tty*,proc*,ptrace,kern*,random,vnode}.h /opt/iPhoneOS.sdk/usr/include/sys
	cp -af /opt/MacOSX.sdk/System/Library/Frameworks/IOKit.framework/Headers/* /opt/iPhoneOS.sdk/usr/include/IOKit
	cp -af /opt/MacOSX.sdk/usr/include/{ar,launch,libcharset,localcharset,libproc,tzfile}.h /opt/iPhoneOS.sdk/usr/include
	cp -af /opt/MacOSX.sdk/usr/include/mach/{*.defs,{mach_vm,shared_region}.h} /opt/iPhoneOS.sdk/usr/include/mach
	cp -af /opt/MacOSX.sdk/usr/include/mach/machine/*.defs /opt/iPhoneOS.sdk/usr/include/mach/machine
	curl --retry 3 -sSfL "https://cdn.jsdelivr.net/gh/ProcursusTeam/Procursus/build_info/availability.h" -o /opt/iPhoneOS.sdk/usr/include/os/availability.h

	# Delete the macOS SDK, we don't need it anymore.
	rm -rf /opt/MacOSX.sdk

	# Patch the iOS headers
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/stdlib.h
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/time.h
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/unistd.h
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/mach/task.h
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/mach/mach_host.h
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/ucontext.h
	sed -i -E s/'__IOS_PROHIBITED|__TVOS_PROHIBITED|__WATCHOS_PROHIBITED'//g /opt/iPhoneOS.sdk/usr/include/signal.h
	sed -i -E /'__API_UNAVAILABLE'/d /opt/iPhoneOS.sdk/usr/include/pthread.h

    popd

    purge_packages

    rm -rf "${td}"
    rm -rf /var/lib/apt/lists/*
    rm "${0}"
}

main "${@}"
