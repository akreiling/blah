"""Real-window covenant journey from an explicit expert/first-drop checkpoint.

Core tests measure keeper rolls and combat balance. This checkpoint supplies expert practice,
ordinary equipment, recovery supplies, and the first blade in a ground pile, not trial sites,
guardians, their wounds, later pieces, or ascension. Actual commands generate and play the hunt.
"""

import json
import pathlib
import tempfile

from isle import Isle
from landmarks import paths


def walk_to(isle, target):
    target = tuple(target)
    reply = isle.request("state", game=True, map="cells")
    for _ in range(400):
        me = reply["game"]["player"]["pos"]
        if tuple(me) == target:
            return reply
        cells, routes = paths(reply["map"], me)
        if target in routes:
            route = routes[target]
        else:
            frontier = [at for at in routes if routes[at] and not cells[at]["remembered"]]
            assert frontier, "no reachable exploration frontier toward the call"
            at = min(frontier, key=lambda at: (
                abs(at[0] - target[0]) + abs(at[1] - target[1]), len(routes[at])))
            route = routes[at]
        reply = isle.batch(route[0], game=True, map="cells", interrupt=False)
        assert reply["game"]["player"]["deaths"] == 0
    raise AssertionError("could not walk to the covenant destination")


def mythic():
    with Isle(seed=7) as isle:
        checkpoint = isle.batch("Wren\n", game=True)["game"]
        assert isle.quit() == 0
    checkpoint["creatures"], checkpoint["spawns"] = [], []
    checkpoint["wild_ready"] = 2**64 - 1
    player = checkpoint["player"]
    for stat in ["str", "dex", "int", "con"]:
        player[stat] = 14
    player["practice"]["skills"] = [{"practice": 30_000} for _ in range(12)]
    player["hp"] = player["max_hp"] = 42
    player["focus"] = player["max_focus"] = 32
    player["weapon"], player["armor"], player["ring"] = "LongSword", "Plate", True
    player["sack"] = ["Potion"] * 8
    checkpoint["covenant"]["first_dropped"] = True
    checkpoint["piles"] = [{"at": "Surface", "pos": player["pos"], "items": [{"Weapon": "Oathfang"}]}]
    with tempfile.TemporaryDirectory(prefix="isle-mythic-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(checkpoint))
        with Isle(save=path) as isle:
            reply = isle.batch("get oathfang\n", "inventory\n", "wield oathfang\n",
                              game=True, state=True, interrupt=False)
            assert reply["game"]["covenant"]["awakened"]
            assert len(reply["game"]["covenant"]["trials"]) == 2
            assert any("Oathfang's call" in line["text"] for line in reply["news"])
            assert "DEMIGOD · EXILE SET" not in reply["screen"]["panel"]
            destinations = reply["game"]["covenant"]["trials"]
            for index, trial in enumerate(destinations):
                walk_to(isle, trial["pos"])
                reply = isle.batch("look\n", game=True)
                assert any("challenge" in line["text"] for line in reply["news"])
                reply = isle.batch(",", game=True)
                assert reply["game"]["covenant"]["trials"][index]["phase"] == "Dormant"
                # Recover between finite trials. This is ordinary explicit rest, not fixture healing.
                reply = isle.batch("rest\n" * 24, "chant shield\n", "cast\n", "challenge\n",
                                  game=True, interrupt=False)
                original = reply["game"]["covenant"]["trials"][index]["guards"]
                reply = isle.batch("challenge\n", game=True)
                assert reply["game"]["covenant"]["trials"][index]["guards"] == original
                for turn in range(160):
                    game, player = reply["game"], reply["game"]["player"]
                    if game["covenant"]["trials"][index]["phase"] == "Cleared":
                        break
                    assert player["deaths"] == 0, isle.screen()
                    enemies = [c for c in game["creatures"] if c["id"] in original]
                    nearby = [c for c in enemies if max(abs(c["pos"][i] - player["pos"][i]) for i in (0, 1)) <= 1]
                    if player["hp"] < 18 and "Potion" in player["sack"]:
                        command = "quaff"
                    elif player["chanted"]:
                        command = "cast"
                    elif index == 0 and turn % 5 == 2 and player["focus"] >= 5:
                        command = "chant spark"
                    elif nearby:
                        command = "attack"
                    else:
                        target = min(enemies, key=lambda c: max(abs(c["pos"][i] - player["pos"][i]) for i in (0, 1)))
                        dx, dy = (target["pos"][i] - player["pos"][i] for i in (0, 1))
                        command = ("n" if dy < 0 else "s" if dy > 0 else "") + (
                            "w" if dx < 0 else "e" if dx > 0 else "")
                    reply = isle.batch(command + "\n", game=True)
                assert reply["game"]["covenant"]["trials"][index]["phase"] == "Cleared", isle.screen()
                walk_to(isle, trial["pos"])
                item = {"Armor": "AshenCarapace"} if index == 0 else {"Relic": "ExileHeart"}
                reply = isle.batch(",", game=True)
                assert item in reply["game"]["player"]["sack"]
                reply = isle.batch("wear carapace\n" if index == 0 else "wear exile heart\n", game=True, state=True)
                assert ("DEMIGOD · EXILE SET" in reply["screen"]["panel"]) == (index == 1)
            assert reply["game"]["player"]["max_hp"] == 62
            saved = reply["game"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True, state=True)
            assert reply["game"] == saved
            assert "DEMIGOD · EXILE SET" in reply["screen"]["panel"]
            health = reply["game"]["player"]["hp"]
            reply = isle.batch("remove exile heart\n", game=True, state=True)
            assert reply["game"]["player"]["max_hp"] == 42
            assert "DEMIGOD · EXILE SET" not in reply["screen"]["panel"]
            reply = isle.batch("wear exile heart\n", game=True, state=True)
            assert reply["game"]["player"]["max_hp"] == 62
            assert reply["game"]["player"]["hp"] == min(health, 42)
            assert isle.quit() == 0
    print("PASS mythic: expert checkpoint, first pickup, called destinations, real walking and trials, rewards, ascension, reload")


if __name__ == "__main__":
    mythic()
