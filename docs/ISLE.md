# Isle

Isle is a solo island adventure in the manner of *Island of Kesmai* (Kesmai, 1985), being merged with
blah one system at a time. Its wilderness now uses blah's seeded procedural terrain and evolved regional
life. Towns are generated on dry ground, each over its own generated Deep, floor after floor down.
Callings are still Isle's original system.

- `crates/isle-core`: the rules. Pure and deterministic; the whole `Game` serializes, dice included.
- `crates/isle`: the GPUI window. Run it with `just isle`.

## How it plays

You type a command and press Enter. Each action resolves one round immediately: your command first,
then every creature in turn. The world waits while you think or type. `wait` (`wa`) lets a round pass
while you *stand ready*, striking back at anything hunting you from the next cell.

- Movement: arrow keys take one step without clearing the command line or moving its cursor.
  Typed directions `n s e w ne nw se sw` still work, up to three steps a round (`n n e`).
- With an empty command line, `<` goes upstairs, `>` downstairs, and `,` picks up everything at your
  feet (subject to sack space). These act without Enter and take one round, just like `up`, `down`,
  and `get all`. While editing a command or creating a character, they insert punctuation instead.
- Fighting: `attack <who>`, `nock` then `shoot <who>`. A target is the letter the IN SIGHT list shows
  (`shoot a`), a name (`shoot orc`), or an ordinal and name (`sh 2 gob`).
- Magic: `chant <spell>` one round, `cast [who]` the next.
- Rounds chain with `;` (`nock; shoot a`), at most five per line. A chain stops if you are hurt, die,
  or a new creature comes into sight. The rest is discarded, not queued for later.
- Enter on an empty line repeats the last typed command line, not arrow movement. `quit` saves and leaves.
  `look`, `inventory`, `list`, and `help` take no time.

For the original clock-driven pace, use `just isle --round 10` (0.5–60 seconds per round). Commands
queue up to five rounds ahead instead of resolving immediately. With nothing queued you stand ready.
`pause` toggles the clock and `stop` clears the queue; both take effect immediately. Without a clock,
`pause` is unnecessary.

### Editing commands

Printable keys, spaces, and punctuation insert at the cursor. Editing takes no turns.

| Keys | Action |
|---|---|
| Option-Left / Option-Right (Alt on other keyboards), Alt-B / Alt-F | Back / forward one word |
| Ctrl-B / Ctrl-F | Back / forward one character |
| Ctrl-A / Ctrl-E, Home / End | Start / end of line |
| Backspace / Delete, Ctrl-H / Ctrl-D | Delete before / at the cursor |
| Ctrl-W | Kill the preceding whitespace-delimited word |
| Option-Backspace / Alt-D | Kill back / forward one word |
| Ctrl-U / Ctrl-K | Kill to the start / end of the line |
| Ctrl-Y | Yank (restore) the last killed text |
| Ctrl-P / Ctrl-N | Previous / next typed command; next past the newest restores your draft |
| Escape | Clear the line |

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

Beyond towns, the surface continues into blah's seeded biomes, with trees that hide what
lies beyond and rocks that block movement and sight. Deep water and snow peaks are impassable; shallow
water can be waded. Explored wilderness stays remembered across saves, including west and north of town.
Every town guarantees an outfitter (`buy`, `sell`, `list`), a temple (`heal` for five gold a level),
a revival spot, dungeon stairs, and a safe central arrival square. Paths connect all services and the
town's edges. Names, building sizes, service placement, and layout orientation vary with the seed.
The entire town footprint sits on passable dry land; ocean sectors need not contain a town.
The starting town is found near the world's origin, and more towns appear as you explore.
`look` gives distances and directions to the current town's services.

Entering a temple makes it your revival point, even if you do not need healing. The fallen wake at the
last temple visited, leaving their gold where they fell. Evolved wildlife lives outside town footprints
and cannot attack into it. As you explore, individuals arrive beyond sight from their own region's species
pools, with productivity limiting the population to 2–12. Distant wildlife is forgotten; regional life
regenerates exactly from the seed.

Individual genomes determine health, armor, attack, speed, senses, and terrain access. Skittish creatures
flee (cornered, provoked ones fight back), territorial creatures defend home or retaliate, and aggressive
ones hunt. They stay within seven cells of home. Faster creatures bank speed between rounds and can take
up to two actions per round. The IN SIGHT panel shows their regional names and temperaments; target those
names or the usual letters. Wildlife drops coins and sometimes scavenged gear, with stronger creatures
leaving better finds.

Every town's stairs lead down to its own Deep, one 64×32 floor per depth with no bottom. Each floor is a
maze of rooms joined by corridors, the same each time for that town and depth. It has two safe landings:
one at the stairs up, where you arrive, and one at the stairs down, in the room farthest away. Deep life
never enters a landing or attacks into one. Climbing up from a floor puts you on the lower landing of the
floor above. What you have seen, what lies on the floor, and who lives there persist per floor across
visits and saves; a floor you are not on waits unchanged.

Deep life is evolved, not authored. The Deep is a fitness environment like a biome: cool, damp, unchanging
stone with little food, which favors small, bold, well-legged creatures and barely rewards coloring. Each
floor grows two to four of blah's clades, chosen by how well they could fit, evolved from their clade base
for the same number of generations as surface regions, and named for the Deep (Hollow Tickler, Crag
Slither). Every floor down is colder and hungrier, which selects bigger, bolder bodies, and its survivors
are hardened besides: below the first floor, each depth adds a third of their health, +1 to hit and to top
damage, and armor every other floor. Experience grows faster than that. A typical creature goes from 8
health, +2 to hit, and 16 experience at depth 1 to about 27 health, +7, and 140 at depth 5, and 59 health,
+13, 7–14 damage, and about 510 experience at depth 10. They follow the same genome-driven rules as
wildlife, within fourteen cells of home.

Each room but the landings has a spawn point that refills sixty rounds after its creature dies, out of
sight. The remaining room farthest from the stairs up is a den: its keeper is the strongest of four draws
and sits on a hoard, returning four hundred rounds after it falls.

Treasure is random and scales with what it took to get. Every floor is stocked once, when first reached,
with one to six piles; deeper floors have more and richer ones. A kill drops coins, and evolved life may
also drop gear it gathered: the more experience it is worth, the likelier the drop and the better the gear.
A find never holds gear priced under a sixth of its value, so deep finds run to longbows, chainmail, plate,
and the ring of warding rather than daggers. Den hoards are worth eight times the keeper's experience and
always include a potion.

Saves live in the platform data directory (`isle/save.json`) and are rewritten after every round. `--new`
starts another character and keeps the old save beside it; `--save PATH` and `--seed N` pick the file and
the island's dice.

Save compatibility is not maintained during this merge. Use `just isle --new` if an older save will not load.

## Agent control and inspection

Launch with `just isle --control /absolute/private-directory/isle.sock` to enable a local Unix debug
socket. Without `--control` there is no listener. The socket is owner-only (mode `0600`), existing paths
are never replaced, and quitting removes it. A controlled window opens without taking keyboard focus, so
typing elsewhere does not leak into the game.

Open a connection, send one JSON object followed by a newline, read one JSON line back, then close.
Requests are limited to 64 KiB and handled one at a time.

```json
{"op":"state"}
{"op":"text","text":"Wren\nfighter\n"}
{"op":"key","key":"ctrl-a"}
{"op":"batch","steps":[{"text":"s s s\n"},{"key":"right"},{"text":">"}]}
```

`text` delivers each printable character through the keyboard handler; a newline means Enter. `key` takes
GPUI key notation (`right`, `alt-left`, `ctrl-y`). A `batch` runs its steps in order as one request; it
does not bypass character creation, editing, punctuation shortcuts, or round rules. Like a `;` chain, a
batch stops after any keystroke that hurts you, kills you, or brings a new creature into sight; send
`"interrupt": false` to run every step regardless.

Every reply says what the request did: `steps` (keystrokes delivered), `interrupted`, `round`, and `news`
(log lines it produced). Anything more is opt-in:

- `"state": true` (the default for `op: state`) adds `screen`: what the window shows. That is the 13×13
  `view` as two-character cells, the `panel` lines, the last ten `log` lines, and the `prompt` text and
  cursor. About 1 KB.
- `"map": "rows"` adds the current level's `terrain` (one character per cell, as in `map.rs`) and `view`
  (blank where unseen), with size, `origin: [x, y]`, and stair positions. In town and underground this is
  a 50×28 window starting at that town's northwest corner, or the whole 64×32 floor of a Deep at origin `[0, 0]`.
  Outside town it is a 50×28 window centered on the character. `towns` lists nearby generated settlements
  and the starting town, with their names, footprints, arrival points, shops, temples, and stairs.
  Row/column indices are relative to `origin`, while stair positions remain level coordinates.
  `"map": "cells"` gives every
  cell's tile, zone, walkability, visibility, and memory instead (~150 KB).
- `"game": true` adds the whole serializable game, as saved.

The map and game options are **omniscient**: they reveal what the character has not discovered. Errors
return `{"error":"..."}`. A timeout leaves the outcome uncertain, so inspect state before repeating.

`crates/isle/scripts/isle.py` wraps this in Python: `Isle(seed=7)` launches a private window and save,
`batch(...)`, `screen()`, `quit()`, and panel parsers `hp` and `sighted`. Run these after
`cargo build -p isle`:

- `python3 crates/isle/scripts/smoke.py` checks the protocol against a real window in both pacing modes.
- `python3 crates/isle/scripts/wilderness.py` checks leaving town, biome rendering, local map origins,
  wilderness memory, and save/reload.
- `python3 crates/isle/scripts/wildlife.py` checks evolved population, reload, the IN SIGHT panel,
  and shooting an actual spawned individual by its regional name.
- `python3 crates/isle/scripts/towns.py` visits two generated towns, exercises their services, and checks
  stair returns and revival after reload.
- `python3 crates/isle/scripts/deep.py` descends below two towns and to a second floor, and checks separate
  layouts, evolved life off the landings, tougher life deeper down, stair returns, and that each floor's life
  and memory persist across visits and reloads.
- `python3 crates/isle/scripts/seed7.py` plans routes from the map and fights from the window's panel. It
  walks to the stairs, heads for the far end of the Deep, and retreats to the temple to heal. Games are deterministic per seed and input, so its
  output repeats exactly.
