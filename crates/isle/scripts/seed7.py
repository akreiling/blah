"""Play seed 7's generated town and dungeon with a frail fighter (12 health, armor class 10).

Run after `cargo build -p isle`:  python3 crates/isle/scripts/seed7.py
Uses the omniscient town map to plan walks, and only the window's panel to make combat decisions.
"""

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


def fight(isle, retreat_at):
    """Stand on the stairs: wait for hunters to come, attack the nearest. Returns 'clear', 'retreat', or 'died'."""
    for _ in range(60):
        screen = isle.screen()
        seen = sighted(screen)
        if not seen:
            return "clear"
        if hp(screen)[0] <= retreat_at:
            say(isle.batch("<"), "retreat")
            return "retreat"
        letter, name, distance, _ = seen[0]
        result = say(isle.batch(f"attack {letter.lower()}\n" if distance <= 1 else "wait\n"), f"{name} at {distance}")
        if any(line["tone"] == "Death" for line in result["news"]):
            return "died"
    return "stalemate"


def main():
    with Isle(seed=7) as isle:
        say(isle.batch("Wren\n", "fighter\n"), "create")
        level = isle.request("state", map="rows")["map"]
        town = next(t for t in level["towns"] if t["name"] == level["name"])
        start, stairs, temple = (tuple(town[k]) for k in ["start", "stairs_down", "temple"])
        walk(isle, route(level, start, stairs), "to the stairs")
        for trip in range(4):
            say(isle.batch(">"), "descend")
            # Retreat before another strong blow can kill this frail character.
            outcome = fight(isle, retreat_at=7)
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
