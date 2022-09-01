#!/usr/bin/env bash

set -x
set -euo pipefail


main() {
  yum -y install epel-release
  yum install -y gcc-aarch64-linux-gnu gcc-c++-aarch64-linux-gnu binutils-aarch64-linux-gnu binutils gcc-c++ glibc-devel
  yum clean all

  rpm_repository_prefix=http://mirror.centos.org/altarch/7/os/aarch64/Packages
  glibc_common_rpm=glibc-common-2.17-317.el7.aarch64.rpm
  glibc_rpm=glibc-2.17-317.el7.aarch64.rpm
  glibc_devel_rpm=glibc-devel-2.17-317.el7.aarch64.rpm
  glibc_static_rpm=glibc-static-2.17-317.el7.aarch64.rpm
  kernel_headers_rpm=kernel-headers-4.18.0-193.28.1.el7.aarch64.rpm
  glibc_headers_rpm=glibc-headers-2.17-317.el7.aarch64.rpm
  libgcc_rpm=libgcc-4.8.5-44.el7.aarch64.rpm
  
  prefix=/usr/aarch64-linux-gnu
  
  local td
  td="$(mktemp -d)"
  
  pushd "${td}"
  
  mkdir glibc-common && cd glibc-common
  curl --retry 3 $rpm_repository_prefix/$glibc_common_rpm -O
  rpm2cpio $glibc_common_rpm | cpio -idmv
  mkdir -p $prefix/share
  mv usr/share/i18n $prefix/share
  cd ..
  
  mkdir glibc && cd glibc
  curl --retry 3 $rpm_repository_prefix/$glibc_rpm -O
  rpm2cpio $glibc_rpm | cpio -idmv
  mv lib64 $prefix/lib
  mv lib/* usr/lib64/* $prefix/lib
  mv etc var $prefix
  ln -sf ld-2.17.so $prefix/lib/ld-linux-aarch64.so.1
  cd ..
  
  mkdir glibc-devel && cd glibc-devel
  curl --retry 3 $rpm_repository_prefix/$glibc_devel_rpm -O
  rpm2cpio $glibc_devel_rpm | cpio -idmv
  for path in usr/lib64/*.so; do
    if [[ $(readlink "$path") ]];then
      linkname=$prefix/lib/$(basename "$path")
      targetname=$(readlink "$path"|xargs basename)
      ln -s "$targetname" "$linkname"
      rm "$path"
    fi
  done
  mv usr/lib64/* $prefix/lib 
  sed -i "s~GROUP.*~GROUP ( $prefix/lib/libpthread.so.0 $prefix/lib/libpthread_nonshared.a )~g" $prefix/lib/libpthread.so
  sed -i "s~GROUP.*~GROUP ( $prefix/lib/libc.so.6 $prefix/lib/libc_nonshared.a  AS_NEEDED ( $prefix/lib/ld-linux-aarch64.so.1 ) )~g" $prefix/lib/libc.so
  cd ..
  
  mkdir glibc-static && cd glibc-static 
  curl --retry 3 $rpm_repository_prefix/$glibc_static_rpm -O
  rpm2cpio $glibc_static_rpm | cpio -idmv
  mv usr/lib64/* $prefix/lib
  cd ..
  
  mkdir kernel-headers && cd kernel-headers
  curl --retry 3 $rpm_repository_prefix/$kernel_headers_rpm -O
  rpm2cpio $kernel_headers_rpm | cpio -idmv
  mv usr/include $prefix
  cd .. 

  mkdir glibc-headers && cd glibc-headers
  curl --retry 3 $rpm_repository_prefix/$glibc_headers_rpm -O
  rpm2cpio $glibc_headers_rpm | cpio -idmv
  mv usr/include/scsi/* $prefix/include/scsi
  rmdir usr/include/scsi
  mv usr/include/* $prefix/include
  cd ..

  mkdir libgcc && cd libgcc 
  curl --retry 3 $rpm_repository_prefix/$libgcc_rpm -O
  rpm2cpio $libgcc_rpm | cpio -idmv
  mv lib64/* $prefix/lib
  ln -s libgcc_s.so.1 $prefix/lib/libgcc_s.so
  cd ..

  popd

  rm -rf "${td}"

}

main "${@}"