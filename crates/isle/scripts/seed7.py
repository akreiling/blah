"""Play seed 7's starting town and the Deep beneath it with a frail fighter (12 health, armor class 10).

Run after `cargo build -p isle`:  python3 crates/isle/scripts/seed7.py
Uses the omniscient maps to plan walks, and only the window's panel to make combat decisions.
"""

import collections

from isle import Isle, hp, sighted, panel_value
from smoke import route


def say(result, label):
    news = " / ".join(line["text"] for line in result["news"])
    print(f"[{result['round']:>3}] {label}: {news}" + ("  (interrupted)" if result["interrupted"] else ""))
    return result


def walk(isle, route, label):
    """Walk a planned town route; stop rather than continuing after an unexpected interruption."""
    for leg in route:
        result = say(isle.batch(leg), label)
        if result["interrupted"]:
            raise RuntimeError(f"{label} was interrupted: {sighted(isle.screen())}")


def farthest(level, start):
    """The walkable cell farthest from start by cardinal steps."""
    todo, dist = collections.deque([start]), {start: 0}
    while todo:
        x, y = todo.popleft()
        for nxt in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]:
            if 0 <= nxt[0] < level["width"] and 0 <= nxt[1] < level["height"] and nxt not in dist \
                    and level["terrain"][nxt[1]][nxt[0]] in ".<":
                dist[nxt] = dist[(x, y)] + 1
                todo.append(nxt)
    return max(dist, key=lambda p: (dist[p], p))


def position(isle):
    return tuple(isle.request("state", state=False, game=True)["game"]["player"]["pos"])


def explore(isle, level, retreat_at):
    """Walk toward the far end of the Deep, fighting what comes. Returns 'deep', 'retreat', or 'died'."""
    stairs = tuple(level["stairs_up"])
    for key in route(level, stairs, farthest(level, stairs)):
        for _ in range(40):
            screen = isle.screen()
            seen = sighted(screen)
            if hp(screen)[0] <= retreat_at:
                for step in route(level, position(isle), stairs):
                    if any(line["tone"] == "Death" for line in say(isle.batch(step), "flee")["news"]):
                        return "died"
                say(isle.batch("<"), "retreat")
                return "retreat"
            near = [s for s in seen if s[2] <= 1]
            if near:
                result = say(isle.batch(f"attack {near[0][0].lower()}\n"), f"{near[0][1]} beside")
            elif any("hunting" in s[3] for s in seen):
                result = say(isle.batch("wait\n"), f"{seen[0][1]} at {seen[0][2]}")
            else:
                break
            if any(line["tone"] == "Death" for line in result["news"]):
                return "died"
        if any(line["tone"] == "Death" for line in say(isle.batch(key), "deeper")["news"]):
            return "died"
    return "deep"


def main():
    with Isle(seed=7) as isle:
        say(isle.batch("Wren\n", "fighter\n"), "create")
        level = isle.request("state", map="rows")["map"]
        town = next(t for t in level["towns"] if t["name"] == level["name"])
        start, stairs, temple = (tuple(town[k]) for k in ["start", "stairs_down", "temple"])
        walk(isle, route(level, start, stairs), "to the stairs")
        for trip in range(4):
            say(isle.batch(">"), "descend")
            deep = isle.request("state", state=False, map="rows")["map"]
            print(f"trip {trip + 1}: {deep['name']}")
            # Retreat before another strong blow can kill this frail character.
            outcome = explore(isle, deep, retreat_at=6)
            print(f"trip {trip + 1}: {outcome}; {panel_value(isle.screen(), 'Level')}; HP {hp(isle.screen())}")
            if outcome != "retreat":
                break
            walk(isle, route(level, stairs, temple), "to the temple")
            say(isle.batch("heal\n"), "heal")
            walk(isle, route(level, temple, stairs), "back to the stairs")
        screen = isle.screen()
        print("\n".join(screen["view"]))
        print("\n".join(screen["panel"]))
        isle.quit()


if __name__ == "__main__":
    main()
