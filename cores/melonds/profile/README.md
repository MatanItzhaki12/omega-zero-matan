# melonDS's optimisation profile

`build.sh build` compiles melonDS against the counts in `src/` (`-fprofile-use`): which way each
branch of the core's C++ went, and how often each function ran, while the Pokémon DS games ran on
an RG SP. GCC lays the hot paths out straight and inlines by them, which gains the emulation about
a tenth of its speed on the H700 (HeartGold's New Bark Town: 57.6 fps to 63.4, raw). The JIT's
generated code is not compiled by GCC and is not affected.

A count file belongs to the object it is beside (`src/NDS.gcda` to `src/NDS.o`) and to the code
that object was compiled from. When a patch or a melonDS update changes a function, its counts no
longer fit, and GCC builds that function without them and warns (`coverage mismatch`): the core
still builds and runs, only that much slower. Retrain after such a change.

The counts hold no game data, only numbers of times code ran.

## Retraining

1. Build the counting core, on the box the device build wants (`task core:melonds` shows the
   way), or across from an x86 Linux box with Debian bullseye's aarch64 GCC 10, the version the
   release build uses (`CC=aarch64-linux-gnu-gcc CXX=aarch64-linux-gnu-g++`):

   ```sh
   sh cores/melonds/build.sh profile <commit> vendor/melonds-src.tar.gz target-device/melonds-profile melonds_counting.so
   ```

2. On the console, run it through each game for some minutes with `GCOV_PREFIX` naming a
   directory: in a frontend (omega-zero with it as `System/melonds_libretro.so`), or a small
   libretro runner. Each run adds to the counts. What the profile was made from: HeartGold
   walking about New Bark Town from a save, Platinum continuing a save in the player's room,
   White continuing a save and walking about Nuvema Town, and White and White 2 from boot
   through their openings with no save (3,000 to 4,000 frames each), with the core options
   omega-zero sets (`crates/omega-zero/src/core.rs`).

   ```sh
   GCOV_PREFIX=/tmp/counts GCOV_PREFIX_STRIP=0 <runner> ...
   ```

3. The counts land under `GCOV_PREFIX` at the path the tree was built in; copy its `src/` here:

   ```sh
   cp -r /tmp/counts/<build path>/melonDS-<commit>/src cores/melonds/profile/
   ```

`build.sh stamp` names each count file with its checksum, so `task core:melonds` rebuilds the
core when they change.
