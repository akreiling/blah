"""Drive a real Isle window through its opt-in control socket.

    with Isle(seed=7) as isle:
        isle.batch("Wren\n")
        print(isle.screen()["panel"])

Requests return only what they did (steps run, interruption, round, new log lines) unless asked for more.
"""

import json
import pathlib
import socket
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
BINARY = ROOT / "target/debug/isle"
KEYS = {"up", "down", "left", "right", "enter", "escape", "backspace", "delete", "home", "end"}


class Isle:
    """A fresh Isle process with a private save and socket, removed on exit."""

    def __init__(self, seed=7, round_seconds=None, save=None):
        self.seed, self.round_seconds, self.save_path = seed, round_seconds, save

    def __enter__(self):
        self._dir = tempfile.TemporaryDirectory(prefix="isle-ctl-", dir="/tmp")
        root = pathlib.Path(self._dir.name)
        self.socket = root / "control.sock"
        self.save = pathlib.Path(self.save_path) if self.save_path else root / "save.json"
        args = [str(BINARY), "--save", str(self.save), "--seed", str(self.seed), "--control", str(self.socket)]
        if self.round_seconds:
            args += ["--round", str(self.round_seconds)]
        self._log = (root / "app.log").open("w+")
        self.app = subprocess.Popen(args, stdout=self._log, stderr=self._log)
        deadline = time.monotonic() + 15
        while not self.socket.exists():
            if self.app.poll() is not None:
                raise RuntimeError("isle exited: " + self.output())
            if time.monotonic() > deadline:
                raise TimeoutError("isle control socket did not appear")
            time.sleep(0.02)
        return self

    def __exit__(self, *_):
        try:
            if self.app.poll() is None:
                self.app.terminate()
                self.app.wait(timeout=10)
        finally:
            self._log.close()
            self._dir.cleanup()

    def output(self):
        self._log.flush()
        self._log.seek(0)
        return self._log.read()

    def request(self, op, **fields):
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(10)
            client.connect(str(self.socket))
            client.sendall((json.dumps({"op": op, **fields}) + "\n").encode())
            reply = json.loads(client.makefile("r").readline())
        if "error" in reply:
            raise RuntimeError(reply["error"])
        return reply

    def batch(self, *steps, **options):
        """Steps are typed text, or key names such as "right" and "ctrl-a" (any string with a dash or in KEYS)."""
        encoded = [{"key": s} if s in KEYS or ("-" in s and len(s) > 1 and " " not in s) else {"text": s} for s in steps]
        return self.request("batch", steps=encoded, **options)

    def screen(self, **options):
        return self.request("state", **options)["screen"]

    def quit(self):
        self.request("text", text="quit\n")
        return self.app.wait(timeout=10)

def hp(screen):
    """Current and maximum health from the panel."""
    line = next(l for l in screen["panel"] if l.startswith("HP "))
    now, most = line[3:].split("/")
    return int(now), int(most)


def sighted(screen):
    """[(letter, name, distance, notes)] from the panel's IN SIGHT list; empty when nothing stirs."""
    panel = screen["panel"]
    rows = panel[panel.index("IN SIGHT") + 1:] if "IN SIGHT" in panel else []
    found = []
    grouped = []
    for row in rows:
        if row == "Nothing stirs.":
            continue
        if row.startswith(" ") and grouped:
            grouped[-1] += " " + row.strip()
        else:
            grouped.append(row)
    for row in grouped:
        letter, rest = row.split(" ", 1)
        words = rest.split(" ")
        at = next(i for i, w in enumerate(words) if w.isdigit())
        found.append((letter, " ".join(words[:at]), int(words[at]), " ".join(words[at + 1:])))
    return found


def creature(save, kind, at, pos, hp, **fields):
    """A fixed-table creature for a save fixture, with a fresh id from the save."""
    save["next_id"] += 1
    made = dict(id=save["next_id"] - 1, kind=kind, wild=None, energy=0, at=at, pos=list(pos), home=list(pos), hp=hp,
                hunting=False, last_seen=None, lost=0, stunned=0, spawn=None)
    return {**made, **fields}


def panel_value(screen, label):
    return next((l[len(label) + 1:] for l in screen["panel"] if l.startswith(label + " ")), None)
