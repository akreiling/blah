"""Exercise wilderness movement, rendering, map windows, memory, and reload in a real Isle app."""

import json
import pathlib
import tempfile

from isle import Isle


def at(level, pos):
    x, y = pos
    ox, oy = level["origin"]
    return level["cells"][y - oy][x - ox]


def wilderness():
    with Isle(seed=7) as isle:
        save = isle.batch("Wren\n", "fighter\n", game=True)["game"]
        assert isle.quit() == 0
    # Isolate terrain from combat, and begin at the west edge so this test stays short.
    save["player"]["pos"] = [-1, 9]
    save["creatures"], save["spawns"] = [], []
    with tempfile.TemporaryDirectory(prefix="isle-wilderness-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            level = isle.request("state", map="cells")["map"]
            assert level["origin"] == [-26, -5]
            y = next(y for y in range(1, 22) if at(level, (-1, y))["walkable"])
            # Enter town, follow its tree border, then step onto a known walkable wild cell.
            isle.batch("right", *[("down" if y > 9 else "up")] * abs(y - 9), interrupt=False)
            reply = isle.batch("left", state=True, game=True, map="cells")
            assert reply["game"]["player"]["pos"] == [-1, y]
            assert reply["screen"]["view"][6][12:14] == "@ "
            level = reply["map"]
            wild = at(level, (-1, y))
            assert wild["walkable"] and wild["visible"] and wild["remembered"]
            assert level["name"] != "Harrowmere"
            rows = isle.request("state", map="rows")["map"]
            assert rows["origin"] == level["origin"]
            assert len(rows["terrain"]) == 28 and all(len(row) == 50 for row in rows["terrain"])
            assert len(rows["view"]) == 28 and all(len(row) == 100 for row in rows["view"])
            reply = isle.batch("right", *["right"] * 8, game=True, interrupt=False)
            assert reply["game"]["player"]["pos"][0] >= 7
            explored = reply["game"]["explored"]
            index = (y % 16) * 16 + 15  # x=-1 is the last column of chunk -1.
            assert explored["-1"][str(y // 16)][index // 64] & (1 << (index % 64))
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            # Check before walking back: seeing the cell again must not be what restores its memory.
            resumed = isle.request("state", game=True)["game"]
            assert resumed["explored"] == explored
            assert resumed["player"]["pos"] == reply["game"]["player"]["pos"]
            assert isle.quit() == 0
    print("PASS wilderness: leave town, render biomes, bounded map origins, signed memory, save/reload")


if __name__ == "__main__":
    wilderness()
