#!/usr/bin/env bash
# CI: run `cargo test <args>` and, when it fails, turn each failure into a GitHub annotation
# (the panic site plus its whole message up to rustc's backtrace note, at most 80 lines of 300
# characters; and cargo's own `error:` lines), so a failed run can be diagnosed from the check's
# annotations without signing in to read the log.
#   tools/ci-cargo-test.sh --workspace --locked
set -uo pipefail
log="${RUNNER_TEMP:-/tmp}/cargo-test-$$.log"
st=0
cargo test "$@" 2>&1 | tee "$log" || st=$?
if [ "$st" -ne 0 ]; then
  awk 'function flush() { if (on) print "::error title=cargo test::" msg; on = 0 }
       /panicked at/ { flush(); on = 1; n = 0; msg = "" }
       on && (/^note: run with `RUST_BACKTRACE/ || /^stack backtrace:/ || /^---- .* stdout ----$/ || /^failures:$/ || n >= 80) { flush(); next }
       on { l = substr($0, 1, 300); gsub(/%/, "%25", l); gsub(/\r/, "", l); msg = msg (n ? "%0A" : "") l; n++ }
       END { flush() }' "$log" | head -40
  grep -E '^error(\[|:)' "$log" | head -10 | sed 's/^/::error title=cargo::/'
fi
rm -f "$log"
exit "$st"
