#!/bin/sh
# Stock gpSP's Advance Wars serial protocol, flipped from lone master to joiner mid-game as a
# session started while the game is on its link screen does: alone it overflows its peer
# queue; with serstate cleared as the session starts, as omega-zero clears it through gpSP's
# symbol table (crates/omega-retro/src/gpsp.rs), it does not.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
tarball="${1:?usage: run.sh TARBALL}"
cc="${CC:-cc}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

top="$(tar -tzf "$tarball" | head -1 | cut -d/ -f1)"
tar -xzf "$tarball" -C "$work"
src="$work/$top"
[ -f "$src/serial_proto.c" ] || { echo "$tarball has no $top/serial_proto.c" >&2; exit 1; }

asan="-fsanitize=address"
if ! echo 'int main(void){return 0;}' | "$cc" $asan -x c -o "$work/probe" - 2>/dev/null; then
	echo "note: $cc has no working -fsanitize=address, running on assertions alone"
	asan=""
fi

build() {
	mkdir -p "$1"
	cp "$src/serial_proto.c" "$src/serial.h" "$1/"
	"$cc" -I"$1" -I"$here" $asan $2 -g -O1 -o "$1/awtest" "$here/awtest.c"
}

build "$work/stock" ""
build "$work/reset" "-DFRONTEND_RESET"

if "$work/stock/awtest" >"$work/stock.log" 2>&1; then
	echo "FAIL: stock gpSP passed, so this test no longer proves anything" >&2
	exit 1
fi
echo "ok: stock gpSP overflows on the flip, as it must"
grep -m1 -E 'ERROR|SUMMARY|Assertion|assert' "$work/stock.log" || true

if ! "$work/reset/awtest"; then
	echo "FAIL: stock gpSP still overflows with serstate cleared as the session starts" >&2
	exit 1
fi
echo "ok: with serstate cleared as the session starts, the queue holds across the flip"
