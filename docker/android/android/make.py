'''
    make
    ====

    utilities to process makefiles. this parser is not sophisticated
    nor correct, but it tries to avoid a few common pitfalls by
    handling conditional blocks, and first separating all conditional
    blocks into sections, and then parsing comment blocks within those
    sections.

    validate conditional directives are:
    - ifeq
    - ifneq
    - ifdef
    - ifndef
    - else
    - endif

    makefiles are whitespace-sensitive, but not with leading whitespace
    for conditional directives. for example, this is valid (replacing the
    spaces with tabs):

        # ---------------
        # Section 1.
        # ---------------
        ifneq ($(USE_A),)
            # -----------
            # Section 2.
            # -----------
            ifneq ($(USE_B),)
                SOURCES=b.cc
            else
                SOURCES=a.cc
            endif
        else
            SOURCES=c.cc
        endif

    our goals are fairly different from a regular parser: we want to detect
    and excise sections based on the comments, while ensuring that we do
    not produce invalid output. other than unbalanced conditional directives,
    we do not actually care about the actual contents.

    for this, we use a 3 step parsing approach:
    1. break up document into blocks separated by directives
        - each block can be a regular or directive block
        - directive blocks have a start and end directive as well as contents
        - directives can be infinitely nested: the contents can also be a list
    2. break each text block based on comment sections
    3. group blocks within comment sections

    for example, in the above, we want the entire makefile to be inside the
    section 1 comment block, so removing it would remove that whole tree.
    similarly, the inner directive block should be inside the section 2
    comment block. we would therefore produce something like this:

        CommentBlock: Section 1
          Directive Block:
            start=ifneq ($(USE_A),)
            end=endif
            children:
              CommentBlock: Section 2
                Directive Block:
                  start=ifneq ($(USE_B),)
                  end=endif
                  children:
                    Block: `SOURCES=b.cc\nelse\nSOURCES=a.cc`
              Block: `else\nSOURCES=c.cc`
'''

import re2 as re

from . import util


def loads(contents, *_, **__):
    return Makefile.loads(contents)


def load(fp, *_, **__):
    return Makefile.load(fp)


def dumps(makefile, *_, **__):
    return makefile.dumps()


def dump(makefile, fp, *_, **__):
    return makefile.dump(fp)


class Makefile(list):
    @staticmethod
    def loads(contents, *_, **__):
        directives = _split_directives(iter(contents.splitlines()))[0]
        blocks = directives.split_comments()
        blocks.group_comments()

        return Makefile(blocks)

    @staticmethod
    def load(fp, *_, **__):
        return Makefile.loads(fp.read())

    def dumps(self, *_, **__):
        return str(self)

    def dump(self, fp, *_, **__):
        fp.write(self.dumps() + '\n')

    def filter(self, op):
        return _filter_list(self, op)

    def recurse(self, max_depth=-1, depth=0):
        yield from _recurse_list(self, max_depth, depth)

    def __repr__(self):
        return f'Makefile({str(self)})'

    def __str__(self):
        return '\n'.join([str(i) for i in self])


class Node:
    def is_block(self):
        return False

    def is_block_list(self):
        return False

    def is_comment(self):
        return False

    def is_directive(self):
        return False

    def is_test(self):
        return False

    def is_benchmark(self):
        return False

    def is_dev(self):
        return self.is_test() or self.is_benchmark()

    def has_block_list(self):
        return False

    def filter(self, op):
        raise NotImplementedError

    def recurse(self, max_depth=-1, depth=0):
        raise NotImplementedError


class Block(str, Node):
    @property
    def child(self):
        return str(self)

    def __repr__(self):
        return f'Block({str(self)})'

    def __str__(self):
        return super().__str__()

    def is_block(self):
        return True

    def split_comments(self):
        return _split_comments(str(self))

    def group_comments(self):
        pass

    def filter(self, op):
        return op(self)


class BlockList(list, Node):
    def __init__(self, *args, **kwds):
        super().__init__(*args, **kwds)
        assert all([isinstance(i, Node) for i in self])

    @property
    def child(self):
        return self

    def __repr__(self):
        return f'BlockList({str(self)})'

    def __str__(self):
        return '\n'.join([str(i) for i in self])

    def is_block_list(self):
        return True

    def split_comments(self):
        return BlockList(util.flatten([i.split_comments() for i in self]))

    def group_comments(self):
        self[:] = _group_comments(self)

    def filter(self, op):
        return _filter_list(self, op)

    def recurse(self, max_depth=-1, depth=0):
        yield from _recurse_list(self, max_depth, depth)


class CommentBlock(Node):
    # the child is either a Block or BlockList
    def __init__(self, comment, title, child):
        assert isinstance(child, Node)

        self.comment = comment
        self.title = title
        self.child = child

    def __eq__(self, other):
        return (self.comment, self.title, self.child) == (other.comment, other.title, other.child)

    def __repr__(self):
        return f'CommentBlock({str(self)})'

    def __str__(self):
        return f'{self.comment}\n{str(self.child)}'

    def is_comment(self):
        return True

    def is_test(self):
        return self.title is not None and util.is_test(self.title)

    def is_benchmark(self):
        return self.title is not None and util.is_benchmark(self.title)

    def has_block_list(self):
        return self.child.is_block_list()

    def split_comments(self):
        raise NotImplementedError('cannot split comments in split comment block')

    def group_comments(self):
        raise NotImplementedError('grouping comments should be done outside a comment block')

    def flatten_single(self):
        if isinstance(self.child, list) and len(self.child) == 1:
            self.child = self.child[0]

    def filter(self, op):
        return op(self) and self.child.filter(op)


class DirectiveBlock(Node):
    # the child is either a Block or BlockList
    def __init__(self, start, end, child):
        assert isinstance(child, Node)
        if isinstance(child, list) and len(child) == 1:
            child = child[0]

        self.start = start
        self.end = end
        self.child = child

    def __eq__(self, other):
        return (self.start, self.end, self.child) == (other.start, other.end, other.child)

    def __repr__(self):
        return f'DirectiveBlock({str(self)})'

    def __str__(self):
        result = f'{self.start}\n{str(self.child)}'
        if self.end is not None:
            result += f'\n{self.end}'
        return result

    def is_directive(self):
        return True

    def has_block_list(self):
        return self.child.is_block_list()

    def split_comments(self):
        child = self.child.split_comments()
        # every caller expects a list, so we return a single-element list
        return BlockList([DirectiveBlock(self.start, self.end, child)])

    def group_comments(self):
        self.child.group_comments()
        self.flatten_single()

    def flatten_single(self):
        if isinstance(self.child, list) and len(self.child) == 1:
            self.child = self.child[0]

    def filter(self, op):
        return op(self) and self.child.filter(op)


# split on comment sections, for example the below will split on the
# benchmarks section.
#
#   LOCAL_PATH := $(call my-dir)
#
#   # -----------------------------------------------------------------------------
#   # Benchmarks.
#   # -----------------------------------------------------------------------------
#
#   test_tags := tests
def _split_comments(contents):
    def new_comment(match, nxt=None):
        comment = match.group(1)
        groups = match.groups()[1:]
        lines = [i for i in groups if i is not None]
        title = '\n'.join([re.sub(r'[ \t]*#[ \t]*', '', i) for i in lines])
        if nxt is None:
            data = contents[match.end():]
        else:
            data = contents[match.end():nxt.start()]
        if nxt is not None:
            assert data.endswith('\n')
            data = data[:-1]
        return CommentBlock(comment, title, Block(data))

    # if we just have 1 or 2 characters, can falsely match.
    # headers can be `# -----`, `# ======`, or `########`.
    # the title can be prefixed, suffixed, or sandwiched by the header.
    def title_pattern():
        line = fr'{sp}*#{sp}*{comment}'
        return fr'(?:(?:{line}{nl})*{line})'

    def sandwich_pattern(sep):
        # matches header-title-header
        title = title_pattern()
        return fr'{sp}*{sep}{nl}({title}){nl}{sp}*{sep}'

    def suffix_pattern(sep):
        # matches title-header
        title = title_pattern()
        return fr'({title}){nl}{sp}*{sep}'

    def prefix_pattern(sep):
        # matches header-title, needs to be last due to greedy regex
        title = title_pattern()
        return fr'{sp}*{sep}{nl}({title})'

    def sep_pattern(sep):
        sandwich = sandwich_pattern(sep)
        suffix = suffix_pattern(sep)
        prefix = prefix_pattern(sep)
        return fr'(?:{sandwich})|(?:{prefix})|(?:{suffix})'

    def create_pattern(*seps):
        groups = []
        for sep in seps:
            groups.append(fr'(?:{sep_pattern(sep)})')
        return fr'(?m)^({"|".join(groups)}){nl}?'

    sep1 = r'#\s+={5,}'
    sep2 = r'#\s+-{5,}'
    sep3 = r'#{6,}'
    sp = r'[ \t]'
    nl = r'(?:\r\n|\r|\n)'
    # can have empty headers, such as `#####\n#`
    comment = r'[^\x00-\x08\x0A-\x1F]*'
    pattern = create_pattern(sep1, sep2, sep3)

    blocks = BlockList()
    if not contents:
        return blocks

    matches = list(re.finditer(pattern, contents))
    if len(matches) == 0:
        blocks.append(Block(contents))
    else:
        first = matches[0]
        last = matches[-1]
        if first.start() != 0:
            assert contents[first.start() - 1] == '\n'
            blocks.append(Block(contents[:first.start() - 1]))
        for (match, nxt) in util.windows(matches, 2):
            blocks.append(new_comment(match, nxt))
        blocks.append(new_comment(last))

    return blocks


# lines is an iterable over each line in the content. splits like something
# above into a start token of `ifneq ($(ENV2),)`, and end of `endif`,
# and the internal contents as a `Block`.
#
#   ifneq ($(ENV2),)
#       benchmark_src_files += bench1.cc
#   else
#       benchmark_src_files += bench2.cc
#   endif
def _split_directives(lines, in_scope=False):
    def add_current(blocks, current):
        if current:
            blocks.append(Block('\n'.join(current)))

    # we ignore else since removing it won't actually affect the code
    start_directives = ('ifeq', 'ifneq', 'ifdef', 'ifndef')
    end_directives = ('endif',)

    blocks = BlockList()
    current = []
    for line in lines:
        trimmed = line.lstrip()
        if trimmed.startswith(start_directives):
            start = line
            add_current(blocks, current)
            child, end = _split_directives(lines, True)
            directive = DirectiveBlock(start, end, child)
            directive.flatten_single()
            blocks.append(directive)
            current = []
        elif in_scope and trimmed.startswith(end_directives):
            end = line
            add_current(blocks, current)
            return blocks, end
        else:
            current.append(line)

    add_current(blocks, current)

    return blocks, None


# this groups directives and comments so any directives within a
# comment block are properly grouped. say i have the following:
#
#   LOCAL_PATH := $(call my-dir)
#
#   # -----------------------------------------------------------------------------
#   # Section 1.
#   # -----------------------------------------------------------------------------
#   LOCAL_SRC_FILES := src.c
#   ifneq ($(ENV2),)
#       benchmark_src_files += bench1.cc
#   else
#       benchmark_src_files += bench2.cc
#   endif
#
#   # -----------------------------------------------------------------------------
#   # Section 2.
#   # -----------------------------------------------------------------------------
#   LOCAL_CFLAGS := $(test_c_flags)
#
#   normally, we'd have 5 sections: block, comment, directive, block, comment
#   however, we want to group it in block, comment, comment, where the directive
#   and subsequent block are in the comment.
def _group_comments(blocks):
    def add_current(result, current):
        if isinstance(current.child, list) and len(current.child) == 1:
            current.child = current.child[0]
        result.append(current)

    def new_comment(block):
        current = CommentBlock(block.comment, block.title, BlockList())
        if block.child:
            current.child.append(block.child)
        return current

    result = BlockList()
    current = BlockList()
    for block in blocks:
        # any comments cannot have been grouped already, so we assume str values
        assert not block.is_comment() or isinstance(block.child, str)
        assert not block.is_block_list()
        if not block.is_comment():
            block.group_comments()

        if current.is_comment() and block.is_comment():
            # new comment replaces the old one
            current.flatten_single()
            result.append(current)
            current = new_comment(block)
        elif block.is_comment():
            # first comment block seen in the file
            result += current
            current = new_comment(block)
        elif current.is_comment():
            # regular block after a comment block
            current.child.append(block)
        else:
            # regular block before any comment blocks
            current.append(block)

    if current.is_comment():
        current.flatten_single()
        result.append(current)
    else:
        result += current

    return result


# retain all items matching the condition in a list
def _filter_list(lst, op):
    # use slice assignment to ensure this happens in-place
    lst[:] = [i for i in lst if i.filter(op)]
    return lst


# yield iteratively all child blocks
def _recurse_list(lst, max_depth=-1, depth=0):
    if depth != max_depth:
        for node in lst:
            yield node
            if node.has_block_list():
                yield from node.child.recurse(max_depth, depth + 1)
