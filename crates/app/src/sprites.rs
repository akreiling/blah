//! Creature sprites as GPU images, built once and kept while they're on screen.
//!
//! GPUI samples images with a linear filter, which would smear a 16×16 sprite stretched over a tile. So each
//! sprite is enlarged here by an integer factor with nearest-neighbor, to exactly the device pixels it
//! covers on screen, and the GPU draws it 1:1. GPUI also snaps image origins to whole device pixels, so the
//! result stays crisp.
//!
//! An image is made the first time its sprite is drawn and kept until a frame passes without it, at which
//! point it leaves GPUI's texture atlas too. Monsters are keyed by their id (each has its own sprite, which
//! never changes), species by theirs.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{RenderImage, Window};
use image::{Frame, RgbaImage};

use blah_core::{MonsterId, SPRITE_SIZE, SpeciesId, Sprite};

/// Side of a sprite on screen, in logical pixels: twice its own size, so every sprite pixel is a whole
/// number of device pixels at the usual scale factors.
pub const SPRITE_PX: f32 = 2.0 * SPRITE_SIZE as f32;

/// What a cached image shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// One monster's own sprite.
    Monster(MonsterId),
    /// A species' typical sprite.
    Species(SpeciesId),
}

/// Sprite images by what they show, at the window's current scale.
#[derive(Default)]
pub struct SpriteCache {
    /// Device pixels per sprite pixel that the images were made at.
    factor: usize,
    images: HashMap<Key, Arc<RenderImage>>,
    /// Keys asked for since [`SpriteCache::begin`].
    used: HashSet<Key>,
}

impl SpriteCache {
    /// Starts a frame. If the window's scale changed (it moved to another display), every image is remade.
    pub fn begin(&mut self, window: &mut Window) {
        let factor = ((SPRITE_PX * window.scale_factor()) / SPRITE_SIZE as f32).round().max(1.0) as usize;
        if factor != self.factor {
            for (_, image) in self.images.drain() {
                window.drop_image(image).ok();
            }
            self.factor = factor;
        }
        self.used.clear();
    }

    /// The image for `key`, made from `sprite` if it isn't cached yet.
    pub fn get(&mut self, key: Key, sprite: &Sprite) -> Arc<RenderImage> {
        self.used.insert(key);
        let factor = self.factor.max(1);
        self.images.entry(key).or_insert_with(|| enlarge(sprite, factor)).clone()
    }

    /// Ends a frame: images not asked for since [`SpriteCache::begin`] are released.
    pub fn end(&mut self, window: &mut Window) {
        let stale: Vec<Key> = self.images.keys().filter(|k| !self.used.contains(k)).copied().collect();
        for key in stale {
            if let Some(image) = self.images.remove(&key) {
                window.drop_image(image).ok();
            }
        }
    }
}

/// `sprite` scaled up `factor` times with nearest-neighbor, as a GPUI image.
fn enlarge(sprite: &Sprite, factor: usize) -> Arc<RenderImage> {
    let side = SPRITE_SIZE * factor;
    let mut bytes = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            // GPUI wants BGRA with straight alpha; sprites are RGBA with straight alpha.
            let [r, g, b, a] = sprite.pixel(x / factor, y / factor);
            bytes.extend_from_slice(&[b, g, r, a]);
        }
    }
    // The buffer type says RGBA, but GPUI only reads the bytes, which are BGRA as it expects.
    let buffer = RgbaImage::from_raw(side as u32, side as u32, bytes).expect("buffer is sized to the image");
    Arc::new(RenderImage::new([Frame::new(buffer)]))
}
