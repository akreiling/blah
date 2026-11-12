# Isle world art

The map is painted in code: ground in `src/art.rs`, palettes and objects in `src/paint.rs`. Creature bodies
remain genome-generated. The player is a code-drawn figure shaped by practice and current gear; fixed-table
creatures keep text markers.

The PixelLab tilesets and object sheet in this directory (`terrain_<set>.png`, `objects.png`, `manifest.json`)
are no longer drawn. They stay as a record of the earlier art and of what the generation tools produced.

Each ground pixel is computed from its world position. Built ground (town streets and walls) owns exactly its
cell's square. Natural ground is warped by seeded world-space noise, and its terrain at a pixel is the vote of
the four nearest cell centers, weighted by distance, so corners round off. Underground, floors and walls are
natural too, giving organic caves. Edge shading reads the terrain a few pixels away: wet sand and shallows at
shores, darker grass at path and wall edges, pale rims on cave walls.

Palettes follow the biome (`style_of`): Moss (temperate), Fable (warm lowlands), and Hearth (cold, wet, or
high). The four nearest cell centers blend the palettes, so looks change gradually across a border. The Deep
paints in its stone (limestone, slate, or basalt) instead. Grass tufts and sand ripples sit on a shared 9px
world lattice and grow only where their whole footprint has that terrain.

The renderer uses only known terrain within one cell, substituting the center's ground, tint, and look for
unknown cells. Cached ground images are per position and neighborhood, evicted when unused, and rebuilt at
display-scale changes. Preview three biomes with coasts, roads, walled yards, objects, and items with
`mise exec -- cargo test -p isle contact_sheet -- --ignored`
(`target/isle-world-art/paint-contact.png`). Measure CPU raster/cache costs with
`mise exec -- cargo test -p isle ground_render_cost -- --ignored --nocapture`.

The player's 32×32 figure is drawn locally in `src/hero.rs`, with no generated assets. The highest weapon,
magic, or stealth practice rank chooses the clothing and headgear; ties favor the held weapon, then the
earlier practice in the skills list. Novices remain wanderers, and Skilled or better adds gold trim.
Equipped weapons, bows, armor, and rings layer onto the body. A yellow ring at the feet marks the player;
a translucent shadow inside it and a dark rim around it ground the figure and keep the ring legible on pale
sand and stone. The figure keeps its colors under every sky and stone. Preview the callings and gear, then
the same figures on grass under each sky, sand, dirt, rock, water, and each Deep stone, with
`mise exec -- cargo test -p isle hero_contact_sheet -- --ignored`
(`target/isle-world-art/hero-contact.png`).

Presentation-only weather grades surface ground and scenery in world-space patches. The round drives slow
overcast/clear/mist changes; a separate 180 ms visual timer cycles eight cached grass-sway frames.
Deep geology comes in seeded three-floor bands, with broad stone veins, damp slate patches, and narrow
basalt seams. Seam heat and visible living cave light pulse through GPU opacity; remembered ground does
not animate. Heroes and dropped items keep their original colors. Animation never changes rules or saves.
Only visible cells retain their sway frames; unused images and display-scale variants are released.
Preview palettes with `mise exec -- cargo test -p isle mood_contact_sheet -- --ignored`
(`target/isle-world-art/mood-contact.png`), or launch with `--sky overcast|clear|mist` and
`--stone limestone|slate|basalt`. The cost test above includes cold, sway-fill, cached, and scrolling frames.

## The PixelLab library (retired)

Use the existing gitignored `.env.local` entry `PIXELLAB_SECRET`, loaded by mise. Never paste the key into a
command, source file, manifest, or log.

```sh
python3 -m venv target/pixellab-venv
target/pixellab-venv/bin/python -m pip install 'Pillow>=12,<14'
target/pixellab-venv/bin/python crates/isle/scripts/world_art.py status
mise exec -- target/pixellab-venv/bin/python crates/isle/scripts/world_art.py generate terrain-shore --budget 5
target/pixellab-venv/bin/python crates/isle/scripts/world_art.py process
target/pixellab-venv/bin/python crates/isle/scripts/world_art.py contact
target/pixellab-venv/bin/python -m unittest discover -s crates/isle/scripts -p test_world_art.py
```

Run from the repository root. Library entries are defined in `world_art.py`; add one there, then generate it.
Each run spends at most `--budget` generations (default 5, the trial's daily slow allowance), so the library
can grow a few entries a day. Generation sends only the art descriptions and style settings to PixelLab's
documented REST API, never game saves, source files, or personal content. The authorization token goes only
to `api.pixellab.ai`; redirects are refused.

Originals and receipts stay under ignored `target/isle-world-art/library/`. An entry with a receipt is never
generated again. A job still running when a run ends is recorded and collected by the next run, not
resubmitted; a request whose outcome is unknown blocks that entry until inspected. `process`, `contact`,
and `status` work offline and cost nothing. Review the contact sheet and the real game window before
committing changed art.
