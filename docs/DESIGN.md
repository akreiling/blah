# Design

For the current detailed systems plan, start with [Growing Into Power](SYSTEMS_ROADMAP.md). Its focused
documents cover committed character growth, physical and magical disciplines, emergent skill weaving,
treasure, apex encounters, generated arcs, and implementation checkpoints. They supersede earlier progression
and difficulty assumptions in this overview.

This is a design document, not a claim that every feature below exists. See [First playable](#first-playable)
for the implemented slice and the [code-inspected baseline](systems/baseline.md) for its present limits.

A top-down, tile-based, turn-based wanderer in the spirit of early-80s overworld RPGs, set in an endless
procedural world. The twist is that the monsters aren't authored. They are populations with genomes that are
shaped by the biomes they live in, change slowly across space and time, and branch into new species. Nobody,
including us, knows what lives past the next ridge.

Working name: `blah`, until the game has a real one.

The wanderer is also shaped by history: what you practice becomes what you are. Early potential becomes a
committed identity, then real power, then refinement. Physical and magical disciplines can weave into
combinations we never anticipated. The replayability goal is: **"I want to try this next."**

## Ground rules

- **Terrain is a pure function of `(seed, x, y)`.** The same seed always gives the same land.
- **Life is recorded fact.** Populations grow out of what is already explored, so a world is its seed plus its
  history. Every step of the sim is a pure function of `(seed, region, neighbor populations, elapsed time)`, with
  RNG streams keyed by hashing those inputs. Given the same history, the result is the same, so tests can
  replay it.
- **Time is an input.** Regions don't tick while nobody is looking. They catch up from elapsed game time when
  next needed. Offline real-world time never trains the character or advances the world.
- **The network never blocks play.** Names and sprites from remote models replace procedural placeholders once
  they arrive, or never. Offline is a fully supported way to play.
- **Remote generation is scarce.** Names and sprites are requested on sight, with bounded retries and one
  accepted result per record. Future encounter arcs may be drafted a bounded distance ahead of discovery.
  Revealing a site commits an available valid draft or an offline fallback; late results cannot rewrite it.
  Every service has explicit spending limits.
- **Power is independent.** Neither ordinary creatures nor apex foes scale to the player's build. A veteran
  can wipe out lesser foes, while exceptional encounters demand refined power and understanding.

## World

The world is an endless grid of tiles, generated in square chunks. Each tile samples four noise fields:

| Field       | Drives                                                     |
| ----------- | ---------------------------------------------------------- |
| elevation   | water, coast, plains, hills, mountains                     |
| moisture    | desert ↔ swamp axis                                        |
| temperature | tundra ↔ jungle axis; falls with elevation                 |
| age         | how long life has existed there (see Succession)           |

Biome comes from elevation, moisture, and temperature, using a Whittaker-style lookup table. The table is data,
not code, so adding or retuning a biome is a table edit.

A **region** is the ecological unit: a connected stretch of mostly one biome, about 1,000 to 4,000 tiles, so a
walk crosses one in a minute or two. Like terrain, it is a pure function of `(seed, tile)` and never depends on
where exploring started. An unbounded flood fill from the player would depend on exactly that, so regions are
built from bounded, local steps instead:

- **Cells** split the plane into a Voronoi diagram around one hashed point per 48-tile grid square. Cells are
  what stop a continent of jungle from becoming one region.
- **Pieces** are a cell's same-biome components, with slivers under 32 tiles folded into a neighbor.
- **Joins** attach a piece too small to stand alone (under 800 tiles, usually a biome cut off by a cell edge) to
  an adjacent large piece, preferring its own biome. A piece under 400 tiles may join another biome. Joins
  reach at most two pieces out and never chain, and a region is capped at 5,000 tiles.

Water never shares a region with land: swimmers and walkers live in different worlds. A region's id is the
first tile of its large piece, and its placeholder name ("the Glass Marsh") comes from the seed, the id, and
word lists flavored by biome, until a remote model renames it. Looking up a region costs a hash lookup once the
cells around it are labeled, about a millisecond per cell. The game keeps a cache of labeled cells.

## Life

### Genome and phenotype

A possible expanded genome is a fixed-length vector of genes in `[0, 1]`, grouped as below. The current slice
has eight genes; expansion requires a gameplay purpose rather than reaching a predetermined count.

- **Body:** size, limb count, segmentation, symmetry, covering (fur, scale, chitin, slime, feather)
- **Movement:** walk, swim, burrow, climb, fly (weights, not exclusive)
- **Metabolism:** diet (plant ↔ meat), heat tolerance, cold tolerance, water retention, activity cycle
- **Mind:** aggression, sociality (pack size), boldness, intelligence
- **Defense:** resistance per element, toxicity, armor
- **Look:** palette genes, patterning

Phenotype is a pure function from genome to traits: stats, behavior, which tiles it can enter, and a
description in words. The description in words drives naming and sprite generation, so a creature looks like
what its genes say it is.

### Biomes as fitness functions

Every biome scores a phenotype. A desert rewards water retention and heat tolerance and punishes heavy fur.
Swamp rewards swimming. Mountains reward climbing and cold tolerance. Biome productivity sets carrying capacity
for plant eaters, plant eaters set it for predators, and so on up the food chain. What a biome rewards,
camouflage included, is a row of the same data table that defines the biomes, and the weights that combine a
row into one score sit beside the table.

### Populations

A region holds a set of species populations. Each one is a count plus a small sample of representative
genomes, about 32 of them. One generation means selecting by fitness, recombining, and mutating. When you meet
a monster, it is drawn from the local sample, so individuals vary.

**Generation length scales with body size.** Vermin turn over fast and are always shifting. Apex predators
barely change and act like landmarks.

**Migration scales with movement.** Fliers spread quickly and colonize first. Swimmers follow water. Burrowers
crawl outward slowly. Migrants that reach a new region face that region's fitness function.

### Speciation

Every species has a reference genome. When a regional sub-population drifts beyond a genetic-distance
threshold from it, the sub-population becomes a new species with a parent link. The species graph is a family
tree, and every node records where it diverged.

### Growth at the frontier

When the player reveals a region for the first time, its life is grown on the spot. We seed it from
neighboring regions that already exist, plus founder genomes drawn from `(seed, region)` when it has no
neighbors yet, then run history forward. That makes the world path-dependent: the route you explore shapes
what arises. Once grown, a region is recorded and only changes by drift.

**Distance creates unfamiliarity, not an automatic power multiplier.** Life's history can make distant
regions strange, but evolution is adaptation, not a ladder toward stronger combatants. Exceptional individuals
realize inherited capabilities through their own development and ecological support. See
[Ecology and independent enemy power](systems/ecology.md).

### Succession

The age field decides how much history a region has on reveal:

- **Old regions** get deep history right away: established species, full food webs, apex predators already
  present.
- **Young regions** (lava fields, new floodplains, scoured tundra) start nearly empty. Pioneers arrive over game
  time by migration, then settle and branch. New species appear while the player is around.

### Drift

Visited regions keep evolving at a low rate and catch up on elapsed time when revisited. A speciation event in
a region the player knows produces a news line (*"Something new has been seen in the Glass Marsh."*) without a
name. The species gets a name when the player first sees it.

## The wanderer

Solo, classless, but committed. Meaningful practice grows foundations, proficiencies, and integrations.
Earned structural adaptations make some futures natural and others difficult. A mature character cannot
become a different identity simply by changing equipment; a new character explores a different life.

There are multiple physical and magical disciplines, not one martial track and a menu of damage colors.
Their interactions can produce a close-range magical blaster, long-range shuriken specialist, battlemage
thief, poison-stealth mage, warded raging juggernaut, area frost mage, or something nobody designed in advance.
These examples are not classes or a whitelist of legal builds.

The long arc leads to extraordinary specialized power. It does not end in every character mastering every
discipline. Equipment, treasure, skills, healing, gathering, stealth, and magic all support this growth.

- **Survival and access:** bodily adaptation, equipment, supplies, and supported magic can make different
  habitats reachable. Do not reduce every route to one mandatory skill-tree gate.
- **Lore:** adds precision, historical comparisons, and eventually deeper genome information. Basic danger
  and actionable warning must be readable without advanced Lore.
- **Beastcraft:** later development around companions and the pen, not a prerequisite for the next slice.

Details: [Character development](systems/character.md), [Disciplines](systems/disciplines.md),
[Weaving](systems/weaving.md), and [Learning](systems/learning.md).

**Death costs time.** You wake at home after a long stretch of game time. Once drift exists, the world
changes during that interval. Skills and owned equipment remain; elapsed time grants no mastery. The current
slice advances its turn counter but does not simulate drift.

## Taming and the pen

One tamed **companion** travels with you. It has its own temperament genes, so loyalty and aggression decide
how well it obeys and whether it holds, flees, or goes berserk. Other tamed creatures live at the **pen**, your
home base.

The pen is artificial selection using the same genetics engine that the wild uses for natural selection. It
also does what geography can't: crossing a desert line with a tundra line. Beastcraft improves the odds of
getting the traits you want and unlocks crosses between species.

Later possibility: releasing bred creatures into the wild as invasive species that spread by the normal
migration rules.

## Combat

Combat happens on the overworld grid, turn-based, with no separate battle screen. Monsters, the companion,
and the wanderer all act on the same map. Behavior comes from phenotype: pack animals flank, bold ones charge,
timid ones flee when hurt, nocturnal ones are slow by day. Terrain access comes from movement genes, so a
swimmer retreats into water and a burrower goes underground.

## Power and encounter arcs

Ordinary encounters demonstrate earned strength. Major sites build through attraction, warning, escalating
evidence, and a formidable apex to a durable aftermath and worthwhile treasure. The lead-up teaches aspects
of the foe's actual capabilities rather than taxing potions through repetitive mandatory fights.

The engine owns creatures, terrain, capabilities, placement, rewards, and encounter state. OpenRouter may
draft the arrangement and dramatic buildup from a bounded factual snapshot. It cannot invent executable
mechanics, tailor hidden counters to the current character, or rewrite a revealed site.

Start with authored encounter fixtures and offline fallbacks. Add generated drafts only after the format,
validation, and actual gameplay work. See [Encounter arcs](systems/encounters.md),
[OpenRouter drafting](systems/arc-generation.md), and the [reservoir example](systems/reservoir-example.md).

## Generated names and sprites

The provider/model details below are earlier planning notes, not a live integration. Verify current APIs,
model availability, pricing, and quotas when implementing the clients. No remote clients exist in the
baseline slice.

Both are generated **when the player first sees a species**, not at speciation. Most species are born in places
nobody looks, and generating on first sight keeps spending tied to what actually gets seen.

- **Names and bestiary lines:** `openai/gpt-6-luna` on OpenRouter, a fast, cheap text model. Input is the
  phenotype description plus biome and parent species name, never the raw genome. Output is a name and one
  line of bestiary text.
- **Judging art:** `typesafe/jev-1.14` on OpenRouter, a decision model rather than a text generator. It
  answers typed questions about text or images: pick an option, score against a rubric, judge a statement
  true or false, or place a bounding box. Input costs $0.042 per million tokens and output is free. It never
  writes content. It is called through OpenRouter's `POST /api/alpha/decisions` endpoint, not chat completions,
  with a body of `{model, state, questions}`; the question types are `choice`, `score`, `noul`, and `box`. Its
  jobs:
  - check that a sprite depicts its phenotype before we cache it
  - score how much a child sprite resembles its parent, which is the measurement for the lineage experiment
  - box the creature in a sprite so it is centered and scaled on its tile
- **Sprites:** PixelLab (see below). The prompt comes from the phenotype description, and a fixed game palette
  keeps every species looking like part of the same game. A child species is drawn by editing its parent's
  sprite, so lineages look related.
- **Individuals** vary procedurally on top of the species sprite: palette shifts from color genes, size from
  the size gene. This costs nothing and is instant.
- **Fallbacks are first-class:** a name made of the region's biome epithet and the clade's root ("Dune
  Skitter"), and a mirrored procedural sprite built from the genome, shown immediately and replaced when the
  remote result arrives.

Budget rules:

- Generate once per species and cache in the world save forever.
- Requests go through a queue with a daily cap per service. When the cap is reached, placeholders stay and the
  queue resumes later.
- Tests and the default dev build never touch the network. Remote clients are behind a trait with a fixture
  implementation.
- Terrain tiles, the wanderer, and UI art are never generated at runtime. They are a deliberate one-time
  batch, checked in as assets.

Because art and names are recorded facts, two worlds with the same seed evolve the same species with
different names and portraits.

### Review

Jev reviews everything Luna and PixelLab produce before it is cached. The pipeline is generate, then review,
then either accept and cache, or reject and keep the placeholder. Review works unattended because the questions
are fixed in code, the criteria are concrete, and each verdict is about one artifact. Jev can also interrogate
the systems design extensively through a [structured question bank](review/README.md). Those development-time
judgments are advisory: question framing matters, repeated votes are not independent evidence, and actual
validators and playtests decide whether the system works.

- **Names and bestiary lines.** Luna returns about five candidates in one call. Jev picks the one that best
  fits the traits and biome. It rejects any candidate that contradicts the trait list, reads as silly or
  modern, is a known creature or brand name, or is too close to an existing species name. If every candidate
  fails, there is one retry, then the fallback name stays. Luna is cheap, so this runs every time.
- **Sprites.** Jev checks that the sprite:
  - depicts the phenotype
  - is a single creature on a transparent background, not a scene
  - for a child species, resembles its parent without being a copy of it
  - stays consistent in style with a few accepted sprites

  A `box` question crops the creature so it can be centered on its tile. A rejected sprite leaves the
  procedural placeholder in place. Because generations are scarce, a species gets at most one retry, with the
  prompt adjusted from the failed checks, and only when the daily budget allows.
- **Thresholds.** Jev's probabilities are calibrated, so "accept at 0.7 or above" means something. The
  thresholds are tuned during the lineage experiment.
- **Verdicts** are stored with the art, so rejections can be studied and prompts improved.

### PixelLab specifics

From the PixelLab docs (`https://api.pixellab.ai/mcp/docs`). PixelLab bills in "generations." The free tier
(as of 2026-11) gives 40 fast generations once. After those are used, it gives 5 slower generations a day,
banking up to 20. `GET /v2/balance` is the source of truth, and the queue checks it before spending anything.

How we spend it:

- **Spend the 40 fast generations on things made once:** the lineage experiment, terrain tilesets, the
  wanderer. These are checked in as assets.
- **Species sprites come from the daily allowance.** Roughly 5 new species a day get real art, and the rest
  keep their procedural sprite until the queue reaches them. "Slower" only means more latency, and the
  placeholder already covers that.
- **The queue is ordered by what the player cares about:** companion and pen species first, then species by
  how often they've been encountered, then the most recent sightings. A species seen once and never again may
  never get art, and that's fine.

- **One API token works for MCP and REST.** The game's `gen` crate calls the REST v2 API
  (`https://api.pixellab.ai/v2/llms.txt`) with `Authorization: Bearer <token>`. The MCP server is for us during
  development, for the one-time asset batch.
- **Jobs are asynchronous.** A create call returns an id, and results take minutes. That fits the
  placeholder-then-replace design. Failed jobs aren't charged, and canceling an unstarted job is free.
- **Root species sprite:** `create_image_pixen` at 1 generation, square 16–28px, `no_background`, and a top-down
  `view`. The other 1-generation option is `create_image_pixflux`, which needs at least 32×32 but accepts an init
  image. The character tools (directions, animations) cost much more, and their text-to-creature path mostly
  assumes humanoids or fixed quadruped templates. One facing direction is enough for this game.
- **Child species sprite:** edit the parent's sprite rather than drawing from scratch. Either
  `edit_image_pixen` (1 generation, a text edit such as "flatter, wetter, add fins" derived from the change in
  phenotype), or `create_image_pixflux` with the parent as `init_image` at moderate `init_image_strength`
  (about 50 keeps the composition, the default 150 is a real edit). At 16px tiles only the pixen edit applies.
  The lineage experiment tunes its edit prompts and the review thresholds.
- **Palette:** pass the game palette as `palette`. `reduce_colors` (0.1 generation) snaps a result onto it
  afterward if needed.
- **Terrain:** `create_topdown_tileset` standard costs 1 generation and produces a 16-tile Wang autotile set
  between a lower and an upper terrain, at a 16 or 32px tile size. Tilesets chain: pass one tileset's base tile
  id as `lower_base_tile_id` to the next (ocean → beach → grass → forest). Generating every biome pair would be
  combinatorial, so we only make transitions for biomes that the noise fields can place next to each other.
- **Tile size:** 16px. It is closer to the genre and puts more of the world on screen. Sprites are drawn
  scaled up. The cost is that `pixflux` init-image edits need at least 32×32, so lineages use
  `edit_image_pixen`.

## UI

This earlier conceptual layout includes future features and older labels. The detailed
[interface plan](systems/interface.md) and [resource model](systems/actions.md) define the current prototype
direction.

```
┌──────────────────────────────────┬───────────────────┐
│                                  │ WANDERER    Lv 7  │
│                                  │ HP ████████░  41  │
│                                  │ MP ███░░░░░░  12  │
│        map viewport              │ Food 220  Gold 88 │
│        (tiles, 17×17)            ├───────────────────┤
│                                  │ COMPANION         │
│                                  │ [spr] Mirebiter   │
│               @                  │ HP ██████░  loyal │
│                                  ├───────────────────┤
│                                  │ NEARBY            │
│                                  │ [spr] Ashcrawler  │
│                                  │   fire res ▲▲     │
│                                  │   speed    ▼      │
├──────────────────────────────────┴───────────────────┤
│ › The ashcrawlers here are hardier than those behind │
│ › Something new has been seen in the Glass Marsh.    │
│ › Mirebiter growls at something you can't see.       │
└──────────────────────────────────────────────────────┘
```

- **NEARBY** compares each species in view with its clade in the most recent of your last 8 regions that has
  it, so you feel the gradient as you walk. How much it shows depends on Lore.
- **The message log** is both combat narration and world news.
- **Overlays**, one key each: bestiary (species list with lineage tree, each branch tagged with where it
  diverged), genome inspector (on a target, gated by Lore), development and technique weaving, world map
  (with a life overlay that colors regions by dominant lineage), pen.
- **Controls** are keyboard-first, one key per verb: attack, tame, look, inspect, camp, and so on.

## First playable

The full design is a lot for one person. A second opinion from Jev put it at "too large, likely to stall,"
which matches our own read. So the first version is a thin slice through both halves of the game:
- a small genome, of about 8 genes
- a procedural sprite drawn from the genome
- monsters drawn from each region's biome
- bump-to-attack combat on the overworld

The slice uses eight genes: size, limbs, covering, heat, cold, water, aggression, and hue. Covering bands run
fur, feather, chitin, scale, slime, from warm and dry to bare and wet, so a small mutation is a plausible
change.

Each world has eight clades, one per silhouette family. Genes stretch and recolor a clade's silhouette but never
swap it, which is what keeps a lineage recognizable across biomes.

Selection is proportional to fitness rather than by tournament, so pressure scales with how much a trait
matters. A slight camouflage edge stays slight instead of counting as much as surviving the heat, which keeps
every lineage from turning the color of its biome. Pigment follows genes: heat- and cold-hardy creatures bleach
pale, and water-adapted ones go dark. `just bestiary out.png [seed]` draws every clade across a row of biomes.
It is the visual check that evolution shows.

In the slice, a region's life is a pure function of `(seed, region)`: the clades that settle it, each from its
own founders, evolved for the same fixed number of generations under the region's biome, with no seeding from
neighbors and no elapsed time. Dying advances the turn counter, but the world does not drift.

That makes a playable loop early: wander, meet evolved creatures, fight. Each half then gets deeper without
waiting on the other. Taming, the pen, drift, and remote art come only once that loop is fun. The other
risk is evolution that players can't see. The slice has to make differences between regions visible from the
first day.

The next work follows the [checkpoint plan](systems/checkpoints.md): prove useful skill interactions, make
characters grow into them, preserve the result, and build a complete rewarding journey before expanding
ecology or remote generation.

## Architecture

- **`core`** is a pure library holding world generation, biomes, genetics, the population sim, combat rules,
  and abilities. It does no I/O, uses no wall clock, and makes no network calls. Nearly all tests live here,
  and sim tests are property-style ("500 generations in desert raise mean water retention").
- **`gen`**, planned, will hold remote clients behind fixture-capable interfaces, plus request queues,
  validation handoff, spending caps, and durable request accounting. It is not an existing crate.
- **`app`** is the GPUI frontend. The map is a grid of tile elements, which is plenty at viewport size; it
  becomes one `canvas` element if that stops being fast enough. Panels and overlays are ordinary views.
  GPUI's actions and keybindings carry the one-key commands. GPUI comes from crates.io at an exact version
  (`=0.2.2`) because its API changes often. It uses the `runtime_shaders` feature, which compiles the Metal
  shaders at launch, so building needs only Xcode and not its separately downloaded Metal Toolchain.
- **Save**, planned, is one versioned SQLite file per character/world. It records character development,
  item ownership, encounter definitions and progress, learning and reward claims, and later regional
  history. Base terrain is regenerated from a saved seed and compatible generator version; persistent
  terrain changes are overlays. Incompatible rules cannot silently reinterpret old saves. See
  [Architecture and persistence](systems/architecture.md).

## Toolchain and secrets

- `mise.toml` pins Rust stable and `just`. The `justfile` holds the tasks (`run`, `test`, `lint`).
- API keys (`OPENROUTER_API_KEY`, `PIXELLAB_SECRET`) live in `.env.local`. That file is gitignored, and mise
  loads it into the environment. If a key is missing, the game runs offline.

## Open questions

- Recovery and supplies: tune the selected camp/Focus prototype without turning travel into bookkeeping.
- Towns, people, quests: do they exist, and what do they offer (trade, rumors of strange species, pen
  upgrades)?
- How a region's size cap and the chunk size interact with sim cost at the frontier.
- Whether there is an ultimate ending beyond the current endgame of specialized refinement and exceptional
  site-scale foes.
