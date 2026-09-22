#!/bin/sh
set -e
BIN="/usr/bin/cassandra"
if command -v setcap >/dev/null 2>&1 && [ -f "$BIN" ]; then
  setcap -r "$BIN" || true
fi
exit 0
