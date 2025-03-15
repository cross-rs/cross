on:
  pull_request:
    types: [labeled, unlabeled, opened, synchronize, reopened]

name: Changelog check

jobs:
  changelog:
    name: Changelog check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: ./.github/actions/setup-rust

      - name: Get Changed Files
        id: files
        uses: tj-actions/changed-files@v41
        with:
          separator: ';'
          files: |
            .changes/*.json

      - name: Validate Changelog
        id: changelog
        run: |
          set -x
          set -e
          IFS=';' read -a added_modified <<< '${{ steps.files.outputs.all_changed_files }}'
          IFS=';' read -a removed <<< '${{ steps.files.outputs.deleted_files }}'
          added_count=${#added_modified[@]}
          removed_count=${#removed[@]}
          if ${{ !contains(github.event.pull_request.labels.*.name, 'no changelog' ) }}; then
            if [[ "$added_count" -eq "0" ]] && [[ "$removed_count" -eq "0" ]]; then
              echo "Must add or remove changes or add the 'no changelog' label"
              exit 1
            else
              cargo xtask changelog validate
            fi
          fi
