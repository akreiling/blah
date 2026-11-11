# Isle

Isle is a solo island adventure in the manner of *Island of Kesmai* (Kesmai, 1985), being merged with
blah one system at a time. Its wilderness now uses blah's seeded procedural terrain and evolved regional
life. Towns are generated on dry ground; the dungeon and callings are still Isle's original systems.

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
names or the usual letters. Wildlife drops small amounts of gold, not manufactured gear.

Town stairs currently share the authored Undercroft: rats, goblins, goblin archers, orcs, skeletons (arrows
do half), and the pit troll. Upstairs returns to the exact town entrance used, including after a reload.
Separate generated dungeons are the next integration step.

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
  a 50×28 window starting at that town's northwest corner, or the dungeon at origin `[0, 0]`.
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
- `python3 crates/isle/scripts/seed7.py` plans town routes from the map and fights from the window's panel. It walks to the stairs,
  fights from them, and retreats to the temple to heal. Games are deterministic per seed and input, so its
  output repeats exactly.
