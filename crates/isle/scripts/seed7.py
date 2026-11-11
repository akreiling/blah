"""A plan that knows seed 7: a frail fighter (12 health, armor class 10) and a skeleton by the stairs.

Run after `cargo build -p isle`:  python3 crates/isle/scripts/seed7.py
Uses only what the window shows, except the route, which was worked out from this seed's map.
"""

from isle import Isle, hp, sighted, panel_value

TO_STAIRS = ["s s s\n"] * 4 + ["e\n", "se\n", "s\n", "s\n"]
STAIRS_TO_TEMPLE = ["n n nw\n", "w n n\n"] + ["n n n\n"] * 3 + ["n e e\n"] + ["e e e\n"] * 3 + ["e\n", "n n n\n"]
TEMPLE_TO_STAIRS = ["s s s\n", "w w w\n", "w w w\n", "w w w\n", "w w w\n"] + ["s s s\n"] * 4 + ["e\n", "se\n", "s\n", "s\n"]


def say(result, label):
    news = " / ".join(line["text"] for line in result["news"])
    print(f"[{result['round']:>3}] {label}: {news}" + ("  (interrupted)" if result["interrupted"] else ""))
    return result


def walk(isle, route, label):
    """Walk a fixed route; on interruption, fall back to fighting what appeared, then carry on."""
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
        walk(isle, TO_STAIRS, "to the stairs")
        for trip in range(4):
            say(isle.batch(">"), "descend")
            # A skeleton hits for 2-7: leave while one more blow cannot kill. A skeleton and goblin together can
            # deal 13, more than this character's 12 health, so a camp at the stairs is not winnable at level 1.
            outcome = fight(isle, retreat_at=7)
            print(f"trip {trip + 1}: {outcome}; {panel_value(isle.screen(), 'Level')}; HP {hp(isle.screen())}")
            if outcome != "retreat":
                break
            walk(isle, STAIRS_TO_TEMPLE, "to the temple")
            say(isle.batch("heal\n"), "heal")
            walk(isle, TEMPLE_TO_STAIRS, "back to the stairs")
        screen = isle.screen()
        print("\n".join(screen["view"]))
        print("\n".join(screen["panel"]))
        isle.quit()


if __name__ == "__main__":
    main()
