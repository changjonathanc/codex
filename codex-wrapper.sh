#!/bin/sh
set -eu

# Harbor invokes `codex exec ...`; the lean release target is `codex-exec`.
if [ "${1:-}" = "exec" ]; then
    shift
fi

# Harbor's stock Codex adapter adds this npm-only compatibility flag. The lean
# release binary has unified exec enabled by default, so remove only this pair.
if [ "${6:-}" = "--enable" ] && [ "${7:-}" = "unified_exec" ]; then
    arg1=$1
    arg2=$2
    arg3=$3
    arg4=$4
    arg5=$5
    shift 7
    set -- "$arg1" "$arg2" "$arg3" "$arg4" "$arg5" "$@"
fi

exec "${CODEX_EXEC_BIN:-/usr/local/lib/codex-exec}" "$@"
