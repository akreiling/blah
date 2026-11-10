//! Regions: the ecological units that populations will live in and migrate between.
//!
//! A region is a pure function of `(seed, tile)`, never of which tile was asked about first. It is built in
//! three steps, each one local:
//!
//! 1. **Cells.** The plane is split into Voronoi cells around one hashed point per [`CELL_SPACING`]-wide
//!    grid square. A tile belongs to the cell of its nearest point. Cells bound region size, so a continent
//!    of jungle still breaks into walkable regions.
//! 2. **Pieces.** Within one cell, 4-connected tiles of the same biome form a component. Components under
//!    [`SLIVER_TILES`] (a scrap of beach, a lone hilltop) merge into a neighboring component of the cell
//!    (see [`merge`]), and what results are the cell's pieces. A piece only depends on its own cell.
//! 3. **Joins.** A piece of at least [`CORE_TILES`] is the core of a region. A smaller piece is usually a
//!    biome that a cell edge has cut off from the rest of itself, so it joins the adjacent core of the same
//!    biome, in its own cell or across the edge. A piece under [`STRAY_TILES`] may join a core of another
//!    biome when none of its own is adjacent. Joins only ever go from a non-core to a core, so they never
//!    chain, and a region is its core plus the pieces that chose it.
//!
//! Water only ever merges or joins with water, and land with land.
//!
//! A region's id is the first tile of its core in row-major order (smallest `y`, then `x`), which names it
//! the same way whichever of its tiles was looked up.
//!
//! Labeling a cell samples every tile in it, which costs about a millisecond. Finding a region needs its
//! own cell and its neighbors, and describing one needs the ring of cells beyond that. [`World`]'s region
//! methods label from scratch on every call. A caller that asks often (the game, every step) keeps a
//! [`RegionCache`], which remembers recently labeled cells and returns exactly what the uncached methods do.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fmt;

use crate::biome::BiomeId;
use crate::hash::{derive, hash_point};
use crate::names;
use crate::world::World;

/// Spacing of the grid that cell points are jittered from, in tiles. A cell averages this squared in area,
/// about 2,300 tiles. A region is a share of one cell plus pieces of its neighbors that joined it.
pub const CELL_SPACING: i64 = 48;

/// Components under this many tiles merge into a neighboring component in their cell, of the same kind
/// (water or land), before anything else happens.
pub const SLIVER_TILES: u32 = 32;

/// Pieces of at least this many tiles are region cores; smaller pieces join an adjacent core if one fits.
pub const CORE_TILES: u32 = 800;

/// Pieces under this many tiles may join a core of another biome when no core of their own biome is
/// adjacent. Larger pieces only join their own biome, and otherwise stand alone.
pub const STRAY_TILES: u32 = 400;

/// The most tiles a region may have. A core stops taking pieces that would carry it past this; since a
/// core is at most one cell, about 3,400 tiles, the cap binds only where many pieces crowd one core.
pub const MAX_REGION_TILES: u32 = 5_000;

/// How far a cell point may sit from its square's center, along each axis, in tiles.
///
/// Kept under a third of the spacing so that a tile's nearest point is always in the 3×3 squares around
/// its own. With the point anywhere in its square, a point two squares away can be the nearest one.
/// Exactness: the farthest a tile can be from its own square's point is `√2 · (S/2 + J)`, and the nearest a
/// point two squares away can be is `S + (S/2 - J)`; at `J = 5S/16` that is `1.15 S < 1.19 S`.
const JITTER: i64 = CELL_SPACING * 5 / 16;

/// A bound on how far a tile can be from its own cell's point, along each axis. A tile's own cell point is
/// at least as near as its square's point, and that one is at most `√2 · (S/2 + J)` away.
const REACH: i64 = (2 * (CELL_SPACING / 2 + JITTER) * (CELL_SPACING / 2 + JITTER)).isqrt() + 1;

/// Marks a tile, in a cell's label grid, as belonging to another cell.
const OUTSIDE: u16 = u16::MAX;

/// Identifies one region of one world: the first tile of the region's core in row-major order.
///
/// Ordered by that tile's `y`, then `x`. An id only means something in the world it came from.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionId {
    // Field order is the sort order.
    y: i64,
    x: i64,
}

impl RegionId {
    /// The first tile of the region's core in row-major order, as `(x, y)`. It is always in the region.
    #[must_use]
    pub const fn anchor(self) -> (i64, i64) {
        (self.x, self.y)
    }
}

impl fmt::Debug for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegionId({}, {})", self.x, self.y)
    }
}

/// An inclusive rectangle of tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bounds {
    /// Westmost column.
    pub min_x: i64,
    /// Northmost row.
    pub min_y: i64,
    /// Eastmost column.
    pub max_x: i64,
    /// Southmost row.
    pub max_y: i64,
}

impl Bounds {
    /// Whether `(x, y)` lies inside, edges included.
    #[must_use]
    pub const fn contains(&self, x: i64, y: i64) -> bool {
        self.min_x <= x && x <= self.max_x && self.min_y <= y && y <= self.max_y
    }

    const fn union(self, o: Bounds) -> Bounds {
        Bounds {
            min_x: if o.min_x < self.min_x { o.min_x } else { self.min_x },
            min_y: if o.min_y < self.min_y { o.min_y } else { self.min_y },
            max_x: if o.max_x > self.max_x { o.max_x } else { self.max_x },
            max_y: if o.max_y > self.max_y { o.max_y } else { self.max_y },
        }
    }
}

/// Mean field values over a region's tiles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Means {
    /// Mean elevation.
    pub elevation: f32,
    /// Mean moisture.
    pub moisture: f32,
    /// Mean temperature.
    pub temperature: f32,
    /// Mean age, which decides how much history the region has when it is revealed.
    pub age: f32,
}

/// Everything terrain knows about one region.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    /// The region's id.
    pub id: RegionId,
    /// The placeholder name, such as "the Glass Marsh". Starts lowercase when it starts with "the", so it
    /// reads right mid-sentence.
    pub name: String,
    /// The biome of the core's largest component: what the region is. Its other tiles are either slivers
    /// under [`SLIVER_TILES`] or pieces under [`STRAY_TILES`] that joined it. They are always water if this
    /// is water, and land if it is land.
    pub biome: BiomeId,
    /// Number of tiles.
    pub tiles: u32,
    /// The smallest rectangle holding every tile.
    pub bounds: Bounds,
    /// A tile of the region's own biome in its core, as far from the core's edge as any: a place to stand
    /// that is unmistakably here.
    pub representative: (i64, i64),
    /// Mean field values over all its tiles.
    pub mean: Means,
}

impl World {
    /// The region holding tile `(x, y)`. Labels cells from scratch; see [`RegionCache`] to keep the work.
    /// Defined for `|x|, |y| ≤ 2^62`.
    #[must_use]
    pub fn region_at(&self, x: i64, y: i64) -> RegionId {
        RegionCache::new(*self).region_at(x, y)
    }

    /// The region `id` names, or `None` if it isn't a region of this world.
    #[must_use]
    pub fn region(&self, id: RegionId) -> Option<Region> {
        RegionCache::new(*self).region(id)
    }

    /// The regions sharing an edge with region `id`, sorted and without repeats. Empty if `id` isn't a region
    /// of this world.
    #[must_use]
    pub fn region_neighbors(&self, id: RegionId) -> Vec<RegionId> {
        RegionCache::new(*self).region_neighbors(id)
    }
}

/// Memoized region lookups for one world, owned by whoever asks often.
///
/// Holds up to a fixed number of labeled cells and drops the least recently used. Every answer is equal to
/// the uncached [`World`] method's; the cache only saves time.
#[derive(Clone)]
pub struct RegionCache {
    world: World,
    seed: u64,
    cells: HashMap<(i64, i64), Cached>,
    capacity: usize,
    clock: u64,
}

#[derive(Clone)]
struct Cached {
    labels: CellLabels,
    /// The core each piece belongs to, once worked out.
    roots: Vec<Option<PieceRef>>,
    /// For each core, once worked out, the pieces of its region.
    members: Vec<Option<Vec<PieceRef>>>,
    /// The pieces adjacent to each piece, once worked out.
    adjacent: Vec<Option<Vec<PieceRef>>>,
    used: u64,
}

impl fmt::Debug for RegionCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegionCache").field("cells", &self.cells.len()).field("capacity", &self.capacity).finish()
    }
}

/// One piece: a cell and an index into its pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PieceRef {
    cell: (i64, i64),
    index: u16,
}

/// What joining needs to know about a piece.
struct Key {
    id: RegionId,
    biome: BiomeId,
    water: bool,
    tiles: u32,
}

impl Piece {
    fn key(&self) -> Key {
        Key { id: self.id, biome: self.biome, water: self.water, tiles: self.tiles }
    }
}

/// How much a piece wants to join a core, highest first: its own biome, then the larger core, then the one
/// whose id comes first.
type JoinKey = (bool, u32, std::cmp::Reverse<RegionId>);

/// Whether piece `p` may join `core`, and if so how much it wants to. Only water joins water and land
/// joins land; a piece joins another biome only if it is under [`STRAY_TILES`].
fn join_key(p: &Key, core: &Key) -> Option<JoinKey> {
    let own = core.biome == p.biome;
    (core.water == p.water && (own || p.tiles < STRAY_TILES)).then_some((own, core.tiles, std::cmp::Reverse(core.id)))
}

impl RegionCache {
    /// Cells a [`RegionCache::new`] cache holds: some 300,000 tiles, at about ten kilobytes per cell.
    /// Describing a region and listing its neighbors touch up to a few dozen cells.
    pub const DEFAULT_CAPACITY: usize = 128;

    /// An empty cache for `world` holding [`Self::DEFAULT_CAPACITY`] cells.
    #[must_use]
    pub fn new(world: World) -> RegionCache {
        RegionCache::with_capacity(world, RegionCache::DEFAULT_CAPACITY)
    }

    /// An empty cache for `world` holding up to `capacity` cells (at least one). Any capacity gives the
    /// same answers; a small one only relabels more.
    #[must_use]
    pub fn with_capacity(world: World, capacity: usize) -> RegionCache {
        RegionCache { world, seed: cell_seed(&world), cells: HashMap::new(), capacity: capacity.max(1), clock: 0 }
    }

    /// The world this cache answers for.
    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }

    /// Cells currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether no cells are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// See [`World::region_at`].
    pub fn region_at(&mut self, x: i64, y: i64) -> RegionId {
        let piece = self.piece_at(x, y);
        let root = self.root(piece);
        self.piece(root).id
    }

    /// See [`World::region`].
    pub fn region(&mut self, id: RegionId) -> Option<Region> {
        let root = self.core(id)?;
        let core = self.piece(root);
        let (biome, representative) = (core.biome, core.representative);
        let (mut tiles, mut bounds, mut sums) = (0, core.bounds, [0.0; 4]);
        for member in self.members(root) {
            let piece = self.piece(member);
            tiles += piece.tiles;
            bounds = bounds.union(piece.bounds);
            for (sum, add) in sums.iter_mut().zip(piece.sums) {
                *sum += add;
            }
        }
        let n = f64::from(tiles);
        let [elevation, moisture, temperature, age] = sums.map(|s| (s / n) as f32);
        Some(Region {
            id,
            name: names::region_name(self.world.seed(), id, biome),
            biome,
            tiles,
            bounds,
            representative,
            mean: Means { elevation, moisture, temperature, age },
        })
    }

    /// See [`World::region_neighbors`].
    pub fn region_neighbors(&mut self, id: RegionId) -> Vec<RegionId> {
        let Some(root) = self.core(id) else {
            return Vec::new();
        };
        let mut found = BTreeSet::new();
        for member in self.members(root) {
            for other in self.adjacent(member) {
                let other = self.root(other);
                if other != root {
                    found.insert(self.piece(other).id);
                }
            }
        }
        found.into_iter().collect()
    }

    /// The core that `id` names, if it names one.
    fn core(&mut self, id: RegionId) -> Option<PieceRef> {
        let piece = self.piece_at(id.x, id.y);
        (self.piece(piece).id == id && self.root(piece) == piece).then_some(piece)
    }

    /// A region's pieces, core first, given its core: the core and every piece it admitted.
    ///
    /// The candidates are the pieces that want this core (see [`Self::wanted`]): those touching it, and
    /// those one step further out that reach it through one of the first. The core takes the ones touching
    /// it first, then the others, each in [`join_key`] order (then by id), skipping any that would take the
    /// region past [`MAX_REGION_TILES`] and, among the far ones, any whose way in was skipped. A piece the
    /// core skips stands alone.
    fn members(&mut self, core: PieceRef) -> Vec<PieceRef> {
        if let Some(members) = &self.cell(core.cell).members[core.index as usize] {
            return members.clone();
        }
        let members = self.admit(core);
        if let Some(cached) = self.cells.get_mut(&core.cell) {
            cached.members[core.index as usize] = Some(members.clone());
        }
        members
    }

    fn admit(&mut self, core: PieceRef) -> Vec<PieceRef> {
        let c = self.piece(core).key();
        if c.tiles < CORE_TILES {
            return vec![core];
        }
        let mut near = Vec::new();
        for other in self.adjacent(core) {
            if self.wanted(other) == Some(core) {
                let o = self.piece(other).key();
                near.push((join_key(&o, &c).expect("a piece only wants a core it may join"), other, o.tiles));
            }
        }
        let mut far = BTreeSet::new();
        for &(_, via, _) in &near {
            for other in self.adjacent(via) {
                if other != core && !near.iter().any(|n| n.1 == other) && self.wanted(other) == Some(core) {
                    far.insert(other);
                }
            }
        }
        let mut far: Vec<_> = far
            .into_iter()
            .map(|other| {
                let o = self.piece(other).key();
                (join_key(&o, &c).expect("a piece only wants a core it may join"), other, o.tiles)
            })
            .collect();
        // Highest join key first, then lowest piece.
        near.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        far.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut members = vec![core];
        let mut total = c.tiles;
        for (_, other, tiles) in near {
            if total + tiles <= MAX_REGION_TILES {
                total += tiles;
                members.push(other);
            }
        }
        let admitted_near = members.clone();
        for (_, other, tiles) in far {
            if total + tiles <= MAX_REGION_TILES && self.adjacent(other).iter().any(|a| admitted_near[1..].contains(a))
            {
                total += tiles;
                members.push(other);
            }
        }
        members
    }

    fn piece_at(&mut self, x: i64, y: i64) -> PieceRef {
        let cell = cell_of(self.seed, x, y);
        let index = self.cell(cell).labels.label(x, y).expect("a tile lies inside its own cell's label grid");
        PieceRef { cell, index }
    }

    fn piece(&mut self, piece: PieceRef) -> &Piece {
        &self.cell(piece.cell).labels.pieces[piece.index as usize]
    }

    /// The pieces sharing an edge with `piece`, in its cell and across the cell's edge, sorted.
    fn adjacent(&mut self, piece: PieceRef) -> Vec<PieceRef> {
        let cached = self.cell(piece.cell);
        if let Some(found) = &cached.adjacent[piece.index as usize] {
            return found.clone();
        }
        let p = &cached.labels.pieces[piece.index as usize];
        let mut found: BTreeSet<PieceRef> =
            p.inside.iter().map(|&index| PieceRef { cell: piece.cell, index }).collect();
        // Tiles across the edge are looked up after letting go of this cell, which looking them up may evict.
        let across = p.across.clone();
        for (x, y) in across {
            found.insert(self.piece_at(x, y));
        }
        let found: Vec<PieceRef> = found.into_iter().collect();
        if let Some(cached) = self.cells.get_mut(&piece.cell) {
            cached.adjacent[piece.index as usize] = Some(found.clone());
        }
        found
    }

    /// The core of the region `piece` is in: `piece` itself for a core or a piece left alone.
    fn root(&mut self, piece: PieceRef) -> PieceRef {
        if let Some(root) = self.cell(piece.cell).roots[piece.index as usize] {
            return root;
        }
        let root = match self.wanted(piece) {
            Some(core) if self.members(core).contains(&piece) => core,
            _ => piece,
        };
        // Working it out can evict this piece's cell when the capacity is tiny; the memo is then simply lost.
        if let Some(cached) = self.cells.get_mut(&piece.cell) {
            cached.roots[piece.index as usize] = Some(root);
        }
        root
    }

    /// The core a non-core piece wants to join, whether or not the core takes it.
    ///
    /// That is the best adjacent core it may join (see [`Self::core_for`]). Failing that, it is the core that
    /// an adjacent piece of its kind wants, if the piece may join that core by the same rules. That is as far
    /// as joins reach: two steps, never a chain.
    fn wanted(&mut self, piece: PieceRef) -> Option<PieceRef> {
        let p = self.piece(piece).key();
        if p.tiles >= CORE_TILES {
            return None;
        }
        if let Some(core) = self.core_for(piece, &p) {
            return Some(core);
        }
        let mut best: Option<(JoinKey, PieceRef)> = None;
        for other in self.adjacent(piece) {
            let o = self.piece(other).key();
            if o.tiles >= CORE_TILES || o.water != p.water {
                continue;
            }
            let Some(core) = self.core_for(other, &o) else { continue };
            let c = self.piece(core).key();
            if let Some(key) = join_key(&p, &c)
                && best.is_none_or(|(b, _)| key > b)
            {
                best = Some((key, core));
            }
        }
        best.map(|(_, core)| core)
    }

    /// The adjacent core that non-core `piece` (described by `p`) joins directly, if any: the best by
    /// [`join_key`].
    fn core_for(&mut self, piece: PieceRef, p: &Key) -> Option<PieceRef> {
        let mut best: Option<(JoinKey, PieceRef)> = None;
        for other in self.adjacent(piece) {
            let o = self.piece(other).key();
            if o.tiles < CORE_TILES {
                continue;
            }
            if let Some(key) = join_key(p, &o)
                && best.is_none_or(|(b, _)| key > b)
            {
                best = Some((key, other));
            }
        }
        best.map(|(_, core)| core)
    }

    /// The labels of `cell`, from the cache or freshly made.
    fn cell(&mut self, cell: (i64, i64)) -> &mut Cached {
        self.clock += 1;
        let clock = self.clock;
        if !self.cells.contains_key(&cell) {
            if self.cells.len() >= self.capacity {
                let oldest = self.cells.iter().min_by_key(|(_, c)| c.used).map(|(&k, _)| k);
                if let Some(oldest) = oldest {
                    self.cells.remove(&oldest);
                }
            }
            let labels = label_cell(&self.world, self.seed, cell);
            let n = labels.pieces.len();
            self.cells.insert(
                cell,
                Cached { labels, roots: vec![None; n], members: vec![None; n], adjacent: vec![None; n], used: clock },
            );
        }
        let cached = self.cells.get_mut(&cell).expect("just inserted");
        cached.used = clock;
        cached
    }
}

/// The seed cell points are hashed from, apart from the noise fields' seeds.
fn cell_seed(world: &World) -> u64 {
    derive(world.seed(), b"cells___")
}

/// The point of grid square `(gx, gy)`, which its cell is grown around.
fn cell_point(seed: u64, gx: i64, gy: i64) -> (i64, i64) {
    let h = hash_point(seed, gx, gy);
    let span = (2 * JITTER + 1) as u64;
    let jx = (h % span) as i64 - JITTER;
    let jy = ((h >> 32) % span) as i64 - JITTER;
    (gx * CELL_SPACING + CELL_SPACING / 2 + jx, gy * CELL_SPACING + CELL_SPACING / 2 + jy)
}

/// Picks the nearest of the points of the 3×3 squares around `(gx, gy)` for tile `(x, y)`. Ties at equal
/// distance go to the smallest square `y`, then `x`, so the answer never depends on scan order.
fn nearest(x: i64, y: i64, gx: i64, gy: i64, point: impl Fn(i64, i64) -> (i64, i64)) -> (i64, i64) {
    let mut best = (i64::MAX, 0, 0);
    for sy in gy - 1..=gy + 1 {
        for sx in gx - 1..=gx + 1 {
            let (px, py) = point(sx, sy);
            let d2 = (x - px) * (x - px) + (y - py) * (y - py);
            if (d2, sy, sx) < best {
                best = (d2, sy, sx);
            }
        }
    }
    (best.2, best.1)
}

/// The cell (named by its grid square) that tile `(x, y)` belongs to.
fn cell_of(seed: u64, x: i64, y: i64) -> (i64, i64) {
    nearest(x, y, x.div_euclid(CELL_SPACING), y.div_euclid(CELL_SPACING), |sx, sy| cell_point(seed, sx, sy))
}

/// One cell, labeled: which piece each of its tiles is in, and what is known about each piece.
#[derive(Clone, Debug)]
struct CellLabels {
    /// Top-left tile of the label grid, which is the cell's bounding box.
    x0: i64,
    y0: i64,
    width: usize,
    height: usize,
    /// Piece index per tile, row-major, or [`OUTSIDE`] for tiles of other cells.
    labels: Vec<u16>,
    /// The cell's pieces, in order of first tile.
    pieces: Vec<Piece>,
}

/// A merged group of components within one cell.
#[derive(Clone, Debug)]
struct Piece {
    /// The piece's first tile, which is its region's id if it is a core.
    id: RegionId,
    /// Its largest component's biome.
    biome: BiomeId,
    water: bool,
    tiles: u32,
    bounds: Bounds,
    /// Sums of elevation, moisture, temperature, and age, for means.
    sums: [f64; 4],
    representative: (i64, i64),
    /// Pieces of the same cell that share an edge with this one, sorted.
    inside: Vec<u16>,
    /// Tiles of other cells that share an edge with this piece, sorted.
    across: Vec<(i64, i64)>,
}

impl CellLabels {
    /// The piece index of tile `(x, y)`, or `None` if it isn't in this cell.
    fn label(&self, x: i64, y: i64) -> Option<u16> {
        let (dx, dy) = (x - self.x0, y - self.y0);
        if dx < 0 || dy < 0 || dx as usize >= self.width || dy as usize >= self.height {
            return None;
        }
        let label = self.labels[dy as usize * self.width + dx as usize];
        (label != OUTSIDE).then_some(label)
    }

    fn coords(&self, i: usize) -> (i64, i64) {
        (self.x0 + (i % self.width) as i64, self.y0 + (i / self.width) as i64)
    }
}

/// What labeling needs of each tile in a cell.
struct Sampled {
    biome: BiomeId,
    water: bool,
    fields: [f32; 4],
}

/// The tiles of `cell` within its bounding box, `None` for tiles of other cells, plus the box's top-left
/// corner and size.
fn sample_cell(world: &World, seed: u64, (gx, gy): (i64, i64)) -> (Vec<Option<Sampled>>, (i64, i64), usize, usize) {
    let (px, py) = cell_point(seed, gx, gy);
    // Every tile within REACH of the point lies in squares gx-1..=gx+1, whose 3×3 neighborhoods span
    // gx-2..=gx+2; hashing those 25 points once saves hashing nine per tile.
    let mut points = [(0, 0); 25];
    for (i, p) in points.iter_mut().enumerate() {
        *p = cell_point(seed, gx - 2 + (i % 5) as i64, gy - 2 + (i / 5) as i64);
    }
    let point = |sx: i64, sy: i64| {
        let (i, j) = (sx - (gx - 2), sy - (gy - 2));
        debug_assert!((0..5).contains(&i) && (0..5).contains(&j), "square ({sx}, {sy}) outside the 5×5");
        points[(j * 5 + i) as usize]
    };
    let side = 2 * REACH + 1;
    let mut inside = Vec::with_capacity((side * side) as usize);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for y in py - REACH..=py + REACH {
        for x in px - REACH..=px + REACH {
            let mine = nearest(x, y, x.div_euclid(CELL_SPACING), y.div_euclid(CELL_SPACING), point) == (gx, gy);
            if mine {
                (min_x, min_y, max_x, max_y) = (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y));
            }
            inside.push(mine);
        }
    }
    let (width, height) = ((max_x - min_x + 1) as usize, (max_y - min_y + 1) as usize);
    let mut tiles = Vec::with_capacity(width * height);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let i = ((y - (py - REACH)) * side + (x - (px - REACH))) as usize;
            tiles.push(inside[i].then(|| {
                let t = world.tile(x, y);
                Sampled {
                    biome: t.biome,
                    water: t.biome.biome().water,
                    fields: [t.elevation, t.moisture, t.temperature, t.age],
                }
            }));
        }
    }
    (tiles, (min_x, min_y), width, height)
}

/// Splits `cell` into pieces. Depends only on the world and the cell.
fn label_cell(world: &World, seed: u64, cell: (i64, i64)) -> CellLabels {
    let (tiles, (x0, y0), width, height) = sample_cell(world, seed, cell);
    let components = components(&tiles, width, height);
    let groups = merge(&components);

    // Number pieces by their first tile; scanning row-major meets them in that order.
    let mut piece_of_group = vec![OUTSIDE; components.sizes.len()];
    let mut grid = CellLabels { x0, y0, width, height, labels: vec![OUTSIDE; width * height], pieces: Vec::new() };
    for (i, tile) in tiles.iter().enumerate() {
        let Some(tile) = tile else { continue };
        let group = groups.group[components.of[i] as usize] as usize;
        let (x, y) = grid.coords(i);
        if piece_of_group[group] == OUTSIDE {
            piece_of_group[group] = grid.pieces.len() as u16;
            grid.pieces.push(Piece {
                id: RegionId { x, y },
                biome: groups.biome[group],
                water: tile.water,
                tiles: 0,
                bounds: Bounds { min_x: x, min_y: y, max_x: x, max_y: y },
                sums: [0.0; 4],
                representative: (x, y),
                inside: Vec::new(),
                across: Vec::new(),
            });
        }
        let p = piece_of_group[group];
        grid.labels[i] = p;
        let piece = &mut grid.pieces[p as usize];
        piece.tiles += 1;
        piece.bounds = piece.bounds.union(Bounds { min_x: x, min_y: y, max_x: x, max_y: y });
        for (sum, v) in piece.sums.iter_mut().zip(tile.fields) {
            *sum += f64::from(v);
        }
    }

    // Adjacency, within the cell and across its edge.
    for i in 0..tiles.len() {
        let p = grid.labels[i];
        if p == OUTSIDE {
            continue;
        }
        let (x, y) = grid.coords(i);
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            match grid.label(nx, ny) {
                Some(q) if q == p => {}
                Some(q) => grid.pieces[p as usize].inside.push(q),
                None => grid.pieces[p as usize].across.push((nx, ny)),
            }
        }
    }
    for piece in &mut grid.pieces {
        piece.inside.sort_unstable();
        piece.inside.dedup();
        piece.across.sort_unstable();
        piece.across.dedup();
    }

    // Representative tiles: the deepest tile of the piece's own biome, by steps from the piece's edge.
    let depth = depths(&grid.labels, width, height);
    let mut deepest: Vec<Option<u32>> = vec![None; grid.pieces.len()];
    for (i, tile) in tiles.iter().enumerate() {
        let Some(tile) = tile else { continue };
        let p = grid.labels[i] as usize;
        if tile.biome == grid.pieces[p].biome && deepest[p].is_none_or(|d| depth[i] > d) {
            deepest[p] = Some(depth[i]);
            grid.pieces[p].representative = grid.coords(i);
        }
    }
    grid
}

/// The same-biome, 4-connected components of a cell.
struct Components {
    /// Component per tile, or `u32::MAX` outside the cell. Numbered in order of first tile, row-major.
    of: Vec<u32>,
    sizes: Vec<u32>,
    biomes: Vec<BiomeId>,
    water: Vec<bool>,
    /// Components sharing an edge with each component, sorted.
    neighbors: Vec<Vec<u32>>,
}

/// The 4-neighbors of grid index `i` that lie inside the grid.
fn grid_neighbors(i: usize, width: usize, height: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % width, i / width);
    [
        (x + 1 < width).then(|| i + 1),
        (x > 0).then(|| i - 1),
        (y + 1 < height).then(|| i + width),
        (y > 0).then(|| i - width),
    ]
    .into_iter()
    .flatten()
}

fn components(tiles: &[Option<Sampled>], width: usize, height: usize) -> Components {
    let mut of = vec![u32::MAX; tiles.len()];
    let (mut sizes, mut biomes, mut water) = (Vec::new(), Vec::new(), Vec::new());
    let mut stack = Vec::new();
    for start in 0..tiles.len() {
        let Some(first) = &tiles[start] else { continue };
        if of[start] != u32::MAX {
            continue;
        }
        let c = sizes.len() as u32;
        let mut size = 0;
        of[start] = c;
        stack.push(start);
        while let Some(i) = stack.pop() {
            size += 1;
            for n in grid_neighbors(i, width, height) {
                if of[n] == u32::MAX && tiles[n].as_ref().is_some_and(|t| t.biome == first.biome) {
                    of[n] = c;
                    stack.push(n);
                }
            }
        }
        sizes.push(size);
        biomes.push(first.biome);
        water.push(first.water);
    }
    let mut neighbors = vec![Vec::new(); sizes.len()];
    for i in 0..tiles.len() {
        let a = of[i];
        if a == u32::MAX {
            continue;
        }
        for n in grid_neighbors(i, width, height) {
            let b = of[n];
            if b != u32::MAX && b != a {
                neighbors[a as usize].push(b);
            }
        }
    }
    for list in &mut neighbors {
        list.sort_unstable();
        list.dedup();
    }
    Components { of, sizes, biomes, water, neighbors }
}

/// Which merged group each component ended up in, and each group's biome.
struct Groups {
    /// Group per component, as the group's smallest component number.
    group: Vec<u32>,
    /// Biome per group (indexed by group number): its largest component's.
    biome: Vec<BiomeId>,
}

/// Merges sliver components into their neighbors, deterministically.
///
/// Components under [`SLIVER_TILES`] are taken smallest first (ties by first tile). If the group the
/// component is in by then is still under the threshold, it joins the largest neighboring group of the same
/// kind, water or land, ties going to the group whose first tile comes first. A group with no such neighbor
/// stays as it is. A group never holds two components at or above the threshold, so its largest component
/// is unambiguous and gives the piece its biome.
fn merge(c: &Components) -> Groups {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    // Largest by size, then first tile, the order that picks a group's biome.
    let rank = |i: usize| (c.sizes[i], std::cmp::Reverse(i));

    let n = c.sizes.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut size = c.sizes.clone();
    let mut host: Vec<usize> = (0..n).collect();
    let mut neighbors: Vec<BTreeSet<usize>> =
        c.neighbors.iter().map(|l| l.iter().map(|&k| k as usize).collect()).collect();

    let mut order: Vec<usize> = (0..n).filter(|&i| c.sizes[i] < SLIVER_TILES).collect();
    order.sort_by_key(|&i| (c.sizes[i], i));
    for i in order {
        let g = root(&mut parent, i);
        if size[g] >= SLIVER_TILES {
            continue;
        }
        let candidates: BTreeSet<usize> = neighbors[g].iter().map(|&k| root(&mut parent, k)).collect();
        // A root is always its group's smallest component number, so comparing roots compares first tiles.
        let best = candidates
            .into_iter()
            .filter(|&h| h != g && c.water[h] == c.water[g])
            .max_by_key(|&h| (size[h], std::cmp::Reverse(h)));
        let Some(h) = best else { continue };
        let (keep, gone) = (g.min(h), g.max(h));
        parent[gone] = keep;
        size[keep] = size[g] + size[h];
        host[keep] = if rank(host[g]) >= rank(host[h]) { host[g] } else { host[h] };
        let moved = std::mem::take(&mut neighbors[gone]);
        neighbors[keep].extend(moved);
    }

    let group: Vec<u32> = (0..n).map(|i| root(&mut parent, i) as u32).collect();
    let biome = (0..n).map(|g| c.biomes[host[g]]).collect();
    Groups { group, biome }
}

/// Steps from each tile to the nearest tile outside its piece (or outside the cell), by 4-connected moves
/// within the piece. Edge tiles are 0.
fn depths(labels: &[u16], width: usize, height: usize) -> Vec<u32> {
    let mut depth = vec![u32::MAX; labels.len()];
    let mut queue = VecDeque::new();
    for i in 0..labels.len() {
        if labels[i] == OUTSIDE {
            continue;
        }
        let (x, y) = (i % width, i / width);
        let on_grid_edge = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
        if on_grid_edge || grid_neighbors(i, width, height).any(|n| labels[n] != labels[i]) {
            depth[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for n in grid_neighbors(i, width, height) {
            if labels[n] == labels[i] && depth[n] == u32::MAX {
                depth[n] = depth[i] + 1;
                queue.push_back(n);
            }
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_by_three_finds_the_true_nearest_point() {
        // Against a 7×7 brute force, which no jitter can beat.
        let seed = cell_seed(&World::new(8));
        for y in (-3_000_i64..3_000).step_by(7) {
            for x in (-3_000_i64..3_000).step_by(11) {
                let (gx, gy) = (x.div_euclid(CELL_SPACING), y.div_euclid(CELL_SPACING));
                let mut best = (i64::MAX, 0, 0);
                for sy in gy - 3..=gy + 3 {
                    for sx in gx - 3..=gx + 3 {
                        let (px, py) = cell_point(seed, sx, sy);
                        let d2 = (x - px) * (x - px) + (y - py) * (y - py);
                        best = best.min((d2, sy, sx));
                    }
                }
                assert_eq!(cell_of(seed, x, y), (best.2, best.1), "tile ({x}, {y})");
            }
        }
    }

    #[test]
    fn labels_cover_exactly_the_cell() {
        let world = World::new(3);
        let seed = cell_seed(&world);
        for cell in [(0, 0), (-1, 2), (17, -40)] {
            let labels = label_cell(&world, seed, cell);
            // A margin around the label grid too: every tile is labeled iff it is in the cell.
            for y in labels.y0 - 8..labels.y0 + labels.height as i64 + 8 {
                for x in labels.x0 - 8..labels.x0 + labels.width as i64 + 8 {
                    assert_eq!(labels.label(x, y).is_some(), cell_of(seed, x, y) == cell, "tile ({x}, {y})");
                }
            }
        }
    }

    /// Cores in a block of cells, as region ids.
    fn regions_in(cache: &mut RegionCache, cells: impl Iterator<Item = (i64, i64)>) -> Vec<RegionId> {
        let mut ids = Vec::new();
        for cell in cells {
            for index in 0..cache.cell(cell).labels.pieces.len() as u16 {
                let piece = PieceRef { cell, index };
                if cache.root(piece) == piece {
                    ids.push(cache.piece(piece).id);
                }
            }
        }
        ids
    }

    fn block(n: i64) -> impl Iterator<Item = (i64, i64)> {
        (-n..n).flat_map(move |gy| (-n..n).map(move |gx| (gx, gy)))
    }

    #[test]
    fn only_cores_are_regions() {
        let world = World::new(2);
        let mut cache = RegionCache::new(world);
        let id = cache.region_at(500, 500);
        let region = cache.region(id).unwrap();
        // Any tile of the region but its anchor names nothing.
        let (x, y) = region.representative;
        let other = RegionId { x, y };
        assert_ne!(other, id);
        assert_eq!(cache.region(other), None);
        assert_eq!(cache.region_neighbors(other), []);
        assert_eq!(world.region(other), None);
    }

    /// What `Region::biome` promises: every tile of another biome is a sliver of its piece, or in a small
    /// piece that joined, and never of the other kind (water or land).
    #[test]
    fn off_biome_tiles_are_slivers_or_strays() {
        for seed in [0, 4, 42] {
            let world = World::new(seed);
            let mut cache = RegionCache::new(world);
            for cell in [(0, 0), (5, -3), (-20, 11), (40, 40)] {
                let (tiles, _, width, height) = sample_cell(&world, cache.seed, cell);
                let comps = components(&tiles, width, height);
                for (i, tile) in tiles.iter().enumerate() {
                    let Some(tile) = tile else { continue };
                    let piece = PieceRef { cell, index: cache.cell(cell).labels.labels[i] };
                    let (piece_biome, piece_tiles) = (cache.piece(piece).biome, cache.piece(piece).tiles);
                    let root = cache.root(piece);
                    let region_biome = cache.piece(root).biome;
                    assert_eq!(tile.water, region_biome.biome().water, "seed {seed} cell {cell:?}");
                    if tile.biome != piece_biome {
                        let size = comps.sizes[comps.of[i] as usize];
                        assert!(size < SLIVER_TILES, "seed {seed} cell {cell:?}: {size}-tile off-biome component");
                    }
                    if piece_biome != region_biome {
                        assert!(piece_tiles < STRAY_TILES, "seed {seed} cell {cell:?}: {piece_tiles}-tile stray");
                    }
                }
            }
        }
    }

    /// Size distribution over every region whose core lies in a block of cells. Run with `--nocapture` to see
    /// the numbers.
    #[test]
    fn region_sizes_are_bounded() {
        let (mut sizes, mut joined) = (Vec::new(), 0);
        for seed in [1, 4] {
            let mut cache = RegionCache::new(World::new(seed));
            for id in regions_in(&mut cache, block(7)) {
                sizes.push(cache.region(id).unwrap().tiles);
                let root = cache.core(id).unwrap();
                let biome = cache.piece(root).biome;
                for member in cache.members(root) {
                    let piece = cache.piece(member);
                    if piece.biome != biome {
                        joined += u64::from(piece.tiles);
                    }
                }
            }
        }
        sizes.sort_unstable();
        let n = sizes.len();
        let pct = |p: usize| sizes[(n - 1) * p / 100];
        let band = |s: &&u32| (1_000..=4_000).contains(*s);
        let in_band = sizes.iter().filter(band).count();
        let tiles: u64 = sizes.iter().map(|&s| u64::from(s)).sum();
        let tiles_in_band: u64 = sizes.iter().filter(band).map(|&s| u64::from(s)).sum();
        println!(
            "{n} regions: min {} p10 {} p25 {} median {} p75 {} p90 {} max {}; {in_band} ({:.0}%) in \
             1000..=4000, holding {:.0}% of tiles; {:.1}% of tiles joined a region of another biome",
            sizes[0],
            pct(10),
            pct(25),
            pct(50),
            pct(75),
            pct(90),
            sizes[n - 1],
            100.0 * in_band as f64 / n as f64,
            100.0 * tiles_in_band as f64 / tiles as f64,
            100.0 * joined as f64 / tiles as f64,
        );
        assert!(in_band * 2 > n, "only {in_band} of {n} regions in the target band");
        assert!(sizes[n - 1] <= MAX_REGION_TILES, "a region of {} tiles", sizes[n - 1]);
    }
}
