import sys

# we run this script once every build, and we'd rather
# have much smaller image sizes, so copying without
# any bytecode is a better idea.
sys.dont_write_bytecode = True

__version__ = '0.0.0-dev.0'
__version_info__ = (0, 0, 0, 'dev.0')
__license__ = 'MIT OR Apache-2.0'

__all__ = [
    "make",
    "soong",
]
