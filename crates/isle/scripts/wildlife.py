"""Real-window regression: populate evolved wildlife, reload, display it, and target its regional name."""

import json
import pathlib
import tempfile
import time

from isle import Isle, sighted
from wilderness import at


def wildlife():
    with Isle(seed=7) as isle:
        save = isle.batch("Wren\n", "thief\n", game=True)["game"]
        assert isle.quit() == 0
    # A scoped fixture puts the character on seed 7's eastern shore, not in the safe town.
    save["player"]["pos"] = [74, 26]
    save["creatures"], save["spawns"] = [], []
    with tempfile.TemporaryDirectory(prefix="isle-wildlife-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            assert at(isle.request("state", map="cells")["map"], (74, 26))["walkable"]
            began = time.perf_counter()
            reply = isle.batch("rest\n" * 100, game=True, map="cells", interrupt=False)
            elapsed = time.perf_counter() - began
            save, level = reply["game"], reply["map"]
            creatures = save["creatures"]
            assert 0 < len(creatures) <= 12
            assert all(c["kind"] == "Wild" and c["wild"]["name"] and c["wild"]["genome"] for c in creatures)
            assert all(not (0 <= c["pos"][0] < 50 and 0 <= c["pos"][1] < 28) for c in creatures)
            candidates = []
            ox, oy = level["origin"]
            for c in creatures:
                x, y = c["pos"]
                for dx, dy in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
                    pos = (x + dx, y + dy)
                    if ox <= pos[0] < ox + 50 and oy <= pos[1] < oy + 28 and at(level, pos)["walkable"]:
                        candidates.append((c, pos))
            assert candidates, "a spawned creature has an accessible neighbor"
            creature, pos = candidates[0]
            assert isle.quit() == 0
        # Keep an actual spawned individual's genome. Freeze only its movement to isolate UI and targeting.
        save["player"]["pos"] = pos
        save["creatures"] = [creature]
        creature["stunned"] = 100
        save["wild_ready"] = 2**64 - 1
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            screen = isle.screen()
            observed = sighted(screen)
            name = creature["wild"]["name"]
            assert len(observed) == 1 and observed[0][1] == name and observed[0][2] == 1
            assert screen["view"][6][12:14] == "@ "
            reply = isle.batch("nock\n", f"shoot {name}\n", game=True, interrupt=False)
            assert reply["game"]["player"]["arrows"] == save["player"]["arrows"] - 1
            assert any(name in line["text"] for line in reply["news"]), reply["news"]
            assert isle.quit() == 0
    print(f"PASS wildlife: {len(creatures)} evolved individuals; 100 rounds {elapsed * 1000:.1f} ms; named shot at {name}")


if __name__ == "__main__":
    wildlife()
