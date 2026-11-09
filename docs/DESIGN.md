# Design

A top-down, tile-based, turn-based wanderer in the spirit of early-80s overworld RPGs, set in an endless
procedural world. The twist is that the monsters aren't authored. They are populations with genomes that are
shaped by the biomes they live in, change slowly across space and time, and branch into new species. Nobody,
including us, knows what lives past the next ridge.

Working name: `blah`, until the game has a real one.

## Ground rules

- **Terrain is a pure function of `(seed, x, y)`.** The same seed always gives the same land.
- **Life is recorded fact.** Populations grow out of what is already explored, so a world is its seed plus its
  history. Every step of the sim is a pure function of `(seed, region, neighbor populations, elapsed time)`, with
  RNG streams keyed by hashing those inputs. Given the same history, the result is the same, so tests can
  replay it.
- **Time is an input.** Regions don't tick while nobody is looking. They catch up from elapsed time when they
  are next needed.
- **The network never blocks play.** Names and sprites from remote models replace procedural placeholders once
  they arrive, or never. Offline is a fully supported way to play.
- **Remote generation is scarce.** PixelLab is on a free tier and OpenRouter costs money. We call them only for
  things the player is actually looking at, and only once per thing, ever.

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

A **region** is the ecological unit: a cluster of chunks sharing one biome, found by flood-filling biome
identity across chunk boundaries (capped in size so that huge biomes still split into several regions).
Regions get generated names ("the Glass Marsh") the same way species do.

## Life

### Genome and phenotype

A genome is a fixed-length vector of genes in `[0, 1]`, roughly 32 of them, grouped as:

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
for plant eaters, plant eaters set it for predators, and so on up the food chain. Fitness weights live in the
same data table as the biomes.

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

**Distance from home is evolutionary depth.** Life far from the start has had more steps of divergence behind
it and is stranger and more dangerous. That gives an endless map a difficulty curve that comes from the
system itself.

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

Solo. One character, levels, and an ability tree with these branches:

- **Blade, Bow:** weapons
- **Arcana:** fire, frost, poison
- **Survival:** tolerance for harsh biomes. **This gates movement.** You can see the deep desert but not cross
  it until you can stand the heat, so Survival decides which lineages you can reach.
- **Lore:** gates the genome inspector. Low Lore: *"a scaly thing; seems angry."* Mid Lore: traits and
  resistances. High Lore: raw genes, lineage, and which way the local population is drifting.
- **Beastcraft:** taming, pen size, and breeding control

**Death costs time.** You wake at the pen after a long stretch of game time, and the world has drifted while
you were out. No other penalty is needed.

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

## Generated names and sprites

Both are generated **when the player first sees a species**, not at speciation. Most species are born in places
nobody looks, and generating on first sight keeps spending tied to what actually gets seen.

- **Names and bestiary lines:** `openai/gpt-6-luna` on OpenRouter, a fast, cheap text model. Input is the
  phenotype description plus biome and parent species name, never the raw genome. Output is a name and one
  line of bestiary text.
- **Judging art:** `typesafe/jev-1.14` on OpenRouter, a decision model rather than a text generator. It
  answers typed questions about text or images: pick an option, score against a rubric, judge a statement
  true or false, or place a bounding box. Input costs $0.042 per million tokens and output is free. It never
  writes content. Its jobs:
  - check that a sprite depicts its phenotype before we cache it
  - score how much a child sprite resembles its parent, which is the measurement for the lineage experiment
  - box the creature in a sprite so it is centered and scaled on its tile
- **Sprites:** PixelLab (see below). The prompt comes from the phenotype description, and a fixed game palette
  keeps every species looking like part of the same game. A child species is drawn by editing its parent's
  sprite, so lineages look related.
- **Individuals** vary procedurally on top of the species sprite: palette shifts from color genes, size from
  the size gene. This costs nothing and is instant.
- **Fallbacks are first-class:** a syllable-model name and a mirrored procedural sprite built from the genome,
  shown immediately and replaced when the remote result arrives.

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
  (about 50 keeps the composition, the default 150 is a real edit). Which one gives better lineages is an early
  experiment.
- **Palette:** pass the game palette as `palette`. `reduce_colors` (0.1 generation) snaps a result onto it
  afterward if needed.
- **Terrain:** `create_topdown_tileset` standard costs 1 generation and produces a 16-tile Wang autotile set
  between a lower and an upper terrain, at a 16 or 32px tile size. Tilesets chain: pass one tileset's base tile
  id as `lower_base_tile_id` to the next (ocean → beach → grass → forest). Generating every biome pair would be
  combinatorial, so we only make transitions for biomes that the noise fields can place next to each other.
- **Tile size:** 16 or 32px, decided by the first sprite experiment. 16px is closer to the genre. 32px makes
  `pixflux` init-image edits available.

## UI

```
┌──────────────────────────────────┬───────────────────┐
│                                  │ WANDERER    Lv 7  │
│                                  │ HP ████████░  41  │
│                                  │ MP ███░░░░░░  12  │
│        map viewport              │ Food 220  Gold 88 │
│        (tiles, ~15×15)           ├───────────────────┤
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

- **NEARBY** compares local populations with the region you just left, so you feel the gradient as you walk.
  How much it shows depends on Lore.
- **The message log** is both combat narration and world news.
- **Overlays**, one key each: bestiary (species list with lineage tree, each branch tagged with where it
  diverged), genome inspector (on a target, gated by Lore), ability tree, world map (with a life overlay that
  colors regions by dominant lineage), pen.
- **Controls** are keyboard-first, one key per verb: attack, tame, look, inspect, camp, and so on.

## Architecture

- **`core`** is a pure library holding world generation, biomes, genetics, the population sim, combat rules,
  and abilities. It does no I/O, uses no wall clock, and makes no network calls. Nearly all tests live here,
  and sim tests are property-style ("500 generations in desert raise mean water retention").
- **`gen`** has the remote clients (OpenRouter, PixelLab REST v2) behind traits, plus the job queue (submit,
  poll, collect), spending caps, the balance check, and fixtures.
- **`app`** is the GPUI frontend. The map is a grid of tile elements, which is plenty at viewport size; it
  becomes one `canvas` element if that stops being fast enough. Panels and overlays are ordinary views.
  GPUI's actions and keybindings carry the one-key commands. GPUI comes from crates.io at an exact version
  (`=0.2.2`) because its API changes often. It uses the `runtime_shaders` feature, which compiles the Metal
  shaders at launch, so building needs only Xcode and not its separately downloaded Metal Toolchain.
- **Save** is one SQLite file per world. It holds the seed, revealed regions with their populations and
  last-simulated time, the species graph, the name and sprite cache, the pen, and the wanderer. Terrain is
  never saved, because it can always be regenerated.

## Toolchain and secrets

- `mise.toml` pins Rust stable and `just`. The `justfile` holds the tasks (`run`, `test`, `lint`).
- API keys (`OPENROUTER_API_KEY`, `PIXELLAB_SECRET`) live in `.env.local`. That file is gitignored, and mise
  loads it into the environment. If a key is missing, the game runs offline.

## Open questions

- Food and rest: survival pressure on the wanderer, or just flavor?
- Towns, people, quests: do they exist, and what do they offer (trade, rumors of strange species, pen
  upgrades)?
- How a region's size cap and the chunk size interact with sim cost at the frontier.
- What, if anything, the end game is.
