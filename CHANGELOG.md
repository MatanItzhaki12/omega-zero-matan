# Changelog

## Unreleased

- A UMD's hub is a real one's: white, UMD across it in blue and UNIVERSAL MEDIA DISC under it
  in small grey letters. It is an element of its own, laid over the disc's print rather than
  part of its label art, and it stands still over the disc as the disc spins while the game
  loads. A white ring a render of a UMD leaves round its hole is taken into it.

- Restarting takes START and a held POWER, as powering off takes a held POWER: START with POWER
  held, in either order, restarts once POWER has been held as long as a power-off takes, and
  both let go of sooner do nothing.

- A PSP game's disc spins smoothly as it loads. Its game was hashed for RetroAchievements
  while the achievements client was held, and the screen, which looks in on the client every
  frame, waited out the third of a second hashing a disc takes. A game is hashed on a thread of
  its own now, a cartridge's ROM as much as a disc, and only its hash handed to the client. The
  site's answers, a game's achievements among them, which take the client up to a second to
  take in, are taken in on a thread of their own too, and the screen and the game look in on
  the client only when it is free, so no game hitches as its achievements load. The note that a core is
  starting is written to the card off the screen's thread too, and a slow frame says in the
  log where its time went.

- Putting a PSP game away no longer holds the shelf up as it comes back: its core closes on
  the emulator's thread, and the next game's core waits for it rather than the screen.

- The autosave no longer hitches a game once a minute: the core's state is taken and written on
  the autosave's own thread, rather than the screen waiting for the core to give it up.

- A cart coming out writes its state to the card on a thread of its own, once the state is
  taken, so the cart's slide out is not held up by the card. The next game to start waits for
  the write before it reads its resume, so a game put back in at once picks up where it was.

- Starting a game no longer stalls as the one put away before it is still being written to the
  card. The small files omega-zero keeps there, its state, Recents, Favorites and the note of
  a game starting, are written on a thread of the card's own, in order, the newest of each
  file only; each waits on the card behind the game's state, which the screen waited out with
  it. Powering off and restarting wait for them.

- Save Backup looks for a save once it is written, rather than as the write begins, so an
  autosave's or an eject's save is copied on that pass rather than the next.

- A slow frame says in `omega-zero.log` where its time went, step by step, and RetroAchievements
  logs each step of a game's achievements loading with its time. README-FORK.md has what each
  thread does and what the log says.

## omega-zero 2.6.7

- A PSP game's disc spins in its UMD's window as the game loads: the disc is drawn on its own,
  turned by the GPU, and a game not ready by the top of the UMD's rise holds it there, spinning,
  until it is, then it goes in. A game ready sooner goes in as before.

## omega-zero 2.6.6

- AirPods are changed by nothing the handheld does but what is asked of it in the
  HEADPHONES menu: the packet that turns on what they keep for Apple's own devices is sent
  only once Adaptive is picked, rather than each time they connect, and connecting reads them
  only, as a phone they also belong to may count on how they were.
- Headphones connected without their sound are asked to connect it twice at most, then left
  be, rather than every 15 s, so a phone taking them back is not fought for them.

## omega-zero 2.6.5

- The game's menu lists Wi-Fi's networks and Bluetooth's devices: Y on Wi-Fi or Bluetooth in
  its SETTINGS page, and A joins a network (saved or open; a new one with a password is joined
  from Settings, which has the keyboard) or pairs, connects or disconnects a device. X looks
  again.

## omega-zero 2.6.4

- START held, then POWER, restarts the handheld, as POWER held, then START, did already.
- A game's sound is 26 ms closer to it: the ring under it is kept 2.4 video frames full rather
  than 4, as menu sounds, mixed in at its front, already were.
- A game's core is opened, and its resume state read off the card, on a thread of their own,
  so the cart goes on going in smoothly meanwhile; a PSP game's took long enough to stall it.
- Headphones connecting say so in one toast with the mode they connected in, rather than the
  mode first and the connection after.
- A UMD's scanned label has UMD across its hub, as a generated one has, and a darker ring
  where the hub meets the print.

- omega-zero checks for an update by itself, once a boot, as Wi-Fi joins a network (at boot or
  later): one found is told of in a toast, and Settings > Update offers to INSTALL it.
- An update found stays on Settings > Update until it is installed, closing Settings or not.
  UP TO DATE, and what went wrong, go after 4 seconds, or as Settings closes.

## omega-zero 2.6.3

- Headphones switched to another noise control mode on themselves, by a stem or a touch, say
  the mode in a toast.
- A noise control mode picked in the menu is sent once the row has rested on it for half a
  second: stepping from noise cancelling past Adaptive to transparency goes straight to
  transparency.
- Headphones that connect are set to their volume here before the sound goes to them, so they
  never play for a moment at whatever they were left at. Toasts say when the headphones connect
  and disconnect, and the speaker is muted as they disconnect.
- The headphones' menu has the case's battery on a row of its own, beside the buds', and the
  AirPods' beacon fills it in where their channel has not said it.
- The game's menu has Wi-Fi and Bluetooth on its SETTINGS page, to switch each on or off.

## omega-zero 2.6.2

- Headphones that connect by themselves get the game's sound: AirPods taken out of their case
  reconnect without their A2DP stream, so the sound stayed on the speaker. Headphones connected
  for 3 s without it are asked to connect again, which brings it.
- Bluetooth headphones' own volume is the volume while sound goes to them: the console's
  buttons set it, and a swipe on AirPods (or the volume on any headphones) shows on the gauge
  and is kept. The sound is no longer turned down twice.
- Noise control set quickly, a step after a step, stays where it was set: the modes the
  headphones say back on their way to it no longer pull it back. A mode changed on the
  headphones themselves shows in the menu, as AirPods are asked for their notifications until
  they answer.
- The game menu's Headphones row shows only once the headphones have said something, and their
  page goes back to MENU, and Settings' back to Bluetooth, if they go while it is open.
- On headphones connected, Bluetooth's bar says Y FORGET and START MENU, which fit, rather than
  three hints that ran off its end; X still looks for devices.

## omega-zero 2.6.1

- Bluetooth headphones play the game's sound. They connected and dropped at once, as nothing
  on the handheld took audio over Bluetooth: Bluetooth now starts bluez-alsa beside bluetoothd,
  the sound moves to the headphones while they are connected and back to the speaker when they
  go, and they keep a volume of their own, as wired ones do. The sound is written to them as
  little ahead as bluez-alsa takes, for as little delay as the headphones allow.
- AirPods and Galaxy Buds have a HEADPHONES menu: START on them in Settings > Bluetooth, and
  Headphones in the game's menu while they are connected. Noise control (noise cancelling,
  Adaptive, transparency or ambient sound, and off, as each pair has them), AirPods'
  conversational awareness, which turns the game down while you talk, and how much of the room
  their Adaptive lets in, and the battery of each bud and the case.
- AirPods are talked to over Apple's own protocol, as LibrePods worked it out, and read their
  battery from the beacon AirPods send, as OpenPods does, where that says none. Galaxy Buds
  (Buds+ and every pair since) are talked to as GalaxyBudsClient does.
- A toast says when the buds run low: at 20%, 10% and 5%, each once until they charge.
- Settings > Update brings the card's whole System folder to the new release and fetches only
  the files that changed, as small patches where it can: a release publishes a manifest of
  every file and its SHA-256, and patches from the three releases before. The cores, the
  scripts and PPSSPP's files now update too, not only omega-zero. The cores' source archives
  stay in the release's zip. Updating from 2.6.0 still brings omega-zero alone; Update once
  more on 2.6.1 brings the rest.

## omega-zero 2.6.0: PSP

- PSP games, on PPSSPP 1.20.4: put them in `Games/PSP` as `.iso`, `.cso`, `.chd` or `.pbp`. A PSP
  game stands on its own shelf as a UMD, after photos of real ones: a shell whose top is a half
  circle and whose foot is square, a round clear window across its front showing the disc's
  printed side (its label, from ScreenScraper's PSP system), the PSP logo and a pad moulded on the
  foot. It comes in smoky clear plastic, which `cart_shell.txt` can change, and its label's holo
  sheen runs in rings, the way a CD throws the light.
- A core can draw on the GPU: PPSSPP draws with OpenGL ES on a context of its own, sharing its
  frames with omega-zero's, which shows them as any core's.
- A PSP game fills the screen's width by default (Fit), keeping its shape; Settings > Gameplay >
  PSP Screen sets it to Fit, Actual (480x272) or Stretch, and a game's menu (SCREEN > Picture)
  sets it for that game.
- The PSP's stick is the D-pad while L2 is held, by default; a game's menu (GAMEPLAY > D-Pad as
  Stick) makes it the D-pad until L2 is pressed again, or never, for that game.
- RetroAchievements for PSP games, known by their disc's PARAM.SFO and EBOOT.BIN; a `.cso` is
  read for its hash as an `.iso` is.
- Save Backup copies PSP saves with the rest, and Sync From Backup offers a PSP save's folder as
  one save, named for its game, and copies all of it back together.
- A label is opaque over a clear shell: what is behind the plastic no longer shows through it.
- A PSP game's label is looked up on ScreenScraper by the game's ID (ULUS-10466 and the like),
  read from the disc without reading all of it; a disc label is trimmed to its print and centred,
  and its clear middle shows the metal hub a real UMD has.
- A PSP game has no fast forward (PPSSPP keeps its own pace), and its L2 reaches the game's stick
  setting rather than being taken for rewind: held L2 makes the D-pad the stick again.
- Saving, backups and label cutting run at a lower priority than the game, so a game stutters less
  while they work.
- The fork is omega-zero-matan: its name, its updates (from MatanItzhaki12/omega-zero-matan), its
  boot screens and About page say so, crediting Omega Zero by @oauramos.

## omega-zero 2.5.3

- The cart picked on the shelf turns as it comes to the middle, to where its sway reaches on
  the side it came in from (from the right, its right side to the back; from the left, its
  left), eased over a third of a second, and its sway goes on from there rather than starting
  again from the middle. It no longer leans the other way into the scroll first.
- Save Backup looks for a network before it tries the NAS. With none, saves wait and nothing
  fails: Back Up Now says NO WI-FI, no failure toast shows, and it looks again every 5 seconds,
  copying as soon as the handheld has an address. While it waits, Wi-Fi, when it is switched on,
  is asked to join a saved network again, every half minute at most; switched off, it stays off.
- A connection to the NAS that fails is tried once more 2 seconds later, as a radio that dozed
  can drop the first.
- The link session tests wait for their host to listen rather than a fixed 150 ms, which a busy
  machine could miss.

## omega-zero 2.5.2

- A save made in a game is backed up: the backup waited for nothing as it was asked, so it looked
  at the Saves folder before the game's save was written there, called everything copied, and
  looked again only minutes later. It now waits 3 seconds for the save being written, even when
  the ask comes with a change of NAS or folder.
- Save Backup's toasts have a SAVE BACKUP title of their own, and the log says how many saves
  each backup copied.

## omega-zero 2.5.1

- Settings > Save Backup has Sync From Backup, under Back Up Now: FROM BACKUP lists the saves on
  the NAS unlike the handheld's, or not on it, by game. A on one asks, A again copies it onto
  the handheld. The handheld's own is kept first in `Config/save_backup_undo/`, and the game's
  resume state set aside, as it holds the save the game had: the game starts from the one copied.
- A backup that fails is tried again after 5 seconds, then 10, 20 and 40, then every minute,
  rather than after a minute at once; and at once when Save Backup is opened or Wi-Fi comes back.
- A toast at the top of the screen says when saves were backed up, and when a backup failed: a
  failure once, not again until saves copy again.
- The log says what the NAS answered in full when it could not be used.

## omega-zero 2.5.0: Save Backup

- Settings > Save Backup copies every save on the card to a folder on a NAS over Samba (SMB 2
  and 3), as each is made. FIND NAS lists the NASes answering on the Wi-Fi by name, with a row
  to type one's address; User and Password are typed on the keyboard, none signing in as a
  guest; FOLDER lists the NAS's shares and their folders, opens them, makes a new one, and picks
  the one open. Back Up Now says where it stands and copies at once.
- What is copied is the card's whole Saves folder, whatever wrote it, so a system or emulator
  added later is backed up with no change. Only what changed is copied, as `Config/save_backup.list`
  keeps it, each over the one on the NAS by way of a file beside it renamed over it. With no
  network, or the NAS away, saves wait, and the newest of each is copied once the NAS answers:
  after the next save, when Wi-Fi comes back, or within minutes. The first time, or after
  another NAS or folder is picked, the whole Saves folder is copied.
- The shelf comes up on the game it was on when the handheld went off, on the same shelf, the
  game not opened: a game that was playing is still picked up again as before.
- The holo sheen is the clear and glitter carts': a cart in solid plastic has none.
- The Settings window holds a seventh CONFIGS row, Save Backup; on a 480-line screen it reaches
  up over the top bar to hold every row.
- Tests: Save Backup against a real Samba server when one is named (from the panel down to the
  file on the server), LAN play end to end (a room opened, found as RetroArch looks for rooms,
  joined with its password and a wrong one turned away), the shelf's cart at power-on and the
  sheen's shells.
- The README and README-FORK describe everything since 2.1.2: link play nearby, on the LAN and
  online, Run-Ahead, the holo sheen, START + POWER restarting, Save Backup and the files and keys
  they keep.

## omega-zero 2.4.3

- LAN play with RetroArch: Local has WITH, omega-zero or RetroArch. With RetroArch, a GBA game
  on gpSP links over the cable with RetroArch on the same Wi-Fi through its netplay, straight,
  with no relay or lobby: HOST opens a room RetroArch finds in its own room list, and JOIN lists
  the rooms on the Wi-Fi, as RetroArch looks for them. RetroArch answers only while it hosts
  with its relay off.
- Joining online no longer shows PASSWORD and SERVER, which are the host's: a room with a
  password says so when lit, and A asks for it, naming whose room it is, as RetroArch does. A
  wrong one asks again.
- Online, JOIN has ADVANCED, and under it ADDRESS: an address to join at past the lobby, such
  as a friend's RetroArch hosting with its port open, or over a VPN.
- The LINK panel says when a link goes through RetroArch's netplay. The rooms playing your
  game come first in the list, then those with no password.

## omega-zero 2.4.2

- The LINK panel's values end in one line: NAME's and PASSWORD's, which have no arrows, stood
  further right than PLAY's, ROLE's and SERVER's.

## omega-zero 2.4.1

- The LINK screen's rows are a settings list: up and down move between them, left and right
  change the one lit. PLAY, Local or Online, is on top for a GBA game on gpSP; Online is the
  cable's, so a game on the wireless adapter goes back to the cable for it.
- Online, the LINK screen has NAME, PASSWORD and SERVER under ROLE. A on NAME or PASSWORD types
  it on the keyboard, holding what is there now; SERVER picks the relay. They are the same as
  Settings > Gameplay > Online Play's.
- Online rooms can have a password, as RetroArch's do: a room hosted with one lets in only who
  gives it, and the rooms with one are listed to join, taking the password typed. A wrong or
  missing one says so.

## omega-zero 2.4.0: online play for gpSP

- GBA games on gpSP link online, with RetroArch running gpSP, with a GBA on a GB-Link adapter
  through its netplay bridge, and with another handheld on omega-zero: Pokémon trades and
  battles, Advance Wars. RetroArch's netplay is spoken as RetroArch 1.22 speaks it, in the mode
  it plays gpSP's link packets in, the core named gpSP and its version gpSP v1.0 as RetroArch
  names them.
- The LINK screen has VIA, Nearby or Online, on Y, for a GBA game on gpSP over the cable; the
  wireless adapter's games stay nearby, as GB-Link leaves them, a relay's delay being more than
  they take. Online, HOST opens a room through one of RetroArch's relays and puts it up in its
  lobby, saying who you are and through which relay while it waits; JOIN lists the rooms open on
  gpSP with no password, each host's name and their game, to pick one. Linked, it says with whom.
- Settings > Gameplay > Online Play: the Name the rooms show, typed on the keyboard (your
  RetroAchievements name, or RG SP, until one is typed), and the Relay a room is opened through:
  New York, Madrid, Sao Paulo or Singapore, Madrid to begin with.
- The in-game Run-Ahead row's hint says it is for gpSP, as Settings' does.

## omega-zero 2.3.5: Run-Ahead is gpSP's

- Run-Ahead runs on gpSP alone. mGBA answers as soon on its own, and had no time for a frame
  played twice: before it gave up on a game, its first presses held up frames and the sound. The
  game's menu shows Run-Ahead only for a game on gpSP, and Settings > Gameplay says it is for
  gpSP; Run-Ahead still steps aside on a gpSP game whose frames played again stop fitting.

## omega-zero 2.3.4: Run-Ahead's budget is what the screen waits for

- Run-Ahead's budget for a frame played again is 11 ms, not 14: the handheld waits 12 ms for a
  present's frame before it shows the one before again. mGBA's took 12 to 17 ms with its state
  kept every frame, late but mostly under 14, so it never gave up and still held up frames; now
  it does, after a few presses. gpSP's take 4 to 9 ms and keep Run-Ahead.

## omega-zero 2.3.3: Run-Ahead steps aside where it does not fit

- Run-Ahead times every frame it plays again, and gives up on a game when half of the latest,
  at least eight, ran over the 14 ms a frame is given: the game plays on as it would without it,
  and the log says so. On the H700 mGBA's frames played again took 15 to 22 ms, so with Run-Ahead
  on its games held up a frame at nearly every press rather than answering it sooner; gpSP's take
  4 to 9 ms, and keep it.

## omega-zero 2.3.2: gpSP starts every time, and Run-Ahead in the game's menu

- gpSP no longer crashes as a game starts. Its recompiler wants its code cache within the 128 MB
  an arm64 branch reaches, and it looked for a free block that close in a process that may have
  none: when it found none it went on with no cache at all, and the first block it translated
  was written through a null pointer. Now the cache is part of gpSP's own library, a block of
  its zeroed data made executable, always beside its code; and should it ever be missing, gpSP
  refuses the game rather than crash (`cores/gpsp/static-jit-cache.patch`). The core comes with
  the release's zip: Settings > Update brings omega-zero alone.
- The game's menu has Run-Ahead under GAMEPLAY, as Settings > Gameplay does, switched on or off
  without leaving the game. The PlayStation has none.

## omega-zero 2.3.1: run-ahead without the crackle, and START restarts

- Run-Ahead no longer crackles. It put the game back a frame after every frame it showed, and a
  core keeps part of its sound outside its states, so the sound broke sixty times a second. Now
  the game plays on as it always does, keeping a state of each frame, and only a change of
  buttons plays the frame before again with them: a press is still answered a frame sooner, and
  while the buttons hold the sound runs unbroken.
- Pressing START while POWER is held, before the hold powers off, restarts the handheld instead,
  the game saved first as a power-off saves it. START goes to neither the game nor a menu.

## omega-zero 2.3.0: run-ahead

- Settings > Gameplay has Run-Ahead, off to begin with. On, each present shows the frame after
  the one the game is at, played with the same buttons, so a press is answered a frame sooner:
  the game's own frame of lag is hidden. The game stays at its own frame, which its sound, its
  achievements, rewind and the states saved all come from. It costs the core a second frame a
  present, about 3 ms of a GBA game's 16.7 on the H700.
- It runs at normal speed only, never in fast-forward, rewinding or over a link, and never on the
  PlayStation, whose states are too big to keep one a frame. A core that will not save or load
  its state for it has it switched off for that game.
- The tests run on Linux, the render tests drawn by Mesa's surfaceless EGL on its software
  renderer, so CI no longer waits on a Mac to be free before a release goes out. A Mac still runs
  them as it did.
- A computer's own builds of the cores live in `vendor/host/`, apart from the console's in
  `vendor/`, which on Linux have the same names; `task core` fetches or builds them for a Linux PC
  as well as a Mac, and a Mac's old ones in `vendor/` are still found. On a computer with no sound
  device, omega-zero plays into a silent ring rather than none.

## omega-zero 2.2.2: the latency trace says where a press waits

- With `System/trace-latency.on` on the card, each press's line also says how long the core took
  over the frame that answered it, and the pace line every ten seconds how many ticks the
  emulator left unanswered, the longest frame the core took, and how long it waited for room in
  the sound ring. Without the file nothing is measured or written.
- A test drives the emulator as the handheld does and checks that a press set before a present's
  tick is in that present's frame.

## omega-zero 2.2.1: the holo sheen, on or off

- Settings > Labels has a Holo Sheen row, under Fetch Labels: A, left or right turns the rainbow
  on the cart picked off, and on again. It is on to begin with, and kept as it was set.
- Labels arriving from the workers are uploaded half as much at a time, two Game Boy Advance
  carts a frame rather than four, so a burst of them holds up a frame less.
- mGBA is built for the H700's Cortex-A53, as the other cores are. It comes with the release's
  zip: Settings > Update brings omega-zero alone.

## omega-zero 2.2.0: lighter on the handheld

- A cart's label, a menu's face or a toast drawn again at the size it was is written over its
  texture where it is, rather than the texture being made again: scrolling to carts whose labels
  are still arriving holds up fewer frames.
- Rewind's snapshots are written into the buffers the last ones left, rather than into new ones
  and then copied again, and the sound a frame makes trades buffers with the core rather than
  taking a new one each frame: less work on the emulator's thread while a game plays.
- omega-zero is built for the H700's Cortex-A53, as its cores already were.
- `task deploy:wifi` and `task deploy:device` keep the build they replace as
  `System/omega-zero.previous`, as Settings > Update does, and deploy:wifi no longer puts the
  console's password on sshpass's command line.
- Releases build faster: the cores are kept between releases that build them the same, and a
  release's push no longer runs the tests twice. Task and the Rust cache action are pinned.

## omega-zero 2.1.7: a quieter holo sheen, on the cart picked

- The holo sheen is subtler, its rainbow fainter and its band of light softer.
- Only the cart picked has it, fading in as it comes to the middle of the shelf: the carts
  beside it, and those scrolling past to a new one, have none. The shelf draws one more face
  over one cart rather than over every cart in sight, so it sways as smoothly as before.

## omega-zero 2.1.6: the holo sheen on the handheld, the IP on Network

- The holo sheen was drawn only over a cart drawn flat, never over one drawn as its model, as
  every cart on the handheld is: it is now drawn over the model's face too, and a little
  brighter where the light catches it, so it shows on a label's own art.
- Settings > Network says the handheld's address under the networks once it has one, IP
  192.168.1.20, over what to press for a scan until one is made.
- The save states' X DELETE is square at both ends, its pill's rounded end squared off as its
  word's box is.

## omega-zero 2.1.5: holo labels, X DELETE as a button

- The labels' glitter is gone for a holo sheen, as on a holographic Pokémon card: a faint
  rainbow over the label, and a brighter band of light that slides across it, on the diagonal,
  as the cart sways on the shelf. It is drawn over the cart in twelve frames, the one its sway
  faces each frame.
- The save states' X DELETE is a button as RETURN and LOAD are: X in a pill and DELETE big in
  the bar's font, on the bar's row between them.

## omega-zero 2.1.4: foil on the labels, parts that peek

- Every cart's label has a foil over it, as the holographic labels of Nintendo's own Pokémon
  paks do: the light breaks on it in small facets like shattered glass, each a little lighter
  or darker and faintly coloured, a soft band of sheen crosses it on the diagonal, and here and
  there a glint. It is slight, so the label's art reads through it.
- A clear cart's clock chip, crystal, ROM and coin cell stand lower on the board, so only their
  tops and pins peek over a GBA cart's label.

## omega-zero 2.1.3: X DELETE fits the bar, toasts over it

- The save states' X DELETE stands on the kit's hint line, a SAVE tab and the key in its dark
  well, as tall as the bar and level with RETURN and LOAD, rather than bare on the game's
  screenshot.
- Over the save states, a toast (State deleted, State loaded and the rest) stands just above the
  bar rather than at the top, where it covered the SAVE STATES panel.

## omega-zero 2.1.2: delete save states in a game

- In a game's save states (MENU twice, or MENU > States), X asks whether to delete the save
  shown: a DELETE STATE panel says how old it is, and the bar turns to B CANCEL and A DELETE.
  A deletes it and its picture, and the next save shows, or the game once none are left; B, or
  X again, keeps it. The bar shows X DELETE between RETURN and LOAD. An Undo Save of the save
  deleted goes with it.

## omega-zero 2.1.1: Update installs on the card

A bug fix for Settings > Update.

- Update put the new omega-zero beside the program it was started as, which is the loader
  `launch_frontend.sh` starts it through, `/lib/ld-linux-aarch64.so.1`, and said CARD FULL when
  that read-only folder refused it. It now always replaces `System/omega-zero` on the card, and
  says CARD FULL only when the card is full; CARD READ-ONLY and WRITE FAILED say what else went
  wrong.
- What Update says, a check's answer or what went wrong, stays on its row until Settings closes;
  opened again, the row shows the version running. An update put in place still asks for its
  restart.

## omega-zero 2.1.0: clear carts show their board

A small update, the first released on its own: Settings > Update on the console fetches it.

- The parts on a clear cart's board keep some of their own colour through the plastic: the gold
  contacts along the foot and the black chips read as such, not as more green. The clock's chip,
  its crystal, the ROM and the coin cell stand higher on the board, so more of them shows over
  a GBA cart's label.
- Releases are numbered for what they hold: a big feature release moves the middle number, a
  small update or a bug fix the last (`scripts/version.sh big` or `small`, or `task version:big`
  and `task version:small`, name the next one from this changelog's Unreleased section). The
  fork's releases start at 2.0.0; 1.2.0 and before are upstream's.
- CI names each failing test in an annotation anyone can read on the run.

## omega-zero 2.0.0: Slot's buttons, favorites, RetroAchievements and updates on the console

This fork's first release over omega-zero 1.2.0, everything it adds until now.

The buttons are Slot's again, a FAVORITES shelf stands first, and the LCD over a game can be
turned off.

- On the shelf MENU opens Settings, START picks the core (X still does too), and Y stars the cart
  or takes its star off. The bar says MENU SETTINGS.
- Starred carts stand on a FAVORITES shelf of their own, the first of the shelves, in name order,
  across every system. They are kept in `Config/favorites.txt`, one `gba/<name>` to a line, the
  file Slot writes, so a card that ran Slot keeps its favorites.
- A tap of MENU in a game opens MENU over it, the game paused and dimmed behind: Resume, Save
  State, Load State, States, Screen Filter, Colour Correction and Exit Game. A second tap straight
  after still opens the save states, and holding MENU still ejects.
- Screen Filter, in Settings' CONFIGS and in MENU, draws a Game Boy or GBA game through the LCD it
  always had, a plain grid between its pixels, or nothing. SELECT+A and SELECT+B step through them
  in a game, as on Slot. It is kept as `screen_filter` in `omega-zero.state`.
- Slot's other settings, in two panels CONFIGS opens: GAMEPLAY holds Fast Forward Sound and
  Rumble, which stood in CONFIGS, then Load State on Insert, Save State on Eject, Rewind, Turbo
  Buttons, Frame Blending (Off, Mix, LCD Ghosting) and the Game Boy's Palette (Auto and mGBA's 48);
  SHELF holds Recents, Game Titles, the Clock's 12 or 24 hours and Lid Power Off. Fast Forward can
  be OFF. Frame Blending and the Palette are in MENU over a game too.
- RECENTS, once shown, is the first shelf: the last ten games put in, kept in
  `Config/recents.txt` as Slot keeps them. Game Titles names the cart picked under the top bar.
- Every favorite wears a gold star at its corner, on every shelf, and FAVORITES stands its carts
  by system, then by name.
- Holding POWER powers off at once, as on Slot: the POWER menu and its Restart are gone.
- MENU over a game is laid out as Slot's in-game menu: Resume; Undo Save and Undo Load for 30
  seconds after a save or a load; States while there are any; Settings, which opens Screen
  (Screen Filter, Palette, Colour Correction, Frame Blending, Brightness, Blue Light), Gameplay
  (Fast Forward and the rest of GAMEPLAY) and Controls; and Exit Game. B steps back a page. The
  game is not dimmed under Screen, whose panel stands at the bottom, so each change shows at once.
  The menu comes up a moment after the tap, so a double tap goes straight to the save states.
  Turbo Buttons and Rewind switch in a game, not only at the next one.
- On a row of carts of different shapes, as FAVORITES and a SNES shelf are, a narrow cart beside
  a wide one no longer reaches into it or covers it: the carts in view stand the widest one's
  width apart, and the row is drawn from the outside in, the cart picked in front.
- RetroAchievements, through rcheevos 12.5 (MIT): with Settings > Gameplay > RetroAchievements
  on and an account in `Config/retroachievements.ini`, every Game Boy, Game Boy Color, GBA and
  SNES game is checked for achievements as it plays, each one earned said in a toast, with the
  game's progress when it starts. The password is swapped for the site's token on the first
  sign-in. Hardcore loads no state, rewinds nothing and picks up from no resume. omega-retro now
  keeps the memory map a core declares, which achievements read the game's memory by.
- Signing in to RetroAchievements on the keyboard: Settings > Gameplay > Account, A for the user
  name and then the password, as Labels signs in to ScreenScraper; signing in turns achievements
  on, and Y signs out and forgets the account.
- A cart's shell set in `Labels/cart_shell.txt`, the file Slot and Cart Studio write, is now
  drawn; `Labels/cart_shell.ini` still wins for a game both name.
- Labels are drawn for the carts in view first, on FAVORITES and RECENTS as on a system's shelf:
  a boot starts with the shelf in view, and a shelf or cart come into view moves its carts to the
  front of the labels still waiting.
- RetroAchievements has a panel of its own, GAMEPLAY's last row opening it: Achievements on or
  off, Account and Hardcore.
- MENU over a game: Settings > Achievements says whether RetroAchievements found the game, with
  the title and number it took the ROM for or why not, how many of its achievements are unlocked
  and their points, and lists every one of them, locked ones dim, with its description; and
  switches RetroAchievements and Hardcore. Hardcore switched on in a game restarts it.
- MENU over a game has Restart, which starts the game over from power on, through the core's
  `retro_reset`; achievements start watching again with it.
- A clear or glitter shell is clear: the board and the cart's hollow show through the plastic,
  and a GBA cart's board shows over its label too.
- A RetroAchievements sign-in the network refuses, as at a boot before Wi-Fi is up, is tried
  again after 5, 10, 20 and 40 seconds and then each minute; a wrong password is not. Signed in
  late, the game in play has its achievements loaded then.
- RetroAchievements offline: the site's answers are kept in `Config/retroachievements/cache/`,
  so a game played before signs in and loads with no network; an unlock earned offline is kept
  in `Config/retroachievements/pending.txt` and sent, with how long ago it was earned, the next
  time the site answers.
- Sliding from GAMEPLAY to ACHIEVEMENTS and back, the panel leaving keeps its own face: each
  options panel has a texture of its own.
- Clear shells are clearer, the plastic pale where nothing stands behind it, and show the board
  Pokémon Emerald's pak is built on: the flash, the clock's chip and crystal, the ROM under a coin
  cell, small parts, the maker's lettering, traces and the contacts. A GBA cart's board spans its
  hollow, wider than the label, so its edge no longer lines up with the label's well. Where no
  board stands behind the plastic, the shelf shows faintly through it.
- RetroAchievements moved out of GAMEPLAY to CONFIGS, under Labels, a row with only its ›; Fast
  Forward's speed moved from CONFIGS into GAMEPLAY to make room.
- GAMEPLAY › Emulators: each system's default core, kept in `Config/default_core.ini`, which every
  game not switched by hand runs on, and a row a system that puts the games switched by hand back
  on it. In the core picker Y puts the game back on the default, and picking the default keeps no
  core of its own for it.
- The boot animation is off unless SHELF › Boot Animation switches it on (`boot_animation`).
- L2 and R2 pressed together lock fast forward on, and off again; pressing R2 twice no longer
  does. Where a game has L2 and R2 of its own, it is SELECT+L2+R2.
- Going deeper, a screen comes in from the right, as Settings does from the shelf; going back,
  from the left.
- Brightness' three lowest steps dim the picture too, below where the backlight goes; from the
  fourth step up nothing changed.
- Settings › Update fetches the latest release from GitHub and puts its omega-zero in place,
  keeping the old one as `System/omega-zero.previous`, then restarts onto it. It always stands
  above About: on a 480-line screen Settings' modal reaches a pixel into the margin above it to
  hold GENERAL's six rows. A repository with no release yet says NO RELEASE YET, not NO
  NETWORK.
- GitHub Actions: the tests run on every push and pull request, and a push to main with a new
  version builds and publishes its release, with `omega-zero-aarch64` for Update.

## omega-zero 1.2.0

Supafaust runs SNES games, a game that takes the console down can no longer trap it at boot, and
labels take a quarter of the room and no longer hold the boot.

- SNES games run on Supafaust. Chosen with X, Supafaust took omega-zero down on a game's first
  frame and left the console on its boot screen: it forgets the defaults of its own options once
  a game has loaded, and read an empty one back. omega-zero now answers every option a core
  declares, with the core's own default where omega-zero sets none, as any libretro frontend does.
- A game whose core takes omega-zero down as it starts is no longer opened again at every boot.
  The next boot leaves it on the shelf, selected, rather than resume it, so the console can no
  longer get stuck on its boot screen over one game. That boot says so after the opening, on its
  terminal: which game went down, on which system and core, and how to try it another way. A, B
  or START goes on to the shelf once its prompt is up.
- When omega-zero goes down, the card keeps what it last said in `crash/omega-zero.log` and its
  whole memory as it went down in `crash/omega-zero.core.gz`, for a debugger. It starts again once
  the core is written, some seconds later. A crash within ten minutes of the last keeps the first
  core, and none is written when it would leave the card less than 512 MB.
- Labels take about a quarter of the room on the card. Once a label is cut to its cart, the
  label itself is shrunk to the smallest size that still covers every face its system draws it
  on, since no face is drawn bigger than that; until now each one was kept 600 pixels wide. Every
  cart looks as it did. Labels already on the card, or copied there by hand, are shrunk once in
  the background after the boot.
- Labels fetches each label at about two thirds of the width it did, so a sync downloads about
  half as much.
- The boot no longer reads any label before the shelf comes up. Every cart starts in its
  generated label and is drawn in its own on threads that stand back from the shelf and the
  game, the carts on screen first, a few a frame, while the opening plays.

## omega-zero 1.1.0

The SNES and the PlayStation join the Game Boys: their games run, and they stand on shelves of
their own, SNES carts and PlayStation discs modelled like the Game Boy carts.

- SNES games go in `Games/SNES` (`.sfc`, `.smc`). A game from the USA stands as the American SNES
  cart; one from Japan, Europe or anywhere else stands as the Super Famicom cart, as it was sold.
  The region comes from the ROM's header, a tag in the file name such as `(USA)` or `(Japan)`
  wins over it, and `cart_shell.ini` can set a cart's shape to `ntsc` or `sfc`.
- Two SNES cores, Snes9x and Supafaust, chosen per cart with X, as for the GBA's two. Snes9x is
  the default.
- PlayStation games go in `Games/PS1` as `.cue` with their `.bin`, `.chd`, `.pbp` or `.m3u`, also
  one folder down. They run on PCSX ReARMed, which carries its own BIOS; a real one in `BIOS/` is
  used when it is there.
- A PlayStation game stands on its shelf as its disc. A game on several discs is one entry, its
  discs stacked one behind another with a gap between them, and the whole stack goes down into
  the handheld. Files named `(Disc 1)`, `(Disc 2)` and so on are put together as one game, with
  a playlist made for them under `Config/playlists/PS1/`. In a game, SELECT+MENU opens DISC to
  put another disc in, and the disc in the drive is the one there when the game is resumed.
- In a SNES or PlayStation game, X, Y, L1 and R1 go to the game. A PlayStation game takes L2 and
  R2 too: fast-forward is SELECT+R2 held, and it has no rewind. The SNES keeps L2 rewind and R2
  fast-forward.
- The SNES picture is drawn at twice its size; the PlayStation's fills the screen at 4:3. A
  game made for 50 Hz runs at 50 Hz.
- Labels fetches SNES labels, and the label of each PlayStation disc.
- Snes9x is free for non-commercial use only; Supafaust and PCSX ReARMed are GPL-2.0. Their
  licences and source are in `System/licenses/`.

## omega-zero 1.0.1

The boot no longer waits on the labels: each one is cut to its cart once, as it is fetched.

- A card with hundreds of labels boots as quickly as one with none. Each label is cut to its
  cart's face once, right after Labels fetches it, and kept in `Labels/<system>/.cut/`; a boot
  draws the carts from those small cuts, on every core. Until now each boot decoded every
  full-size label before its first frame, and with a few hundred of them the boot logo stayed up
  long enough to look frozen. Labels already on the card, or copied there by hand, are cut once
  in the background after the boot, the carts on screen first, and wear a generated label until
  then.

## omega-zero 1.0.0

Labels fetch in the build you download: omega-zero now has its own ScreenScraper key.

- Labels, in Settings, fetches the scanned label of every cart that has none in the release
  build too. Until now a release carried no ScreenScraper developer key, so the screen said it
  could not reach ScreenScraper. You still sign in with your own ScreenScraper account.

## omega-zero 0.0.4

The slot under the shelf is gone: carts go down into the handheld, and the game comes on like a
tube TV.

- The slot under the shelf is gone: a cart goes into the handheld itself. From the middle of the
  shelf it rises a little, then is pulled down out of the bottom of the screen, and the game
  comes on as a tube TV does: a dot, a line across, then the picture opening white and fading to
  its colours. Leaving a game plays it backwards, and a cart the core turns away goes back out
  the same way under its warning sign.
- The carts stand in the middle between the top bar and the bottom bar, at their usual sizes. A
  Game Boy cart no longer grows on its way in.
- The shelf's system, GBA, GB or GBC, is in a pill between the bottom bar's buttons, always. It
  rises in as L1 or R1 changes shelf, and a letter jumped to with Up or Down shows in it for two
  seconds.
- Config/theme.txt's `recess` and `opening` coloured only the slot and are no longer read.

## omega-zero 0.0.3

Settings in the style of Mega Man Zero's menus, with Network, Bluetooth, Theme and a POWER menu;
every screen drawn in the UI kit's 8/16-bit pixel art; carts as low-poly models drawn the way a
PlayStation drew 3D; and an opening after Mega Man X5's boot terminal at power-on.

- Everything around the shelf is on the menus' 2-pixel grid: the slot's edges and its scoop, the
  slot's name in the menus' type and the theme's dim colour, the top bar's clock, figure and
  "ssh" (drawn bold the kit's way), and a generated label's title, now always Pixel Code at its
  own size. A cart the core turns away comes out under a pixel-art warning sign in the theme's
  colour. The background drifts two pixels at a time, a refused cart shakes a whole pixel at a
  time, and the boot screen fades out in four steps.
- The save states, MENU ×2 in a game, are drawn in the menus' pixel art. The shelf's top bar
  stands over the screenshot in place of a dark strip of their own; a SAVE STATES panel at the
  top in the middle says how long ago the save shown was made, with a dot for each save under
  it, the one shown lit; and the bar along the bottom has B RETURN and A LOAD. Nothing over the
  screenshot is see-through any more.
- Save states only save and load: Y no longer deletes the save shown, and X no longer undoes the
  last save or load.
- `<omega-hints>` joins the UI kit: a screen's keys on one line of the kit, on a ground of their
  own, for where the bar's hints would sit on a picture.
- The link screen (SELECT + MENU in a game) is drawn in the menus' kit: the theme's backdrop, a
  LINK panel with ROLE (HOST or JOIN, Left and Right swap it) and MODE (CABLE or WIRELESS,
  SELECT switches it where the game can), what is happening as a note under them, and the
  bottom bar with RETURN and LINK, SELECT MODE and ◀▶ SWAP between them. The plug, its cable,
  the port, the wireless adapter, its radio arcs and the clicks of a plug going home are pixel
  art on the 2-pixel grid, the plug still the purple of the real cable. A link that fails lifts
  the plug out and shakes it a pixel either way instead of tilting it, and the arcs pulse in four
  steps.
- The carts are low-poly models drawn the way a PlayStation drew 3D: a face and a side for
  every cut of the outline, each lit on its own, corners that land on the 2-pixel grid as they
  turn, textures mapped without perspective, and colour in 15 bits through the PlayStation's
  dither. The one you are on sways twenty degrees either way, seen from closer than before, so
  its side and the step under a GBA cart's grip show. The cart going into the slot and the core
  picker's lid are the same model. A cart is one draw instead of up to seventeen.
- The volume, brightness, blue light and rewind show as a gauge drawn like the menus: the
  level's name in a tab, then a cell for each step, lit as far as the level reaches, and its
  icon, on the 2-pixel grid in the colours of Settings > Theme, with no dark band behind it. It
  stands at the top of the screen, on the shelf just under the top bar, and fades out in four
  steps. What just happened shows the same way, a small panel titled SAVE STATE, LINK or COLOUR
  over "State saved", "Link ended", "Correction on" and the rest.
- Fast forward's badge in the corner is a key cap holding ▶▶ and the speed, ×2 to ×6, lit
  while it is latched. A link's badge shows a link, lit while hosting and dark when joined, and
  broken once the other handheld is lost.
- The core picker is pixel art on the 2-pixel grid: the open cart in its own plastic, the board,
  the ROM chip marked with the game's title, the sockets and the chip, with outlines and flat
  colours. The title and the cores' names are in Pixel Code at its own size and spacing. The
  chip hops between the sockets a pixel at a time, level, and the shadows under it and under
  the lid are checker dither.
- About is a window like Settings, drawn in the menus' kit in place of the sticker. OMEGA-ZERO
  lists the version, the build (its commit, with -DIRTY after it for a build with changes not
  committed), the author, @oauramos, the handheld as BaseOS names it, and how full its
  battery is and whether it is charging. CREDITS under it says what omega-zero is made with and
  on, a line each. The barcode, the ANBERNIC wordmark and the made-up rating lines are gone.
- A bar along the bottom of the shelf: SELECT CONTROLS on the left, START SETTINGS on the
  right. The slot sits a bar higher to make room for it.
- Game Boy carts stand at 80% of their size on the shelf and grow to full size as they go into
  the slot.
- The handheld starts on an old PC's BIOS screen, after Award's, in place of the logo: the
  OmegaZero BIOS, "An Anbernic Ally", oauramos Software Inc., the H700, a memory test, BaseOS's
  Plug and Play Extension and Press START to enter SETTINGS, with the OmegaZero logo in 16
  colours where Award put its Energy Star. It is set in the opening's letters, where its
  terminal prints. `task bootlogos` renders it; `task bootlogos:logo` renders the old logo.
- An opening after Mega Man X5's NOM ENGINEER WORK SYSTEM boot screen, told about the handheld
  it runs on. omega-zero's first frames show the boot screen where the handheld drew it, and it
  fades to black. Then a terminal fades in, in X5's wide mint letters and its beeps: the model,
  the memory counted up, the H700, the Mali-G31, the battery and its charge, a login as root,
  `>omega -boot -a` typed out and three files read. After ten seconds it fades out and the shelf
  fades in. Any button cuts it short, and that press does nothing on the shelf. Once the
  shelf shows through, a press ends the opening at once and works as usual. POWER and the lid
  end it at once. A boot back into a game, or to the clock, cuts as before.
- START on the shelf opens Settings, a window in the style of Mega Man Zero's menus, in place of
  the old quick menu. CONFIGS holds Fast Forward (2X, 3X, 4X or 6X), Fast Forward Sound, Colour
  Correction, Rumble and Labels; GENERAL holds Date & Time, Network, Theme and About.
- A row that Left and Right change shows ◀ ▶ while it is lit. A row that opens a screen ends
  in ›.
- A tap of SELECT on the shelf opens CONTROLS: what every button does, on the shelf and in a
  game, on one panel. SELECT held with another button works as before.
- The Wi-Fi screen is now Network, under Settings. Its NETWORK panel switches Wi-Fi, Network
  Time and SSH. AVAILABLE NETWORKS shows each network's signal, a padlock on those with a
  passphrase, and CONNECTED, JOINING or SAVED. Y forgets a saved network.
- Network looks for networks only when X is pressed, once each time, and the bar says X SCAN.
  Until then it lists the network joined and the saved ones, with a note to press X. It no
  longer scans every 12 seconds while it is open, which cost battery and, on the RG35XX SP,
  came before reboots that sometimes hung.
- Wi-Fi, Network Time and SSH are each ON or OFF, and Y on any of them opens its panel. Wi-Fi's
  shows the network joined, its signal, the handheld's address, `<name>.local` and the radio's
  MAC address. Network Time's shows whether a time server has set the clock, and the date, time
  and zone. SSH's shows how to log in from a computer: the address, `<name>.local`, user `root`,
  the password and port 22. SFTP uses the same login.
- SSH ON starts the SSH server and OFF stops it, leaving a login already made to carry on; the
  card remembers which.
- The first time omega-zero runs on a card whose `baseos.conf` sets no `ssh_password`, it gives
  root a password of six random digits, at once and in `baseos.conf`, so BaseOS keeps it. A
  password you set there yourself is kept.
- Cart Labels is no longer on the Wi-Fi screen: it is Labels, in Settings.
- Theme, Date & Time and Labels are panels over the background, and the passphrase
  keyboard is a window.
- In the menus the bottom bar says B RETURN and A CONFIRM. A screen's own keys sit in the middle
  of it: Y INFO and Y FORGET on Network, START FETCH and Y SIGN OUT on Labels. Between its two
  ends there is nothing but those keys, over the background.
- Every move between screens slides, over a background that keeps moving. Going into a screen,
  the one you leave goes out to the right and the new one comes in from the left; coming back is
  the mirror. Leaving the shelf, the top bar goes up, the carts sideways and the slot down.
  Settings, from the shelf, slides the other way round: the carts go out to the left as it comes
  in from the right, and it goes back out to the right. Inside Settings every screen slides the
  usual way.
- The menus, both bars and the slot take their colours from Settings > Theme > Colour.
  Night is grey; Amber is Mega Man Zero's own colours.
- Theme is what Background was: the background's pattern, its motion and its colour. It also
  picks the font of the bottom bar's big words, Bold or Thin.
- Restart no longer freezes the console after Settings > Network has been open. The RTL8821CS's
  Wi-Fi driver hung the kernel's shutdown when it had just gone into its power saving, as it does
  after scanning for networks, so the console stayed frozen until it was reset. omega-zero now
  lets go of the driver itself before a restart or power off: wpa_supplicant and the DHCP client
  stop, the interfaces go down, and the device is unbound from its driver.
- The top bar lights Wi-Fi and ssh within a second of a network being joined, and puts them out
  as quickly when it is lost. After boot they stayed dark until a menu was opened.
- Bluetooth, under Network in Settings, on handhelds that have it. Its screen switches it on and
  off and, while it is on, lists the devices paired, then those a look finds. A pairs with a new
  device and connects it, connects a paired one, or disconnects one connected; Y forgets a
  paired one. Settings names the device connected. The card remembers whether Bluetooth is on,
  bringing it up at boot, and the top bar lights its icon while it is. Pairing asks nothing on
  screen, which most gamepads and headphones take. Game sound over Bluetooth headphones and
  Bluetooth gamepads as controllers are still to come.
- The Bluetooth screen looks for devices only when asked: it opens on the paired devices and
  "Press X to look for devices.", and X SCAN looks for 12 seconds, saying "Looking for devices…"
  while the devices it finds join the list, which keeps them until the screen is left. Devices
  are listed by name: one found that gives none, most often a beacon nobody can pair with, is
  left out rather than shown by its address.
- Holding POWER opens POWER, a panel like the other menus over the background, with RESTART and
  POWER OFF and B RETURN and A CONFIRM on the bar; B goes back to where you were, a game
  included. It comes down from the top as the screen under it drops away, and goes back up. After
  a choice, in place of the black screen, a RESTART or POWER OFF panel comes down the same way and
  says "Restarting…" or "Turning off…" until the handheld goes down, and the bar sinks away.

### For developers

- `assets/ui/flow.html` walks every new screen with the slides the device draws, and is the
  reference they are built to. Keys stand in for the buttons, slow motion runs the slides five
  times slower, and the URL's hash picks the screen, colour, panel size and any moment of a
  slide: `flow.html#screen=network&from=settings&t=0.5&colour=amber&shot=1`.
- The storybook has three new components: `<omega-row>`, `<omega-list-row>` and `<omega-bar>`.
- And four for pages, which the site is now drawn with: `<omega-button>`, `<omega-screen>`, `<omega-card>`, `<omega-code>`.
- The kit is in Rust too, in `crates/omega-ui/src/kit/` (palette, pixel, modal, panel, action,
  row, list_row, bar, text, layout). `tests/suite/kit/` holds it to Chrome's screenshots of the
  HTML kit pixel for pixel; `make-refs.sh` takes them again.
- Each menu screen's input and drawing is a file of its own in `crates/omega-zero/src/screens/`,
  its faces are made in `frontend/sync_*.rs`, and `nav.rs` slides between them.
- `card/System/bluetooth.sh` drives the radio for omega-zero (up, down, status, scan on or off,
  pair, connect, disconnect, forget) and prints each device as a tab-separated line;
  `card/test-bluetooth.sh` tests it against fakes of BlueZ's tools.
- The daemons wifi.sh and bluetooth.sh leave running, and a dropbear omega-zero starts again,
  no longer hold the framebuffer open or share omega-zero's session and signals.
- Now that root's password is random, `task deploy:wifi` and `task log:wifi` log in with
  `CONSOLE_PASSWORD` or, without it, the first line of a git-ignored `.console-password` at the
  top of the repo.

## omega-zero 0.0.2

- A cart leaning half off the screen is drawn whole on every GPU: some drivers dropped it for a
  frame. Turning carts go through a shader of their own and everything else through the one it
  always had, so the shelf costs no more to draw than before carts turned.

### For developers

- `task deploy:wifi` builds omega-zero, copies it onto a console on the home network over SSH
  and reboots into it; `task log:wifi` prints its log. `PROFILE=device-dev` builds in seconds.
- Each crate's tests build as one binary, and the suite runs in about 42 s instead of 130 s.

## omega-zero 0.0.1

The first release, for the RG35XX SP and the other H700 handhelds BaseOS runs on.

### Every panel, natively

- omega-zero lays itself out for the panel it finds, from the model BaseOS names: 640x480 on
  the RG35XX SP instead of the RG SP's 720x480 picture shrunk to fit.
- On a 640x480 panel, GBA games fill the width (640x427) through a sharp scaler, over the
  console's body carrying the GAME BOY ADVANCE logo. Game Boy games stay at 3x through the LCD
  mask.
- Your own overlays: a `.png` the size of the screen in `Overlays/GBA`, `Overlays/GB` or
  `Overlays/GBC`, laid over the game. One that would cover the game is ignored.
- The boot logo is only ever replaced with one the size of the device's own. Logos ship for
  640x480, 720x480 and 720x720 panels.
- Rumble works on devices whose motor is the PMIC's `moto` pin rather than a force feedback
  node.
- Hosting a link works on devices whose Wi-Fi driver gives no second interface.
- A panel mounted on its side is turned in software (the RG28XX).
- `System/probe.sh` writes a hardware report, also from an empty `RUN_PROBE` file at boot, for
  device reports.

### Wi-Fi, SSH and labels

- Wi-Fi in the quick menu: switch it on, pick a network and type its passphrase on an
  on-screen keyboard. Networks are remembered in `Config/wifi.conf` and joined again at boot.
- Once joined, the Wi-Fi screen shows the address and name to reach the handheld over SSH and
  SFTP.
- Network Time keeps the clock right through BaseOS whenever Wi-Fi is joined.
- A link takes the radio over from home Wi-Fi and gives it back when it ends.
- Handhelds without Wi-Fi don't show the row.
- Cart Labels, on the Wi-Fi screen: sign in with your ScreenScraper account and fetch the
  scanned label of every cart that has none, from the ROM's own region first. Labels go on the
  shelf as they arrive. Yours are never replaced.

### A new look

- Every word on screen is in Pixel Code, a pixel font set on its own grid so it stays sharp.
- A top bar over the shelf: Wi-Fi, Bluetooth and ssh on the left, lit while they are up; the
  clock in the middle; the battery on the right.
- The shelf's background is yours to pick under START > Background: a grid, raised plates
  (riveted or not), dotted lines, a dithered grid, bits or a plain colour, drifting or still, in
  eight colours, every pattern in the same 2x2 pixels as the words over it. A wallpaper on the
  card still works.
- The slot below the shelf is bigger: a cart slides into it at full size instead of shrinking.
- The quick menu scrolls when its rows outgrow the screen.
- On the shelf START opens the quick menu and X the core picker; MENU opens nothing there.
- The carts on the shelf are solid. The one you are on floats and sways ten degrees either way,
  showing its edge, and every cart leans into a scroll. It squares up as it slides into the
  slot or opens for the core picker.
- The boot logo is the OMEGA ZERO title in pixel art, edged in white, over the shelf's grid.
  Its source is `assets/logo/omega-zero-logo.html`; `task bootlogos` renders it for every
  panel, and `STYLE=smooth` renders the drawn-straight version instead.
