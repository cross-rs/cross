import copy
import os
import sys

TEST_DIR = os.path.dirname(os.path.realpath(__file__))
PROJECT_DIR = os.path.dirname(TEST_DIR)
sys.path.insert(0, PROJECT_DIR)

from android import make


def test():
    path = os.path.join(TEST_DIR, 'Android.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    stripped = contents[:-1]
    assert repr(makefile) == f'Makefile({stripped})'
    assert str(makefile) == stripped
    assert len(makefile) == 9

    assert not makefile[0].is_dev()
    assert makefile[1].is_dev()
    assert makefile[1].is_benchmark()
    assert makefile[2].is_dev()
    assert makefile[2].is_test()
    assert makefile[6].title == 'Other section.'

    filtered = copy.deepcopy(makefile)
    filtered.filter(lambda x: not x.is_dev())
    assert type(filtered) is make.Makefile
    assert len(filtered) == 2
    assert not filtered[0].is_comment()
    assert filtered[1].title == 'Other section.'

    assert makefile == make.load(open(path))
    assert contents == makefile.dumps() + '\n'


def test_nested():
    path = os.path.join(TEST_DIR, 'Nested.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    assert len(makefile) == 6

    assert makefile[0].is_block()
    assert makefile[0].child.startswith('# this is a special makefile')

    assert makefile[1].is_directive()
    assert len(makefile[1].child) == 2
    assert makefile[1].child[0].is_block()
    assert makefile[1].child[1].is_comment()
    assert makefile[1].child[1].title == 'Benchmarks.'

    outer = makefile[1].child[1]
    assert len(outer.child) == 3
    assert outer.child[0].is_block()
    assert outer.child[1].is_directive()
    assert outer.child[2].is_block()

    inner = outer.child[1]
    assert inner.child.is_block()


def test_comments():
    path = os.path.join(TEST_DIR, 'Comments.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    assert len(makefile) == 1

    assert makefile[0].is_block()
    assert makefile[0].child.startswith('# 1) sample grouping:')


def test_grouped():
    path = os.path.join(TEST_DIR, 'Grouped.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    assert len(makefile) == 3

    assert makefile[0].is_block()
    assert makefile[0].child.startswith('LOCAL_PATH := $(call my-dir)')

    comment = makefile[1]
    assert comment.is_comment()
    assert len(comment.child) == 3
    assert comment.child[0].child.startswith('LOCAL_SRC_FILES := src.c')
    assert comment.child[1].is_directive()
    assert len(comment.child[2].child) == 0

    directives = comment.child[1]
    inner_comment = directives.child
    assert inner_comment.is_comment()
    assert len(inner_comment.child) == 2
    assert inner_comment.child[0].is_directive()
    assert inner_comment.child[1].child.startswith('else')

    inner = inner_comment.child[0]
    assert inner.child.lstrip().startswith('benchmark_src_files')

    assert makefile[2].is_comment()


def test_recurse():
    path = os.path.join(TEST_DIR, 'Nested.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    nodes = list(makefile.recurse())
    assert len(nodes) == 11

    assert nodes[0] == makefile[0]
    assert nodes[1] == makefile[1]
    assert nodes[2] == makefile[1].child[0]
    assert nodes[3] == makefile[1].child[1]
    assert nodes[4] == makefile[1].child[1].child[0]
    assert nodes[5] == makefile[1].child[1].child[1]
    assert nodes[6] == makefile[1].child[1].child[2]
    assert nodes[7] == makefile[2]
    assert nodes[8] == makefile[3]
    assert nodes[9] == makefile[4]
    assert nodes[10] == makefile[5]


def test_multiline():
    path = os.path.join(TEST_DIR, 'Multiline.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    assert len(makefile) == 2

    assert makefile[0].is_block()
    assert makefile[0].child.startswith('# this is a special makefile')

    assert makefile[1].is_directive()
    comment = makefile[1].child[1]
    assert comment.is_comment()
    assert comment.title == 'new rules\n$(1): rule 1\n$(2): rule 2'
    assert str(comment.child).startswith('\ninclude')


def test_fake_title():
    path = os.path.join(TEST_DIR, 'FakeTitle.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    assert len(makefile) == 1

    comment = makefile[0]
    assert comment.is_comment()
    assert comment.title == ''
    assert str(comment.child).startswith('LOCAL_PATH := $(call my-dir)')


def test_filter():
    path = os.path.join(TEST_DIR, 'Nested.mk')
    contents = open(path).read()
    makefile = make.loads(contents)
    assert str(makefile) + '\n' == contents
    assert len(makefile) == 6
    assert makefile[1].is_directive()
    assert len(makefile[1].child) == 2

    filtered = copy.deepcopy(makefile)
    filtered.filter(lambda x: not x.is_dev())
    assert len(filtered) == 4
    assert filtered[0].is_block()
    assert filtered[1].is_directive()
    assert filtered[2].is_block()
    assert filtered[3].is_comment()

    directive = filtered[1]
    assert len(directive.child) == 1
    assert directive.child[0].is_block()

    assert filtered[3].title.lstrip().startswith('Other section.')


def test_split_directives():
    path = os.path.join(TEST_DIR, 'Nested.mk')
    contents = open(path).read()
    iterable = iter(contents.splitlines())
    blocks = make._split_directives(iterable)[0]
    assert len(blocks) == 3

    assert blocks[0].is_block()
    assert blocks[0].startswith('# this is a special makefile')

    assert blocks[2].is_block()
    assert blocks[2].lstrip().startswith('# Other section.')

    assert not blocks[1].is_comment()
    assert blocks[1].is_directive()
    assert blocks[1].has_block_list()

    directives = blocks[1].child
    assert len(directives) == 3
    assert directives[0].is_block()
    assert directives[1].is_directive()
    assert directives[2].is_block()

    assert not directives[1].child.has_block_list()
    assert directives[1].child.lstrip().startswith('benchmark_src_files')

    path = os.path.join(TEST_DIR, 'Grouped.mk')
    contents = open(path).read()
    iterable = iter(contents.splitlines())
    blocks = make._split_directives(iterable)[0]
    assert len(blocks) == 3

    assert blocks[0].is_block()
    assert blocks[1].is_directive()
    assert blocks[2].is_block()

    directives = blocks[1].child
    assert len(directives) == 3
    assert directives[0].is_block()
    assert directives[1].is_directive()
    assert directives[2].is_block()


def test_split_comments():
    path = os.path.join(TEST_DIR, 'Android.mk')
    contents = open(path).read()
    blocks = make._split_comments(contents)
    assert repr(blocks) == f'BlockList({contents})'
    assert str(blocks) == contents
    assert len(blocks) == 9

    assert not blocks[0].is_dev()
    assert blocks[1].is_dev()
    assert blocks[1].is_benchmark()
    assert blocks[1].title == 'Benchmarks.'
    assert blocks[2].is_dev()
    assert blocks[2].is_test()
    assert blocks[2].title == 'Unit tests.'
    assert blocks[3].is_test()
    assert blocks[3].title == 'test executable'
    assert blocks[4].is_test()
    assert blocks[4].title == 'Unit tests.'
    assert blocks[5].is_test()
    assert blocks[5].title == 'test executable'
    assert not blocks[6].is_dev()
    assert blocks[6].title == 'Other section.'
    assert blocks[7].is_test()
    assert blocks[7].title == 'Unit tests.'
    assert blocks[8].is_test()
    assert blocks[8].title == 'test executable'

    path = os.path.join(TEST_DIR, 'Empty.mk')
    contents = open(path).read()
    blocks = make._split_comments(contents)
    assert len(blocks) == 1
    assert repr(blocks) == 'BlockList(\n)'
    assert str(blocks) == '\n'
    assert str(blocks[0]) == '\n'

    blocks = make._split_comments('')
    assert len(blocks) == 0
    assert repr(blocks) == 'BlockList()'
    assert str(blocks) == ''


def test_block():
    data = '''LOCAL_PATH := $(call my-dir)
include $(CLEAR_VARS)'''
    block = make.Block(data)
    assert repr(block) == f'Block({data})'
    assert str(block) == data
    assert block.is_block()
    assert not block.is_block_list()
    assert not block.is_comment()
    assert not block.is_directive()
    assert not block.is_dev()


def test_block_list():
    data1 = 'LOCAL_PATH := $(call my-dir)'
    data2 = 'test_tags := tests'
    blocks = make.BlockList([make.Block(data1), make.Block(data2)])
    assert repr(blocks) == f'BlockList({data1}\n{data2})'
    assert str(blocks) == f'{data1}\n{data2}'
    assert not blocks.is_block()
    assert blocks.is_block_list()
    assert not blocks.is_comment()
    assert not blocks.is_directive()
    assert not blocks.is_dev()


def test_comment_block():
    # single block
    comment = '''# -----------------------------------------------------------------------------
# Benchmarks.
# -----------------------------------------------------------------------------
'''
    title = 'Benchmarks.'
    data = 'test_tags := tests'
    block = make.CommentBlock(comment, title, make.Block(data))
    assert repr(block) == f'CommentBlock({comment}\n{data})'
    assert str(block) == f'{comment}\n{data}'
    assert not block.is_block()
    assert not block.is_block_list()
    assert block.is_comment()
    assert not block.is_directive()
    assert block.is_dev()

    title = 'Other Section.'
    blocks = make.BlockList([
        make.Block('LOCAL_PATH := $(call my-dir)'),
        make.Block('test_tags := tests'),
    ])
    block = make.CommentBlock(comment, title, blocks)
    assert repr(block) == f'CommentBlock({comment}\n{str(blocks)})'
    assert str(block) == f'{comment}\n{str(blocks)}'
    assert not block.is_block()
    assert not block.is_block_list()
    assert block.is_comment()
    assert not block.is_directive()
    assert not block.is_dev()


def test_directive_block():
    start_inner = '    ifneq ($(USE_B),)'
    end_inner = '    endif'
    data_inner = '''        SOURCES=b.cc
    else
        SOURCES=a.cc'''
    inner = make.DirectiveBlock(start_inner, end_inner, make.Block(data_inner))
    str_inner = f'{start_inner}\n{data_inner}\n{end_inner}'
    assert repr(inner) == f'DirectiveBlock({str_inner})'
    assert str(inner) == str_inner
    assert not inner.is_block()
    assert not inner.is_block_list()
    assert not inner.is_comment()
    assert inner.is_directive()
    assert not inner.is_dev()

    data_else = '''else
    SOURCES=c.cc'''
    else_block = make.Block(data_else)
    blocks = make.BlockList([inner, else_block])
    str_blocks = '\n'.join([str(i) for i in blocks])
    assert repr(blocks) == f'BlockList({str_blocks})'
    assert str(blocks) == str_blocks

    start = 'ifneq ($(USE_A),)'
    end = 'endif'
    block = make.DirectiveBlock(start, end, blocks)
    str_block = f'{start}\n{str_blocks}\n{end}'
    assert repr(block) == f'DirectiveBlock({str_block})'
    assert str(block) == str_block
    assert not block.is_block()
    assert not block.is_block_list()
    assert not block.is_comment()
    assert block.is_directive()
    assert not block.is_dev()
