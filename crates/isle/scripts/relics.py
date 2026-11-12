"""Walk from town to a natural relic, then check its visible costs and single worn slot."""

import json
import pathlib
import tempfile

from isle import Isle
from landmarks import paths

RELICS = {
    "EmberCharm": ("ember charm", "+2 Spark damage; -3 Heal recovery (minimum 1)"),
    "Stillstone": ("stillstone", "+2 active ward protection; -4 maximum Focus"),
    "HuntersToken": ("hunter's token", "+2 archery accuracy; -15 percentage points to hiding attempts"),
}


def relics():
    with Isle(seed=7) as isle:
        fresh = isle.batch("Wren\n", game=True)["game"]
        assert isle.quit() == 0
    # Suppress combat and fill the sack, but inject neither sites nor relics. Walk from the starting town.
    fresh["creatures"], fresh["spawns"] = [], []
    fresh["wild_ready"] = 2**64 - 1
    fresh["player"]["sack"] = ["Potion"] * 12
    with tempfile.TemporaryDirectory(prefix="isle-relics-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(fresh))
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True, map="cells")
            for _ in range(1500):
                cells, routes = paths(reply["map"], reply["game"]["player"]["pos"])
                sites = [site for site in reply["game"]["landmarks"]
                         if site["kind"] != "CinderWatch" and not site["opened"] and tuple(site["pos"]) in routes]
                if sites:
                    site = min(sites, key=lambda site: len(routes[tuple(site["pos"])]))
                    reply = isle.batch(*routes[tuple(site["pos"])], ",", game=True,
                                       map="cells", interrupt=False)
                    assert next(s for s in reply["game"]["landmarks"] if s["pos"] == site["pos"])["opened"]
                    pile = next((p for p in reply["game"]["piles"]
                                 if p["pos"] == site["pos"] and p["at"] == "Surface"), None)
                    finds = [item["Relic"] for item in pile["items"]
                             if isinstance(item, dict) and "Relic" in item] if pile else []
                    if finds:
                        relic = finds[0]
                        assert reply["game"]["player"]["sack"] == ["Potion"] * 12
                        assert not reply["game"]["player"]["relic"]
                        assert isle.quit() == 0
                        break
                    continue
                frontier = [at for at in routes if routes[at] and not cells[at]["remembered"]]
                assert frontier, "the starting island has an exploration frontier"
                at = min(frontier, key=lambda at: (len(routes[at]), at[1], at[0]))
                reply = isle.batch(routes[at][0], game=True, map="cells", interrupt=False)
            else:
                raise AssertionError("no relic found during the bounded walk from town")

        name, effect = RELICS[relic]
        with Isle(save=path) as isle:
            reply = isle.request("state", game=True)
            assert reply["game"]["player"]["sack"] == ["Potion"] * 12
            reply = isle.batch("get relic\n", game=True)
            assert reply["game"]["player"]["sack"] == ["Potion"] * 12
            reply = isle.batch("quaff\n", "get relic\n", game=True, interrupt=False)
            assert reply["game"]["player"]["sack"].count({"Relic": relic}) == 1
            before = reply["game"]
            reply = isle.batch("inventory\n", game=True)
            assert reply["round"] == before["round"]
            assert any(name in line["text"] and effect in line["text"] for line in reply["news"])
            reply = isle.batch(f"wear {name}\n", game=True)
            assert reply["round"] == before["round"] + 1
            assert reply["game"]["player"]["relic"] == relic
            assert {"Relic": relic} not in reply["game"]["player"]["sack"]
            assert reply["game"]["player"]["practice"] == before["player"]["practice"]
            assert reply["game"]["rng"] == before["rng"]
            panel = isle.screen()["panel"]
            assert f"Relic {name}" in panel
            assert effect in " ".join(panel), "both sides of the trade-off appear on the ordinary panel"
            expected_max = before["player"]["max_focus"] - (4 if relic == "Stillstone" else 0)
            assert reply["game"]["player"]["max_focus"] == expected_max
            worn = reply["game"]["player"]
            assert isle.quit() == 0
        with Isle(save=path) as isle:
            assert isle.request("state", game=True)["game"]["player"] == worn
            reply = isle.batch("remove relic\n", game=True)
            assert not reply["game"]["player"]["relic"]
            assert reply["game"]["player"]["focus"] == worn["focus"], "removal cannot refill Focus"
            assert reply["game"]["player"]["max_focus"] == before["player"]["max_focus"]
            assert reply["game"]["player"]["sack"].count({"Relic": relic}) == 1
            assert not any(row.startswith("Relic ") for row in isle.screen()["panel"])
            # Reopening the same cache cannot duplicate the find.
            reply = isle.batch(",", game=True)
            assert reply["game"]["player"]["sack"].count({"Relic": relic}) == 1
            assert isle.quit() == 0
    print(f"PASS relics: natural {name}, full sack, pickup, visible trade-offs, wear, no-refill removal, reload")


if __name__ == "__main__":
    relics()
