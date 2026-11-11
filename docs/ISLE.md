# Isle

Isle is a second, separate game in this workspace: a solo island adventure in the manner of *Island of
Kesmai* (Kesmai, 1985). It shares no rules with blah. Its maps, creatures, and text are original.

- `crates/isle-core`: the rules. Pure and deterministic; the whole `Game` serializes, dice included.
- `crates/isle`: the GPUI window. Run it with `just isle`.

## How it plays

You type a command and press Enter. Each action resolves one round immediately: your command first,
then every creature in turn. The world waits while you think or type. `wait` (`wa`) lets a round pass
while you *stand ready*, striking back at anything hunting you from the next cell.

- Movement: `n s e w ne nw se sw`, up to three steps a round (`n n e`).
- Fighting: `attack <who>`, `nock` then `shoot <who>`. A target is the letter the IN SIGHT list shows
  (`shoot a`), a name (`shoot orc`), or an ordinal and name (`sh 2 gob`).
- Magic: `chant <spell>` one round, `cast [who]` the next.
- Rounds chain with `;` (`nock; shoot a`), at most five per line. A chain stops if you are hurt, die,
  or a new creature comes into sight. The rest is discarded, not queued for later.
- Enter on an empty line repeats the last command line; Up recalls it. `quit` saves and leaves.
  `look`, `inventory`, `list`, and `help` take no time.

For the original clock-driven pace, use `just isle --round 10` (0.5–60 seconds per round). Commands
queue up to five rounds ahead instead of resolving immediately. With nothing queued you stand ready.
`pause` toggles the clock and `stop` clears the queue; both take effect immediately. Without a clock,
`pause` is unnecessary.

## Callings

Following Legends of Kesmai's classes:

| Calling | Health | Notes |
|---|---|---|
| Fighter | most | short sword, leather, best melee |
| Thief | low | dagger, shortbow and arrows; `hide`, then triple damage in melee, double with arrows |
| Thaumaturge | good | warrior-priest: mace, leather; `heal` (self) and `stun` (target) |
| Wizard | least | staff, no armor; `spark` (target) and `shield` (self, turns arrows); metal armor stops chanting |

Kesmai's martial artist and knight are not in yet.

## The island

Harrowmere holds the outfitter (`buy`, `sell`, `list`) and the temple (`heal` for five gold a level;
the fallen wake here, leaving their gold where they fell). Woods beasts keep within seven cells of their
lairs, so the town square is safe. South of town the old watchtower's stairs lead to the Undercroft:
rats, goblins, goblin archers, orcs, skeletons (arrows do half), and the pit troll in its lair.

Saves live in the platform data directory (`isle/save.json`) and are rewritten after every round. `--new`
starts another character and keeps the old save beside it; `--save PATH` and `--seed N` pick the file and
the island's dice.
