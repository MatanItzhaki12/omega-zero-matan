#!/usr/bin/env python3
"""Writes HeartGold's and SoulSilver's function tables (IPKE.txt, IPGE.txt) for the one-screen
layouts, from Pokemon Fulldex's (mamesj/pokemon-fulldex, GPL-3.0) SoulSilver table and its
HeartGold shifts:

    hgss_tables.py <fulldex>/layouts/soulsilver-us.json <fulldex>/tools/soulsilver/heartgold-us.json

Fulldex's table names, for each of the game's applications (the Bag, the party, the Pokegear...),
the functions it runs every frame, and when it starts and ends, and the functions the touch
screen's buttons are read in, each by the section of code it is in: the ARM9's main binary or
an overlay. HeartGold's code is SoulSilver's at the same addresses but for three ranges of three
overlays, shifted by a few bytes, which its shifts name.
"""

import json
import sys

HEADER = """# Pokemon {game} ({code}): the functions the one-screen layouts watch, from Pokemon Fulldex's
# layouts/soulsilver-us.json{shifts} (mamesj/pokemon-fulldex, GPL-3.0),
# written by hgss_tables.py.
# Each line: address, t (Thumb), module, the function's first four bytes (- for the main binary,
# always loaded; ? to read them from the player's own ROM, its overlays sharing addresses), and
# its name: an application's own name for its per-frame function, :start and :end after it for
# the functions it starts and ends with.
"""


def main(soulsilver_json, heartgold_json):
    ss = json.load(open(soulsilver_json))
    hg_shifts = json.load(open(heartgold_json))["shifts"]
    sources = {s["name"]: s["source"] for s in ss["sections"]}

    def module(section):
        source = sources[section]
        if source == "main":
            return "main"
        if isinstance(source, int):
            return f"ov{source}"
        return None  # the ITCM's: none of the layouts' functions is there

    entries = []
    for app in ss["apps"]:
        name, section, exec_addr, init_addr, init_section, exit_addr, exit_section = app
        entries.append((exec_addr, section, name))
        if init_addr:
            entries.append((init_addr, init_section, name + ":start"))
        if exit_addr:
            entries.append((exit_addr, exit_section, name + ":end"))
    for name, section, addr in ss["focus"]:
        entries.append((addr, section, name))

    def shifted(addr, section):
        for start, end, by in hg_shifts.get(section, []):
            if start <= addr < end:
                return addr + by
        return addr

    for code, game, shift in (("IPGE", "SoulSilver", False), ("IPKE", "HeartGold", True)):
        lines = []
        for addr, section, name in sorted(entries, key=lambda e: (e[0], e[2])):
            mod = module(section)
            if mod is None:
                continue
            if shift:
                addr = shifted(addr, section)
            first = "-" if mod == "main" else "?"
            lines.append(f"{addr:08x} t {mod} {first} {name}")
        with open(f"{code}.txt", "w", newline="\n") as out:
            shifts = "\n# and tools/soulsilver/heartgold-us.json" if shift else ""
            out.write(HEADER.format(game=game, code=code, shifts=shifts))
            out.write("\n".join(lines) + "\n")
        print(f"{code}.txt: {len(lines)} functions")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
