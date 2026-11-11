<h1 align="center"><img src="assets/logo/omega-zero-logo@2x.png" width="420" alt="omega-zero-matan"></h1>

<p align="center"><b>A cartridge-shelf frontend for Anbernic H700 handhelds, tuned for the RG SP.</b><br>
GBA, Game Boy, Game Boy Color, SNES, PlayStation and PSP, on a shelf of carts, discs and UMDs
that go down into the handheld when you play them.</p>

<p align="center">
<a href="https://github.com/MatanItzhaki12/omega-zero-matan/releases/latest"><b>Download</b></a> ·
<a href="README-FORK.md"><b>User guide</b></a> ·
<a href="CHANGELOG.md"><b>Changelog</b></a> ·
<a href="DEVELOPING.md"><b>Developing</b></a> ·
<a href="https://github.com/oauramos/omega-zero"><b>Omega Zero</b></a> ·
<a href="https://github.com/slot-cfw/slot"><b>Slot</b></a>
</p>

omega-zero-matan runs on [BaseOS](https://github.com/pvaibhav/BaseOS) and replaces its menu with
a 3D shelf of your games, drawn the way a PlayStation drew 3D. Pick a cart and it slides down into
the handheld; take it out and the game is saved where you left it. Around the shelf are the
conveniences of a modern handheld: save states, rewind, fast forward, RetroAchievements, link play
over Wi-Fi and online, Bluetooth headphones, saves backed up to a NAS, and updates from the
console itself.

## Where it comes from

This project stands on two others. Most of what you see is theirs.

- **[Omega Zero](https://github.com/oauramos/omega-zero)** by Otávio
  ([@oauramos](https://github.com/oauramos)) made the shelf, the low-poly carts with their real
  labels, the Mega Man Zero-style menus, the boot terminal and the UI kit everything is drawn
  in. This fork began from Omega Zero 1.2.0, its latest release.
- **[Slot](https://github.com/slot-cfw/slot)** by Brandon T. Kowalski
  ([@BrandonKowalski](https://github.com/BrandonKowalski)) is the frontend Omega Zero itself
  grew out of. The code that hosts the cores, with their saves, sound and input, is still
  largely his, and this fork brings back Slot's buttons, favorites, recents, in-game menu and
  settings.

On top of these, omega-zero-matan adds RetroAchievements, PSP support, link play, Save Backup,
Bluetooth headphones, menu sounds and music, and a long list of fixes for running all day on the
RG SP. Each of them is drawn in Omega Zero's own kit, so it looks like the rest. The
[user guide](README-FORK.md) covers every change in full.

<img src="site/media/readme/highlights.png" width="760" alt="">

## Highlights

**The shelf**
- A shelf for each system, with Favorites and Recents in front. Game Boy carts come in their
  own plastics, with clear shells showing the board inside, and SNES carts in the American or the
  Super Famicom shape. PlayStation discs stand as a stack for a game on several, and PSP UMDs
  show their label through the window.
- Real label scans, fetched from ScreenScraper with your account. A holo sheen shows on clear and
  glitter carts.
- Soft menu sounds and a gentle tune on the shelf, each switched off in Settings › Shelf.
- L2 goes back to the first cart, and R2 picks one at random when nothing comes to mind.

**Playing**
- Six cores: mGBA and gpSP (GBA), mGBA (Game Boy), Snes9x and Supafaust (SNES), PCSX ReARMed
  (PlayStation) and PPSSPP (PSP), each picked per system or per game.
- Resume on insert, save states with a switcher, undo save and load, rewind, fast forward,
  turbo, Run-Ahead on gpSP, and screen filters.
- Screenshots on SELECT + X, kept as PNGs in `Screenshots/`.
- The games' clocks (Pokémon's berries and tides, Boktai's sun) follow your time zone.

**Connected**
- RetroAchievements for GB, GBC, GBA, SNES and PSP, with hardcore and unlocks sent once you're back
  online.
- Link play for GBA games, between two handhelds, with RetroArch on a PC, or online through
  RetroArch's lobby.
- Save Backup: every save copied to a folder on your NAS as it is made.
- Bluetooth headphones, with AirPods and Galaxy Buds controls.
- Updates from Settings › Update, checked and installed on the console.

**On the handheld**
- One build lays itself out for each H700 panel. Holding POWER powers off, and START while
  holding POWER restarts.
- Every slow job runs on a thread of its own, off the screen's: saves, labels, achievements and
  opening a core. A frame is never held up by the card.

<img src="site/media/readme/supported-devices.png" width="760" alt="">

## Supported devices

One build runs on all of them. omega-zero reads the model from BaseOS at boot and lays itself out
for its panel.

| Device | Panel | Status |
|---|---|---|
| RG SP | 720x480 | This fork's main target, in daily use |
| RG35XX SP | 640x480 | Omega Zero's main target |
| RG34XX SP, RG34XX | 720x480 | Untested |
| RG35XX Plus / 2024, H, Pro | 640x480 | Untested |
| RG40XX H, RG40XX V | 640x480 | Untested |
| RG CubeXX | 720x720 | Experimental |
| RG28XX | 480x640, turned | Experimental |

Own an untested one? Omega Zero's [device reports](https://github.com/oauramos/omega-zero/wiki/Device-Reports)
cover the same handhelds.

<img src="site/media/readme/quick-start.png" width="760" alt="">

## Quick start

1. Write the [BaseOS image](https://github.com/pvaibhav/BaseOS/releases/latest) for your model to
   a microSD card, put it in **SD1** and boot once.
2. Unzip the [latest release](https://github.com/MatanItzhaki12/omega-zero-matan/releases/latest)
   to the root of that card.
3. Put your games in `Games/GBA`, `Games/GB`, `Games/GBC`, `Games/SNES`, `Games/PS1` and
   `Games/PSP`.
   - A PlayStation game is a `.cue` with its `.bin`, a `.chd`, a `.pbp` or an `.m3u`. A real
     BIOS such as `scph5501.bin` is used from `BIOS/` when it is there.
   - A PSP game is an `.iso`, `.cso`, `.chd` or eboot `.pbp`. Its saves are kept in
     `Saves/PSP/SAVEDATA`, as on a memory stick.
4. Boot. On the shelf, MENU opens Settings, and a tap of SELECT shows what every button does.

Later versions install themselves from Settings › Update.

## Controls at a glance

| On the shelf | | In a game | |
|---|---|---|---|
| A / hold A | Play / play from the start | Tap MENU | The in-game menu |
| ◀ ▶ / ▲ ▼ | Browse / jump a letter | Hold MENU | Save a state and eject |
| L1 / R1 | Previous / next shelf | SELECT + L1 / R1 | Load / save a state |
| L2 / R2 | First cart / a random one | SELECT + X | Screenshot |
| Y | Favorite | L2 / R2 | Rewind / fast forward |
| START | Pick the core | SELECT + A / B | Screen filter |
| X on Recents | Take it off Recents | SELECT + ▲ ▼ | Brightness |
| MENU | Settings | Hold POWER | Power off |

The full list, with every setting and the files kept on the card, is in the
[user guide](README-FORK.md).

<img src="site/media/readme/screens.png" width="760" alt="">

## Screens

<p align="center">
<a href="https://oauramos.github.io/omega-zero/"><img src="site/media/loops/lid.webp" width="640" alt="An RG35XX SP drawn in 3D, its lid open on the shelf of carts, Emerald in the middle"></a>
<br><sub>The handheld in 3D, with the shelf on its screen; more loops play on Omega Zero's <a href="https://oauramos.github.io/omega-zero/">site</a></sub>
</p>

<p align="center">
<img src="site/media/settings.png" width="380" alt="Settings in Night">
<img src="site/media/amber.png" width="380" alt="The same Settings in Amber, Mega Man Zero's own orange and brown">
<br><sub>Settings, in Night and in Amber</sub>
</p>

<p align="center">
<img src="site/media/network.png" width="380" alt="Network: Wi-Fi, Network Time and SSH, and the networks around">
<img src="site/media/bluetooth.png" width="380" alt="Bluetooth: the radio on and the devices around">
<br><sub>Network and Bluetooth</sub>
</p>

## Building from source

The project is a Rust workspace. The cores are libretro's, built and patched by the scripts in
`cores/`. With [Task](https://taskfile.dev) and the pinned toolchain:

```sh
task test          # every test, the render tests drawn in software
task lint          # clippy over every target, warnings denied (lint:device for the handheld)
task run           # the shelf in a window on the computer
task dist:device   # the card tree for the H700
```

Pushing a new version to `main` builds and publishes the release. [DEVELOPING.md](DEVELOPING.md)
maps the code, and shows how to add a setting, a screen, a toast or a sound.

## Credits and licence

omega-zero-matan is made by **Matan ([@MatanItzhaki12](https://github.com/MatanItzhaki12))**. It
is a fork of [Omega Zero](https://github.com/oauramos/omega-zero) by **Otávio
([@oauramos](https://github.com/oauramos))**, which is itself a modified version of
[Slot](https://github.com/slot-cfw/slot) by **Brandon T. Kowalski**. Most of the code is still theirs.

- [BaseOS](https://github.com/pvaibhav/BaseOS) by pvaibhav.
- Emulation by [mGBA](https://mgba.io) (MPL-2.0), [gpSP](https://github.com/libretro/gpsp)
  (GPL-2.0), [Snes9x](https://github.com/libretro/snes9x) (free for non-commercial use only),
  [Supafaust](https://github.com/libretro/supafaust) (GPL-2.0),
  [PCSX ReARMed](https://github.com/libretro/pcsx_rearmed) (GPL-2.0) and
  [PPSSPP](https://www.ppsspp.org) (GPL-2.0). Their licences, and the source of all but mGBA,
  ship in `System/licenses/`.
- Achievements by [rcheevos](https://github.com/RetroAchievements/rcheevos) (MIT).
- Type is [Pixel Code](https://github.com/qwerasd205/PixelCode) by Qwerasd (SIL OFL 1.1). Every
  icon is drawn as pixel art. The LCD mask is derived from GigaHerz's LCD3x.
- Cart labels come from [ScreenScraper](https://www.screenscraper.fr), with your own account.
- The menu sounds and the shelf's tune are synthesized by omega-zero as they play. No recording
  ships.

Licensed under the GPL-3.0-or-later; see [LICENSE](LICENSE). It is written with Claude, and
reviewed and tested on the handheld by its maintainer.

Game Boy, Game Boy Color, Game Boy Advance, Super Nintendo Entertainment System and Super Famicom
are trademarks of Nintendo. PlayStation, PSP and UMD are trademarks of Sony Interactive
Entertainment. Anbernic is a trademark of its owner. This project is not affiliated with any of
them.
