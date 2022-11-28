#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. freebsd-arch.sh

export BSD_ARCH=
case "${ARCH}" in
    x86_64)
        BSD_ARCH=amd64
        ;;
    i686)
        BSD_ARCH=i386
        ;;
esac

# we prefer those closer in geography to the US. they're triaged in
# order of ease of use, reliability, and then geography. the mirror
# list is at https://docs.freebsd.org/en/books/handbook/mirrors/.
# these mirrors were known to work as of 2022-11-27. this does
# not include any mirrors that are known to be rate-limited or
# commercial.
export BSD_HOME=(
    # these do not return HTML, and only list the directories
    "ftp.freebsd.org/pub/FreeBSD/releases"
    # these return HTML output, and therefore are lower priority
    "ftp11.freebsd.org/pub/FreeBSD/releases"
    "ftp3.br.freebsd.org/pub/FreeBSD/releases"
    "ftp2.uk.freebsd.org/pub/FreeBSD/releases"
    "ftp2.nl.freebsd.org/pub/FreeBSD/releases"
    "ftp6.fr.freebsd.org/pub/FreeBSD/releases"
    "ftp1.de.freebsd.org/pub/FreeBSD/releases"
    "ftp2.de.freebsd.org/pub/FreeBSD/releases"
    "ftp5.de.freebsd.org/pub/FreeBSD/releases"
    "ftp2.ru.freebsd.org/pub/FreeBSD/releases"
    "ftp2.gr.freebsd.org/pub/FreeBSD/releases"
    "ftp4.za.freebsd.org/pub/FreeBSD/releases"
    "ftp2.za.freebsd.org/pub/FreeBSD/releases"
    "ftp4.tw.freebsd.org/pub/FreeBSD/releases"
    "ftp3.jp.freebsd.org/pub/FreeBSD/releases"
    "ftp6.jp.freebsd.org/pub/FreeBSD/releases"
    # these only support HTTP, and not implicit
    # FTP as well, and have HTML output
    "http://ftp.uk.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.nl.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.fr.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.at.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.dk.freebsd.org/FreeBSD/releases"
    "http://ftp.cz.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.se.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.lv.freebsd.org/freebsd/releases"
    "http://ftp.pl.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.ua.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.gr.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.ru.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.nz.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.kr.freebsd.org/pub/FreeBSD/releases"
    "http://ftp.jp.freebsd.org/pub/FreeBSD/releases"
)
export BSD_MAJOR=12
