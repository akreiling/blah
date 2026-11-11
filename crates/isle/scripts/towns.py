"""Real-window checks for generated services, nonzero map origins, revival, and stair return."""

import json
import pathlib
import tempfile
import time

from isle import Isle
from smoke import route


def walk(isle, level, start, goal):
    reply = isle.batch(*route(level, tuple(start), tuple(goal)), game=True, interrupt=False)
    assert reply["game"]["player"]["pos"] == goal
    return reply


def towns():
    began = time.perf_counter()
    with Isle(seed=7) as isle:
        reply = isle.batch("Wren\n", "fighter\n", game=True, map="rows")
        save = reply["game"]
        known = reply["map"]["towns"]
        home = next(t for t in known if t["origin"] == [0, 0])
        other = next(t for t in known if t["origin"] != [0, 0])
        assert isle.quit() == 0
    for town in [home, other]:
        # Select an actual generated settlement, then exercise its services with normal keyboard input.
        fixture = json.loads(json.dumps(save))
        fixture["player"]["pos"] = town["start"]
        template = fixture["creatures"][0]
        fixture["creatures"], fixture["spawns"] = [], []
        with tempfile.TemporaryDirectory(prefix="isle-towns-", dir="/tmp") as directory:
            path = pathlib.Path(directory) / "save.json"
            path.write_text(json.dumps(fixture))
            with Isle(save=path) as isle:
                reply = isle.batch("look\n", map="rows", state=True)
                level = reply["map"]
                assert level["origin"] == town["origin"] and level["name"] == town["name"]
                assert any("Outfitter:" in line["text"] for line in reply["news"])
                walk(isle, level, town["start"], town["shop"])
                reply = isle.batch("list\n", "sell potion\n", "buy potion\n", game=True, interrupt=False)
                assert any("You buy" in line["text"] for line in reply["news"])
                assert "Potion" in reply["game"]["player"]["sack"]
                walk(isle, level, town["shop"], town["temple"])
                reply = isle.batch("heal\n", game=True)
                assert reply["game"]["revival"] == town["temple"]
                assert any(town["name"] in line for line in isle.screen()["panel"])
                walk(isle, level, town["temple"], town["stairs_down"])
                reply = isle.batch(">", "<", game=True, interrupt=False)
                assert reply["game"]["player"]["at"] == "Surface"
                assert reply["game"]["player"]["pos"] == town["stairs_down"]
                saved = reply["game"]
                assert isle.quit() == 0
            # Resume the visit and verify a real death returns to this town, not the starting town.
            saved["player"]["pos"] = town["start"]
            saved["player"]["hp"] = 1
            x, y = town["start"]
            troll = dict(template, kind="Troll", wild=None, hp=70, at="Surface",
                         pos=[x+1, y], home=[x+1, y], hunting=True, spawn=None)
            saved["creatures"] = [troll]
            path.write_text(json.dumps(saved))
            with Isle(save=path) as isle:
                reply = isle.batch("rest\n" * 20, game=True)
                assert reply["interrupted"]
                assert reply["game"]["player"]["deaths"] == 1
                assert reply["game"]["player"]["pos"] == town["temple"]
                assert any(town["name"] in line for line in isle.screen()["panel"])
                assert isle.quit() == 0
        print(f"PASS town {town['name']} at {town['origin']}: shop, temple binding, stairs, reload, revival")
    print(f"PASS towns: two real settlements in {(time.perf_counter() - began) * 1000:.1f} ms")


if __name__ == "__main__":
    towns()
