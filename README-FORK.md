# omega-zero-matan: what this fork changes

omega-zero-matan, this fork of [Omega Zero](https://github.com/oauramos/omega-zero) 1.2.0, brings back the parts of
[Slot](https://github.com/BrandonKowalski/slot) that omega-zero left behind (its buttons, favorites,
recents, in-game menu and settings) and adds RetroAchievements. Everything is drawn in omega-zero's
own kit, so the new screens look like the rest of it. It is in daily use on an **Anbernic RG SP**.

The short version is in the [README](README.md#about-this-fork). This page covers each change in
full: what it does, its defaults, and the files it keeps on the card.

- [Controls](#controls)
- [Favorites and Recents](#favorites-and-recents)
- [Screen filter](#screen-filter)
- [Settings](#settings)
- [The in-game menu](#the-in-game-menu)
- [The POWER button](#the-power-button)
- [Labels and cart shells](#labels-and-cart-shells)
- [Run-Ahead](#run-ahead)
- [Link play: nearby, on the LAN and online](#link-play-nearby-on-the-lan-and-online)
- [Save Backup](#save-backup)
- [RetroAchievements](#retroachievements)
- [Files on the card](#files-on-the-card)
- [Building and installing](#building-and-installing)
- [Credits and licences](#credits-and-licences)

## Controls

The buttons are Slot's. The CONTROLS panel (a tap of SELECT on the shelf) lists them for the shelf
you are on.

### On the shelf

| Button | Does |
|---|---|
| A | Play the cart, picking up from its last state (see Load State on Insert) |
| Hold A | Play the cart from the start |
| ◀ ▶ | Browse |
| ▲ ▼ | Jump to the previous or next letter |
| L1 / R1 | Previous or next shelf |
| **Y** | Add the cart to Favorites, or take it off |
| **START** | Pick the core, on GBA and SNES carts (X still works too) |
| **MENU** | Settings |
| SELECT | CONTROLS |

### In a game

| Button | Does |
|---|---|
| **Tap MENU** | The in-game menu |
| Tap MENU twice | Save states: ◀ ▶ to pick one, A loads it, X deletes it (asked first) |
| Hold MENU | Save a state and eject the cart |
| SELECT + MENU | Link play, or DISC on a PlayStation game on several discs |
| **SELECT + A / B** | Next or previous screen filter |
| SELECT + Y | Colour correction on or off |
| SELECT + L1 / R1 | Load the newest state / save a state |
| SELECT + ▲ ▼ / ◀ ▶ | Brightness / blue light |
| L2 | Rewind |
| R2 | Fast forward while held |
| L2 + R2 | Lock fast forward on, or off again (R2 alone lets it go too) |
| Hold X / Y | Turbo A / B |
| L1 / R1 | Stretch or restore the picture, on the Game Boys |
| Tap / hold POWER | Sleep / power off |
| START while holding POWER | Restart |

## Favorites and Recents

**Favorites.** Y on a cart adds it to a FAVORITES shelf, and every favorite gets a small gold
star at its top right corner, on every shelf it stands on. FAVORITES is sorted by system, in the
shelves' order (GBA, GB, GBC, SNES, PS1), then by name within each system. Taking the last star
off empties the shelf, and omega-zero moves on to the next one.

**Recents.** With Settings › Shelf › Recents on, a RECENTS shelf stands first: the last ten games
played, newest first. The boot opens on it, unless the handheld went off on another shelf.

Both lists are kept in Slot's own files and format (`Config/favorites.txt` and
`Config/recents.txt`, one `gba/<name>` per line), so a card moved over from Slot keeps them.

Both shelves draw each cart on the same face as its own system's shelf. A cart is drawn once, and a
new label shows on every shelf at the same time.

## Screen filter

A Game Boy, Game Boy Color or GBA game shown at 3x goes through one of three filters:

| Filter | Looks like |
|---|---|
| LCD | omega-zero's original LCD3x mask: scanlines and the panel's RGB stripes (default) |
| Grid | A thin dark line between the game's pixels, like Slot's Grid |
| Off | The game's pixels as they are |

Change it in Settings › CONFIGS › Screen Filter, in the in-game menu's Screen page, or with
SELECT + A / B in a game. A toast names the filter you switched to.

## Settings

MENU on the shelf opens Settings. Because the Settings window has room for only a few rows,
Slot's extra settings live in two panels that CONFIGS opens, like THEME.

**CONFIGS:** Colour Correction, Screen Filter, Gameplay ›, Shelf ›, Labels ›, RetroAchievements ›,
Save Backup › (see [Save Backup](#save-backup)). Fast Forward's speed moved into GAMEPLAY to make
room for RetroAchievements. On a 480-line screen the Settings window reaches up over the top bar
to hold all seven.

**GENERAL:** Date & Time, Network, Bluetooth, Theme, Update, About (see
[Updating on the console](#updating-on-the-console)).

**GAMEPLAY ›** (each row shows a line under the panel saying what it does)

| Row | Choices | Default |
|---|---|---|
| Fast Forward | OFF, 2X, 3X, 4X, 6X: how fast fast-forward plays | 6X |
| Fast Forward Sound | On, Off | Off |
| Rumble | On, Off | On |
| Load State on Insert | On, Off: off starts every game from its beginning | On |
| Save State on Eject | On, Off: the battery save is written either way | On |
| Rewind | On, Off | On |
| Turbo Buttons | On, Off | On |
| Run-Ahead | On, Off: a press answered a frame sooner, on gpSP (see [Run-Ahead](#run-ahead)) | Off |
| Frame Blending | Off, Mix, Ghosting (mGBA only; gpSP takes it as Mix) | Off |
| Game Boy Palette | Auto, then mGBA's 48 palettes (original Game Boy games on mGBA) | Auto |
| Online Play › | The Name online rooms show and the Relay a room is opened through | |
| Emulators › | Opens the EMULATORS panel below | |

**GAMEPLAY › Emulators ›** (B goes back to GAMEPLAY)

| Row | Choices | Default |
|---|---|---|
| GBA Default | mGBA, gpSP: the core every GBA game runs on unless switched by hand | mGBA |
| GBA Switched Games | How many GBA games were switched to another core in the core picker; A puts them all back on the default | |
| SNES Default | Snes9x, Supafaust | Snes9x |
| SNES Switched Games | The same, for the SNES | |

In the core picker (START on the shelf), Y puts the game back on its system's default, and
picking the default keeps no core of its own for the game, so it follows the default from then
on. A toast says so when the picker opens on a game switched by hand.

**RetroAchievements ›** (in CONFIGS, under Labels; B goes back to Settings)

| Row | Choices | Default |
|---|---|---|
| Achievements | On, Off | Off |
| Account | Sign in on the keyboard (see [RetroAchievements](#retroachievements)) | |
| Hardcore | On, Off | Off |

**SHELF ›**

| Row | Choices | Default |
|---|---|---|
| Recents | Show, Hide | Hide |
| Game Titles | Show, Hide: the picked cart's name under the top bar | Hide |
| Clock | 12-hour, 24-hour: the top bar and Date & Time | 24-hour |
| Lid Power Off | 1, 3, 5, 10, 15, 30 minutes | 3 |
| Boot Animation | On, Off: the terminal that types out before the shelf at boot | Off |

**LABELS ›** fetches labels from ScreenScraper, and has **Holo Sheen**: the rainbow on a clear or
glitter cart's label, on or off (on).

**NETWORK** shows the handheld's address under the networks once it has one.

Turbo and Rewind change at once, even in a game. Frame Blending and the Palette reach a running
game straight away.

## The in-game menu

A tap of MENU in a game opens a menu laid out like Slot's. The game pauses behind it, with the
top bar's clock and battery shown over it. The menu comes up 180 ms after the tap, so a double tap
goes straight to the save states without the menu flashing up first.

| Page | Rows |
|---|---|
| MENU | Resume · Undo Save · Undo Load · States · Settings › · Restart · Exit Game |
| SETTINGS | Screen › · Gameplay › · Achievements › · Controls › |
| SCREEN | Screen Filter · Palette · Colour Correction · Frame Blending · Brightness · Blue Light |
| GAMEPLAY | Fast Forward · Fast Forward Sound · Rewind · Turbo Buttons · Run-Ahead (gpSP) · Rumble · Load State on Insert · Save State on Eject |
| ACHIEVEMENTS | Game · Unlocked · View All › · RetroAchievements · Hardcore |
| ALL ACHIEVEMENTS | Every achievement of the game, unlocked or locked |
| CONTROLS | Every button, for the system being played |

- **Undo Save** takes back the state just saved, and puts back the oldest state that the save
  pushed out of a full ring. **Undo Load** returns to where the game was before the load. Each
  is offered for 30 seconds.
- **Restart** starts the game over from power on, like the console's reset button. The battery
  save is untouched; an Undo Save or Undo Load on offer is dropped. It is hidden during a link.
- Rows a game has no use for are hidden, as on Slot: States with no states saved, Exit Game on a
  card with one cart, the Palette away from original Game Boy games, Achievements on the
  PlayStation.
- The SCREEN page does not dim the game, and its panel sits at the bottom, so each change shows
  on the game as you make it.
- B steps back one page, MENU closes the menu from any page, and holding MENU still ejects.

## The POWER button

Holding POWER powers off straight away, as on Slot. The game's state is saved first and the
"Turning off…" screen shows. The POWER menu, with its Restart, is gone. A tap of POWER still puts
the handheld to sleep.

Pressing START while POWER is held, before the hold powers off, restarts the handheld instead,
the game saved first as a power-off saves it.

When the handheld comes on again, the shelf is on the game it was on when it went off, on the
same shelf (RECENTS, FAVORITES or the system's). The game is not opened: only a game that was
playing when the handheld went off is picked up again, as before.

## Labels and cart shells

- **`Labels/cart_shell.txt`.** omega-zero only read `Labels/cart_shell.ini`. It now also reads
  `cart_shell.txt`, the file Slot and [Cart Studio](https://studio.slot-cfw.fyi) write, in the
  same `Name = outline colour finish` format. When a game is in both files, the `.ini` wins.
- **Labels come in where you are looking.** At boot the labels of the shelf in view are drawn
  first, Favorites and Recents included. Moving to another shelf or cart moves the carts on
  screen to the front of the labels still waiting, so you never have to visit a cart's own
  system shelf to see its new label.
- **Clear shells look clear.** A clear or glitter shell shows much more of what is behind the
  plastic: the board is seen through it, its top edge above the label on a GBA cart, and where
  there is no board the cart's hollow inside shows, tinted by the plastic. A clear cart's clock
  chip, crystal, ROM and coin cell stand low enough that only their tops peek over the label.
- **The holo sheen.** The cart picked, when its shell is clear or glitter plastic, has a sheen
  over its label as on a holographic Pokémon card: a faint rainbow, and a band of light that
  slides across it on the diagonal as the cart sways. A cart in solid plastic has none, and
  Settings › Labels › Holo Sheen turns it off for all.
- **Panels slide whole.** Going from GAMEPLAY to ACHIEVEMENTS and back, the panel leaving
  keeps its own size instead of being drawn with the one arriving.
- **Mixed shelves draw cleanly.** Carts on screen are spaced by the widest of them, and the
  picked cart is drawn in front. A narrow cart beside a wide one, such as a Game Boy pak beside
  a GBA cart on Favorites, no longer covers it.

## Run-Ahead

Settings › Gameplay › **Run-Ahead**, off to begin with, and in a gpSP game's menu under GAMEPLAY.
On, each frame shown is the one after the frame the game is at, played with the same buttons, so
a press is answered a frame sooner: the game's own frame of lag is hidden. The game stays at its
own frame, which its sound, achievements, rewind and states all come from, and only a change of
buttons plays the frame before again, so the sound runs unbroken.

- It is gpSP's alone. mGBA answers as soon on its own and has no time on the H700 for a frame
  played twice.
- It runs at normal speed only: never in fast forward, rewinding, over a link, or on the
  PlayStation.
- Every frame played again is timed. When half of the latest (at least eight) run over 11 ms,
  the time the screen waits for a frame, Run-Ahead steps aside for that game, and the log says so.

## Link play: nearby, on the LAN and online

SELECT + MENU in a GBA or Game Boy game opens the LINK screen. Its rows are a settings list: ▲ ▼
move between them, ◀ ▶ change the one lit, and A links (or, on NAME, PASSWORD and ADDRESS, types
it on the keyboard).

| Row | Shows |
|---|---|
| PLAY | Local or Online: for a GBA game on gpSP over the cable |
| WITH | Local: omega-zero (another handheld) or RetroArch (on this Wi-Fi) |
| ROLE | Host or Join |
| MODE | Cable or Wireless, for a game that has both, linking with omega-zero |
| NAME | Your name in rooms: your RetroAchievements name, or RG SP, until one is typed |
| PASSWORD | Hosting: the room's password, or none |
| SERVER | Hosting online: the relay, New York, Madrid, São Paulo or Singapore |
| ADVANCED › ADDRESS | Joining online: an address to join at, past the lobby |

- **With omega-zero** the handhelds link straight to each other, over the cable or the wireless
  adapter, with mGBA or gpSP.
- **With RetroArch** (Local) a GBA game on gpSP links over the cable with RetroArch on the same
  Wi-Fi, through RetroArch's netplay, straight, with no relay or lobby. HOST opens a room that
  RetroArch finds in its own room list; JOIN lists the rooms on the Wi-Fi as RetroArch looks for
  them. RetroArch answers only while it hosts with **Use Relay Server** off.
- **Online** a GBA game on gpSP links with anyone through RetroArch's lobby: RetroArch on a PC,
  a GBA on a GB-Link adapter through its netplay bridge, or another handheld on omega-zero. HOST
  opens a room through the relay picked and puts it in RetroArch's lobby; JOIN lists the gpSP
  rooms, those playing your game first. A room with a password says so when lit, and A asks for
  it; a wrong one asks again. The panel notes that the link goes through RetroArch's netplay.
- Online and on the LAN the link is RetroArch's netplay as RetroArch 1.22 speaks it, in the mode
  it carries gpSP's link packets in. Only the cable goes online: the wireless adapter's games
  stay nearby, a relay's delay being more than they take.
- **RetroArch's side**: gpSP's link mode must match. For Pokémon Ruby, Sapphire, Emerald,
  FireRed and LeafGreen set gpSP's core option *Serial mode* to the Pokémon cable
  (`gpsp_serial = "mul_poke"` in `config/gpSP/gpSP.opt`): left on auto, RetroArch picks the
  wireless adapter for Emerald, FireRed and LeafGreen. During netplay RetroArch reads saves from
  `saves/gpSP/.netplay/`.
- Settings › Gameplay › Online Play holds the same Name and Relay.

## Save Backup

Settings › **Save Backup** copies every save on the card to a folder on a NAS, over Samba
(SMB 2 and 3, as Synology, QNAP, TrueNAS, Unraid and Windows shares all speak it), as each save
is made.

| Row | Does |
|---|---|
| Save Backup | On, Off (Off) |
| NAS › | FIND NAS: the NASes answering on this Wi-Fi, by name, and Type Its Address for one that is not listed (a name or an address, and `:port` if not 445) |
| User | The NAS's user, typed on the keyboard; none signs in as a guest |
| Password | The user's password, typed on the keyboard |
| Folder › | FOLDER: the NAS's shares, then the folders in the one opened. A opens one, B goes back up, **New Folder** makes one there, **Use This Folder** picks the one open |
| Back Up Now | Where it stands: UP TO DATE, COPYING, *n* WAITING. A copies now. Its note says when the last copy was, or why the NAS could not be used |
| Sync From Backup › | FROM BACKUP: the saves on the NAS unlike the handheld's, or not on it at all, by game. A on one asks, and A again copies it onto the handheld |

How it works:

- **What is copied** is the card's whole `Saves` folder, with its folders: omega-zero's `.sav`
  files and whatever a core keeps there, a PlayStation's memory cards included. A system or an
  emulator added later is backed up with no change. On the NAS it goes under the folder picked,
  as on the card: `<folder>/GBA/Pokemon - Emerald Version (USA, Europe).sav`.
- **When**: after every save the game makes (the autosave, a save state, ejecting, powering off),
  when Wi-Fi comes back, and every few minutes besides, for a save a core wrote by itself.
- **Only what changed** is copied: `Config/save_backup.list` keeps the size and CRC of each
  file as the NAS last got it. A save made over the old one replaces it on the NAS. It is
  written beside it first and renamed over it, so a copy cut short never leaves half a save.
- **No NAS, no network, another network**: the saves wait. A failed try is tried again after 5
  seconds, then 10, 20 and 40, then every minute, and at once when Wi-Fi comes back or Save
  Backup is opened. The next time the NAS answers, each one waiting is copied as it stands then,
  so a game saved twice while away is copied once, the newest.
- **Toasts** at the top of the screen say when saves were backed up, and when a backup failed.
  A failure is told once, and not again until saves copy again, so a NAS that stays away is not
  told of at every try.
- **Sync From Backup** compares the NAS's folder with the handheld's Saves, reading each save
  back, since a save is the same size whatever it holds. Copying one onto the handheld keeps the
  handheld's own first in `Config/save_backup_undo/<date>/`, and sets the game's resume state
  aside, as a state holds the save the game had and would bring it back: the game starts from
  the save copied.
- **The first time**, or after picking another NAS or folder, everything in `Saves` is copied.
- The NAS's user and password are kept in `Config/omega-zero.state` on the card, as written.

## RetroAchievements

Game Boy, Game Boy Color, GBA and SNES games are checked for
[RetroAchievements](https://retroachievements.org) as they play. This uses
[rcheevos](https://github.com/RetroAchievements/rcheevos) 12.5, the site's own client library, so
achievements are evaluated exactly as in RetroArch.

### Signing in

1. Settings › RetroAchievements › **Account**, then press A.
2. Type your user name, press START, then type your password and press START.
3. RetroAchievements turns on and signs in. The note under the panel shows *Signed in as …*, or
   the reason it could not sign in.

On the first successful sign-in omega-zero swaps the password for the login token the site
returns, as RetroArch does, so the password is not kept on the card. Y on Account signs out and
deletes the saved account.

You can also write `Config/retroachievements.ini` by hand:

```ini
username = YourName
password = YourPassword
```

Requests go out through `curl`, like the label downloads do.

### No network yet, or none at all

- **Sign-in retries.** If the handheld boots before Wi-Fi is up, the sign-in isn't given up: it
  says *Waiting for network* once and tries again after 5, 10, 20 and 40 seconds, then every
  minute. A wrong password or an expired token is not retried. Once signed in, a game that's
  already running has its achievements loaded then, without restarting it.
- **Offline play.** What the site answers is kept in `Config/retroachievements/cache/`: your
  sign-in, each game's identity and achievements, and which of them you had. With no network, a
  game you've played online before signs in and loads its achievements from there. A game never
  played online needs the network once.
- **Offline unlocks.** An achievement earned with no network still pops up, and is written to
  `Config/retroachievements/pending.txt`. The next time the site answers anything, every kept
  unlock is sent, saying how long ago it was earned, and a toast says how many were sent. They
  survive powering off. An offline unlock also counts as unlocked in the kept data, so it isn't
  awarded twice.
- A sign-in from the cache only accepts the token the site handed back for it, never a typed
  password, so a wrong password can't sign in offline.
- Leaderboard scores set offline are not kept.

### Playing

- When a game starts, a toast shows your progress, e.g. *3 of 52 unlocked*.
- Each achievement you earn pops up with its title. Mastering a game, leaderboard scores, and
  unlocks held back while offline (and sent later) get toasts too.
- **Hardcore** loads no states, rewinds nothing, and starts every game from its beginning rather
  than its last state, as the site asks. Fast forward still works. Switched on in a game, the
  game restarts, as RetroArch does, so nothing done before counts.
- Turning RetroAchievements on in a game signs in, and achievements start with the next game.
  Turning it off signs out at once.

### In a game

MENU › Settings › **Achievements** shows the game as RetroAchievements has it:

| Row | Shows |
|---|---|
| Game | FOUND, with the title and game number it was matched to in the note, so you can check the ROM was taken for the right game. NOT FOUND, with the reason, when the site does not know the ROM's hash. LOOKING while it asks, SIGNED OUT, OFF, or NOT STARTED when achievements were switched on after the game began |
| Unlocked | How many of the game's achievements you have, e.g. 12/58, with the points in the note |
| View All › | Every achievement, six at a time: unlocked ones bright, locked ones dim, measured ones with their progress (37/60). The note under the list gives the lit one's description and points |
| RetroAchievements | On, Off |
| Hardcore | On, Off |

Unlocked and View All show once the game is found.
- Without hardcore, loading a state or rewinding restarts achievement tracking, so the jump in
  the game's memory cannot earn anything.

### How it works

- `crates/omega-cheevos` builds rcheevos and a small C shim (`c/omega_cheevos.c`) and wraps them
  for Rust. Every call into the client is made under one lock. The emulator thread runs the
  per-frame check, and the main thread signs in and reads where things stand.
- Nothing slow is done where the screen or the game waits for it. A game is hashed on a thread
  of its own (`omega-cheevos-hash`), once for each game, and only its hash is handed to the
  client. The site's answers are taken in on another (`omega-cheevos-answers`): a game's
  achievements take the client up to a second to take in. The screen and the emulator only
  look in on the client when it is free, and skip a frame when it is not. Hardcore's check
  waits, so it is never missed.
- omega-retro now keeps the memory map a core declares (`RETRO_ENVIRONMENT_SET_MEMORY_MAPS`).
  rcheevos' `rc_libretro` reads the game's memory through it, which the GBA needs for both of its
  work RAMs.
- Games are identified by hashing the ROM file, and a PSP game by the files on its disc. The
  zip and encrypted-ROM hashers are left out of the build.

### Not yet

- PlayStation games: their discs are not hashed.
- Achievement badges: the list is text only.

## Threads, and what the log says

The screen draws on the main thread, and nothing that can take longer than a frame is done
there:

| Work | Where |
|---|---|
| Running the game | The emulator's thread (`omega-emu`) |
| Opening a core, reading its save and resume | `omega-core-open`, at low priority, so the cart going in is drawn meanwhile |
| Closing a core | The emulator's thread, as it stops. The next core waits for it, so two of one core are never open at once, and so do powering off and restarting |
| Making a PSP game's spinning disc | `omega-disc` |
| The autosave: the core's state and writing it | `omega-autosave` |
| Writing a cart's state as it comes out | `omega-eject-save`. The core's state is taken first, before it closes. The next core waits for the write before it reads a resume |
| omega-zero's state, Recents, Favorites, the note of a game starting | `omega-card`, in order, the newest of each file only; powering off and restarting wait for it. Tests write at once |
| Wi-Fi, Bluetooth, headphones, SSH, Save Backup, updates | Their own worker threads |
| RetroAchievements: hashing, the network, taking in answers | `omega-cheevos-hash`, `omega-cheevos-net`, `omega-cheevos-answers` |

`omega-zero.log` on the card says where a slow frame's time went, so a hitch can be found
without a debugger:

- `slow frame going in: N ms: update …, emulator …, draw …, swap …` for a frame of a cart going
  in that took over 30 ms;
- `slow advance` (input, presses, update), `slow update`, `slow app update` and `slow timers`,
  each step's share of a frame's work over 30 ms;
- `omega-cheevos: <thread> waited N ms for the client and held it N ms`, for a call into
  RetroAchievements' client that held it, or waited for it, 50 ms or more;
- `cheevos: N ms: …`, the steps of signing in and of a game's achievements loading, timed from
  omega-zero's start: each request, each answer, and the game as RetroAchievements knows it.

## Files on the card

| File | Holds |
|---|---|
| `Config/favorites.txt` | Favorites, one `gba/<name>` per line (Slot's format) |
| `Config/recents.txt` | The last ten games played, newest first (Slot's format) |
| `Config/retroachievements.ini` | The RetroAchievements user name, and the password until it is swapped for a token |
| `Config/retroachievements/cache/` | The site's answers, for signing in and loading games offline |
| `Config/retroachievements/pending.txt` | Unlocks earned offline, waiting to be sent |
| `Config/default_core.ini` | Each system's default core, where EMULATORS changed it: `GBA = gpsp` |
| `Config/save_backup.list` | What Save Backup's NAS has: each save's path, size and CRC, as last copied |
| `Config/save_backup_undo/` | The handheld's own saves as they were before Sync From Backup replaced them |
| `System/trace-latency.on` | When it is there, each press's latency is written to the log |
| `System/omega-zero.previous` | The omega-zero an update replaced, to go back to |
| `Labels/cart_shell.txt` | Cart shells as Slot and Cart Studio write them |

New keys in `Config/omega-zero.state`. A card from before this fork reads each one as its default.

| Key | Values |
|---|---|
| `screen_filter` | `lcd`, `grid`, `off` |
| `auto_load`, `eject_save`, `rewind`, `turbo` | `1`, `0` |
| `blending` | `off`, `mix`, `ghosting` |
| `gb_palette` | `auto`, or a palette's name as mGBA spells it |
| `show_recents`, `show_titles`, `clock_24h` | `1`, `0` |
| `lid_off_min` | `1`, `3`, `5`, `10`, `15`, `30` |
| `cheevos`, `cheevos_hardcore`, `boot_animation` | `1`, `0` |
| `holo`, `run_ahead` | `1`, `0` |
| `online_name`, `online_password`, `online_address` | Link play's name, room password and address to join at |
| `relay` | `0` New York, `1` Madrid, `2` São Paulo, `3` Singapore |
| `backup` | `1`, `0` |
| `backup_host`, `backup_name`, `backup_user`, `backup_password`, `backup_share`, `backup_dir` | Save Backup's NAS, its name, user, password, share and folder |
| `shelf`, `picked`, `picked_platform` | The shelf and the game the shelf was on when the handheld went off |

`ff_speed` also takes `0` now, for OFF.

## Building and installing

Each version is released on GitHub with the whole card tree as a zip, and `omega-zero-aarch64`
on its own with its SHA-256. On a card that already runs omega-zero, copy `omega-zero-aarch64`
over `System/omega-zero`, or update from the console. The cores and scripts are unchanged.

### Updating on the console

Settings › **Update**, with Wi-Fi on. Its value is the version running.

1. A asks GitHub for the latest release. The row says *UP TO DATE*, or *GET V…* for a newer one.
2. A again downloads that release's `omega-zero-aarch64`, checks it against the SHA-256 published
   beside it and that it is a program for the handheld, and puts it in place. The one it replaces
   is kept as `System/omega-zero.previous`.
3. The row says *RESTART*: A restarts the handheld onto the new version.

*NO NETWORK*, *BAD DOWNLOAD* and the like say what stopped it, and leave omega-zero as it was.
Only omega-zero itself is updated this way; a release that changes the cores says so, and needs
its zip.

### Releases

GitHub Actions run the tests on every push and pull request (`.github/workflows/ci.yml`). A push
to `main` whose version in `crates/omega-zero/Cargo.toml` has no release yet runs them again,
builds the card tree for the H700 and publishes the release `v<version>`, with the zip,
`omega-zero-aarch64` and its sum (`.github/workflows/release.yml`). Bumping the version is all a
release takes. Without the `OMEGA_SS_DEVID` and `OMEGA_SS_DEVPASSWORD` secrets the build still
goes out, but its Cart Labels cannot reach ScreenScraper.

Releases are numbered for what they hold. A big feature release moves the middle number (2.1.1
to 2.2.0), and a small update or a bug fix the last (2.1.0 to 2.1.1). The fork's releases start
at 2.0.0; 1.2.0 and before are upstream's. To release:

1. Write what changed under `## Unreleased` in `CHANGELOG.md`.
2. `scripts/version.sh small` or `big` (or `task version:small`, `task version:big`): it bumps
   every crate's version and `Cargo.lock`, names the Unreleased section, and starts a fresh one
   over it.
3. Commit and push to `main`.

The upstream route works as before and builds in an arm64 container:

```sh
task build:device
```

Or cross-compile without Docker, with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild),
which also cross-compiles rcheevos' C:

```sh
cargo zigbuild --profile device --target aarch64-unknown-linux-gnu.2.28 \
  -p omega-zero --no-default-features --features device
```

The binary is at `target/aarch64-unknown-linux-gnu/device/omega-zero`. Copy it to
`System/omega-zero` on the card, and keep the old one beside it (for example as
`omega-zero.1.2.0-original`) so you can go back.

### Tests

The new behaviour is covered by `crates/omega-zero/tests/suite/slot_controls.rs`, the unit tests in
`favorites.rs`, `achievements.rs` and `omega-cheevos`, and the updated settings, toast and
state tests. Tests that only showed the POWER menu are kept but ignored. So is the comparison of
Settings against `flow.html`'s screenshots, which show the CONFIGS rows from before this fork.

CI runs every test on Linux, the render tests drawn by Mesa's surfaceless EGL on its software
renderer, on both the 720x480 and 640x480 canvases. Link play's tests open real rooms on the
machine's own network: netplay as RetroArch speaks it, LAN discovery, passwords, and rooms
through a relay stood in for by a test.

Save Backup's tests in `tests/suite/save_backup.rs` also run against a real Samba server when one
is named, from the settings panel down to the file on the server:

```sh
OMEGA_SMB_TEST="127.0.0.1:445|user|password|Share" OMEGA_SMB_DIR=/path/the/share/serves \
  cargo test -p omega-zero --test suite save_backup::
```

Without them, those tests pass at once.

## Credits and licences

Everything credited in the [README](README.md#credits-and-licence) still applies. In addition:

- The controls, settings and in-game menu follow [Slot](https://github.com/BrandonKowalski/slot) by
  Brandon T. Kowalski (GPL-3.0). The Game Boy palette list and the frame blending options are
  ported from it.
- [rcheevos](https://github.com/RetroAchievements/rcheevos) by RetroAchievements.org (MIT),
  vendored in `crates/omega-cheevos/rcheevos`.
- [smb2](https://github.com/vdavid/smb2) (MIT or Apache-2.0), Save Backup's
  Samba client.
- Link play speaks [RetroArch](https://github.com/libretro/RetroArch)'s netplay protocol, lobby and
  LAN discovery as RetroArch does.
- `libretro.h` from [libretro-common](https://github.com/libretro/libretro-common) (MIT), in
  `crates/omega-cheevos/c/include`.

The fork is licensed, like omega-zero, under the GPL-3.0-or-later. It was written with Claude.
