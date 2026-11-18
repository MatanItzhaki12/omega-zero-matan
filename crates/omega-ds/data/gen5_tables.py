#!/usr/bin/env python3
"""Writes the fifth generation's function tables for the one-screen layouts (IREO.txt Black 2,
IRDO.txt White 2, IRBO.txt Black, IRAO.txt White), cut to the functions the layouts watch from
the whole tables Pokemon Fulldex's (mamesj/pokemon-fulldex, GPL-3.0) tools make:

    python3 <fulldex>/tools/bw2/funcmap.py <pokebw2>/config/b2_us/arm9 - IREO.full
    python3 <fulldex>/tools/bw2/funcmap.py <pokebw2>/config/w2_us/arm9 - IRDO.full
    python3 <fulldex>/tools/bw1/funcmap.py <aletheia>/layout/function_addrs.tsv - IRBO.full
    python3 <fulldex>/tools/bw1/funcmap.py <aletheia>/layout/function_addrs.tsv - IRAO.full --white
    gen5_tables.py IREO.full IRDO.full IRBO.full IRAO.full

Black 2's and White 2's names are the pokebw2 decompilation's (fuddlesworth/pokebw2, 753b3fd);
Black's and White's the aletheia decompilation's (Project-Holon/aletheia), but for the functions
Fulldex matched to Black 2's, which carry Black 2's name (its tools/bw1/aliases.txt), so that the
same layouts follow both pairs of games. Each table holds addresses and names, not the games'
code: an overlay function's first bytes, which tell the overlays loaded at one address apart, are
read from the player's own ROM (?).
"""

import sys

# The functions the layouts watch (src/gen5.rs), in the order they are listed there.
NAMES = [
    # the overworld, its X menu and saving from it
    "FieldmapProc_Update",
    "EventFieldMenu_Callback",
    "EventSave_Callback",
    "CARD_WaitBackupAsync",
    "REPORT_EVENT_Main",
    # battles: any, the menus on the touch screen, its pages and questions
    "BtlMain_ProcMain",
    "BtlvCore_WaitCommandSelect",
    "BtlvCore_WaitMoveSelect",
    "BtlvCore_WaitTargetSelect",
    "BPlistMain_Task",
    "BBagMain_Task",
    "BChoiceMain_Task",
    "BtlvInput_YesNoTask",
    "BtlvCore_PollYesNo",
    "ShinkaDemo_Main",
    # the applications
    "ItemMenuProc_Main",
    "PokeListProc_Main",
    "TrainerCard_Main",
    "TRCARD_SYS_StateWaitBadgeProc",
    "TRCARD_SYS_StateWaitEditProc",
    "TownMap_ProcMain",
    "ConfigPanel_Main",
    "PStatus_ProcMain",
    "ZknListSeq_Main",
    "ZukanDetail_Main",
    "ZukanOpen_Main",
    "ZUKAN_Update",
    "ZUKAN_TOROKU_GRAPHIC_Main",
    "BTLV_INPUT_ProcessTouchInput",
    "Box2Proc_Main",
    "MUSICAL_DRESSUP_SYS_Main",
    # the start menu, the C-Gear's question after CONTINUE, a new game and naming
    "StartMenu_Main",
    "StartMenu_Continue",
    "ConfirmDialog_Update",
    "IntroProc_Main",
    "NameEntry_ProcMain",
]

GAMES = [
    ("IREO", "Black 2", "pokebw2's config/b2_us"),
    ("IRDO", "White 2", "pokebw2's config/w2_us"),
    ("IRBO", "Black", "aletheia's layout/function_addrs.tsv"),
    ("IRAO", "White", "aletheia's layout/function_addrs.tsv and Fulldex's White shifts"),
]

HEADER = """# Pokemon {game} ({code}): the functions the one-screen layouts watch, cut from the table
# Pokemon Fulldex's tools (mamesj/pokemon-fulldex, GPL-3.0) make from {source},
# by gen5_tables.py. Each line: address, a (ARM) or t (Thumb), module, the function's first four
# bytes (- for the main binary, always loaded; ? to read them from the player's own ROM, its
# overlays sharing addresses), and its name.
"""


def main(paths):
    for (code, game, source), path in zip(GAMES, paths):
        by_name = {}
        for line in open(path):
            parts = line.split()
            if len(parts) == 5 and not line.startswith("#"):
                by_name.setdefault(parts[4], line.rstrip("\n"))
        kept = [by_name[n] for n in NAMES if n in by_name]
        with open(f"{code}.txt", "w", newline="\n") as out:
            out.write(HEADER.format(game=game, code=code, source=source))
            out.write("\n".join(kept) + "\n")
        missing = [n for n in NAMES if n not in by_name]
        print(f"{code}.txt: {len(kept)} functions; not in {game}: {', '.join(missing) or 'none'}")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    main(sys.argv[1:])
