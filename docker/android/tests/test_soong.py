import copy
import os
import sys

TEST_DIR = os.path.dirname(os.path.realpath(__file__))
PROJECT_DIR = os.path.dirname(TEST_DIR)
sys.path.insert(0, PROJECT_DIR)

from android import soong


def test():
    path = os.path.join(TEST_DIR, 'Android.bp')
    contents = open(path).read()
    lexer = soong.Lexer()
    tokens = list(lexer.tokenize(contents))
    assert (tokens[0].type, tokens[0].value) == ('IDENT', 'sample_array')
    assert (tokens[51].type, tokens[51].value) == ('IDENT', 'srcs')
    assert (tokens[52].type, tokens[52].value) == ('COLON', ':')
    assert (tokens[53].type, tokens[53].value) == ('LBRACKET', '[')
    assert (tokens[54].type, tokens[54].value) == ('STRING', '"tree.cc"')

    parser = soong.Parser()
    result = parser.parse(iter(tokens))
    assert len(result) == 7

    assert result[0].is_assignment()
    assert result[0].to_str() == '''sample_array = [
    "value1",
    "value2",
]'''

    assert result[1].is_scope()
    assert result[1].name == 'cc_defaults'
    assert result[1].name.is_ident()
    assert result[1].map['name'] == 'target'
    assert result[1].map['tidy_checks'] == 'sample_array'
    assert result[1].map.get('srcs') is None
    assert result[1].map.is_map()

    assert result[2].is_scope()
    assert result[2].name == 'cc_library_static'
    assert result[2].map['name'] == 'static_lib'

    ast = soong.loads(contents)
    assert ast == result
    ast = soong.load(open(path))
    assert ast == result
    lines = contents.splitlines()
    assert ast.dumps() == '\n'.join(lines[1:5] + lines[10:])

    assert ast[4].is_test()
    assert ast[4].map.is_test()

    filtered = copy.deepcopy(ast)
    filtered.filter(lambda x: not (x.is_scope() and x.is_dev()))
    assert type(filtered) is soong.Ast
    assert len(filtered) == 5
    assert filtered == ast[:4] + [ast[6]]

    map = filtered[1].map
    assert 'cflags' in map
    map.filter(lambda k, v: k != 'cflags')
    assert 'cflags' not in map
    assert len(map['array']) == 2
    map['array'].filter(lambda x: x != '-short')
    assert len(map['array']) == 1

    custom = filtered[4].map
    assert 'whole_static_libs' in custom
    custom['whole_static_libs'].filter(lambda x: x.str_op(lambda y: 'gtest' not in y.lower()))
    assert custom['whole_static_libs'] == ['libz']

    assert 'host_ldlibs' in custom
    custom['host_ldlibs'].filter(lambda x: x.str_op(lambda y: 'gtest' not in y.lower()))
    assert custom['host_ldlibs'] == []


def test_addition():
    path = os.path.join(TEST_DIR, 'Addition.bp')
    ast = soong.load(open(path))
    assert len(ast) == 27
    assert ast[0].is_assignment()
    assert ast[1].is_binary_operator_assignment()
    assert ast[2].is_assignment()
    assert ast[3].is_binary_operator_assignment()
    assert ast[4].is_assignment()
    assert ast[5].is_binary_operator_assignment()
    assert ast[6].is_scope()
    assert ast[7].is_binary_operator_assignment()
    assert ast[8].expr.is_binary_operator()

    assert ast[0].name == 'list'
    assert ast[0].expr == ['value1']
    assert ast[1].name == 'list'
    assert ast[1].op == '+='
    assert ast[1].expr == ['value2']

    assert ast[8].expr.lhs == 'number'
    assert ast[8].expr.op == '+'
    assert ast[8].expr.rhs == 4
    assert ast[11].expr.lhs == 'scope'
    assert ast[11].expr.op == '+'
    assert ast[11].expr.rhs.is_map()

    assert ast[12].expr.lhs == 4
    assert ast[12].expr.op == '+'
    assert ast[12].expr.rhs == 'number'
    assert ast[15].expr.lhs.is_map()
    assert ast[15].expr.op == '+'
    assert ast[15].expr.rhs == 'scope'

    assert ast[16].expr.lhs == 4
    assert ast[16].expr.op == '+'
    assert ast[16].expr.rhs == 1
    assert ast[19].expr.lhs == {}
    assert ast[19].expr.op == '+'
    assert ast[19].expr.rhs == {'name': 'target'}

    assert ast[20].expr.lhs.is_binary_operator()
    assert ast[20].expr.lhs.lhs == 4
    assert ast[20].expr.lhs.rhs == 1
    assert ast[20].expr.op == '+'
    assert ast[20].expr.rhs == 2

    assert ast[26].name == 'files'
    assert ast[26].expr.is_list()
    assert len(ast[26].expr) == 3

    assert ast[26].expr[0].lhs == 'home'
    assert ast[26].expr[0].lhs.is_ident()
    assert ast[26].expr[0].rhs == 'file.c'
    assert ast[26].expr[0].rhs.is_string()

    assert ast[26].expr[1].lhs == 'test/'
    assert ast[26].expr[1].lhs.is_string()
    assert ast[26].expr[1].rhs == 'test'
    assert ast[26].expr[1].rhs.is_ident()

    assert ast[26].expr[2].lhs == 'home'
    assert ast[26].expr[2].lhs.is_ident()
    assert ast[26].expr[2].rhs == 'test'
    assert ast[26].expr[2].rhs.is_ident()

    # test a few binops, just in case
    binop = ast[26].expr[1]
    assert binop.str_op(lambda x: 'test' in x.lower())
    assert binop.lhs.str_op(lambda x: 'test' in x.lower())


def test_empty():
    path = os.path.join(TEST_DIR, 'Empty.bp')
    ast = soong.load(open(path))
    assert len(ast) == 0


def test_list_map_parse():
    path = os.path.join(TEST_DIR, 'ListMap.bp')
    ast = soong.load(open(path))
    assert len(ast) == 1

    scope = ast[0]
    assert scope.is_scope()
    assert scope.name == 'scope'
    map = scope.map['key']

    assert map.value.is_list()
    assert len(map.value) == 1
    assert map.value[0].is_map()

    inner = map.value[0]
    assert len(inner) == 2
    assert inner['name'] == 'art'
    assert inner['deps'].value == soong.List([soong.String('"dependency"')])


def test_is_non_test():
    path = os.path.join(TEST_DIR, 'NonTest.bp')
    ast = soong.load(open(path))
    assert len(ast) == 1

    scope = ast[0]
    assert scope.is_scope()
    assert scope.name == 'cc_defaults'
    assert scope.map['name'].value == 'lib-non-test-defaults'


def test_ast():
    array = soong.List([soong.String('"value1"'), soong.String('"value2"')])
    assignment = soong.Assignment(soong.Ident('name'), array)
    value = soong.MapValue('=', soong.String('"value"'))
    map = soong.Map({soong.Ident('key'): value})
    scope = soong.Scope(soong.Ident('name'), map)
    ast = soong.Ast([assignment, scope])
    assert repr(ast) == '''Ast(name = ["value1", "value2"]
name {key = "value"})'''
    assert str(ast) == '''name = ["value1", "value2"]
name {key = "value"}'''
    assert ast.to_str() == '''name = [
    "value1",
    "value2",
]
name {
    key = "value",
}'''


def test_assignment():
    array = soong.List([soong.String('"value1"'), soong.String('"value2"')])
    assignment = soong.Assignment(soong.Ident('name'), array)
    assert repr(assignment) == 'Assignment(name = ["value1", "value2"])'
    assert str(assignment) == 'name = ["value1", "value2"]'
    assert assignment.to_str(pretty=False) == 'name = ["value1", "value2"]'
    assert assignment.to_str() == '''name = [
    "value1",
    "value2",
]'''
    assert assignment.to_str(depth=1) == '''name = [
        "value1",
        "value2",
    ]'''


def test_binary_operator_assignment():
    ident = soong.Ident('name')
    expr = soong.Integer('1')
    assignment = soong.BinaryOperatorAssignment(ident, '+=', expr)
    assert repr(assignment) == 'BinaryOperatorAssignment(name += 1)'
    assert str(assignment) == 'name += 1'
    assert assignment.to_str(pretty=False) == 'name += 1'
    assert assignment.to_str() == 'name += 1'


def test_binary_operator():
    ident = soong.Ident('name')
    expr = soong.Integer('1')
    operator = soong.BinaryOperator(ident, '+', expr)
    assert repr(operator) == 'BinaryOperator(name + 1)'
    assert str(operator) == 'name + 1'
    assert operator.to_str(pretty=False) == 'name + 1'
    assert operator.to_str() == 'name + 1'


def test_scope():
    value = soong.MapValue(':', soong.String('"value"'))
    map = soong.Map({soong.Ident('key'): value})
    scope = soong.Scope(soong.Ident('name'), map)
    assert repr(scope) == 'Scope(name {key: "value"})'
    assert str(scope) == 'name {key: "value"}'
    assert scope.to_str(pretty=False) == 'name {key: "value"}'
    assert scope.to_str() == '''name {
    key: "value",
}'''
    assert scope.to_str(depth=1) == '''name {
        key: "value",
    }'''


def test_map():
    value = soong.MapValue(':', soong.String('"value"'))
    map = soong.Map({soong.Ident('key'): value})
    assert repr(map) == 'Map({key: "value"})'
    assert str(map) == '{key: "value"}'
    assert map.to_str(pretty=False) == '{key: "value"}'
    assert map.to_str() == '''{
    key: "value",
}'''
    assert map.to_str(depth=1) == '''{
        key: "value",
    }'''

    map = soong.Map()
    assert str(map) == '{}'
    assert map.to_str() == '{}'


def test_recurse():
    path = os.path.join(TEST_DIR, 'Android.bp')
    ast = soong.load(open(path))
    cc_defaults = ast[1]
    assert cc_defaults.name == 'cc_defaults'
    for (key, value, depth, parent) in cc_defaults.map.recurse():
        assert depth == 1

    cc_test = ast[4]
    assert cc_test.name == 'cc_test'
    seen = []
    for (key, value, depth, parent) in cc_test.map.recurse():
        if depth > 1 and parent.is_map():
            seen.append(key)
    assert seen == ['array', 'option']


def test_list():
    sequence = soong.List([soong.String('"value1"'), soong.String('"value2"')])
    assert repr(sequence) == 'List(["value1", "value2"])'
    assert str(sequence) == '["value1", "value2"]'
    assert sequence.to_str(pretty=False) == '["value1", "value2"]'
    assert sequence.to_str() == '''[
    "value1",
    "value2",
]'''
    assert sequence.to_str(depth=1) == '''[
        "value1",
        "value2",
    ]'''

    sequence = soong.List([soong.String('"value"')])
    assert repr(sequence) == 'List(["value"])'
    assert str(sequence) == '["value"]'
    assert sequence.to_str() == '["value"]'

    sequence = soong.List([])
    assert sequence.to_str() == '[]'


def test_map_value():
    value = soong.MapValue(':', soong.String('"value"'))
    assert repr(value) == 'MapValue(: "value")'
    assert str(value) == ': "value"'
    assert value.to_str() == ': "value"'

    value = soong.MapValue('=', soong.String('"value"'))
    assert repr(value) == 'MapValue( = "value")'
    assert str(value) == ' = "value"'
    assert value.to_str() == ' = "value"'


def test_list_map():
    value = soong.MapValue(':', soong.String('"value"'))
    map = soong.Map({soong.Ident('key'): value})
    sequence = soong.List([map])
    assert repr(sequence) == 'List([{key: "value"}])'
    assert str(sequence) == '[{key: "value"}]'
    assert sequence.to_str(pretty=False) == '[{key: "value"}]'
    assert sequence.to_str() == '''[{
    key: "value",
}]'''


def test_ident():
    ident = soong.Ident('name')
    assert repr(ident) == 'Ident(name)'
    assert str(ident) == 'name'
    assert ident.to_str() == 'name'


def test_string():
    string = soong.String('"value1"')
    assert repr(string) == 'String("value1")'
    assert str(string) == 'value1'
    assert string.to_str() == '"value1"'


def test_integer():
    number = soong.Integer('3')
    assert repr(number) == 'Integer(3)'
    assert str(number) == '3'
    assert number.to_str() == '3'


def test_bool():
    boolean = soong.Bool(True)
    assert repr(boolean) == 'Bool(true)'
    assert str(boolean) == 'true'
    assert boolean.to_str() == 'true'
