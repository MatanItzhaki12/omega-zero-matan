# Security

omega-zero-matan runs on a handheld that holds saves, a RetroAchievements account and, where Save
Backup is set up, a NAS's user and password; and it updates itself from this repository's
releases. A weakness in any of those is worth hearing about privately before it is public.

## Reporting

Please report a vulnerability through GitHub's private reporting: **Security › Report a
vulnerability** on this repository. Say what it is, how it is reached, and what it lets someone
do. You will hear back within a week, and the fix goes out as a release, with credit where you
want it.

Please do not open a public issue for it.

## What is in scope

- The update: Settings › Update checks each file it fetches against the SHA-256 of the release's
  manifest before it puts it in place (`crates/omega-zero/src/update.rs`).
- Save Backup's NAS login, kept in `Config/omega-zero.state` on the card.
- The RetroAchievements token, kept in `Config/retroachievements.ini`.
- SSH, which is off unless switched on in Settings › Network.

The emulator cores (mGBA, gpSP, Snes9x, Supafaust, PCSX ReARMed, PPSSPP) are their projects';
please report a weakness in one of them to it.
