"""Real-window checks for the techniques skilled practice unlocks.

Run after `cargo build -p isle`. Core tests fix each technique's arithmetic; this drives a real window
to check that the skills screen prices them, that they fire with no command of their own, and that a
readied advantage reaches the panel.
"""

import json
import pathlib
import tempfile

from isle import Isle, creature

# Evidence that reaches skilled, and the order the save stores practice in.
SKILLED = 4_000
PRACTICE = ["dagger", "blades", "blunt", "staff", "archery", "evocation", "warding", "restoration", "binding",
            "stealth", "vitality", "focus"]


def learn(save, practice):
    save["player"]["practice"]["skills"][PRACTICE.index(practice)] = {"practice": SKILLED}


def skills(isle):
    """The skills screen: its panel rows and the benefit lines it logs."""
    reply = isle.batch("skills\n", game=True, state=True)
    isle.batch("escape")
    return reply["screen"]["panel"], " | ".join(line["text"] for line in reply["news"])


def novice_prices_name_the_rank_to_come(path):
    with Isle(seed=7) as isle:
        save = isle.batch("Wren\n", game=True)["game"]
        panel, news = skills(isle)
        assert "blades: novice" in panel and "archery: novice" in panel
        for promise in ["feint at skilled", "parry at skilled", "stagger at skilled", "sweep at skilled",
                        "steady shot at skilled"]:
            assert promise in news, (promise, news)
        assert "Focus)" not in news, "nothing is priced before it is earned"
        assert isle.quit() == 0
    path.write_text(json.dumps(save))
    return save


def a_skilled_blade_parries_and_ripostes(save, path):
    """A troll that keeps swinging: the blade turns a blow on its own, and the panel readies the answer."""
    x, y = save["player"]["pos"]
    fight = json.loads(json.dumps(save))
    learn(fight, "blades")
    # Health and a long-lived opponent, so one fight shows the whole exchange.
    fight["player"]["hp"] = fight["player"]["max_hp"] = 120
    fight["creatures"] = [creature(fight, "Troll", "Surface", [x + 1, y], 5_000, hunting=True,
                                   last_seen=[x, y])]
    fight["spawns"] = []
    path.write_text(json.dumps(fight))
    with Isle(save=path) as isle:
        panel, news = skills(isle)
        assert "blades: skilled" in panel
        assert "parry (2 Focus): once a round in single combat" in news, news
        readied, riposte, focus = None, None, fight["player"]["focus"]
        for _ in range(60):
            reply = isle.batch("attack troll\n", game=True, state=True, interrupt=False)
            lines = [line["text"] for line in reply["news"]]
            focus = min(focus, reply["game"]["player"]["focus"])
            if any("You parry the pit troll's blow" in line for line in lines):
                readied = reply["screen"]["panel"]
            if any("You riposte, hitting the pit troll" in line for line in lines):
                riposte = lines
            if readied and riposte:
                break
        assert readied, "a skilled blade turns one of sixty troll blows without being told to"
        assert any("riposte ready" in row for row in readied), readied
        assert riposte, "the readied riposte lands on the next attack"
        assert focus < fight["player"]["focus"], "techniques are paid for in Focus"
        assert isle.quit() == 0


def a_skilled_archer_steadies_a_distant_shot(save, path):
    """Three cells of quiet: every arrow that lands is a steady one."""
    x, y = save["player"]["pos"]
    hunt = json.loads(json.dumps(save))
    learn(hunt, "archery")
    hunt["player"]["arrows"] = 60
    # Held still, so the shot keeps its distance and nothing hunts from beside.
    hunt["creatures"] = [creature(hunt, "Troll", "Surface", [x + 3, y], 5_000, stunned=10_000)]
    hunt["spawns"] = []
    path.write_text(json.dumps(hunt))
    with Isle(save=path) as isle:
        panel, news = skills(isle)
        assert "archery: skilled" in panel
        assert "steady shot (1 Focus): +2 to hit and damage" in news, news
        hits = []
        for _ in range(30):
            reply = isle.batch("nock\n", "shoot troll\n", game=True, interrupt=False)
            hits += [line["text"] for line in reply["news"] if "Your arrow" in line["text"] and "strikes" in line["text"]]
            if len(hits) >= 3:
                break
        assert len(hits) >= 3, hits
        assert all("steady strikes" in line for line in hits), hits
        assert isle.quit() == 0


def techniques():
    with tempfile.TemporaryDirectory(prefix="isle-techniques-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        save = novice_prices_name_the_rank_to_come(path)
        a_skilled_blade_parries_and_ripostes(save, path)
        a_skilled_archer_steadies_a_distant_shot(save, path)
    print("PASS techniques: priced at novice, automatic parry and riposte with a panel state, steady shots")


if __name__ == "__main__":
    techniques()
