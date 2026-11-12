"""Summarize Isle's playtime log: real seconds per round, overall, by depth, and by command.

    python3 crates/isle/scripts/playtime.py [LOG]

LOG defaults to the playtime log beside the default save. Each window launched without --control
appends to it; gaps over a minute between inputs count as one minute.
"""

import collections
import json
import pathlib
import sys

HOURS_TARGET = 30


def default_log():
    home = pathlib.Path.home()
    for base in [home / "Library/Application Support", home / ".local/share"]:
        path = base / "isle/save.playlog.jsonl"
        if path.exists():
            return path
    return home / "Library/Application Support/isle/save.playlog.jsonl"


def characters(lines):
    """Entries grouped by character, a new one starting whenever the seed changes."""
    groups, current = [], None
    for entry in lines:
        if "session" in entry:
            if current is None or entry["seed"] != current["seed"]:
                current = {"seed": entry["seed"], "name": entry["name"], "sessions": 0, "entries": []}
                groups.append(current)
            current["sessions"] += 1
        elif current is not None:
            current["entries"].append(entry)
    return groups


def report(group):
    entries = group["entries"]
    rounds = sum(e["to"] - e["from"] for e in entries)
    active = sum(e["active_s"] for e in entries)
    idle = sum(e["idle_s"] for e in entries)
    print(f"{group['name']} (seed {group['seed']}): {group['sessions']} sessions, {rounds} rounds, "
          f"{active / 3600:.2f} active hours (+{idle / 3600:.2f} idle, not counted)")
    if not rounds:
        return
    pace = active / rounds
    last = entries[-1]
    print(f"  {pace:.1f} seconds per round; {3600 / pace:.0f} rounds per hour; "
          f"{HOURS_TARGET} hours at this pace is {HOURS_TARGET * 3600 / pace:,.0f} rounds")
    print(f"  now: round {last['to']}, depth {last['depth']}, level {last['level']}, {last['deaths']} deaths, "
          f"{last['gold']} gold")
    by_depth = collections.defaultdict(lambda: [0, 0.0, 0])
    by_word = collections.defaultdict(lambda: [0, 0.0])
    first_round = {}
    for e in entries:
        n = e["to"] - e["from"]
        d = by_depth[e["depth_before"]]
        d[0] += n
        d[1] += e["active_s"]
        d[2] += e["died"]
        first_round.setdefault(e["depth"], e["to"])
        word = (e["line"].split(";")[0].split() or ["(clock)"])[0].lower()
        by_word[word][0] += n
        by_word[word][1] += e["active_s"]
    print("  depth  rounds  sec/round  deaths  first reached")
    for depth in sorted(by_depth):
        n, s, deaths = by_depth[depth]
        print(f"  {depth:>5}  {n:>6}  {s / n:>9.1f}  {deaths:>6}  round {first_round.get(depth, '-')}")
    print("  most played commands (rounds, sec/round):")
    for word, (n, s) in sorted(by_word.items(), key=lambda kv: -kv[1][0])[:10]:
        print(f"    {word:<12} {n:>6}  {s / n:>6.1f}")


def main():
    path = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_log()
    if not path.exists():
        sys.exit(f"no playtime log at {path}; play Isle first")
    lines = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    print(f"Playtime log: {path}")
    for group in characters(lines):
        report(group)


if __name__ == "__main__":
    main()
