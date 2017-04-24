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

    # The build fails with the default gcc-6-source version (6.3.0-12ubuntu2)
    # Downgrading to the previous version makes the build works
    echo "deb http://archive.ubuntu.com/ubuntu yakkety main universe" >> /etc/apt/sources.list
    apt-get update
    apt-get install -y --no-install-recommends gcc-6-source=6.2.0-5ubuntu12 ${purge_list[@]}

    pushd $td

    apt-get source gcc-mingw-w64-i686
    cd gcc-mingw-w64-*

    # We are using dwarf exceptions instead of sjlj
    sed -i -e 's/libgcc_s_sjlj-1/libgcc_s_dw2-1/g' debian/gcc-mingw-w64-i686.install

    # Only build i686 packages (disable x86_64)
    rm -f debian/*x86-64.install

    # Disable build of fortran,objc,obj-c++ and use configure options
    # --disable-sjlj-exceptions --with-dwarf2
    patch -p0 <<'EOF'
--- debian/rules.ori     2016-08-20 15:24:54.000000000 +0000
+++ debian/rules
@@ -57,9 +57,7 @@
     INSTALL_TARGET := install-gcc
 else
 # Build the full GCC.
-    languages := c,c++,fortran,objc,obj-c++
-    debian_extra_langs := ada
-    export debian_extra_langs
+    languages := c,c++
     BUILD_TARGET :=
     INSTALL_TARGET := install install-lto-plugin
 endif
@@ -86,7 +84,7 @@
 	sed -i 's/@@VERSION@@/$(target_version)/g' debian/control
 	touch $@
 
-targets := i686-w64-mingw32 x86_64-w64-mingw32
+targets := i686-w64-mingw32
 threads := posix win32
 
 # Hardening on the host, none on the target
@@ -213,6 +211,10 @@
 # Enable libatomic
 CONFFLAGS += \
 	--enable-libatomic
+# Enable dwarf exceptions
+CONFFLAGS += \
+	--disable-sjlj-exceptions \
+	--with-dwarf2
 
 spelling = grep -rl "$(1)" $(upstream_dir) | xargs -r sed -i "s/$(1)/$(2)/g"
 
EOF

    # Build the modified mingw packages
    MAKEFLAGS=--silent dpkg-buildpackage -B

    # Replace installed mingw packages with the new ones
    dpkg -i ../g*-mingw-w64-i686*.deb ../gcc-mingw-w64-base*.deb

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
