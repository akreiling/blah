# Isle world art

PixelLab-generated 32×32 terrain tilesets and object icons for Isle. Creature bodies remain genome-generated,
and the player and fixed-table creatures keep their readable text markers.

- `terrain_<set>.png`: a corner-matched (Wang) tileset joining two terrains, packed 4×4 by corner mask
  (NW = 8, NE = 4, SW = 2, SE = 1, set where the corner is the set's terrain `b`). The game draws each cell as
  four quarters, each from the tile centered on that corner of the cell. The renderer then samples this layout
  through seeded world-space noise, bending textures and boundaries across cell edges without independent
  per-tile seams. Slow brightness drift varies broad fields; water keeps its source cell's biome tint.
- `objects.png`: 16 transparent 32×32 objects in the order listed in the manifest.
- `manifest.json`: the PixelLab requests, seeds, billed usage, and hashes of each original response and each
  checked-in sheet. It contains no credentials.

Local processing never resizes or resamples. Shared terrains (the path set's grass and the rocky set's sand) are
recolored to the shore set's grass and sand so the sets meet without seams; object alpha is made binary. No image
is fetched or generated during play.

Natural ground gets sparse, world-anchored pixel details: clustered grass tufts, sand pebbles and ripples,
and small rock cracks. Meadow colors drift in broad patches, while transition tiles keep their original
colors. These details are drawn locally, not generated or fetched, and never cover built ground or water.
Town streets suppress the warp; Deep floors and walls keep their masonry straight. The rigidity field
interpolates across world positions so neighboring cells still sample the same layout.

The renderer uses only known terrain in its 5×5 neighborhood, substituting the center's ground and tint for
unknown cells. Trees and boulders may be mirrored. Cached ground images are per position and neighborhood,
evicted when unused, and rebuilt at display-scale changes. Terrain pairs without a joining set still have a
hard boundary. Preview the runtime warp offline with
`mise exec -- cargo test -p isle contact_sheet -- --ignored`
(`target/isle-world-art/noise-contact.png`). Measure CPU raster/cache costs with
`mise exec -- cargo test -p isle ground_render_cost -- --ignored --nocapture`.

## Growing the library

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
