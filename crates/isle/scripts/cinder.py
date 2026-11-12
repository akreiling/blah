"""Walk to Cinder Watch, prepare, fight its original guards, and claim the lasting reward."""

import json
import pathlib
import tempfile

from isle import Isle, drifted, focus_returned
from landmarks import paths, glyph

KINDS = {"CinderWarden": "cinder warden", "AshSentry": "ash sentry"}


def guards(game):
    return [c for c in game["creatures"] if c["kind"] in KINDS]


def cinder():
    with Isle(seed=7) as isle:
        reply = isle.batch("Wren\n", game=True)
        fresh = reply["game"]
        assert any("Rumor: Cinder Watch" in line["text"] for line in reply["news"])
        assert isle.quit() == 0
    # Suppress unrelated wildlife only. Do not inject a site, guards, gear, or player power.
    fresh["creatures"], fresh["spawns"] = [], []
    fresh["wild_ready"] = 2**64 - 1
    with tempfile.TemporaryDirectory(prefix="isle-cinder-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(fresh))
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True, map="cells")
            tower = tuple(reply["game"]["cinder_watch"]["pos"])
            for _ in range(300):
                if tuple(reply["game"]["player"]["pos"]) == tower:
                    break
                cells, routes = paths(reply["map"], reply["game"]["player"]["pos"])
                if tower in routes:
                    route = routes[tower]
                else:
                    frontier = [at for at in routes if routes[at] and not cells[at]["remembered"]]
                    assert frontier
                    at = min(frontier, key=lambda at: (
                        abs(at[0] - tower[0]) + abs(at[1] - tower[1]), len(routes[at])))
                    route = routes[at]
                reply = isle.batch(route[0], game=True, map="cells", interrupt=False)
            else:
                raise AssertionError("could not walk to the seeded Watch")
            assert reply["game"]["cinder_watch"]["phase"] == "Dormant"
            look = isle.batch("look\n", game=True)
            assert any("plating halves arrow damage" in line["text"] for line in look["news"])
            assert not guards(look["game"])
            reply = isle.batch(",", game=True)
            assert not guards(reply["game"])
            assert not next(s for s in reply["game"]["landmarks"] if s["kind"] == "CinderWatch")["opened"]
            reply = isle.batch("chant shield\n", "cast\n", "chant spark\n", "challenge\n",
                              game=True, interrupt=False)
            assert len(guards(reply["game"])) == 2
            assert "  arrows halved · Spark ignores plating" in isle.screen()["panel"]
            original = reply["game"]["cinder_watch"]["guards"]
            reply = isle.batch("cast ash sentry\n", game=True)
            partial = reply["game"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True)
            assert guards(reply["game"]) == guards(partial)
            assert reply["game"]["cinder_watch"]["guards"] == original
            for _ in range(80):
                game, player = reply["game"], reply["game"]["player"]
                if game["cinder_watch"]["phase"] == "Cleared":
                    break
                assert player["deaths"] == 0, isle.screen()
                enemies = guards(game)
                target = next((c for c in enemies if c["kind"] == "AshSentry"), enemies[0])
                adjacent = next((c for c in enemies if max(abs(c["pos"][i] - player["pos"][i])
                                                         for i in (0, 1)) <= 1), None)
                if player["hp"] <= player["max_hp"] - 8 and "Potion" in player["sack"]:
                    command = "quaff"
                elif player["chanted"]:
                    command = "cast" if player["chanted"] == "Shield" else f"cast {KINDS[target['kind']]}"
                elif player["shield"] <= 2 and player["focus"] >= 4:
                    command = "chant shield"
                elif adjacent:
                    command = f"attack {KINDS[adjacent['kind']]}"
                elif player["focus"] >= 3 and all(
                        max(abs(c["pos"][i] - player["pos"][i]) for i in (0, 1)) > 2 for c in enemies):
                    command = "chant spark"
                else:
                    dx, dy = (target["pos"][i] - player["pos"][i] for i in (0, 1))
                    command = ("n" if dy < 0 else "s" if dy > 0 else "") + (
                        "w" if dx < 0 else "e" if dx > 0 else "")
                reply = isle.batch(command + "\n", game=True)
            assert reply["game"]["cinder_watch"]["phase"] == "Cleared", isle.screen()
            assert reply["game"]["player"]["deaths"] == 0
            assert not guards(reply["game"])
            assert "Potion" not in reply["game"]["player"]["sack"], "the starting recovery supply mattered"
            reply = isle.request("state", game=True, map="cells")
            _, routes = paths(reply["map"], reply["game"]["player"]["pos"])
            reply = isle.batch(*routes[tower], game=True, interrupt=False)
            gold = reply["game"]["player"]["gold"]
            reply = isle.batch(",", game=True)
            assert reply["game"]["player"]["gold"] >= gold + 120
            assert {"Relic": "EmberCharm"} in reply["game"]["player"]["sack"]
            assert {"Herb": "Bogmint"} in reply["game"]["player"]["sack"]
            claimed = reply["game"]["player"]
            claimed_round = reply["round"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            assert isle.request("state", game=True)["game"]["player"] == claimed
            reply = isle.batch("challenge\n", ",", game=True, map="rows", interrupt=False)
            expected = {**claimed, "shield": max(0, claimed["shield"] - 2)}
            if not expected["shield"]:
                expected["shield_focus_credit"] = 0
            expected = drifted(expected, focus_returned(claimed_round, reply["round"]))
            assert reply["game"]["player"] == expected
            assert not guards(reply["game"])
            # Step aside so the hero does not cover the remembered tower marker.
            reply = isle.request("state", game=True, map="cells")
            _, routes = paths(reply["map"], tower)
            step = next(route for route in routes.values() if len(route) == 1)
            reply = isle.batch(*step, game=True, map="rows")
            assert glyph(reply, tower) == "W."
            assert isle.quit() == 0
    print("PASS cinder: natural walk, warning, locked coffer, preparation, combat, partial reload, one-time reward")


if __name__ == "__main__":
    cinder()
