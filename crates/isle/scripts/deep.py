"""Real-window checks for town Deeps: generated floors, evolved Deep life, safe landings, depth, and persistence."""

import json
import pathlib
import tempfile
import time

from isle import Isle, sighted
from smoke import route


def below(save, town, depth=1):
    """Creatures living on a floor of the Deep under a town, by id."""
    return {c["id"]: c for c in save["creatures"]
            if isinstance(c["at"], dict) and c["at"]["Deep"] == [c["at"]["Deep"][0], depth]
            and c["at"]["Deep"][0]["origin"] == town["origin"]}


def descend(isle, town, depth=1):
    reply = isle.batch(">", "rest\n", game=True, map="rows", state=True, interrupt=False)
    level, save = reply["map"], reply["game"]
    assert level["name"] == f"the Deep beneath {town['name']}, depth {depth}", level["name"]
    assert (level["origin"], level["width"], level["height"]) == ([0, 0], 64, 32)
    sx, sy = level["stairs_up"]
    assert level["terrain"][sy][sx] == "<" and save["player"]["pos"] == level["stairs_up"]
    assert reply["screen"]["view"][6][12:14] == "@ "
    life = below(save, town, depth)
    assert life, "the Deep is inhabited"
    for c in life.values():
        lineage = c["wild"]["lineage"]["Deep"]
        assert c["kind"] == "Wild" and c["wild"]["name"] and lineage["town"]["origin"] == town["origin"]
        assert lineage["depth"] == depth
        assert max(abs(c["pos"][0] - sx), abs(c["pos"][1] - sy)) > 2, "nothing lives on the stair landing"
    return level, save, life


def deep():
    began = time.perf_counter()
    with Isle(seed=7) as isle:
        reply = isle.batch("Wren\n", "fighter\n", game=True, map="rows")
        known = reply["map"]["towns"]
        home = next(t for t in known if t["origin"] == [0, 0])
        other = next(t for t in known if t["origin"] != [0, 0])
        isle.batch(*route(reply["map"], tuple(home["start"]), tuple(home["stairs_down"])), interrupt=False)
        home_level, _, home_life = descend(isle, home)
        names = sorted({c["wild"]["name"] for c in home_life.values()})
        reply = isle.batch("<", game=True)
        assert reply["game"]["player"]["pos"] == home["stairs_down"] and reply["game"]["player"]["at"] == "Surface"
        save = reply["game"]
        assert isle.quit() == 0
    with tempfile.TemporaryDirectory(prefix="isle-deep-", dir="/tmp") as directory:
        # Stand on the first floor's lower landing, then take real stairs down a level and back.
        path = pathlib.Path(directory) / "save.json"
        save["player"]["at"] = save["deeps"][0]["at"]
        save["player"]["pos"] = home_level["stairs_down"]
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            _, _, deeper = descend(isle, home, 2)
            mean = lambda life: sum(c["hp"] for c in life.values()) / len(life)
            assert mean(deeper) > mean(home_life), "deeper life is tougher"
            reply = isle.batch("<", game=True)
            assert reply["game"]["player"]["pos"] == home_level["stairs_down"]
            assert reply["game"]["player"]["at"]["Deep"][1] == 1
            save = reply["game"]
            assert isle.quit() == 0
    first = json.loads(json.dumps(save))
    save["player"]["at"] = "Surface"
    with tempfile.TemporaryDirectory(prefix="isle-deep-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        save["player"]["pos"] = other["stairs_down"]
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            level, save, other_life = descend(isle, other)
            assert level["terrain"] != home_level["terrain"], "each town has its own Deep"
            assert below(save, home) == below(first, home), "the first Deep waits unchanged while you are elsewhere"
            save = isle.batch("<", game=True)["game"]
            assert save["player"]["pos"] == other["stairs_down"]
            assert isle.quit() == 0
        save["player"]["pos"] = home["stairs_down"]
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            reply = isle.batch(">", game=True, map="rows", state=True)
            assert reply["map"]["terrain"] == home_level["terrain"]
            assert set(below(first, home)) <= set(below(reply["game"], home)), "the same individuals live there after reload"
            memory = reply["game"]["deeps"]
            assert sorted((m["at"]["Deep"][0]["origin"], m["at"]["Deep"][1]) for m in memory) == sorted(
                [(home["origin"], 1), (home["origin"], 2), (other["origin"], 1)])
            assert all(any(m["seen"]) for m in memory), "each Deep remembers what was seen in it"
            assert isle.quit() == 0
    print(f"PASS deep: {home['name']} ({', '.join(names)}), its depth 2, and {other['name']} are separate and persist;"
          f" {(time.perf_counter() - began) * 1000:.0f} ms")


if __name__ == "__main__":
    deep()
