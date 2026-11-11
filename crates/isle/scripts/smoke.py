"""Regression smoke for the control socket against a real Isle window. Run after `cargo build -p isle`."""

import collections
import json
import pathlib
import tempfile
import time

from isle import Isle, creature, hp, sighted


def route(level, start, goal):
    """Cardinal arrow keys from start to goal over walkable terrain."""
    walkable = set(".,T+<>SH")
    ox, oy = level.get("origin", [0, 0])
    todo, previous = collections.deque([start]), {start: None}
    while todo:
        pos = todo.popleft()
        if pos == goal:
            break
        for dx, dy, key in [(0, 1, "down"), (1, 0, "right"), (-1, 0, "left"), (0, -1, "up")]:
            nxt = (pos[0] + dx, pos[1] + dy)
            x, y = nxt[0] - ox, nxt[1] - oy
            if 0 <= x < level["width"] and 0 <= y < level["height"] and nxt not in previous and level["terrain"][y][x] in walkable:
                previous[nxt] = (pos, key)
                todo.append(nxt)
    keys, pos = [], goal
    while pos != start:
        pos, key = previous[pos]
        keys.append(key)
    return keys[::-1]


def turn_based():
    with Isle(seed=7) as isle:
        reply = isle.request("state")
        assert set(reply) == {"steps", "interrupted", "round", "news", "screen"}, reply.keys()
        assert len(json.dumps(reply)) < 4096, "the default state is what the window shows, not the whole island"
        assert isle.batch("A>,")["steps"] == 3 and isle.screen()["prompt"]["text"] == "A>,"
        isle.batch("escape", "Wren\n", "fighter\n")
        screen = isle.screen()
        assert len(screen["view"]) == 13 and all(len(row) == 26 for row in screen["view"])
        assert screen["view"][6][12:14] == "@ " and hp(screen) == (12, 12) and not sighted(screen)
        reply = isle.request("key", key="up")
        assert set(reply) == {"steps", "interrupted", "round", "news"}, "input replies carry no state unless asked"

        level = isle.request("state", state=False, map="rows")["map"]
        town = next(t for t in level["towns"] if t["name"] == level["name"])
        stairs = town["stairs_down"]
        sx, sy = stairs[0] - level["origin"][0], stairs[1] - level["origin"][1]
        assert (level["width"], level["height"], level["stairs_down"]) == (50, 28, stairs)
        assert level["terrain"][sy][sx] == ">" and level["view"][sy][2*sx:2*sx+2] == "  ", "terrain is omniscient, view is not"
        cells = isle.request("state", state=False, map="cells")["map"]["cells"]
        assert cells[sy][sx]["tile"] == "StairsDown" and not cells[sy][sx]["remembered"]

        isle.batch("look", "ctrl-a")
        reply = isle.batch("right", ">,<", state=True)
        assert reply["screen"]["prompt"] == {"text": ">,<look", "cursor": 3}, "arrows keep the draft; punctuation edits it"
        assert reply["round"] == 2
        isle.batch("escape", "drop potion\n")
        before = isle.request("state")["round"]
        reply = isle.batch(",", game=True)
        assert reply["round"] == before + 1 and reply["game"]["player"]["sack"] == ["Potion"]

        start = tuple(reply["game"]["player"]["pos"])
        keys = route(level, start, tuple(stairs))
        began = time.perf_counter()
        reply = isle.batch(*keys, ">", "<", game=True, interrupt=False)
        elapsed = time.perf_counter() - began
        assert not reply["interrupted"] and reply["steps"] == len(keys) + 2
        assert reply["game"]["player"]["at"] == "Surface" and reply["game"]["player"]["pos"] == stairs
        assert any("descend into the Deep beneath" in line["text"] for line in reply["news"])
        assert json.loads(isle.save.read_text()) == reply["game"]
        print(f"PASS turn-based: {len(keys) + 2} keystrokes, {len(keys) + 2} rounds in one batch, {elapsed * 1000:.1f} ms")
        save = json.loads(isle.save.read_text())
        assert isle.quit() == 0 and not isle.socket.exists()
    return save, town


def interruption(save, town):
    """A rat placed just out of sight interrupts a batch of moves toward it."""
    save["player"]["pos"] = town["start"]
    x, y = town["start"]
    rat = creature(save, "Rat", "Surface", [x + 7, y], 5)
    save["creatures"], save["spawns"] = [rat], []
    with tempfile.TemporaryDirectory(prefix="isle-save-", dir="/tmp") as directory:
        path = pathlib.Path(directory) / "save.json"
        path.write_text(json.dumps(save))
        with Isle(save=path) as isle:
            reply = isle.batch(*["right"] * 5)
            assert reply["interrupted"] and reply["steps"] == 1 and reply["round"] == save["round"] + 1, reply
            assert isle.batch(*["right"] * 2, interrupt=False)["steps"] == 2
            assert isle.quit() == 0
    print("PASS interruption: a new creature stops a batch after the step that revealed it")


def clocked():
    with Isle(seed=7, round_seconds=0.5) as isle:
        isle.batch("Wren\n", "fighter\n", "pause\n")
        state = isle.request("state", game=True)
        assert state["screen"]["panel"][-4].endswith("PAUSED")
        before = state["round"]
        isle.batch(">")
        assert "Next down" in isle.screen()["panel"] and isle.request("state")["round"] == before
        isle.batch("stop\n", "right", "pause\n")
        deadline = time.monotonic() + 3
        while isle.request("state")["round"] == before:
            assert time.monotonic() < deadline, "clock did not advance"
            time.sleep(0.02)
        reply = isle.batch("pause\n", game=True)
        pos = state["game"]["player"]["pos"]
        assert reply["game"]["player"]["pos"] == [pos[0] + 1, pos[1]]
        assert isle.quit() == 0
    print("PASS clocked: shortcuts and arrows queue, pause and stop act at once, the clock plays the queue")


if __name__ == "__main__":
    interruption(*turn_based())
    clocked()
