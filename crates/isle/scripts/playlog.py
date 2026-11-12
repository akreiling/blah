"""Checks the playtime log against a real Isle window. Run after `cargo build -p isle`."""

import json
import pathlib
import tempfile
import time

from isle import Isle


def entries(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def logged():
    with tempfile.TemporaryDirectory(prefix="isle-playlog-", dir="/tmp") as directory:
        save = pathlib.Path(directory) / "save.json"
        log = pathlib.Path(directory) / "save.playlog.jsonl"
        with Isle(seed=7, save=save, flags=["--playlog"]) as isle:
            isle.batch("escape", "Wren\n")
            time.sleep(1.0)
            assert isle.batch("skills\n")["round"] == 0, "skills takes no time"
            time.sleep(1.0)
            first = isle.batch("wait\n")["round"]
            time.sleep(0.5)
            second = isle.batch("wait; wait\n")["round"]
            assert isle.quit() == 0
        lines = entries(log)
        assert lines[0]["session"] > 0 and lines[0]["seed"] == 7, lines[0]
        played = lines[1:]
        assert [(e["from"], e["to"]) for e in played] == [(0, first), (first, second)], played
        assert second - first == 2 and played[1]["line"] == "wait; wait"
        assert 2.0 <= played[0]["active_s"] < 15, "time on skills carries into the next round"
        assert 0.5 <= played[1]["active_s"] < 5 and played[1]["idle_s"] == 0
        assert played[0]["depth"] == 0 and not played[0]["died"] and played[0]["level"] == 1

        with Isle(save=save, flags=["--playlog"]) as isle:
            third = isle.batch("wait\n")["round"]
            assert isle.quit() == 0
        lines = entries(log)
        assert len(lines) == 5 and "session" in lines[3], "a resumed game appends a new session"
        assert (lines[3]["round"], lines[4]["from"], lines[4]["to"]) == (second, second, third)
    print(f"PASS playlog: {len(lines)} lines, rounds 0→{third}, {played[0]['active_s']:.2f}s on the first round")


def unlogged():
    with Isle(seed=7) as isle:
        isle.batch("escape", "Wren\n", "wait\n")
        assert isle.quit() == 0
        assert not isle.save.with_suffix(".playlog.jsonl").exists(), "scripted windows keep no log"
    print("PASS playlog: a controlled window without --playlog writes nothing")


if __name__ == "__main__":
    logged()
    unlogged()
