#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    # Ubuntu mingw packages for i686 uses sjlj exceptions, but rust target
    # i686-pc-windows-gnu uses dwarf exceptions. So we build mingw packages
    # that are compatible with rust.

    # Enable source
    sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources
    apt-get update

    # Install mingw (with sjlj exceptions) to get the dependencies right
    # Later we replace these packages with the new ones
    apt-get install --assume-yes --no-install-recommends g++-mingw-w64-i686

    local dependencies=(build-essential)
    while IFS='' read -r dep; do dependencies+=("${dep}"); done < \
        <(apt-cache showsrc gcc-mingw-w64-i686 | grep Build | cut -d: -f2 | tr , '\n' | cut -d' ' -f2 | sort | uniq)

    install_packages "${dependencies[@]}"

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    apt-get source gcc-mingw-w64
    pushd gcc-mingw-w64-*

    # Only build i686 packages (disable x86_64)
    patch -p0 <<'EOF'
--- debian/control.template.ori        2026-03-11 11:58:47.868983007 +0000
+++ debian/control.template    2026-03-11 11:59:56.789698095 +0000
@@ -1,7 +1,6 @@
 Package: @@PACKAGE@@-mingw-w64
 Architecture: all
 Depends: @@PACKAGE@@-mingw-w64-i686,
-         @@PACKAGE@@-mingw-w64-x86-64,
          ${misc:Depends}
 Recommends: @@RECOMMENDS@@
 Description: GNU @@LANGUAGE@@ compiler for MinGW-w64
@@ -66,57 +65,3 @@
  cross-compiling to 32-bit MinGW-w64 targets, using the Win32
  threading model.
 Build-Profiles: <!stage1>
-
-Package: @@PACKAGE@@-mingw-w64-x86-64
-Architecture: all
-Depends: @@PACKAGE@@-mingw-w64-x86-64-posix,
-         @@PACKAGE@@-mingw-w64-x86-64-win32,
-         ${misc:Depends}
-Description: GNU @@LANGUAGE@@ compiler for MinGW-w64 targeting Win64
- MinGW-w64 provides a development and runtime environment for 32- and
- 64-bit (x86 and x64) Windows applications using the Windows API and
- the GNU Compiler Collection (gcc).
- .
- This metapackage provides the @@LANGUAGE@@ compiler, supporting
- cross-compiling to 64-bit MinGW-w64 targets.
-Build-Profiles: <!stage1>
-
-Package: @@PACKAGE@@-mingw-w64-x86-64-posix
-Architecture: @@ARCH@@
-Depends: @@DEPENDS64P@@,
-         ${misc:Depends},
-         ${shlibs:Depends}
-Suggests: gcc-@@VERSION@@-locales (>= ${local:Version})
-Breaks: @@BREAKS64@@
-Conflicts: @@CONFLICTS64@@
-Replaces: @@REPLACES64@@
-Built-Using: gcc-@@VERSION@@ (= ${gcc:Version})
-Description: GNU @@LANGUAGE@@ compiler for MinGW-w64, Win64/POSIX
- MinGW-w64 provides a development and runtime environment for 32- and
- 64-bit (x86 and x64) Windows applications using the Windows API and
- the GNU Compiler Collection (gcc).
- .
- This package contains the @@LANGUAGE@@ compiler, supporting
- cross-compiling to 64-bit MinGW-w64 targets, using the POSIX
- threading model.
-Build-Profiles: <!stage1>
-
-Package: @@PACKAGE@@-mingw-w64-x86-64-win32
-Architecture: @@ARCH@@
-Depends: @@DEPENDS64W@@,
-         ${misc:Depends},
-         ${shlibs:Depends}
-Suggests: gcc-@@VERSION@@-locales (>= ${local:Version})
-Breaks: @@BREAKS64@@
-Conflicts: @@CONFLICTS64@@
-Replaces: @@REPLACES64@@
-Built-Using: gcc-@@VERSION@@ (= ${gcc:Version})
-Description: GNU @@LANGUAGE@@ compiler for MinGW-w64, Win64/Win32
- MinGW-w64 provides a development and runtime environment for 32- and
- 64-bit (x86 and x64) Windows applications using the Windows API and
- the GNU Compiler Collection (gcc).
- .
- This package contains the @@LANGUAGE@@ compiler, supporting
- cross-compiling to 64-bit MinGW-w64 targets, using the Win32
- threading model.
-Build-Profiles: <!stage1>
EOF

    # Disable build of fortran,objc,obj-c++ and use configure options
    # --disable-sjlj-exceptions --with-dwarf2
    patch -p0 <<'EOF'
--- debian/rules.ori	2026-03-11 12:43:35.486834587 +0000
+++ debian/rules	2026-03-11 12:43:55.873331683 +0000
@@ -26,7 +26,7 @@
 target_version := 13
 target32 := i686-w64-mingw32
 target64 := x86_64-w64-mingw32
-targets := $(target32) $(target64)
+targets := $(target32)
 threads := posix win32
 gnat_arches := alpha amd64 arm64 armel armhf hppa i386 mips64el mipsel ppc64 ppc64el riscv64 s390x sh4 sparc64 x32
 
@@ -289,11 +289,6 @@
 			-B$(build_dir)/$(target32)-$$threads \
 			-D$(upstream_dir) -- \
 			$(CONFFLAGS) --disable-sjlj-exceptions --with-dwarf2; \
-		target=$(target64); \
-		dh_auto_configure \
-			-B$(build_dir)/$(target64)-$$threads \
-			-D$(upstream_dir) -- \
-			$(CONFFLAGS); \
 	done
 else
 	set -e; \
@@ -301,12 +296,7 @@
 	dh_auto_configure \
 		-B$(build_dir)/$(target32) \
 		-D$(upstream_dir) -- \
-		$(CONFFLAGS) --disable-sjlj-exceptions --with-dwarf2; \
-	target=$(target64); \
-	dh_auto_configure \
-		-B$(build_dir)/$(target64) \
-		-D$(upstream_dir) -- \
-		$(CONFFLAGS)
+		$(CONFFLAGS) --disable-sjlj-exceptions --with-dwarf2;
 endif
 	touch $@
 
EOF

    # Build the modified mingw packages
    MAKEFLAGS=--silent dpkg-buildpackage -nc -B --jobs=auto

    # Replace installed mingw packages with the new ones
    dpkg -i ../g*-mingw-w64-i686*.deb ../gcc-mingw-w64-base*.deb

    purge_packages

    popd
    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
