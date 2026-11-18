#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"
# Hidden by default: the core exports only what link.T names (retro_*, omega_ds_*), and code built
# for a shared library otherwise reaches every global function and variable through the PLT and
# GOT and inlines none of them, which cost the emulation a tenth of its speed on the H700.
visibility_cflags="-fvisibility=hidden -fvisibility-inlines-hidden"
# Built against the profile in profile/ (profile-guided optimisation, another tenth): GCC's counts
# of the branches taken while the Pokémon games ran on the RG SP, made with `profile` below as
# profile/README.md tells. Counts that no longer fit the code (a patch changed it since) leave
# just that function unoptimised, with a warning, rather than failing the build.
profile_use_cflags="-fprofile-use -fprofile-correction -Wno-missing-profile -Wno-error=coverage-mismatch"
profile_generate_cflags="-fprofile-generate -fprofile-update=atomic"
# melonDS's libretro core with its ARM64 JIT (ARCH=arm64: the Makefile turns the JIT on only by
# that name, never by uname's aarch64) and the software renderer, threaded; no OpenGL. Its
# patches map the game from its file rather than reading it into memory (mapped-rom.patch),
# give the one-screen layouts the functions the game enters and the layer each pixel came from
# (one-screen.patch), leave the 2D of a frame the frontend will not show undrawn
# (skip-hidden-frames.patch), draw each 2D engine on a thread of its own (threaded-2d.patch),
# hand the frontend both screens where the GPU drew them rather than a copy (zz-direct-frame.patch,
# after threaded-2d.patch, whose lines it borders), keep a signal from stalling local wireless's
# socket read for good (local-wireless.patch), and resume a state saved in the middle of a DMA's
# burst (dma-savestate.patch).
recipe="make platform=unix ARCH=arm64 HAVE_JIT=1 HAVE_OPENGL=0"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT | profile COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

# The profile's files, one per line in a stable order: their paths in the source tree, which
# profile/ keeps (src/NDS.gcda for src/NDS.o: GCC looks for an object's counts beside it, so the
# profile fits wherever the tree is unpacked).
profile_files() {
	[ -d "$here/profile/src" ] || return 0
	(cd "$here/profile" && find src -name '*.gcda' | LC_ALL=C sort)
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/melonDS/archive/$1.tar.gz"
	echo "recipe=$recipe"
	echo "device_cflags=$device_cflags $visibility_cflags"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
	profile_files | while read -r f; do
		echo "profile=$f sha256:$(sha256 "$here/profile/$f")"
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

# unpack COMMIT TARBALL WORKDIR: the patched source in WORKDIR/melonDS-COMMIT
unpack() {
	rm -rf "$3"
	mkdir -p "$3"
	tar -xzf "$2" -C "$3"
	if [ ! -f "$3/melonDS-$1/Makefile" ] || [ ! -f "$3/melonDS-$1/src/libretro/libretro.cpp" ]; then
		echo "$2 does not hold melonDS-$1's libretro core" >&2
		exit 1
	fi
	apply_patches "$3/melonDS-$1"
}

# compile SRC COMMIT OUT FLAGS: the core built in SRC with FLAGS beside the device's, to OUT
compile() {
	flags="$device_cflags $visibility_cflags $4"
	# The Makefile asks git for the version, which here would name omega-zero's commit.
	(cd "$1" && CFLAGS="$flags" CXXFLAGS="$flags" LDFLAGS="$flags" $recipe \
		GIT_VERSION="\" $(printf %s "$2" | cut -c1-7)\"" \
		-j"$(getconf _NPROCESSORS_ONLN)")
	mkdir -p "$(dirname "$3")"
	cp "$1/melonds_libretro.so" "$3"
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/melonDS-$commit"
	unpack "$commit" "$tarball" "$work"

	extra=""
	if [ -n "$(profile_files)" ]; then
		profile_files | while read -r f; do cp "$here/profile/$f" "$src/$f"; done
		extra="$profile_use_cflags"
	fi
	compile "$src" "$commit" "$out" "$extra"
	stamp "$commit" >"$out.meta"
}

# The core that counts its branches as it runs, for profile/README.md: no .meta beside it, so
# that core:melonds never takes it for the shipping core.
profile() {
	commit="$1" tarball="$2" work="$3" out="$4"
	unpack "$commit" "$tarball" "$work"
	compile "$work/melonDS-$commit" "$commit" "$out" "$profile_generate_cflags"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build | profile)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	"$1" "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
