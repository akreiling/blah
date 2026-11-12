"""World-map controls, fog, debug reveal, save isolation, and clock pausing in real windows."""

import time

from isle import Isle


def world_map():
    for flags in [(), ("--debug-map",)]:
        with Isle(seed=7, flags=flags) as isle:
            before = isle.batch("Wren\n", game=True)["game"]
            reply = isle.batch("m", game=True, state=True)
            screen = reply["screen"]
            assert screen["mode"] == "world_map"
            assert screen["center"] == before["player"]["pos"]
            assert screen["debug_reveal"] == bool(flags)
            assert reply["game"] == before, "opening the map neither advances nor alters the game"
            total = len(screen["view"]) * len(screen["view"][0])
            assert 0 < screen["known_cells"] <= total
            if flags:
                assert screen["known_cells"] == total
            else:
                assert screen["known_cells"] < total
            reply = isle.batch(*(["shift-right"] * 40), state=True, game=True, interrupt=False)
            assert reply["game"] == before, "explorer travel must not move the character or discover cells"
            assert reply["screen"]["known_cells"] > 0 if flags else reply["screen"]["known_cells"] == 0
            reply = isle.batch("f", "-", "-", state=True)
            assert reply["screen"]["center"] == before["player"]["pos"]
            assert reply["screen"]["step"] == 8, "zoom out covers a world region, not just smaller glyphs"
            reply = isle.batch("+", "+", "+", state=True)
            assert reply["screen"]["step"] == 1
            reply = isle.batch("escape", game=True, state=True)
            assert reply["game"] == before
            assert "prompt" in reply["screen"], "closing the map restores play"
            reply = isle.batch("look", "m", state=True)
            assert reply["screen"]["prompt"]["text"] == "lookm", "M edits an existing command draft"
            isle.batch("escape")
            assert isle.quit() == 0
        print(f"PASS world map {'debug' if flags else 'seen-only'}: open, pan, zoom, fog, no save changes, close")

    with Isle(seed=7, round_seconds=0.5) as isle:
        isle.batch("Wren\n", "m")
        before = isle.request("state", game=True)["game"]
        time.sleep(1.1)
        after = isle.request("state", game=True)["game"]
        assert after == before, "the live clock pauses while browsing the map"
        isle.batch("escape", "pause\n", interrupt=False)
        assert isle.quit() == 0
    print("PASS world map: clocked play pauses while the map is open")


if __name__ == "__main__":
    world_map()
