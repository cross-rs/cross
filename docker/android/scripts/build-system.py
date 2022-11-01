#!/usr/bin/env python
'''
    Remove most unittests from Android soong blueprint
    files, most of which are identified via a `cc_test*`
    scope identifier, as well as some additional `subdirs`
    identifiers and Makefile specifiers.

    This also allows you to backup and restore these scripts.
    The build files are automatically backed up by default.
'''

import argparse
import glob
import os
import shutil
import subprocess
import sys

SCRIPTS_DIR = os.path.dirname(os.path.realpath(__file__))
PROJECT_DIR = os.path.dirname(SCRIPTS_DIR)
sys.path.insert(0, PROJECT_DIR)

import android
import android.make
import android.soong


def print_verbose(message, verbose):
    if verbose:
        print(message)


def backup(src, args, *_):
    dst = src + '.bak'
    print_verbose(f'creating backup of file "{src}" at "{dst}"', args.verbose)
    shutil.copy2(src, dst)


def restore(dst, args, *_):
    src = dst + '.bak'
    if os.path.exists(src):
        print_verbose(f'restoring from backup "{src}" to "{dst}"', args.verbose)
        shutil.copy2(src, dst)


def filter_map(map, remove):
    keys = list(map)
    for key in keys:
        if not item_op(map[key].value, remove):
            del map[key]
    return True


def filter_list(lst, remove):
    lst.filter(lambda x: item_op(x, remove))
    return True


def item_op(item, remove):
    if item.is_map():
        return filter_map(item, remove)
    elif item.is_list():
        return filter_list(item, remove)
    elif item.is_string() or item.is_binary_operator():
        return item.str_op(lambda y: not any(i in y.lower() for i in remove))
    raise TypeError(f'got unexpected type of {type(item)}')


def remove_soong_tests(path, args, *_):
    print_verbose(f'removing soong tests from "{path}"', args.verbose)
    with open(path) as file:
        ast = android.soong.load(file)
    # remove the test or benchmark scopes, IE, this with `cc_test`
    # or those with `{name: "test"}`, etc.
    ast.filter(lambda x: not (x.is_scope() and x.is_dev()))
    # need to remove test and benchmark subdirs
    test_names = ('test', 'benchmark')
    subdirs = [i for i in ast if i.name == 'subdirs']
    for sub in subdirs:
        assert type(sub.expr) is android.soong.List
        filter_list(sub.expr, test_names)
    # remove gtest dependencies from regular targets.
    for node in ast:
        map = None
        if not node.is_scope() and not node.expr.is_map():
            continue
        if node.is_scope():
            map = node.map
        else:
            map = node.expr
        test_names = ('libgtest', 'test-proto', 'starlarktest')
        for key, value, *_ in map.recurse():
            if value.value.is_list():
                if key == 'testSrcs':
                    value.value.clear()
                else:
                    filter_list(value, test_names)

    with open(path, 'w') as file:
        ast.dump(file)


def remove_makefile_tests(path, args, *_):
    print_verbose(f'removing makefile tests from "{path}"', args.verbose)
    with open(path) as file:
        makefile = android.make.load(file)
    makefile.filter(lambda x: not x.is_dev())
    with open(path, 'w') as file:
        makefile.dump(file)


def remove_tests(path, args, processor):
    if os.path.exists(path + '.bak'):
        restore(path, args)
    elif not args.disable_backup:
        backup(path, args)
    processor(path, args)


def stash(root):
    git_glob = f'{root}/**/.git'
    for path in glob.iglob(git_glob, recursive=True):
        os.chdir(os.path.dirname(path))
        subprocess.check_call(['git', 'stash'])


def main():
    parser = argparse.ArgumentParser()
    action_group = parser.add_mutually_exclusive_group(required=True)
    action_group.add_argument(
        '--backup',
        help='backup build files',
        action='store_true',
    )
    action_group.add_argument(
        '--restore',
        help='restore build files',
        action='store_true',
    )
    action_group.add_argument(
        '--remove-tests',
        help='remove most tests from the build system.',
        action='store_true',
    )
    action_group.add_argument(
        '--stash',
        help='stash all local changes.',
        action='store_true',
    )
    parser.add_argument(
        '--disable-backup',
        help='disable automatic backup of build files during processing.',
        action='store_false',
    )
    flags_group = parser.add_mutually_exclusive_group()
    flags_group.add_argument(
        '--soong-only',
        help='only process soong build files.',
        action='store_true',
    )
    flags_group.add_argument(
        '--makefile-only',
        help='only process makefiles.',
        action='store_true',
    )
    parser.add_argument(
        '-V',
        '--version',
        action='version',
        version=android.__version__
    )
    parser.add_argument(
        '-v',
        '--verbose',
        help='display verbose diagnostic info.',
        action='store_true',
    )
    args = parser.parse_args()
    if args.backup:
        action = backup
    elif args.restore:
        action = restore
    elif args.remove_tests:
        action = remove_tests
    elif args.stash:
        action = stash

    # root_dir is only available 3.10+
    root = os.environ.get('ANDROID_ROOT')
    if root is None:
        root = os.getcwd()
    if args.stash:
        return stash(root)

    if not args.makefile_only:
        soong_glob = f'{root}/**/Android.bp'
        for path in glob.iglob(soong_glob, recursive=True):
            action(path, args, remove_soong_tests)

    if not args.soong_only:
        make_glob = f'{root}/**/Android.mk'
        for path in glob.iglob(make_glob, recursive=True):
            action(path, args, remove_makefile_tests)


if __name__ == '__main__':
    main()
