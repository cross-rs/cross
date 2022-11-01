android
=======

Contains sample Soong blueprint files and Makefiles to test removal of unittests for build configurations.

This requires a Python3 interpreter, and therefore is not run as part of the core test suite. Running the test suite requires:
- sly >= 0.4
- google-re2 >= 1.0
- pytest >= 7
- toml >= 0.10

The module itself and the scripts only require:
- python >= 3.6
- sly >= 0.4
- google-re2 >= 1.0

google-re2 is needed to avoid backtracking regexes, which destroy performance on near-misses for section headers. The below example, if provided with 10,000 characters after the header, will likely never complete. With re2, this completes nearly instantly.

```Makefile
########################################################################
#
....
```
