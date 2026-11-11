#!/bin/sh
# The handheld's frontend, run off screen: no window, no sound. It boots the real frontend on a
# card made for the run, plays a script of button presses (as examples/record.rs reads them),
# and keeps the frames the script asks for as PNGs in OUT, with omega-zero's log beside them.
#
#   scripts/sim.sh [SCRIPT] [OUT]
#
# The card holds two PSP games and one GBA game, a label for the first PSP game where
# docs/psp-labels has one, and settings that boot on the PSP shelf with it picked.
# SCRIPT defaults to the UMD going in. Set SIM_CPUS to pin the run to that many CPUs, and
# SIM_QUOTA to a share of each, as the GitHub workflow does to come near the H700's four A53s.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
script=${1:-$here/scripts/sim/umd-insert.txt}
out=${2:-$here/target/sim}
card=$(mktemp -d)
trap 'rm -rf "$card"' EXIT

stem="2040 - Tekken 6 (USA) (En,Fr,De,Es,It,Ru)"
mkdir -p "$card/Games/PSP" "$card/Games/GBA" "$card/Labels/PSP" "$card/Config" "$out"
head -c 65536 /dev/zero > "$card/Games/PSP/$stem.iso"
head -c 65536 /dev/zero > "$card/Games/PSP/Zzz Other.iso"
head -c 4096 /dev/zero > "$card/Games/GBA/Test.gba"
# a label scan is the game's art, kept off the repo: one is used where the card it came from is
label="$here/docs/psp-labels/$stem.png"
if [ -f "$label" ]; then cp "$label" "$card/Labels/PSP/$stem.png"; fi
printf 'cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=0\nshelf=psp\npicked=%s\npicked_platform=psp\n' \
	"$stem" > "$card/Config/omega-zero.state"

cd "$here"
cargo build -q --release -p omega-zero --example record
run="${CARGO_TARGET_DIR:-target}/release/examples/record"
if [ -n "${SIM_CPUS:-}" ]; then
	run="taskset -c 0-$((SIM_CPUS - 1)) $run"
fi
if [ -n "${SIM_QUOTA:-}" ]; then
	if systemd-run --user --scope -q -p CPUQuota="$SIM_QUOTA" true 2>/dev/null; then
		run="systemd-run --user --scope -q -p CPUQuota=$SIM_QUOTA $run"
	else
		echo "sim: no systemd user session here, so the run is not held to $SIM_QUOTA" >&2
	fi
fi
LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe OMEGA_SILENT=1 \
	$run "$card" "$script" "$out" 2> "$out/omega-zero.log"
grep -E "record:|going in|slow" "$out/omega-zero.log" || true
ls "$out"
