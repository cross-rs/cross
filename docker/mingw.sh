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
    sed -i 's/# deb-src/deb-src/g' /etc/apt/sources.list
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

    apt-get source gcc-mingw-w64-i686
    pushd gcc-mingw-w64-*

    # We are using dwarf exceptions instead of sjlj
    sed -i -e 's/libgcc_s_sjlj-1/libgcc_s_dw2-1/g' debian/gcc-mingw-w64-i686.install.in

    # Only build i686 packages (disable x86_64)
    patch -p0 <<'EOF'
--- debian/control.template.ori	2018-03-12 16:25:30.000000000 +0000
+++ debian/control.template	2018-03-12 16:25:30.000000000 +0000
@@ -1,7 +1,6 @@
 Package: @@PACKAGE@@-mingw-w64
 Architecture: all
 Depends: @@PACKAGE@@-mingw-w64-i686,
-         @@PACKAGE@@-mingw-w64-x86-64,
          ${misc:Depends}
 Recommends: @@RECOMMENDS@@
 Built-Using: gcc-@@VERSION@@ (= ${gcc:Version})
@@ -32,22 +31,3 @@
  This package contains the @@LANGUAGE@@ compiler, supporting
  cross-compiling to 32-bit MinGW-w64 targets.
 Build-Profiles: <!stage1>
-
-Package: @@PACKAGE@@-mingw-w64-x86-64
-Architecture: any
-Depends: @@DEPENDS64@@,
-         ${misc:Depends},
-         ${shlibs:Depends}
-Suggests: gcc-@@VERSION@@-locales (>= ${local:Version})
-Breaks: @@BREAKS64@@
-Conflicts: @@CONFLICTS64@@
-Replaces: @@REPLACES64@@
-Built-Using: gcc-@@VERSION@@ (= ${gcc:Version})
-Description: GNU @@LANGUAGE@@ compiler for MinGW-w64 targeting Win64
- MinGW-w64 provides a development and runtime environment for 32- and
- 64-bit (x86 and x64) Windows applications using the Windows API and
- the GNU Compiler Collection (gcc).
- .
- This package contains the @@LANGUAGE@@ compiler, supporting
- cross-compiling to 64-bit MinGW-w64 targets.
-Build-Profiles: <!stage1>
EOF

    # Disable build of fortran,objc,obj-c++ and use configure options
    # --disable-sjlj-exceptions --with-dwarf2
    patch -p0 <<'EOF'
--- debian/rules.ori	2018-03-12 16:25:30.000000000 +0000
+++ debian/rules	2018-03-12 16:25:30.000000000 +0000
@@ -58,7 +58,7 @@
     INSTALL_TARGET := install-gcc
 else
 # Build the full GCC.
-    languages := c,c++,fortran,objc,obj-c++,ada
+    languages := c,c++
     BUILD_TARGET :=
     INSTALL_TARGET := install install-lto-plugin
 endif
@@ -85,7 +85,7 @@
 	sed -i 's/@@VERSION@@/$(target_version)/g' debian/control
 	touch $@

-targets := i686-w64-mingw32 x86_64-w64-mingw32
+targets := i686-w64-mingw32
 threads := posix win32

 # Hardening on the host, none on the target
@@ -220,6 +220,10 @@
 # Enable libatomic
 CONFFLAGS += \
 	--enable-libatomic
+# Enable dwarf exceptions
+CONFFLAGS += \
+	--disable-sjlj-exceptions \
+	--with-dwarf2
 # Enable experimental::filesystem and std::filesystem
 CONFFLAGS += \
 	--enable-libstdcxx-filesystem-ts=yes
EOF

    # Need symlinks for specific autoconf versions, since it
    # attempts to use autoconf2.69 and autom4te2.69.
    ln -s /usr/bin/autoconf /usr/bin/autoconf2.69
    ln -s /usr/bin/autom4te /usr/bin/autom4te2.69

    # Build the modified mingw packages
    MAKEFLAGS=--silent dpkg-buildpackage -nc -B --jobs=auto

    # Replace installed mingw packages with the new ones
    dpkg -i ../g*-mingw-w64-i686*.deb ../gcc-mingw-w64-base*.deb

    purge_packages

    popd
    popd

    rm -rf "${td}"
    rm "${0}"

    # Unlink our temporary aliases
    unlink /usr/bin/autoconf2.69
    unlink /usr/bin/autom4te2.69
}

main "${@}"
