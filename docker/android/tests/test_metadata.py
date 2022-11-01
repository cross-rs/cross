import os
import sys

import toml

TEST_DIR = os.path.dirname(os.path.realpath(__file__))
PROJECT_DIR = os.path.dirname(TEST_DIR)
sys.path.insert(0, PROJECT_DIR)

import android


# ensure our pyproject and module metadata don't go out-of-date
def test_metadata():
    pyproject_path = open(os.path.join(PROJECT_DIR, 'pyproject.toml'))
    pyproject = toml.load(pyproject_path)
    project = pyproject['project']
    assert project['name'] == android.__name__
    assert project['version'] == android.__version__
    assert project['license']['text'] == android.__license__

    version, dev = android.__version__.split('-')
    major, minor, patch = [int(i) for i in version.split('.')]
    assert (major, minor, patch, dev) == android.__version_info__
