# Changes

This stores changes to automatically generate the changelog, to avoid merge conflicts. Files should be in a JSON format, with the following format:

```json
{
    "description": "single-line description to add to the CHANGELOG.",
    "issues": [894],
    "type": "added",
    "breaking": false
}
```

Valid types are:
- added (Added)
- changed (Changed)
- fixed (Fixed)
- removed (Removed)
- internal (Internal)

`breaking` is optional and defaults to false. if `breaking` is present for any active changes, a `BREAKING:` notice will be added at the start of the entry. `issues` is also optional, and is currently unused, and is an array of issues fixed by the PR, and defaults to an empty array.

The file numbers should be `${pr}.json`. The `pr` is optional, and if not, an issue number should be used, in the `_${issue}.json` format. We also support multiple PRs per entry, using the `${pr1}-${pr2}-(...).json` format.

If multiple changes are made in a single PR, you can also pass an array of entries:

```json
[
    {
        "description": "this is one added entry.",
        "issues": [630],
        "type": "added"
    },
    {
        "description": "this is another added entry.",
        "issues": [642],
        "type": "added"
    },
    {
        "description": "this is a fixed entry that has no attached issue.",
        "type": "fixed"
    }
]
```

See [template](/.changes/template) for sample object and array-based changes.
