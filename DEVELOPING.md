# Developing omega-zero-matan

This page is for working on the code. It maps the workspace, says where each kind of change
goes, and walks through the changes made most often. For what the software does, see the
[user guide](README-FORK.md); for its history, the [changelog](CHANGELOG.md).

## The workspace

| Crate | What it holds |
|---|---|
| `omega-zero` | The app, its screens, the session that ties them to a core, and `main` |
| `omega-ui` | Everything drawn: the shelf, the carts, the bars, panels and rows, toasts, and the UI kit (`kit/`) |
| `omega-gfx` | The OpenGL ES 2 renderer, on the H700's GPU, in a window, or headless for tests |
| `omega-retro` | The libretro host: loads a core, runs it a frame at a time, takes its picture, sound and states |
| `omega-store` | What is kept on the card: games found, settings, states, per-game choices, whole-file writes |
| `omega-input` | Raw buttons turned into taps, holds and SELECT chords (`Action`) |
| `omega-power` | Battery, backlight, lid, LED, rumble, clock, power off; a simulation off the handheld |
| `omega-cheevos` | RetroAchievements, through rcheevos (C, in `rcheevos/`) |
| `omega-pack` | Builds the System update a release publishes, and the handheld's reader of it |
| `omega-sfxcut` | Cut the insert and eject sounds from a recording; only needed to cut them again |

The cores are built from libretro's sources by the scripts in `cores/`, at the commits stamped
there. gpSP is built as libretro ships it; what it needs is done from omega-zero's side, in
`omega-retro/src/gpsp.rs`.

### Inside `omega-zero`

- **`app.rs`** and **`app/`**: `App`, the state of the screen. `App::apply` takes each press and
  `App::update` steps time. Topics with many functions have their own file under `app/`
  (`link.rs` for link play, `saves.rs` for saves and states, `sound.rs` for where the sound
  goes and how loud). Other `impl App` blocks live beside the screen they serve: `options.rs`,
  `pause_menu.rs`, `favorites.rs`, `screens/*.rs`.
- **`session.rs`**: `Session` owns the `App`, the core (`emu.rs`, on its own thread), and the
  sound. Each frame it hands presses to the app, opens or closes cores as the app's phase
  says, and plays what the app asks to be heard.
- **`frontend.rs`** and **`frontend/`**: draws a frame from the app. Each `sync_*.rs` keeps one
  part's faces up to date, baking a face only when what it shows has changed.
- **`audio/`**: the ring the speaker drains (`ring.rs`), the device and host sinks, the cart
  sounds (`sfx.rs`), the opening's chimes, the menus' sounds (`menu_sound.rs`) and the
  shelf's tune (`music.rs`).
- **`device_app.rs`** / **`host_app.rs`**: the main loop on the handheld and in a window.

### Threads

The screen's thread must never wait on the card, the network or a core. Anything slow goes to
a thread of its own and reports back through a channel the app polls in `update`: opening a
core, saving a state, writing settings (`card_writer.rs`), labels, achievements, Save Backup,
Wi-Fi and Bluetooth. The user guide's [Threads](README-FORK.md#threads-and-what-the-log-says)
section lists them and the log lines that time them.

## Recipes

### Add a setting

1. **`omega-store/src/app_state.rs`**: add the field to `AppState` with a doc comment, its
   default in `Default`, and one line at the end of `FIELDS`. `flag!` for on/off, `text!` for
   free text, `may!` for anything else. A file from before the setting keeps its default.
2. **`omega-zero/src/options.rs`**: add an `OptionRow`, put it in its page's `rows()`, and give it
   a `label`, a `note` (the line under the panel), a `value`, and what `step` does.
3. Read it where it applies: `self.state.<field>` in the app.
4. Test it: `the_state_file_keeps_its_keys_and_their_order` in `omega-store` pins the file's
   text, so add the new line to its two expected texts.

### Add a toast

Add the variant to `omega-ui/src/toast.rs`: `ALL`, `index`, `label` (the tab's word) and
`text`, and to the list in `omega-ui/tests/suite/toast.rs`. Show it with
`self.hud.toast(Toast::…, self.now())`.

### Add a sound

Menu sounds need nothing per screen. `menu_sounds.rs` compares the screen before and after
each press: a menu opened chimes up, one closed chimes down, a moved cursor ticks. A new screen
sounds right as long as its state shows in its phase's `Debug`.

A new kind of sound is a variant of `audio::Sfx`, rendered in `menu_sound.rs` (or `chime.rs`
for the opening's), set with `self.sfx = Some(…)` in the app. `Session::play_sfx` mixes it in
from the next sample heard; `Ring::mix` lays sounds over each other and over whatever is
playing.

### Add a screen

Add a `Phase` variant in `app.rs` holding the screen's own state, an `*_input` function for its
presses (called from `App::press`), a `screens/*.rs` drawing it, and a `frontend/sync_*.rs`
keeping its faces. `Phase::Options` panels are the quickest pattern to copy, and
`screens/lone.rs` draws a lone panel with rows and notes.

### Add a button chord

SELECT chords are in `omega-input/src/gesture.rs` (`Gestures::chord`): a bit of its own and an
`Action`. Act on the action in `App::press`, for the phases it belongs to.

## Logging

Write to the log with `crate::log!("topic: what happened")` (`omega_zero::log!` in `main.rs`,
`device_app.rs` and `host_app.rs`). Each line is stamped with the seconds since omega-zero
started:

```
[   12.345] omega-zero: audio: no device
```

On the card the log is `omega-zero.log`, and the one before is `omega-zero.log.1`.

## Testing

```sh
task test           # every test; the render tests draw in software (Mesa's llvmpipe)
task test:canvases  # the render tests on both the 720x480 and 640x480 canvases
task lint           # clippy, warnings denied
task lint:device    # clippy for the handheld's build
task fmt
```

- Unit tests sit beside the code. The app's behaviour is tested through `App` in
  `omega-zero/tests/suite/`, one file per area. Each test boots an app on a temporary card
  (`common::tmp_root_with_carts`) and presses buttons.
- Render tests compare frames against `assets/ui/flow` and the kit's storybook.
- A core's own behaviour is tested against the real core where one is vendored. Tests that
  need the network or a real Samba server are ignored, or pass at once without one.

On the handheld, `task deploy:device` builds and copies the binary over SSH. `task log:device`
shows its log.

## Off-screen runs

`scripts/sim.sh [SCRIPT] [OUT]` boots the real frontend with no window and no sound, on a card
it makes for the run, plays a script of presses, and keeps the frames the script asks for as
PNGs, with the log beside them. The script is `examples/record.rs`'s: `wait N`, `tap BUTTON
[FRAMES]`, `shot NAME`. The first press of a boot only ends its fade-in. `scripts/sim/` holds
the scripts; `umd-insert.txt` puts a UMD in and keeps its spin.

The `sim` workflow runs the same on GitHub's Arm runner, pinned to four cores with each held to
a share of its time to come near the H700's four Cortex-A53s at 1.5 GHz. Its frames and log are
the run's artifact. It is not the handheld: the GPU is Mesa's software renderer rather than the
Mali-G31, and a core's speed only comes near. It shows what is drawn, frame by frame, and so
catches what a frame-time log cannot: the PSP spin's stutter was a disc drawn in some frames
and not others, at a steady 60 frames a second.

## Releasing

1. Write what changed under `## Unreleased` in `CHANGELOG.md`, for the people who play on it.
2. `scripts/version.sh small` for a fix, or `big` for new features. It bumps every crate and
   names the section.
3. Commit and push to `main`. CI tests, builds the card tree and the System update, and publishes
   `v<version>`. A console on Wi-Fi finds it in Settings › Update.

A test build of any branch is one `workflow_dispatch` of `release.yml`. It builds the same
artifacts without publishing a release.

CI installs its Ubuntu packages with `scripts/ci-apt.sh`: it tries the runner's mirror, then
Canonical's and others', skipping any that do not answer, and caches what it fetched. When
GitHub's runners reach no mirror at all, run the checks somewhere else (`cargo fmt --check`, both
clippies and `cargo test --workspace`, as in Testing), then run `release.yml` by hand on `main`
with **checks_run_elsewhere** ticked: it releases without running them on GitHub.

## Style

- `cargo fmt`, and clippy with warnings denied. The workspace's own lints are in the root
  `Cargo.toml`, `.editorconfig` covers the rest.
- Comments say *why*, in plain sentences. Each module opens with a `//!` on what it is for.
- Keep the screen's thread free: no blocking I/O in `App` or a `sync_*`.
- When a change is tried on the handheld and does nothing, take it out again. Failed fixes are
  not left behind.
