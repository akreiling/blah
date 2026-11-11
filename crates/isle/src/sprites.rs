//! Genome sprites as GPUI images. Ported from blah's frontend: integer nearest-neighbor enlargement
//! keeps the pixels sharp, and only images used in the current frame stay in the texture atlas.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use blah_core::{Clade, SPRITE_SIZE, Sprite, World, clades};
use gpui::{RenderImage, Window};
use image::{Frame, RgbaImage};
use isle_core::{Creature, DEEP, Lineage, Wildlife};

/// A 16-pixel sprite displayed at twice its native size in logical pixels.
pub const SPRITE_PX: f32 = 2.0 * SPRITE_SIZE as f32;

pub struct SpriteCache {
    clades: Vec<Clade>,
    world: World,
    factor: usize,
    scale: f32,
    images: HashMap<u32, Arc<RenderImage>>,
    used: HashSet<u32>,
}

impl SpriteCache {
    pub fn new(seed: u64) -> Self {
        Self {
            clades: clades(seed),
            world: World::new(seed),
            factor: 1,
            scale: 1.0,
            images: HashMap::new(),
            used: HashSet::new(),
        }
    }

    pub fn begin(&mut self, window: &mut Window) {
        for image in self.begin_scale(window.scale_factor()) {
            window.drop_image(image).ok();
        }
    }

    fn begin_scale(&mut self, scale: f32) -> Vec<Arc<RenderImage>> {
        self.scale = scale.max(0.5);
        let factor = ((SPRITE_PX * self.scale) / SPRITE_SIZE as f32).floor().max(1.0) as usize;
        let stale = if factor != self.factor { self.images.drain().map(|(_, image)| image).collect() } else { vec![] };
        self.factor = factor;
        self.used.clear();
        stale
    }

    /// Match the enlarged image's device pixels exactly, even on fractionally scaled displays.
    pub fn display_size(&self) -> f32 {
        (SPRITE_SIZE * self.factor) as f32 / self.scale
    }

    /// Creature ids are stable and genomes never change during an individual's life.
    pub fn get(&mut self, creature: &Creature) -> Option<Arc<RenderImage>> {
        self.get_wild(creature.id, creature.wild.as_ref()?)
    }

    fn get_wild(&mut self, id: u32, wild: &Wildlife) -> Option<Arc<RenderImage>> {
        if !self.images.contains_key(&id) {
            let sprite = self.draw(wild)?;
            self.images.insert(id, enlarge(&sprite, self.factor));
        }
        self.used.insert(id);
        self.images.get(&id).cloned()
    }

    fn draw(&self, wild: &Wildlife) -> Option<Sprite> {
        let (clade, tint) = match wild.lineage {
            Lineage::Region(species) => {
                let (x, y) = species.region.anchor();
                (species.clade, self.world.tile(x, y).biome.biome().color)
            }
            Lineage::Deep { clade, .. } => (clade, DEEP.color),
        };
        let plan = &self.clades.iter().find(|c| c.id == clade)?.plan;
        Some(Sprite::draw_tinted(&wild.phenotype(), plan, tint))
    }

    pub fn end(&mut self, window: &mut Window) {
        for image in self.finish() {
            window.drop_image(image).ok();
        }
    }

    fn finish(&mut self) -> Vec<Arc<RenderImage>> {
        let stale: Vec<u32> = self.images.keys().filter(|id| !self.used.contains(id)).copied().collect();
        stale.into_iter().filter_map(|id| self.images.remove(&id)).collect()
    }
}

/// Nearest-neighbor device pixels, converted from straight RGBA to the straight BGRA GPUI expects.
fn enlarge(sprite: &Sprite, factor: usize) -> Arc<RenderImage> {
    let side = SPRITE_SIZE * factor;
    let mut bytes = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let [r, g, b, a] = sprite.pixel(x / factor, y / factor);
            bytes.extend_from_slice(&[b, g, r, a]);
        }
    }
    let buffer = RgbaImage::from_raw(side as u32, side as u32, bytes).expect("buffer is sized to the image");
    Arc::new(RenderImage::new([Frame::new(buffer)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use blah_core::{Gene, Genome, SpeciesId};

    fn wildlife(seed: u64, deep: bool) -> Wildlife {
        let clade = &clades(seed)[0];
        let lineage = if deep {
            let mut game = isle_core::Game::new(seed);
            game.answer("Ash");
            let town = game.level().towns().next().unwrap();
            Lineage::Deep { town, depth: 1, clade: clade.id }
        } else {
            let region = World::new(seed).region_at(0, 0);
            Lineage::Region(SpeciesId { region, clade: clade.id })
        };
        Wildlife { lineage, name: "Test creature".into(), genome: clade.base }
    }

    #[test]
    fn surface_pixels_match_the_original_renderer_and_deep_uses_its_own_tint() {
        let seed = 7;
        let cache = SpriteCache::new(seed);
        let wild = wildlife(seed, false);
        let Lineage::Region(species) = wild.lineage else { unreachable!() };
        let (x, y) = species.region.anchor();
        let expected = Sprite::draw(&wild.phenotype(), &clades(seed)[0].plan, World::new(seed).tile(x, y).biome);
        assert_eq!(cache.draw(&wild), Some(expected));
        let deep = wildlife(seed, true);
        assert_eq!(cache.draw(&deep), Some(Sprite::draw_tinted(&deep.phenotype(), &clades(seed)[0].plan, DEEP.color)));
        assert_ne!(cache.draw(&wild), cache.draw(&deep));
        let round_trip: Wildlife = serde_json::from_str(&serde_json::to_string(&deep).unwrap()).unwrap();
        assert_eq!(cache.draw(&deep), cache.draw(&round_trip));
    }

    #[test]
    fn individual_genes_change_the_picture() {
        let cache = SpriteCache::new(7);
        let wild = wildlife(7, true);
        let mut changed = wild.clone();
        let mut genes = wild.genome.genes();
        genes[Gene::Size as usize] = 1.0;
        genes[Gene::Hue as usize] = (genes[Gene::Hue as usize] + 0.5) % 1.0;
        changed.genome = Genome::new(genes);
        assert_ne!(cache.draw(&wild), cache.draw(&changed));
    }

    #[test]
    fn frames_reuse_visible_images_evict_hidden_ones_and_rebuild_at_new_scales() {
        let wild = wildlife(7, true);
        let mut cache = SpriteCache::new(7);
        cache.begin_scale(1.0);
        let first = cache.get_wild(1, &wild).unwrap();
        let other = cache.get_wild(2, &wild).unwrap();
        assert!(cache.finish().is_empty());
        cache.begin_scale(1.0);
        assert!(Arc::ptr_eq(&first, &cache.get_wild(1, &wild).unwrap()));
        let stale = cache.finish();
        assert_eq!(stale.len(), 1);
        assert!(Arc::ptr_eq(&other, &stale[0]));
        assert_eq!(cache.images.len(), 1);
        assert_eq!(cache.begin_scale(2.0).len(), 1);
        assert_eq!(cache.factor, 4);
        assert!(!Arc::ptr_eq(&first, &cache.get_wild(1, &wild).unwrap()));
        cache.begin_scale(2.0);
        assert_eq!(cache.finish().len(), 1);
        assert!(cache.images.is_empty(), "leaving the map releases its images");
    }

    #[test]
    fn fractional_scales_keep_device_pixels_integral_and_fit_the_cell() {
        let mut cache = SpriteCache::new(7);
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 3.0] {
            cache.begin_scale(scale);
            assert_eq!(cache.display_size() * scale, (SPRITE_SIZE * cache.factor) as f32);
            assert!(cache.display_size() <= SPRITE_PX);
        }
    }

    #[test]
    fn enlarged_pixels_keep_alpha_and_bgra_without_smoothing() {
        let wild = wildlife(7, true);
        let sprite = SpriteCache::new(7).draw(&wild).unwrap();
        for factor in [1, 2, 3, 4] {
            let image = enlarge(&sprite, factor);
            let bytes = image.as_bytes(0).unwrap();
            let side = SPRITE_SIZE * factor;
            assert_eq!(image.size(0), gpui::size(gpui::DevicePixels(side as i32), gpui::DevicePixels(side as i32)));
            for y in 0..side {
                for x in 0..side {
                    let [r, g, b, a] = sprite.pixel(x / factor, y / factor);
                    let at = (y * side + x) * 4;
                    assert_eq!(&bytes[at..at + 4], &[b, g, r, a]);
                }
            }
        }
    }
}
