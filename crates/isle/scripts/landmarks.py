"""Walk to a naturally generated site, open its cache, and check partial claims and reloads."""

from collections import deque
import json
import pathlib
import tempfile

from isle import Isle

DIRS = [(0, -1, "up"), (-1, 0, "left"), (0, 1, "down"), (1, 0, "right")]


def paths(level, start):
    ox, oy = level["origin"]
    cells = {(ox + x, oy + y): cell for y, row in enumerate(level["cells"]) for x, cell in enumerate(row)}
    routes = {tuple(start): []}
    queue = deque([tuple(start)])
    while queue:
        x, y = queue.popleft()
        for dx, dy, key in DIRS:
            at = x + dx, y + dy
            if at not in routes and at in cells and cells[at]["walkable"]:
                routes[at] = routes[(x, y)] + [key]
                queue.append(at)
    return cells, routes


def glyph(reply, pos):
    ox, oy = reply["map"]["origin"]
    x, y = pos
    return reply["map"]["view"][y - oy][2 * (x - ox):2 * (x - ox) + 2]


def landmarks():
    with Isle(seed=7) as isle:
        fresh = isle.batch("Wren\n", game=True)["game"]
        assert isle.quit() == 0
    # Suppress combat only. Leave the character in the starting town, with no injected sites or loot.
    fresh["creatures"], fresh["spawns"] = [], []
    fresh["wild_ready"] = 2**64 - 1
    with tempfile.TemporaryDirectory(prefix="isle-landmarks-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(fresh))
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True, map="cells")
            for _ in range(300):
                if reply["game"]["landmarks"]:
                    break
                cells, routes = paths(reply["map"], reply["game"]["player"]["pos"])
                frontier = [at for at in routes if not cells[at]["remembered"]]
                assert frontier, "the starting land has an exploration frontier"
                target = min(frontier, key=lambda at: (len(routes[at]), at[1], at[0]))
                reply = isle.batch(routes[target][0], game=True, map="cells", interrupt=False)
            else:
                raise AssertionError("walking from town did not discover a site")
            site = reply["game"]["landmarks"][0]
            assert not site["opened"]
            assert any("You discover" in line["text"] for line in reply["game"]["log"])
            reply = isle.request("state", game=True, map="rows")
            assert glyph(reply, site["pos"]) == "&?"
            look = isle.batch("look\n", game=True)
            assert any("unopened cache" in line["text"] for line in look["news"])
            reply = isle.request("state", game=True, map="cells")
            _, routes = paths(reply["map"], reply["game"]["player"]["pos"])
            reply = isle.batch(*routes[tuple(site["pos"])], game=True, interrupt=False)
            assert reply["game"]["player"]["pos"] == site["pos"]
            assert any("Use get all" in line["text"] for line in reply["news"])
            untouched = reply["game"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            before = isle.request("state", game=True)["game"]
            assert before["landmarks"] == untouched["landmarks"]
            gold = before["player"]["gold"]
            reply = isle.batch(",", game=True)
            assert reply["game"]["landmarks"][0]["opened"]
            assert reply["game"]["player"]["gold"] > gold
            assert any("You open the cache" in line["text"] for line in reply["news"])
            claimed = reply["game"]["player"]
            reply = isle.batch(",", game=True)
            assert reply["game"]["player"] == claimed
            assert not any("You open the cache" in line["text"] for line in reply["news"])
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            reply = isle.batch(",", game=True)
            assert reply["game"]["player"] == claimed
            assert isle.quit() == 0
        # Repeat the same natural cache with a full sack. Coins fit; the remaining objects must persist.
        untouched["player"]["sack"] = ["Potion"] * 12
        path.write_text(json.dumps(untouched))
        with Isle(save=path) as isle:
            reply = isle.batch(",", game=True)
            leftovers = next(p for p in reply["game"]["piles"] if p["pos"] == site["pos"] and p["at"] == "Surface")
            assert leftovers["items"]
            claimed_gold = reply["game"]["player"]["gold"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            reply = isle.batch(",", game=True)
            assert leftovers in reply["game"]["piles"]
            assert reply["game"]["player"]["gold"] == claimed_gold
            reply = isle.batch("quaff\n" * 6, ",", game=True, interrupt=False)
            assert not any(p["pos"] == site["pos"] and p["at"] == "Surface" for p in reply["game"]["piles"])
            assert len(reply["game"]["player"]["sack"]) == 6 + len(leftovers["items"])
            assert isle.quit() == 0
    print("PASS landmarks: walk from town, discover, render, open once, full sack, partial claim, and reload")


if __name__ == "__main__":
    landmarks()
