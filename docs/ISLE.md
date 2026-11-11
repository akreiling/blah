# Isle

Isle is a second, separate game in this workspace: a solo island adventure in the manner of *Island of
Kesmai* (Kesmai, 1985). It shares no rules with blah. Its maps, creatures, and text are original.

- `crates/isle-core`: the rules. Pure and deterministic; the whole `Game` serializes, dice included.
- `crates/isle`: the GPUI window. Run it with `just isle`, or `just isle --round 4` for faster rounds.

## How it plays

You type a command and press Enter. Rounds resolve on a clock, ten seconds by default as in the
original: your queued command first, then every creature in turn. With nothing queued you *stand
ready* and strike back at anything hunting you from the next cell.

- Movement: `n s e w ne nw se sw`, up to three steps a round (`n n e`).
- Fighting: `attack <who>`, `nock` then `shoot <who>`. A target is the letter the IN SIGHT list shows
  (`shoot a`), a name (`shoot orc`), or an ordinal and name (`sh 2 gob`).
- Magic: `chant <spell>` one round, `cast [who]` the next.
- Rounds chain with `;` (`nock; shoot a`), at most five ahead. Enter on an empty line repeats the last
  command; Up recalls it. `pause`, `stop` (clear the queue), and `quit` act at once, as do `look`,
  `inventory`, `list`, and `help`.

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
