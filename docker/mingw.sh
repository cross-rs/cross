set -x

main() {
    # Ubuntu mingw packages for i686 uses sjlj exceptions, but rust target
    # i686-pc-windows-gnu uses dwarf exceptions. So we build mingw packages
    # that are compatible with rust.

    # Install mingw (with sjlj exceptions) to get the dependencies right
    # Later we replace these packages with the new ones
    apt-get install -y --no-install-recommends g++-mingw-w64-i686

    local td=$(mktemp -d)

    local dependencies=(
        build-essential
        $(apt-cache showsrc gcc-mingw-w64-i686 | grep Build | cut -d: -f2 | tr , '\n' | cut -d' ' -f2 | sort | uniq)
    )

    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep > /dev/null; then
            purge_list+=( $dep )
        fi
    done

    apt-get update
    apt-get install -y --no-install-recommends ${purge_list[@]}

    pushd $td

    apt-get source gcc-mingw-w64-i686
    cd gcc-mingw-w64-*

    # We are using dwarf exceptions instead of sjlj
    sed -i -e 's/libgcc_s_sjlj-1/libgcc_s_dw2-1/g' debian/gcc-mingw-w64-i686.install
    # Do not install gcc with win32 thread model
    sed -i -e '/i686-w64-mingw32-gcc-win32/d' debian/gcc-mingw-w64-i686.install

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
-threads := posix win32
+threads := posix
 
 # Hardening on the host, none on the target
@@ -216,6 +216,10 @@
 # Enable libatomic
 CONFFLAGS += \
 	--enable-libatomic
+# Enable dwarf exceptions
+CONFFLAGS += \
+	--disable-sjlj-exceptions \
+	--with-dwarf2
 # Enable experimental::filesystem
 CONFFLAGS += \
 	--enable-libstdcxx-filesystem-ts=yes
EOF

    # Build the modified mingw packages
    MAKEFLAGS=--silent dpkg-buildpackage -nc -B --jobs=auto

    # Replace installed mingw packages with the new ones
    dpkg -i ../g*-mingw-w64-i686*.deb ../gcc-mingw-w64-base*.deb

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
