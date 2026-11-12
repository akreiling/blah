# Isle world art

The map is painted in code: ground in `src/art.rs`, palettes and objects in `src/paint.rs`. Creature bodies
remain genome-generated. The player is a code-drawn figure shaped by practice and current gear; fixed-table
creatures keep text markers.

Each ground pixel is computed from its world position. Built ground (town streets and walls) owns exactly its
cell's square. Natural ground is warped by seeded world-space noise, and its terrain at a pixel is the vote of
the four nearest cell centers, weighted by distance, so corners round off. Underground, floors and walls are
natural too, giving organic caves. Edge shading reads the terrain a few pixels away: wet sand and shallows at
shores, darker grass at path and wall edges, pale rims on cave walls.

Palettes follow the biome (`style_of`): Moss (temperate), Fable (warm lowlands), and Hearth (cold, wet, or
high). A noisy threshold picks a nearby cell's look per pixel, giving biome borders a ragged band rather
than a blur. Marshes grow reeds around damp earth and puddles; jungles have dark, dense growth; savannas
have straw-colored grass and bare patches; deserts have lit dune crests and shaded faces. The Deep paints
in its stone (limestone, slate, or basalt) instead. Grass tufts and sand ripples sit on a shared 9px world
lattice and grow only where their whole footprint has that terrain. Water darkens away from shore, with
ripple highlights and glitter. Rock has lit rims, shaded faces, and scree; snow gathers in drifts.

Trees and boulders vary in size as well as mirroring. Jungles grow heavy canopies, savannas and deserts
acacias, and marshes willows. Object images have 12 pixels of headroom above their 32px cell, so big crowns
rise above the grid; the map frame clips the outermost crowns. Stairs keep a small `<` or `>` badge above
actors, including the hero standing on them.

Discovered wilderness cairns, camps, and shrines have their own code-drawn stonework, canvas, and ruins.
They remain on remembered ground; a `?` badge disappears permanently when their cache is opened.
Items left behind render over the site, with creatures and the player keeping ordinary actor priority.
Preview all three in grass, sand, and snow with
`mise exec -- cargo test -p isle landmark_contact_sheet -- --ignored`
(`target/isle-world-art/landmark-contact.png`).

The renderer uses only known terrain within two cells (looks within one), substituting the center's ground,
tint, and look for unknown cells. Cached ground images are per position and neighborhood, evicted when
unused, and rebuilt at display-scale changes. Preview three biomes with coasts, roads, walled yards,
objects, and items with
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
overcast/clear/mist changes; a separate 180 ms visual timer cycles eight cached grass-sway and water-glitter
frames. Mist wisps drift slowly over the ground. A separate shade layer dims remembered land and softens
the edges of sight with ragged, dithered darkness. Like natural ground, the fog blends the nearest cell
centers at a warped point, so steps of cells read as curves and patch corners round off; it also covers
trees, whose crowns thin where they would hide an actor. Unknown land stays blank; sight rules do not change.
Deep geology comes in seeded three-floor bands, with broad stone veins, damp slate patches, and narrow
basalt seams. Seam heat and visible living cave light pulse through GPU opacity; remembered ground does
not animate. Heroes and dropped items keep their original colors. Animation never changes rules or saves.
Only visible cells retain their animation frames; old mist steps, unused images, and display-scale
variants are released.
Preview palettes with `mise exec -- cargo test -p isle mood_contact_sheet -- --ignored`
(`target/isle-world-art/mood-contact.png`), or launch with `--sky overcast|clear|mist` and
`--stone limestone|slate|basalt`. The cost test above includes cold, sway-fill, cached, and scrolling frames.

Preview all twelve land biomes and the edge of sight with
`mise exec -- cargo test -p isle biome_contact_sheet -- --ignored`
(`target/isle-world-art/biome-contact.png`). Review the contact sheets and the real game window before
committing changed art. No external world-art assets or generation service are needed.
