# Isle

Isle is a solo island adventure inspired by *Island of Kesmai* (Kesmai, 1985) and the *Death Dealer*
books' Gath of Baal, being merged with blah one system at a time. Its wilderness uses blah's seeded procedural terrain and evolved regional
life. Towns are generated on dry ground, each over its own generated Deep, floor after floor down.
Characters are classless: practice grants power, while experience records adventuring rank.

Evolved creatures now appear as genome-generated pixel sprites in the map. Their clade determines the
body plan; individual genes determine size, limbs, covering, and color. Surface life keeps its home
biome's tint; Deep life takes a stone tint, except luminous bodies shine pale green. Each sprite carries
its target letter, matching IN SIGHT and commands such as `attack a`. Sprites stay crisp at display scale
and disappear when the creature leaves sight.

The whole map is painted in code at 32×32 per cell, after the offline design studies. Every pixel comes from
its world position, so cells never repeat and neighbors always agree. Natural ground (water, grass, sand,
snow, rock, and Deep caves) takes its shape from nearby cell centers through a smooth warp, so shores wobble,
corners round off, and a lone pond is a blob; town streets and walls keep square edges and straight masonry.
Each biome picks a look from its climate: warm lowlands (desert, savanna, jungle, badlands) are sunlit and
golden, cold, wet, or high land (tundra, taiga, swamp, hills, mountains, peaks) is cool and blue-green, and the
temperate rest is mossy. Looks meet in ragged bands across biome borders. Water and rock keep a little of
their biome's color. Marsh puddles and reeds, dark jungle growth, dry savanna patches, desert dunes, rock
relief, and snow drifts vary natural ground. Trees (conifers, broadleaves, canopies, acacias, and willows),
boulders, doors, stairs, services, and ground items are code-drawn objects in the biome's look; trees and
boulders vary by size and mirroring, with big crowns rising above their cell. Unknown neighbors never shape
a visible edge. Your character is a code-drawn figure: your highest weapon, magic, or stealth
practice rank shapes its clothing and headgear, while your current weapon, bow, armor, and ring appear
on the body. Skilled practice adds gold trim; a shadowed yellow ring marks your feet. Fixed-table creatures keep
their text glyphs, and the text map still represents you as `@`.
`crates/isle/assets/world/README.md` describes the renderer and its preview sheets.

The sky shifts slowly with the round between overcast, clear spells, and mist; sunlight and mist vary
in broad patches across the land. Grass tufts sway, water glitters over shaded depths, and mist wisps drift
while you think. Ragged, soft darkness marks the edges of sight without changing visibility, and stair
badges remain legible above your figure. Deep floors have seeded bands of
limestone, damp slate, and basalt, with broad veins; basalt cracks and living cave light gently pulse.
Remembered ground stays still. These are visual effects only: animation never advances a round or changes
the save. Preview a fixed look with `just isle --sky overcast|clear|mist` or
`just isle --stone limestone|slate|basalt` (the stone override applies underground).

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
  `look`, `inventory`, `skills`, `list`, and `help` take no time.

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

## Practice and experience

Give your name and start playing. There is no class choice. Everyone starts with a short sword, leather,
a shortbow and twelve arrows, a staff and healing potion in the sack, and fifty gold. Everyone can fight,
shoot, hide, and use all four spells: `spark`, `shield`, `heal`, and `stun`. Ordinary attacks and movement
cost no Focus. `chant <spell>` prepares a spell; `cast [who]` releases it.

`skills` shows all twelve character-owned practice ranks in the side panel; Escape or another command
returns to the ordinary view. The ordinary panel shows the current weapon's rank, Focus, and stealth.
Changing equipment does not transfer or erase practice. Overall XP and the displayed adventuring rank
record what you have overcome; they grant no combat bonus, health, Focus, or free healing.

Practice uses five bands: novice, practiced, skilled, adept, and expert, at 0, 1,000, 4,000, 12,000,
and 30,000 evidence. Daggers, blades (both swords), blunt weapons, staves, archery, and each spell's
discipline grow separately. Weapon and Spark ranks improve accuracy and damage. Stealth improves hiding
and opening strikes. Vitality grows from real incoming damage and adds four maximum Health per rank;
Focus practice adds two capacity per rank, from 24 to 32. Growing a capacity never refills it.

Landed damage earns evidence proportional to the danger of the opponent. Each individual can teach only
its original full Health in useful damage, shared across weapons and magic. Overkill, misses, healing the
creature, and save/reload never renew this allowance. Harmless, inert, and skittish creatures teach nothing;
weak threats plateau at lower ranks. Stealth has twelve shared lessons per creature for unseen movement,
opening strikes, and genuine escapes. Utility magic has another twelve shared lessons: a ward must prevent
a blow, a stun must extend its hold, and healing must restore tracked injury from an active encounter.
These useful paid casts also teach Focus, once per cast and only while the encounter has lessons left.

Spells and weapon techniques share Focus. Metal armor adds two Focus to a spell's cost, except Shield.
`rest` explicitly recovers two Health and six Focus per round, but leaves you exposed outside safety.
Focus also returns on its own anywhere: one point every fourth round, and one more in any round a blow of
yours does useful damage to a real threat, at most one such point per round however many foes you hit.
Health never returns on its own. The temple restores both resources. Experience thresholds for ranks 2–10
are 100, 300, 700, 1,500, 3,100, 6,300, 12,700, 25,500, and 51,100. Those thresholds do not predict
time-to-depth-10; the following fixed combat survey measures duel outcomes, not campaign time or floor clears.

Each depth's actual den keeper was spawned by the game, using its rule of choosing the strongest of four
drawn Deep individuals. We ran 256 matched duels per depth and character rank (64 world seeds, four den
spawn rounds). Characters had STR/DEX/CON 14 and started at the named Blades and Vitality ranks, with full
Health and 24 Focus; normal practice gains stayed enabled during the duel. Their loadouts advanced from short
sword and leather (novice), to long sword and leather (practiced),
chainmail (skilled), plate (adept), then plate and the warding ring (expert). The fighter attacked whenever
adjacent and followed the foe along walkable paths when it moved. The game resolved each creature's real
turns and automatic weapon techniques; the fighter used no spells, healing, rest, consumables, or retreat.
Each cell gives observed win rate and mean Health lost among wins. All non-wins were deaths.

| Deep floor | Den HP, mean (range) | Novice | Practiced | Skilled | Adept | Expert |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 9.7 (7–14) | 100.0% / 2.1 | 100.0% / 1.2 | 100.0% / 0.5 | 100.0% / 0.5 | 100.0% / 0.4 |
| 2 | 13.5 (10–20) | 100.0% / 4.5 | 100.0% / 3.0 | 100.0% / 1.2 | 100.0% / 0.5 | 100.0% / 0.5 |
| 3 | 18.0 (13–23) | 96.5% / 9.4 | 99.6% / 6.1 | 100.0% / 2.5 | 100.0% / 1.2 | 100.0% / 0.5 |
| 4 | 23.0 (18–30) | 89.5% / 13.0 | 97.7% / 9.9 | 100.0% / 4.7 | 100.0% / 2.9 | 100.0% / 1.7 |
| 5 | 28.3 (23–39) | 61.3% / 15.7 | 84.8% / 13.9 | 98.0% / 8.9 | 100.0% / 6.1 | 100.0% / 3.7 |
| 6 | 33.8 (26–45) | 35.5% / 17.1 | 72.3% / 15.8 | 91.8% / 12.8 | 99.2% / 9.2 | 100.0% / 6.8 |
| 7 | 40.5 (33–51) | 12.1% / 19.9 | 36.3% / 18.5 | 73.4% / 18.6 | 94.1% / 15.4 | 98.0% / 11.7 |
| 8 | 46.8 (36–60) | 6.2% / 20.0 | 23.8% / 20.6 | 50.4% / 21.0 | 76.6% / 18.3 | 94.1% / 14.9 |
| 9 | 54.0 (44–66) | 0.4% / 22.0 | 6.6% / 20.9 | 26.2% / 23.8 | 51.2% / 24.0 | 78.9% / 22.5 |
| 10 | 61.0 (52–72) | 0.4% / 20.0 | 2.0% / 23.4 | 10.5% / 23.0 | 31.2% / 24.9 | 55.5% / 24.0 |

Health lost is shown in points and only for wins. For example, the expert loadout won 142 of 256 depth-10
duels and died in the other 114; its wins cost 24.0 Health on average out of 42. Depth 10 is still dangerous
for the current top rank, not a victory lap. The survey measures no power beyond expert or depth 10, so a
post-demigod tier remains a separate design discussion. The covenant below provides the first
equipment-based step beyond mortal expert; it does not extend the practice ladder or add post-demigod foes.

## Weapon techniques

At skilled, each weapon family fires one technique of its own. There is no command to learn and nothing to
aim: a technique fires when its moment comes, spends Focus, and never fires against something too weak to
teach you anything. `skills` lists each weapon's technique and what it costs.

| Weapon | Technique | Focus | When it fires and what it does |
|---|---|---|---|
| Dagger | feint | 2 | A missed thrust draws that foe's guard: your next attack on it hits at +4 for double damage |
| Blades | parry | 2 | Once a round, turns one close blow that beats your armor by under four and readies a riposte: +4 to hit and +2 damage |
| Blunt | stagger | 2 | A hit five or more over the foe's armor costs it its next action |
| Staff | sweep | 2 | Each attack carries on to the most dangerous other foe beside you, for two less damage |
| Archery | steady shot | 1 | +2 to hit and damage at three or more cells with nothing hunting you from beside you |

A parry needs a duel: anything else hunting you from the next cell spoils it, so a skilled blade handles one
dangerous opponent well and is still in real danger when surrounded. Steady shot likewise needs quiet and
distance, while sweep and stagger are the techniques that answer a crowd. A readied feint or riposte shows
in the panel and serves only your very next command, whatever that command is.

## The island

Beyond towns, the surface continues into blah's seeded biomes, with trees that thin sight
and rocks that block movement and sight. Sight is round, reaching six cells in every direction, and ray
cast: several rays run between the two cells, and each tree a ray crosses costs three of those six cells,
so you see three cells away past one tree, nothing past two, and can glimpse past the edge of a trunk. Creatures
see you by the same rays. Deep water and snow peaks are impassable; shallow
water can be waded. Explored wilderness stays remembered across saves, including west and north of town.
Every town guarantees an outfitter (`buy`, `sell`, `list`), a temple (`heal` for up to five gold),
a revival spot, dungeon stairs, and a safe central arrival square. Paths connect all services and the
town's edges. Names, building sizes, service placement, and layout orientation vary with the seed.
The entire town footprint sits on passable dry land; ocean sectors need not contain a town.
The starting town is found near the world's origin, and more towns appear as you explore.
`look` gives distances and directions to the current town's services.

The wilderness holds weathered cairns, abandoned camps, and ruined wayside shrines. Their sites and
cache contents come from the island seed, not your build or the order you explore. Each 32×32 sector
tries up to eight dry, open sites; some sectors have none. Sites never replace town cells or alter
terrain. Seeing one logs a discovery, and its pixel-art remains stay on the remembered map.
`look` gives directions to sites currently in sight. A `?` badge marks an unopened cache
(`&?` in the text map, `&.` once opened).

Stand on the site and use `get all`, or press `,` with an empty command line, to open its cache and
take what fits in one round. It holds coins, gear, and extra arrows or a healing potion.
Every shrine cache also holds a relic; cairns and camps have a one-in-four chance.
Opening is permanent: waiting, leaving, dying, and reloading never refill it. A full sack leaves
the remaining contents on the ground, where you can return for them. The site itself remains.

### Cinder Watch

The starting town gives a direction and distance to Cinder Watch, a ruined wilderness tower. Its seeded
site needs a dry, reachable clearing outside towns; islands without a suitable nearby clearing have no
Watch. Discovering it reveals a warning, not an ambush. Stand on the tower and use `challenge` (`chal`)
when ready. Looking, gathering, and trying the barred coffer never start the encounter.

An ash sentry shoots from range while a plated cinder warden closes in. The warden halves arrow damage,
not Spark or melee damage. Prepare Shield before challenging, and bring recovery supplies; the guards
act during spell preparation and healing. Herbs and potions work normally.

Retreat, death, and reload preserve surviving guards, wounds, and their finite practice lessons. Guards
cannot enter or attack into towns, follow you into the Deep, or respawn. Defeat both original guards to
unlock the charred coffer, then `get all`/`,` at the tower for an ember charm, bogmint, and 120 gold.
Full-sack leftovers follow the ordinary cache rules. The tower's brazier goes cold permanently.
Its badge changes from `!` to `?` when cleared, then disappears when claimed
(`W!`, `W?`, and `W.` in the text map).

### Deep conquests

Stairs are always open. Sneaking deeper and taking treasure does not require killing earlier bosses.
Defeating a floor's den keeper permanently records that specific town and depth as conquered.
Death, travel, and save/load never erase it. `look` and the place panel show the current floor's status;
`inventory` recalls all conquests. Older saves begin with no recorded conquests.

The first keeper kill on each of depths 1–10 guarantees the following relic in the keeper's persistent
ground pile, in addition to normal loot. Use `get` to collect it; a full sack leaves it safely on the
ground. The prize is awarded once per town and floor, not once per respawn. Keepers still respawn and
drop their ordinary hoards. Deeper floors retain open exploration and conquest records, but this reward
set covers only the first ten floors; it does not introduce post-demigod content.

Only one relic can be worn at a time. These are build choices, not ten stacking upgrades.
Effects end on removal, capacity increases never refill Health or Focus, and practice is unchanged.
The relics can be dropped and sold like ordinary relics; selling one does not renew its first-clear prize.

| Depth | First-conquest relic | Benefit | Cost |
|---:|---|---|---|
| 1 | Ironroot bead | +1 melee damage | -1 archery accuracy |
| 2 | Mothglass | +15 percentage points to hiding, capped at 100% | -1 armor class |
| 3 | Flint eye | +2 arrow damage | -2 Spark damage |
| 4 | Hollow bell | +4 maximum Focus | -2 melee damage |
| 5 | Red tooth | +3 melee damage | -2 armor class |
| 6 | Pale knot | +4 Heal recovery | -2 Spark damage |
| 7 | Storm lens | +4 Spark damage | -4 maximum Focus |
| 8 | Grave seal | +3 active ward protection | -3 Heal recovery |
| 9 | Night crown | +2 armor class | -15 percentage points to hiding |
| 10 | Deepheart | +8 maximum Health | -6 maximum Focus |

Damage and Heal recovery remain at least one. Conquering depth 10 does not guarantee Oathfang:
the ordinary mythic drop roll below remains separate from the guaranteed Deepheart prize.
This milestone system does not establish a measured weeks-long campaign time.

### The Exile's covenant

The long hunt begins with the **existing respawning depth-10 den keeper**, not a new boss.
Each eligible kill has a 2% initial chance to leave **Oathfang** in its ordinary persistent loot pile.
Every failed kill increases the next chance quadratically:
`200 + floor(9700 × min(misses, 20)² / 400)` basis points. Thus the chances after 0, 5, 10,
15, and 20 misses are 2%, 8.06%, 26.25%, 56.56%, and 99%. It never reaches 100%;
even the twenty-first kill can fail. Misses persist across death, travel between towns, and saves.
Other depths, ordinary creatures on depth 10, and failed fights do not advance the ramp.
The separate seeded roll does not spend combat dice. Once the blade drops, no keeper generates another,
even if you leave it behind. A full sack leaves it on the ground.

Picking up the blade awakens a remembered call:
“They buried my body in the furnace and my name beneath the water.”
`look` and `inventory` recall bearings to the **Ashen Forge** and **Well of Names** from anywhere on
the surface; underground they remind you to return to daylight. Two seeded dry clearings are found by
walking the actual map from home, not by guessing coordinates across water. Small islands may place the
trials closer together. Seeing each site reveals its warning; neither discovery nor `get all` starts combat.

- **Ashen Forge:** use `challenge` at the furnace to wake an ash colossus. It regenerates four Health
  each turn and halves arrow damage. A landed Spark seals its wounds for three of its turns.
  Defeat it, return to the furnace, and `get all` for the **ashen carapace**.
- **Well of Names:** `challenge` wakes two hollow cantors on opposite sides. Every landed blow or shot
  steals two Focus. Wards, binding, breaking firing lines, and closing distance offer different answers.
  Defeat both original cantors and `get all` at the well for the **exile's heart**.

The trials preserve original guardians, wounds, fire suppression, and finite lessons across retreat,
death, and reload. Guardians never enter or attack into towns. Clearing a trial is permanent. A full
sack leaves the unique reward in a persistent ground pile; claiming again cannot duplicate it.
The sites' badges change from `!` to `?` when cleared, then disappear after claiming
(`A!`/`A?`/`A.` for the Forge and `N!`/`N?`/`N.` for the Well in the text map).

| Piece | Power | Price |
|---|---|---|
| Oathfang (Blades weapon) | 7–15 base damage, +2 accuracy; restores one third of useful melee damage as Health, minimum 1 | All weapon techniques cost +1 Focus while held; harmless foes and exhausted damage allowances cannot feed it |
| Ashen carapace (metal armor) | 9 armor | All field Health recovery is halved, minimum 1, including spells, herbs, potions, rest, and Oathfang; temple healing is unchanged |
| Exile's heart (relic) | +6 Spark damage; two passive Focus per fourth round instead of one | Each released spell also costs two Health; insufficient Health refuses the cast without consuming it; no practice for this blood cost |

`inventory` explains each piece before equipping it. Use `wield oathfang`, `wear carapace`, and
`wear exile heart`. Only **all three equipped together** grant demigod power: +20 maximum Health,
four points of armor pierced by attacks and aimed spells, and arrows that ignore arrow-proof plating.
The panel marks the active covenant. Merely owning the pieces is not ascension.
Breaking the set immediately removes the power and clamps Health to the mortal maximum.
Completing it again creates capacity, never free healing. Normal practice, individual trade-offs, and
spell Focus costs remain in force.

Pieces can be dropped and retrieved, but the outfitter refuses to buy them. Death retains equipped and
carried gear as usual. Old saves start with an untouched hunt. There are no post-demigod enemies,
second set, or runtime model calls in this prototype.

The combat survey `measure_covenant_progression_and_both_wilderness_trials` uses real rounds and
128 matched depth-10 den duels (32 world seeds, four spawn rounds). STR/DEX/CON are 14, with expert
Blades and Vitality, ring, 24 Focus, and no spells, healing actions, or retreat. All loadouts start full:

| Loadout | Wins / 128 | Mean Health remaining among wins |
|---|---:|---:|
| Ordinary expert (long sword, plate) | 82 | 14.7 / 42 |
| Oathfang and plate | 122 | 29.8 / 42 |
| Oathfang and carapace | 124 | 31.5 / 42 |
| Complete covenant | 128 | 53.7 / 62 |

These are encounter samples, not campaign odds or a guarantee against every keeper. The separate trial
survey uses seed 7's actual sites, 128 combat seeds, STR/DEX/INT/CON 14, expert practice in all disciplines,
ring, and full Health/Focus. The Forge loadout has Oathfang and plate; the Well loadout has Oathfang and
carapace. Melee-only pursuit wins 116/128 at the Forge and 77/128 at the Well. A prepared policy
(Shield, periodic Spark at the Forge, up to six potions below 18 Health, otherwise pursuit and melee)
wins 127/128 and 128/128 respectively. Preparation matters; these are not tests of every possible strategy.

### Herbs and field recovery

Seeded herb patches grow on open wilderness ground, never inside towns, in water, or in the Deep.
Seeing a patch reveals its plant and effect; `look` gives directions to plants in sight.
Each plant has its own pixel-art flowers (`h*` in the text map).

| Herb | Where it grows | One use restores |
|---|---|---|
| Woundleaf | Grassland, forest, hills | 6 Health |
| Bogmint | Swamp, jungle | 8 Focus |
| Sunroot | Desert, savanna, badlands | 3 Health and 4 Focus |

Stand on a patch and `get herb`, name the plant (`get bogmint`), or use `get all`/`,` to gather it
in one round. Each herb occupies one sack slot. A full sack leaves the plant growing.
Each 16×16 sector tries up to four sites; some sectors have no suitable plant. A harvested patch stays
picked across visits, death, and reloads, and its flowers disappear from the map.

`use woundleaf`, `use bogmint`, or `use sunroot` consumes one carried herb in one round. You can
use it while hunted, unlike `rest`, but creatures still act: gathering and recovery provide no safety.
Recovery never exceeds your current capacities. If the herb cannot restore either resource, you keep it,
but the attempted command still takes a round. Herbs grant no practice or XP; dressing a wound removes
its tracked injury just as a potion does, so later magic cannot claim it again.
You can drop, pick up, or sell gathered herbs (two gold each); outfitters do not stock them.

### Relics and trade-offs

Relics come from wilderness caches, not starting gear or outfitter stock. Their finds use a separate
seeded stream, so opening a cache never spends combat dice. Each has its own ground sprite.
`inventory` describes both the benefit and cost before you equip one; the ordinary side panel repeats
both while it is worn.

| Relic | Benefit while worn | Cost while worn |
|---|---|---|
| Ember charm | +2 Spark damage | −3 Heal recovery, with a minimum of 1; herbs and potions are unaffected |
| Stillstone | +2 melee and missile protection from an active Shield | −4 maximum Focus |
| Hunter's token | +2 archery accuracy, not damage | −15 percentage points to hiding attempts, even at the normal chance cap |

`wear ember charm`, `wear stillstone`, or `wear hunter's token` equips one relic in its own slot,
separate from armor and the ring. Wearing another swaps the old relic into the same sack slot, so a
full sack can still swap. `remove relic` (or name it) returns it to the sack; a full sack refuses removal
without losing anything. Equipping and removing each take one exposed round. Benefits and costs use
the currently worn relic: removing Stillstone ends its protection bonus even if an old Shield still lasts.
The token changes new hiding attempts, not the chance of retaining concealment while walking.

Stillstone clamps current Focus to its reduced capacity. Removing it, swapping relics, and earning
capacity through practice never refill Focus. The cost remains as Focus practice grows; rest, herbs,
the temple, and revival respect the current reduced maximum. Relics grant no practice, change no earned
ranks, and remain worn after death and reload. Removed relics can be dropped, picked up, or sold for
75 gold each. Caches never refill, including after a relic is sold.

### Towns, wildlife, and the Deep

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

Some Deep bodies express cave adaptations, derived from their inherited genes rather than extra random
abilities or their names. The same body on the surface has neither adaptation:

- **Tremorsense:** chitinous or scaled creatures with six or eight legs investigate footsteps along
  connected floor, even around corners. Ordinary steps carry five floor steps; hidden steps carry two.
  Standing still makes no footsteps, and a blocked move makes none. A creature follows the last step
  it actually heard, not your unseen current position. Sound alone does not reveal you to every creature,
  or let an attack pass through a wall. Quiet movement, corners, and leaving its home range can lose it.
- **Luminous:** slimy creatures with at least half cold tolerance light walkable cells within two tiles,
  with walls blocking the light. You cannot hide in the glow; walking into it, or having its source
  approach, breaks hiding. Leaving it restores the ordinary hiding rules. The sprite shines, visible
  lit cells have a green background, and your panel says `lit · no hiding`.

IN SIGHT names each adaptation and gives a short cue. These are the first cave expressions of the
existing genome, not new gene slots or separately evolved abilities. Body selection still follows the
Deep environment above. Safe landings remain safe, and off-floor life and its light stay paused.

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
{"op":"text","text":"Wren\n"}
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
  cursor. About 1 KB. The text map represents a genome sprite as its target letter and `*` (`A*`), not
  image pixels, so agent targeting stays the same.
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
- `python3 crates/isle/scripts/progression.py` checks name-only creation, the practice panel, no-time
  `skills`, shared Focus, explicit recovery, and exact save/reload.
- `python3 crates/isle/scripts/techniques.py` checks that `skills` prices each weapon's technique only once
  it is earned, that a skilled blade parries and ripostes with no command of its own, that the readied
  advantage reaches the panel, and that distant shots fly steady.
- `python3 crates/isle/scripts/wilderness.py` checks leaving town, biome rendering, local map origins,
  wilderness memory, and save/reload.
- `python3 crates/isle/scripts/landmarks.py` walks from town to a naturally generated site, checks its
  map marker and directions, opens its cache, and verifies one-time and partial claims across reloads.
- `python3 crates/isle/scripts/herbs.py` finds natural plants in three biomes, checks full-sack gathering,
  exact recovery, no practice gains, map markers, and permanent harvest state across reloads.
- `python3 crates/isle/scripts/relics.py` walks from town until a natural cache yields a relic, checks
  full-sack leftovers, visible trade-offs, one-slot equipment, exposed rounds, no-refill removal, and reloads.
- `python3 crates/isle/scripts/cinder.py` walks to the natural Watch, reads its warning, prepares and
  fights its guards, reloads partial progress, and checks the permanent reward and quiet tower.
- `python3 crates/isle/scripts/mythic.py` starts from an explicit expert/first-drop checkpoint, then
  picks up Oathfang, walks to both naturally placed trials, fights their real guardians, claims and
  equips the rewards, and checks demigod power, no-refill swapping, and exact reload.
- `python3 crates/isle/scripts/wildlife.py` checks evolved population, reload, the IN SIGHT panel,
  and shooting an actual spawned individual by its regional name.
- `python3 crates/isle/scripts/towns.py` visits two generated towns, exercises their services, and checks
  stair returns and revival after reload.
- `python3 crates/isle/scripts/deep.py` descends below two towns and to a second floor, and checks separate
  layouts, evolved life off the landings, tougher life deeper down, stair returns, and that each floor's life
  and memory persist across visits and reloads.
- `python3 crates/isle/scripts/caves.py` finds naturally spawned cave-adapted genomes, then uses scoped
  encounter reloads to check trait cues, hiding at the glow boundary, walking into light, and exact saves.
- `python3 crates/isle/scripts/seed7.py` plans routes from the map and fights from the window's panel. It
  walks to the stairs, heads for the far end of the Deep, and retreats to the temple to heal. It checks that
  ordinary fights earn blades practice and adventuring rank, then reloads the learned state. Games are
  deterministic per seed and input, so its output repeats exactly.
