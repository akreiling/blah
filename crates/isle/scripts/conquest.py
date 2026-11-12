"""Real-window conquest, prize pickup, equipment, and reload from a wounded-keeper checkpoint.

The game generates the floor and keeper. The checkpoint places the player beside that keeper,
wounds and stuns it, and fills the sack. It injects no conquest marks or rewards and does not
measure fresh-character difficulty or campaign pacing.
"""

import json
import pathlib
import tempfile

from isle import Isle
from smoke import route


def conquest():
    with Isle(seed=7) as isle:
        reply = isle.batch("Wren\n", game=True, map="rows")
        home = next(t for t in reply["map"]["towns"] if t["origin"] == [0, 0])
        isle.batch(*route(reply["map"], tuple(home["start"]), tuple(home["stairs_down"])), interrupt=False)
        reply = isle.batch(">", "rest\n", game=True, map="rows", state=True, interrupt=False)
        floor, checkpoint = reply["map"], reply["game"]
        assert checkpoint["conquests"] == []
        assert any("unconquered" in line for line in reply["screen"]["panel"])
        assert isle.quit() == 0

    at = checkpoint["player"]["at"]
    spawn = next(s for s in checkpoint["spawns"] if s["at"] == at and s["den"])
    keeper = next(c for c in checkpoint["creatures"] if c["id"] == spawn["occupant"])
    keeper["hp"], keeper["stunned"] = 1, 100
    checkpoint["creatures"] = [keeper]
    for state in checkpoint["spawns"]:
        state["ready"] = 2**64 - 1
        if state is not spawn:
            state["occupant"] = None
    x, y = keeper["pos"]
    adjacent = next((nx, ny) for nx, ny in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
                    if floor["terrain"][ny][nx] == ".")
    checkpoint["player"]["pos"] = list(adjacent)
    checkpoint["player"]["str"] = 100
    checkpoint["player"]["sack"] = ["Potion"] * 12
    prize = {"Relic": "IronrootBead"}

    with tempfile.TemporaryDirectory(prefix="isle-conquest-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(checkpoint))
        with Isle(save=path) as isle:
            reply = isle.batch("attack\n", game=True, state=True)
            assert reply["game"]["conquests"] == [at]
            assert any("You conquer" in line["text"] for line in reply["news"])
            assert any("first-conquest prize" in line["text"] for line in reply["news"])
            assert any("conquered" in row and "unconquered" not in row for row in reply["screen"]["panel"])
            dx, dy = x - adjacent[0], y - adjacent[1]
            step = "e" if dx > 0 else "w" if dx < 0 else "s" if dy > 0 else "n"
            isle.batch(step + "\n", interrupt=False)
            reply = isle.batch("get ironroot\n", game=True)
            assert prize not in reply["game"]["player"]["sack"]
            assert any(prize in pile["items"] for pile in reply["game"]["piles"] if pile["at"] == at)
            reply = isle.batch("quaff\n", "get ironroot\n", "inventory\n", game=True, interrupt=False)
            assert reply["game"]["player"]["sack"].count(prize) == 1
            assert any("+1 melee damage; -1 archery accuracy" in line["text"] for line in reply["news"])
            reply = isle.batch("wear ironroot\n", game=True, state=True)
            assert reply["game"]["player"]["relic"] == "IronrootBead"
            assert "Relic ironroot bead" in reply["screen"]["panel"]
            assert "+1 melee damage; -1 archery accuracy" in " ".join(reply["screen"]["panel"])
            saved = reply["game"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True)
            assert reply["game"]["conquests"] == [at]
            assert reply["game"]["player"] == saved["player"]
            reply = isle.batch("look\n", game=True)
            assert any("Depth 1: conquered." in line["text"] for line in reply["news"])
            assert isle.quit() == 0
    print("PASS conquest: generated keeper, first-clear mark and prize, full sack, pickup, trade-off panel, reload")


if __name__ == "__main__":
    conquest()
