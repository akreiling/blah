"""Find natural herbs, gather with a full sack, and use each kind through real keyboard input."""

import copy
import json
import pathlib
import tempfile

from isle import Isle, drifted, focus_returned
from landmarks import glyph, paths

RECOVERY = {"Woundleaf": (6, 0), "Bogmint": (0, 8), "Sunroot": (3, 4)}


def discover(isle, expected):
    # Relocated biome fixtures need an ordinary round to establish the local sight memory.
    reply = isle.batch("rest\n", "look\n", game=True, map="cells", interrupt=False)
    for _ in range(300):
        patches = [p for p in reply["game"]["herbs"] if not p["picked"] and p["herb"] == expected]
        if patches:
            # An older discovery may be outside this inspection window; approach only visible patches.
            cells, routes = paths(reply["map"], reply["game"]["player"]["pos"])
            patches = [p for p in patches if tuple(p["pos"]) in routes and cells[tuple(p["pos"])]["visible"]]
            if patches:
                patch = patches[0]
                rows = isle.request("state", map="rows")
                assert glyph(rows, patch["pos"]) == ("@ " if patch["pos"] == reply["game"]["player"]["pos"] else "h*")
                look = isle.batch("look\n", game=True)
                assert any(expected.lower() in line["text"] for line in look["news"])
                reply = isle.batch(*routes[tuple(patch["pos"])], game=True, map="cells", interrupt=False)
                assert reply["game"]["player"]["pos"] == patch["pos"]
                return patch, reply
        cells, routes = paths(reply["map"], reply["game"]["player"]["pos"])
        frontier = [at for at in routes if routes[at] and not cells[at]["remembered"]]
        assert frontier, "there is walkable land left to explore"
        target = min(frontier, key=lambda at: (len(routes[at]), at[1], at[0]))
        reply = isle.batch(routes[target][0], game=True, map="cells", interrupt=False)
    raise AssertionError(f"no {expected} found during the bounded walk")


def herbs():
    with Isle(seed=7) as isle:
        fresh = isle.batch("Wren\n", game=True)["game"]
        assert isle.quit() == 0
    # No injected herbs or patches. Disable wildlife to isolate gathering and exact recovery.
    fresh["creatures"], fresh["spawns"] = [], []
    fresh["wild_ready"] = 2**64 - 1
    for expected, start in [("Sunroot", None), ("Bogmint", [94, 39]), ("Woundleaf", [-96, 359])]:
        fixture = copy.deepcopy(fresh)
        if start is not None:
            # Actual seed-7 jungle and forest, not hand-authored resource fixtures.
            fixture["player"]["pos"] = start
        with tempfile.TemporaryDirectory(prefix="isle-herbs-", dir="/tmp") as directory:
            path = pathlib.Path(directory) / "save.json"
            path.write_text(json.dumps(fixture))
            with Isle(save=path) as isle:
                patch, reply = discover(isle, expected)
                untouched = reply["game"]
                assert isle.quit() == 0
            untouched["player"]["sack"] = ["Potion"] * 12
            path.write_text(json.dumps(untouched))
            with Isle(save=path) as isle:
                before = isle.request("state", game=True)["game"]
                reply = isle.batch(",", game=True)
                assert reply["round"] == before["round"] + 1
                assert not next(p for p in reply["game"]["herbs"] if p["pos"] == patch["pos"])["picked"]
                assert reply["game"]["player"]["sack"] == ["Potion"] * 12
                assert any("sack is full" in line["text"] for line in reply["news"])
                assert isle.quit() == 0
            with Isle(save=path) as isle:
                reply = isle.batch("quaff\n", f"get {expected.lower()}\n", game=True, interrupt=False)
                assert next(p for p in reply["game"]["herbs"] if p["pos"] == patch["pos"])["picked"]
                assert reply["game"]["player"]["sack"].count({"Herb": expected}) == 1
                before = reply["game"]["player"]
                reply = isle.batch(f"use {expected.lower()}\n", game=True)
                assert reply["game"]["player"] == before, "full resources do not waste a herb"
                _, routes = paths(isle.request("state", map="cells")["map"], patch["pos"])
                step = next(route[0] for route in routes.values() if len(route) == 1)
                reply = isle.batch(step, game=True, map="rows")
                assert glyph(reply, patch["pos"]) != "h*"
                held = reply["game"]
                assert isle.quit() == 0
            held["player"]["hp"] -= 10
            held["player"]["focus"] -= 12
            path.write_text(json.dumps(held))
            with Isle(save=path) as isle:
                before = isle.request("state", game=True)["game"]
                reply = isle.batch(f"use {expected.lower()}\n", game=True)
                health, focus = RECOVERY[expected]
                assert reply["round"] == before["round"] + 1
                assert reply["game"]["player"]["hp"] == before["player"]["hp"] + health
                returned = focus_returned(before["round"], reply["round"])
                assert reply["game"]["player"]["focus"] == min(
                    before["player"]["focus"] + focus + returned, reply["game"]["player"]["max_focus"])
                assert reply["game"]["player"]["practice"] == before["player"]["practice"]
                assert reply["game"]["rng"] == before["rng"]
                assert {"Herb": expected} not in reply["game"]["player"]["sack"]
                after = reply["game"]["player"]
                used = reply["round"]
                reply = isle.batch(f"use {expected.lower()}\n", game=True)
                after = drifted(after, focus_returned(used, reply["round"]))
                assert reply["game"]["player"] == after, "an empty sack has no herb to waste"
                assert isle.quit() == 0
            with Isle(save=path) as isle:
                reply = isle.request("state", game=True)
                assert next(p for p in reply["game"]["herbs"] if p["pos"] == patch["pos"])["picked"]
                assert reply["game"]["player"] == after
                assert isle.quit() == 0
        print(f"PASS {expected}: natural discovery, full sack, gather once, exact recovery, no practice, reload")


if __name__ == "__main__":
    herbs()
