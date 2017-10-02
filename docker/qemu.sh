set -ex

main() {
    local version=2.10.0

    local arch=$1 \
          os=$2 \
          td=$(mktemp -d)

    local dependencies=(
        autoconf
        automake
        bzip2
        curl
        g++
        libglib2.0-dev
        libtool
        make
        patch
        pkg-config
        zlib1g-dev
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    pushd $td

    curl -L http://wiki.qemu-project.org/download/qemu-$version.tar.bz2 | \
        tar --strip-components=1 -xj

    # Allow qemu to run android (bionic libc) binaries
    # AT_SECURE change is based on https://gist.github.com/whitequark/ad704d48a86d889b8cdb
    # RNDGETENTCNT change forward ioctl(_, RNDGETENTCNT, _) to the host
    if [[ "$os" == "android" ]]; then
      patch -p1 <<'EOF'
diff -ur qemu-2.10.0/linux-user/elfload.c qemu-2.10.0.new/linux-user/elfload.c
--- qemu-2.10.0/linux-user/elfload.c	2017-09-27 11:27:13.866595788 -0300
+++ qemu-2.10.0.new/linux-user/elfload.c	2017-09-27 11:58:30.662613425 -0300
@@ -1354,7 +1354,7 @@
                                  ~(abi_ulong)(TARGET_ELF_EXEC_PAGESIZE-1))
 #define TARGET_ELF_PAGEOFFSET(_v) ((_v) & (TARGET_ELF_EXEC_PAGESIZE-1))
 
-#define DLINFO_ITEMS 14
+#define DLINFO_ITEMS 15
 
 static inline void memcpy_fromfs(void * to, const void * from, unsigned long n)
 {
@@ -1782,6 +1782,7 @@
     NEW_AUX_ENT(AT_HWCAP, (abi_ulong) ELF_HWCAP);
     NEW_AUX_ENT(AT_CLKTCK, (abi_ulong) sysconf(_SC_CLK_TCK));
     NEW_AUX_ENT(AT_RANDOM, (abi_ulong) u_rand_bytes);
+    NEW_AUX_ENT(AT_SECURE, (abi_ulong)0);
 
 #ifdef ELF_HWCAP2
     NEW_AUX_ENT(AT_HWCAP2, (abi_ulong) ELF_HWCAP2);
diff -ur qemu-2.10.0/linux-user/ioctls.h qemu-2.10.0.new/linux-user/ioctls.h
--- qemu-2.10.0/linux-user/ioctls.h	2017-09-27 11:27:13.858595669 -0300
+++ qemu-2.10.0.new/linux-user/ioctls.h	2017-09-27 11:43:40.613299859 -0300
@@ -430,6 +430,7 @@
                 MK_PTR(MK_STRUCT(STRUCT_rtentry)))
   IOCTL_SPECIAL(SIOCDELRT, IOC_W, do_ioctl_rt,
                 MK_PTR(MK_STRUCT(STRUCT_rtentry)))
+  IOCTL(RNDGETENTCNT, IOC_R, MK_PTR(TYPE_INT))
 
 #ifdef TARGET_TIOCSTART
   IOCTL_IGNORE(TIOCSTART)
diff -ur qemu-2.10.0/linux-user/syscall.c qemu-2.10.0.new/linux-user/syscall.c
--- qemu-2.10.0/linux-user/syscall.c	2017-09-27 11:27:13.862595729 -0300
+++ qemu-2.10.0.new/linux-user/syscall.c	2017-09-27 11:44:26.133987660 -0300
@@ -55,6 +55,7 @@
 //#include <sys/user.h>
 #include <netinet/ip.h>
 #include <netinet/tcp.h>
+#include <linux/random.h>
 #include <linux/wireless.h>
 #include <linux/icmp.h>
 #include <linux/icmpv6.h>
diff -ur qemu-2.10.0/linux-user/syscall_defs.h qemu-2.10.0.new/linux-user/syscall_defs.h
--- qemu-2.10.0/linux-user/syscall_defs.h	2017-09-27 11:27:13.862595729 -0300
+++ qemu-2.10.0.new/linux-user/syscall_defs.h	2017-09-27 11:46:09.303545817 -0300
@@ -971,6 +971,8 @@
     short revents;    /* returned events */
 };
 
+#define TARGET_RNDGETENTCNT    TARGET_IOR('R', 0x00, int)
+
 /* virtual terminal ioctls */
 #define TARGET_KIOCSOUND       0x4B2F	/* start sound generation (0 for off) */
 #define TARGET_KDMKTONE	       0x4B30	/* generate tone */
EOF
   fi

    ./configure \
        --disable-kvm \
        --disable-vnc \
        --enable-user \
        --static \
        --target-list=$arch-linux-user
    nice make -j$(nproc)
    make install

    # HACK the binfmt_misc interpreter we'll use expects the QEMU binary to be
    # in /usr/bin. Create an appropriate symlink
    ln -s /usr/local/bin/qemu-$arch /usr/bin/qemu-$arch-static

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
