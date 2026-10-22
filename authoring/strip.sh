#!/bin/sh
# Print a C file without comments and blank lines.
perl -0pe 's{/\*.*?\*/}{}gs' "$1" | grep -v '^\s*$'
