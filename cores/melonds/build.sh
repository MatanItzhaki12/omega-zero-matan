#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"
# melonDS's libretro core with its ARM64 JIT (ARCH=arm64: the Makefile turns the JIT on only by
# that name, never by uname's aarch64) and the software renderer, threaded; no OpenGL. Its
# patches map the game from its file rather than reading it into memory (mapped-rom.patch),
# give the one-screen layouts the functions the game enters and the layer each pixel came from
# (one-screen.patch), and leave the 2D of a frame the frontend will not show undrawn
# (skip-hidden-frames.patch).
recipe="make platform=unix ARCH=arm64 HAVE_JIT=1 HAVE_OPENGL=0"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/melonDS/archive/$1.tar.gz"
	echo "recipe=$recipe"
	echo "device_cflags=$device_cflags"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

# Applies the patches with git kept out of any repository, as cores/pcsx_rearmed/build.sh does.
apply_patches() {
	ceiling="$(cd "$1/.." && pwd)"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		GIT_CEILING_DIRECTORIES="$ceiling" git -C "$1" apply -p1 "$p"
	done
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/melonDS-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/Makefile" ] || [ ! -f "$src/src/libretro/libretro.cpp" ]; then
		echo "$tarball does not hold melonDS-$commit's libretro core" >&2
		exit 1
	fi

	apply_patches "$src"

	# The Makefile asks git for the version, which here would name omega-zero's commit.
	(cd "$src" && CFLAGS="$device_cflags" CXXFLAGS="$device_cflags" $recipe \
		GIT_VERSION="\" $(printf %s "$commit" | cut -c1-7)\"" \
		-j"$(getconf _NPROCESSORS_ONLN)")

	mkdir -p "$(dirname "$out")"
	cp "$src/melonds_libretro.so" "$out"
	stamp "$commit" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
