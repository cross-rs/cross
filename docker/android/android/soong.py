'''
    soong
    =====

    utilities to process soong blueprint files. these are a go-like,
    json-like data file format similar. they support nested maps, lists,
    bools, strings, and use of variables. for example:

        array = ["..."]
        cc_defaults {
          name: "target",
          options: array,
          flags: ["..."],
        }
        cc_test {
          name: "test",
          defaults: ["target"],
          srcs: ["test.cc"],
          nested: {
            array: {
              option: false,
            },
          },
        }

    the specification can be found below:
    https://source.android.com/docs/core/tests/development/blueprints
    https://android.googlesource.com/platform/build/soong/+/refs/heads/master/README.md

    they also support single-line C++-style and multiline C-style comments.
    the valid types are:
        - bool (`true`, `false`)
        - int
        - string
        - list (of strings)
        - map

    both lists and maps support optional trailing commas. any value type
    can be present in a map, while only strings are allowed in lists.
    integers, strings, arrays and maps also also support the `+` operator,
    where `+` sums up integers. for strings and arrays, it appends the new
    data. for maps, it produces the union of both keys, and for keys present
    in both, it appends the value on the right-operand to the value in the
    left one.

    variable assignment produces immutable types, except for the `+=` operator.
    `+=` does the described operation above in-place.

    this parser doesn't need to be exactly correct: it does not need to reject
    subtley invalid input. for example `name = {  }` may or may not be correct,
    but it's fine to accept it as long as we output it identically. this is
    supposed to handle all correct input and outputs it as correct output:
    it doesn't need to validate type correctness.

    this uses LALR parsing since it makes the grammar very easy to define and
    the parsing simple. since the build step and repository synchronization
    is much slower, the performance here is practically irrelevant.
'''

import json
import sys

import sly

from . import util

# dictionaries got insertion order in 3.6, guaranteed in 3.7
assert sys.version_info >= (3, 6)

# base character defs
_H = r'[0-9a-f]'
_NL = r'\n|\r\n|\r|\f'
_UNICODE = fr'\\{_H}{1,6}(\r\n|[ \t\r\n\f])?'
_ESCAPE = r'{_UNICODE}|\\[^\r\n\f0-9a-f]'
_SINGLELINE_COMMENT = r'\/\/.*'
# can't use reflags without setting them for all, so do manual dotall
_MULTILINE_COMMENT = r'\/\*[\u0000-\U0010FFFF]*?\*\/'
_COMMENT = fr'(?:{_SINGLELINE_COMMENT})|(?:{_MULTILINE_COMMENT})'


def loads(contents, *_, **__):
    return Ast.loads(contents)


def load(fp, *_, **__):
    return Ast.load(fp)


def dumps(soong, pretty=True, indent=4, *_, **__):
    return soong.dumps(pretty, indent)


def dump(soong, fp, pretty=True, indent=4, *_, **__):
    return soong.dump(fp, pretty, indent)


class Lexer(sly.Lexer):
    tokens = {
        BOOL,
        INTEGER,
        IDENT,
        STRING,
        LBRACKET,
        RBRACKET,
        LBRACE,
        RBRACE,
        COLON,
        COMMA,
        EQUALS,
        PLUS,
    }
    ignore = ' \t'
    ignore_comment = _COMMENT

    # Tokens
    # this uses a string regex based on the CSS2.1 grammar
    STRING = fr'"([^\n\r\f\\"]|\\{_NL}|{_ESCAPE})*"'
    INTEGER = r'\d+'
    BOOL = '(?:true)|(?:false)'
    IDENT = r'[a-zA-Z_][a-zA-Z0-9_]*'
    LBRACKET = r'\['
    RBRACKET = r'\]'
    LBRACE = r'\{'
    RBRACE = r'\}'
    COLON = r':'
    COMMA = r','
    EQUALS = r'='
    PLUS = r'\+'

    @_(r'\n+')
    def newline(self, token):
        self.lineno += token.value.count('\n')

    def error(self, token):
        raise ValueError(f'Illegal character \'{token.value[0]}\'')


class Parser(sly.Parser):
    tokens = Lexer.tokens

    precedence = (
        ('left', PLUS),
    )

    @_('rules')
    def ast(self, prod):
        return Ast(prod.rules)

    @_('empty')
    def ast(self, prod):
        return Ast()

    @_('rules rule')
    def rules(self, prod):
        return prod.rules + [prod.rule]

    @_('rule')
    def rules(self, prod):
        return [prod.rule]

    @_('assignment', 'binary_operator_assignment', 'scope')
    def rule(self, prod):
        return prod[0]

    @_('ident EQUALS expr')
    def assignment(self, prod):
        return Assignment(prod.ident, prod.expr)

    @_('ident PLUS EQUALS expr')
    def binary_operator_assignment(self, prod):
        return BinaryOperatorAssignment(
            prod.ident,
            f'{prod[1]}{prod[2]}',
            prod.expr,
        )

    @_('expr PLUS expr')
    def binary_operator(self, prod):
        return BinaryOperator(prod[0], prod[1], prod[2])

    @_('ident map')
    def scope(self, prod):
        return Scope(prod.ident, prod.map)

    @_('LBRACE pairs RBRACE', 'LBRACE pairs COMMA RBRACE')
    def map(self, prod):
        return Map(prod.pairs)

    @_('LBRACE RBRACE')
    def map(self, prod):
        return Map()

    @_('pairs COMMA pair')
    def pairs(self, prod):
        return prod.pairs + [prod.pair]

    @_('pair')
    def pairs(self, prod):
        return [prod.pair]

    @_('ident COLON expr', 'ident EQUALS expr')
    def pair(self, prod):
        return (prod.ident, MapValue(prod[1], prod.expr))

    @_('ident', 'binary_operator', 'map', 'list', 'string', 'integer', 'bool')
    def expr(self, prod):
        return prod[0]

    @_('LBRACKET sequence RBRACKET', 'LBRACKET sequence COMMA RBRACKET')
    def list(self, prod):
        return List(prod.sequence)

    @_('LBRACKET RBRACKET')
    def list(self, prod):
        return List()

    @_('sequence COMMA list_item')
    def sequence(self, prod):
        return prod.sequence + [prod.list_item]

    @_('list_item')
    def sequence(self, prod):
        return [prod.list_item]

    @_('list_item PLUS list_item')
    def list_item(self, prod):
        return BinaryOperator(prod[0], '+', prod[2])

    @_('string', 'ident', 'map')
    def list_item(self, prod):
        return prod[0]

    @_('IDENT')
    def ident(self, prod):
        return Ident(prod.IDENT)

    @_('STRING')
    def string(self, prod):
        return String(prod.STRING)

    @_('INTEGER')
    def integer(self, prod):
        return Integer(prod.INTEGER)

    @_('BOOL')
    def bool(self, prod):
        return Bool(json.loads(prod.BOOL))

    # needed in case no tokens are produced
    @_('')
    def empty(self, p):
        pass

    def error(self, token):
        raise ValueError(f'Illegal token {repr(token)}')


class Node:
    def is_assignment(self):
        return False

    def is_binary_operator_assignment(self):
        return False

    def is_binary_operator(self):
        return False

    def is_scope(self):
        return False

    def is_map(self):
        return False

    def is_list(self):
        return False

    def is_map_value(self):
        return False

    def is_ident(self):
        return False

    def is_string(self):
        return False

    def is_integer(self):
        return False

    def is_bool(self):
        return False


class Ast(list, Node):
    def __init__(self, values=None):
        if values is None:
            values = []
        valid_nodes = (Assignment, BinaryOperatorAssignment, Scope)
        assert all(isinstance(i, valid_nodes) for i in values)
        super().__init__(values)

    def __repr__(self):
        return f'Ast({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        assert depth == 0
        return '\n'.join([i.to_str(pretty, indent, depth) for i in self])

    @staticmethod
    def loads(contents, *_, **__):
        lexer = Lexer()
        tokens = lexer.tokenize(contents)
        parser = Parser()
        return parser.parse(tokens)

    @staticmethod
    def load(fp, *_, **__):
        return Ast.loads(fp.read())

    def dumps(self, pretty=True, indent=4, *_, **__):
        return self.to_str(pretty, indent)

    def dump(self, fp, pretty=True, indent=4, *_, **__):
        # always write a trailing newline
        fp.write(self.dumps(pretty, indent) + '\n')

    def filter(self, op):
        # use slice assignment to ensure this happens in-place
        self[:] = [i for i in self if op(i)]


class Assignment(Node):
    def __init__(self, name, expr):
        self.name = name
        self.expr = expr

    def __repr__(self):
        return f'Assignment({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        return f'{str(self.name)} = {self.expr.to_str(pretty, indent, depth)}'

    def is_assignment(self):
        return True

    def __eq__(self, other):
        return (self.name, self.expr) == (other.name, other.expr)


class BinaryOperatorAssignment(Node):
    def __init__(self, name, op, expr):
        self.name = name
        self.op = op
        self.expr = expr

    def __repr__(self):
        return f'BinaryOperatorAssignment({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        expr = self.expr.to_str(pretty, indent, depth)
        return f'{str(self.name)} {self.op} {expr}'

    def is_binary_operator_assignment(self):
        return True

    def __eq__(self, other):
        return (self.name, self.op, self.expr) == (other.name, other.op, other.expr)


class BinaryOperator(Node):
    def __init__(self, lhs, op, rhs):
        self.lhs = lhs
        self.op = op
        self.rhs = rhs

    def __repr__(self):
        return f'BinaryOperator({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        lhs = self.lhs.to_str(pretty, indent, depth)
        rhs = self.rhs.to_str(pretty, indent, depth)
        return f'{lhs} {self.op} {rhs}'

    def is_binary_operator(self):
        return True

    def str_op(self, cmp):
        return (
            (self.lhs.is_string() and self.lhs.str_op(cmp))
            or (self.rhs.is_string() and self.rhs.str_op(cmp))
        )

    def __eq__(self, other):
        return (self.lhs, self.op, self.rhs) == (other.lhs, other.op, other.rhs)


class Scope(Node):
    def __init__(self, name, map):
        self.name = name
        self.map = map

    def __repr__(self):
        return f'Scope({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        return f'{str(self.name)} {self.map.to_str(pretty, indent, depth)}'

    def is_scope(self):
        return True

    def __eq__(self, other):
        return (self.name, self.map) == (other.name, other.map)

    def is_art_check(self):
        return 'art-check' in self.name.lower() or self.map.is_art_check()

    def is_test(self):
        return util.is_test(self.name) or self.map.is_test()

    def is_benchmark(self):
        return util.is_benchmark(self.name) or self.map.is_benchmark()

    def is_dev(self):
        return self.is_art_check() or self.is_test() or self.is_benchmark()


class Map(dict, Node):
    def __repr__(self):
        return f'Map({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        fmt = lambda x: x.to_str(pretty, indent, depth + 1)
        result = '{'
        pairs = [f'{fmt(k)}{fmt(v)}' for k, v in self.items()]
        if len(self) == 0:
            result += '}'
        elif pretty:
            result += '\n'
            for pair in pairs:
                result += _indent(indent, depth + 1) + f'{pair},\n'
            result += _indent(indent, depth) + '}'
        else:
            result += ', '.join(pairs) + '}'

        return result

    def is_map(self):
        return True

    def is_art_check(self):
        name = self.get('name')
        if name is None:
            return False
        return 'art-check' in name.value.lower()

    def is_test(self):
        name = self.get('name')
        if name is None:
            return False
        # cannot remove `py2-c-module-_ctypes_test` type tests,
        # since they're needed to be linked in the final binary.
        lower = name.value.lower()
        return util.is_test(lower) and 'py2-c-module' not in lower

    def is_benchmark(self):
        name = self.get('name')
        if name is None:
            return False
        return util.is_benchmark(name.value)

    def is_dev(self):
        return self.is_test() or self.is_benchmark()

    def filter(self, op):
        filtered = {k: v for k, v in self.items() if op(k, v)}
        self.clear()
        self.update(filtered)

    def recurse(self, max_depth=-1, depth=0):
        # recursively find all key/value pairs the current and any submaps
        if depth != max_depth:
            for key, value in self.items():
                yield (key, value, depth + 1, self)
                if value.value.is_map():
                    yield from value.value.recurse(max_depth, depth + 1)


class List(list, Node):
    def __repr__(self):
        return f'List({str(self)})'

    def __str__(self):
        return self.to_str(pretty=False)

    def to_str(self, pretty=True, indent=4, depth=0):
        def fmt(x):
            if x.is_map():
                return x.to_str(pretty, indent, depth)
            return x.to_str(pretty, indent, depth + 1)
        result = '['
        if len(self) <= 1 or not pretty:
            result += ', '.join([fmt(i) for i in self]) + ']'
        else:
            result += '\n'
            for element in self:
                result += _indent(indent, depth + 1) + f'{fmt(element)},\n'
            result += _indent(indent, depth) + ']'

        return result

    def is_list(self):
        return True

    def filter(self, op):
        # use slice assignment to ensure this happens in-place
        self[:] = [i for i in self if op(i)]


class MapValue(Node):
    def __init__(self, delimiter, value):
        # map key/value separators can be `:` or `=`.
        assert delimiter in (':', '=')
        self.delimiter = delimiter
        self.value = value

    def __repr__(self):
        return f'MapValue({str(self)})'

    def __str__(self):
        return self.to_str(False)

    def __eq__(self, other):
        # delimiter doesn't matter for equality comparison
        if isinstance(other, MapValue):
            return self.value == other.value
        return self.value == other

    def __len__(self):
        return len(self.value)

    def to_str(self, pretty=True, indent=4, depth=0):
        value = self.value.to_str(pretty, indent, depth)
        if self.delimiter == '=':
            return f' = {value}'
        return f': {value}'

    def str_op(self, cmp):
        return self.value.str_op(cmp)

    def is_map_value(self):
        return True

    def filter(self, op):
        self.value.filter(op)


class Ident(str, Node):
    def __repr__(self):
        return f'Ident({str(self)})'

    def __str__(self):
        return super().__str__()

    def to_str(self, *_, **__):
        return str(self)

    def is_ident(self):
        return True


class String(str, Node):
    def __repr__(self):
        return f'String({self.to_str()})'

    def to_str(self, *_, **__):
        return f'{super().__str__()}'

    def str_op(self, cmp):
        return cmp(self)

    def __str__(self):
        # `"target"` should be shown as `'target'`, not `'"target"'`
        return super().__str__()[1:-1]

    def __eq__(self, other):
        if type(other) is String:
            return str(self) == str(other)
        # we want to be compare equal to the string's value
        return str(self) == other

    def __ne__(self, other):
        # need to override `__ne__` which normally uses a pyslot
        return not self.__eq__(other)

    def is_string(self):
        return True


class Integer(int, Node):
    def __repr__(self):
        return f'Integer({str(self)})'

    def __str__(self):
        return str(int(self))

    def to_str(self, *_, **__):
        return str(self)

    def is_integer(self):
        return True


class Bool(Node):
    def __init__(self, value=False):
        self.value = value

    def __bool__(self):
        return self.value

    def __repr__(self):
        return f'Bool({json.dumps(self.value)})'

    def __str__(self):
        return json.dumps(self.value)

    def to_str(self, *_, **__):
        return str(self)

    def is_bool(self):
        return True

    def __eq__(self, other):
        return self.value == other.value


def _indent(indent=4, depth=0, char=' '):
    return char * indent * depth
