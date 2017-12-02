set -ex

main() {
    local version=2.10.0

    local arch=$1 \
          os=$2 \
          softmmu=$3 \
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
        python
        zlib1g-dev
        libcap-dev
        libattr1-dev
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
    # https://lists.nongnu.org/archive/html/qemu-trivial/2017-10/msg00025.html
    # https://lists.nongnu.org/archive/html/qemu-trivial/2017-10/msg00023.html
    if [[ "$os" == "android" ]]; then
      patch -p1 <<'EOF'
diff -ur qemu-2.10.0/linux-user/ioctls.h qemu-2.10.0.new/linux-user/ioctls.h
--- qemu-2.10.0/linux-user/ioctls.h	2017-09-27 11:27:13.858595669 -0300
+++ qemu-2.10.0.new/linux-user/ioctls.h	2017-09-27 11:43:40.613299859 -0300
@@ -173,6 +173,11 @@
   IOCTL(SIOCGSTAMP, IOC_R, MK_PTR(MK_STRUCT(STRUCT_timeval)))
   IOCTL(SIOCGSTAMPNS, IOC_R, MK_PTR(MK_STRUCT(STRUCT_timespec)))
 
+  IOCTL(RNDGETENTCNT, IOC_R, MK_PTR(TYPE_INT))
+  IOCTL(RNDADDTOENTCNT, IOC_W, MK_PTR(TYPE_INT))
+  IOCTL(RNDZAPENTCNT, 0, TYPE_NULL)
+  IOCTL(RNDCLEARPOOL, 0, TYPE_NULL)
+
   IOCTL(CDROMPAUSE, 0, TYPE_NULL)
   IOCTL(CDROMSTART, 0, TYPE_NULL)
   IOCTL(CDROMSTOP, 0, TYPE_NULL)
diff -ur qemu-2.10.0/linux-user/syscall.c qemu-2.10.0.new/linux-user/syscall.c
--- qemu-2.10.0/linux-user/syscall.c	2017-09-27 11:27:13.862595729 -0300
+++ qemu-2.10.0.new/linux-user/syscall.c	2017-09-27 11:44:26.133987660 -0300
@@ -59,6 +59,7 @@ int __clone2(int (*fn)(void *), void *child_stack_base,
 #include <linux/icmp.h>
 #include <linux/icmpv6.h>
 #include <linux/errqueue.h>
+#include <linux/random.h>
 #include "qemu-common.h"
 #ifdef CONFIG_TIMERFD
 #include <sys/timerfd.h>
diff -ur qemu-2.10.0/linux-user/syscall_defs.h qemu-2.10.0.new/linux-user/syscall_defs.h
--- qemu-2.10.0/linux-user/syscall_defs.h	2017-09-27 11:27:13.862595729 -0300
+++ qemu-2.10.0.new/linux-user/syscall_defs.h	2017-09-27 11:46:09.303545817 -0300
@@ -1060,6 +1060,13 @@ struct target_pollfd {
 
 #define TARGET_SIOCGIWNAME     0x8B01          /* get name == wireless protocol */
 
+/* From <linux/random.h> */
+
+#define TARGET_RNDGETENTCNT    TARGET_IOR('R', 0x00, int)
+#define TARGET_RNDADDTOENTCNT  TARGET_IOW('R', 0x01, int)
+#define TARGET_RNDZAPENTCNT    TARGET_IO('R', 0x04)
+#define TARGET_RNDCLEARPOOL    TARGET_IO('R', 0x06)
+
 /* From <linux/fs.h> */
 
 #define TARGET_BLKROSET   TARGET_IO(0x12,93) /* set device read-only (0 = read-write) */
EOF
   fi

# Fix build with new glibc
# See https://git.qemu.org/?p=qemu.git;a=commit;h=75e5b70e6b5dcc4f2219992d7cffa462aa406af0

    patch -p1 <<'EOF'
--- a/util/memfd.c
+++ b/util/memfd.c
@@ -31,9 +31,7 @@
 
 #include "qemu/memfd.h"
 
-#ifdef CONFIG_MEMFD
-#include <sys/memfd.h>
-#elif defined CONFIG_LINUX
+#if defined CONFIG_LINUX && !defined CONFIG_MEMFD
 #include <sys/syscall.h>
 #include <asm/unistd.h>
EOF

    patch -p1 <<'EOF'
--- a/configure
+++ b/configure
@@ -3923,7 +3923,7 @@ fi
 # check if memfd is supported
 memfd=no
 cat > $TMPC << EOF
-#include <sys/memfd.h>
+#include <sys/mman.h>
 
 int main(void)
 {
EOF

   local targets="$arch-linux-user"
   local virtfs=""
   case "$softmmu" in
      softmmu)
         if [ "$arch" = "ppc64le" ]; then
            targets="$targets,ppc64-softmmu"
         else
            targets="$targets,$arch-softmmu"
         fi
         virtfs="--enable-virtfs"
         ;;
      "")
         true
         ;;
      *)
         echo "Invalid softmmu option: $softmmu"
         exit 1
         ;;
   esac

    ./configure \
        --disable-kvm \
        --disable-vnc \
        --enable-user \
        --static \
        $virtfs \
        --target-list=$targets
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
