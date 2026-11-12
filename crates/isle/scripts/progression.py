"""Real-window checks for name-only creation, visible practice, Focus, and saved progression.

Run after `cargo build -p isle`. Core tests cover finite encounter credits and rank effects;
this checks the same controls a player uses, without injecting practice or experience.
"""

import json
import pathlib
import tempfile

from isle import Isle, focus_returned, panel_value


def progression():
    with Isle(seed=7) as isle:
        reply = isle.batch("Wren\n", game=True, state=True)
        save = reply["game"]
        player = save["player"]
        assert save["phase"] == "Play" and reply["round"] == 0
        assert "class" not in player and player["xp"] == 0 and player["level"] == 1
        assert panel_value(reply["screen"], "Focus") == "24/24"
        assert panel_value(reply["screen"], "Hand") == "short sword · novice"
        practice = player["practice"]
        reply = isle.batch("skills\n", game=True, state=True)
        assert reply["round"] == 0 and reply["game"]["player"]["practice"] == practice
        news = "\n".join(line["text"].lower() for line in reply["news"])
        assert all(word in news for word in ["blades", "archery", "stealth", "evocation", "vitality", "focus"]), news
        panel = reply["screen"]["panel"]
        assert panel[0] == "PRACTICE" and "dagger: novice" in panel and "focus: novice" in panel
        assert len([line for line in panel if line.endswith(": novice")]) == 12
        isle.batch("escape")
        assert panel_value(isle.screen(), "Hand") == "short sword · novice"
        reply = isle.batch("wield staff\n", game=True, state=True)
        assert panel_value(reply["screen"], "Hand") == "staff · novice"
        assert reply["game"]["player"]["practice"] == practice
        reply = isle.batch("wield sword\n", "chant shield\n", "cast\n", game=True, interrupt=False)
        player = reply["game"]["player"]
        assert 0 < player["focus"] < player["max_focus"]
        assert player["practice"] == practice, "safe spell toggles teach nothing"
        focus = player["focus"]
        waiting = reply["round"]
        reply = isle.batch("wait\n" * 8, "right", "left", game=True, interrupt=False)
        returned = focus_returned(waiting, reply["round"])
        assert returned > 0, "eight rounds of waiting pass at least one Focus tick"
        assert reply["game"]["player"]["focus"] == focus + returned, "waiting returns Focus only on the slow tick"
        assert reply["game"]["player"]["practice"] == practice
        save = reply["game"]
        assert json.loads(isle.save.read_text()) == save
        assert isle.quit() == 0
    with tempfile.TemporaryDirectory(prefix="isle-progression-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            resumed = isle.request("state", game=True)["game"]
            assert resumed == save
            reply = isle.batch("rest\n" * 8, game=True, interrupt=False)
            player = reply["game"]["player"]
            assert player["focus"] == player["max_focus"]
            assert player["practice"] == practice and player["xp"] == 0
            assert isle.quit() == 0
    print("PASS progression: name-only creation, practice panel, skills, shared Focus, explicit recovery, save/reload")


if __name__ == "__main__":
    progression()
