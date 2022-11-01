import re2 as re


def windows(sequence, count):
    for i in range(len(sequence) - count + 1):
        yield sequence[i:i + count]


def flatten(lst):
    return [i for sublist in lst for i in sublist]


def _is_match(pattern, string):
    return re.search(pattern, string) is not None


def is_test(string):
    # need to consider that works like `latest` exist
    # also need to consider `non-test` for `fmtlib`.
    if 'non-test' in string.lower():
        return False
    pattern = r'(?i)(?:^|[^A-Za-z0-9]|g)test'
    return _is_match(pattern, string)


def is_benchmark(string):
    pattern = r'(?i)(?:^|[^A-Za-z0-9])benchmark'
    return _is_match(pattern, string)
