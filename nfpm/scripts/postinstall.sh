#!/bin/sh
set -e
BIN="/usr/bin/cassandra"
if command -v setcap >/dev/null 2>&1 && [ -f "$BIN" ]; then
  setcap cap_dac_read_search+ep "$BIN" || echo "Warning: setcap failed for $BIN (need libcap)" >&2
else
  echo "Note: setcap not found or $BIN missing — run 'sudo ./cap.sh' manually" >&2
fi
exit 0
