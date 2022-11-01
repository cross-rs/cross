import os
import sys

TEST_DIR = os.path.dirname(os.path.realpath(__file__))
PROJECT_DIR = os.path.dirname(TEST_DIR)
sys.path.insert(0, PROJECT_DIR)

from android import util


def test_is_test():
    assert not util.is_test('lib-non-test-defaults')
    assert util.is_test('art-tests')
    assert util.is_test('libgtest')
    assert util.is_test('libgtest_main')
    assert util.is_test('extra-tests')


def test_is_benchmark():
    assert util.is_benchmark('benchmark')
    assert util.is_benchmark('benchmarks')
    assert util.is_benchmark('-benchmarks')
    assert not util.is_benchmark('gbenchmarks')
