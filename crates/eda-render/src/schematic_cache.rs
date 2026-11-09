//! Screen geometry is stable during pointer motion. Narrow hit tests remain live.
use super::*;
use std::sync::Arc;

#[derive(Clone)]
struct Entry {
    source: SchPrimitive,
    shapes: Arc<Vec<Shape>>,
    bounds: Rect,
    used: bool,
}

#[derive(Clone, Default)]
pub(crate) struct Cache {
    key: Vec<u64>,
    fonts: Vec<SchFont>,
    images: Option<Arc<std::collections::BTreeMap<String, String>>>,
    entries: HashMap<Uuid, Entry>,
    #[cfg(test)]
    pub builds: usize,
}

impl Cache {
    pub(super) fn begin(&mut self, scene: &Scene<'_>) {
        let clip = scene.painter.clip_rect();
        let mut key = vec![
            scene.camera.centre_mm[0].to_bits(),
            scene.camera.centre_mm[1].to_bits(),
            scene.camera.pixels_per_mm.to_bits(),
            u64::from(scene.camera.mirrored_x),
        ];
        key.extend(
            [
                scene.rect.min.x,
                scene.rect.min.y,
                scene.rect.max.x,
                scene.rect.max.y,
                clip.min.x,
                clip.min.y,
                clip.max.x,
                clip.max.y,
                scene.painter.ctx().pixels_per_point(),
            ]
            .map(|v| u64::from(v.to_bits())),
        );
        let definitions = scene.painter.ctx().fonts(|f| {
            let locked = f.lock();
            let definitions = locked.fonts.definitions();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for (name, data) in &definitions.font_data {
                name.hash(&mut h);
                (Arc::as_ptr(data) as usize).hash(&mut h);
            }
            definitions.families.hash(&mut h);
            h.finish()
        });
        key.push(definitions);
        let fonts = scene.schematic.map_or(&[][..], |s| s.fonts.as_slice());
        let images = scene.schematic.map(|s| &s.images);
        if self.key != key || self.fonts != fonts || self.images.as_ref() != images {
            self.entries.clear();
            self.key = key;
            self.fonts = fonts.to_vec();
            self.images = images.cloned();
        }
        for entry in self.entries.values_mut() {
            entry.used = false;
        }
    }

    pub(super) fn contains(&self, p: &SchPrimitive) -> bool {
        self.entries
            .get(&p.id)
            .is_some_and(|entry| entry.source == *p)
    }

    /// Prepare one object's shapes without displaying an incomplete sheet.
    pub(super) fn prepare(&mut self, scene: &Scene<'_>, p: &SchPrimitive) {
        if !self.contains(p) {
            self.build(scene, p);
        }
        self.entries.get_mut(&p.id).unwrap().used = true;
    }

    fn build(&mut self, scene: &Scene<'_>, p: &SchPrimitive) {
        let layer = scene.painter.layer_id();
        let saved = scene
            .painter
            .ctx()
            .graphics_mut(|g| std::mem::take(g.entry(layer)));
        let (bounds, _) = scene.primitive(p, None);
        let shapes = scene.painter.ctx().graphics_mut(|g| {
            let captured = std::mem::replace(g.entry(layer), saved);
            captured.all_entries().map(|s| s.shape.clone()).collect()
        });
        self.entries.insert(
            p.id,
            Entry {
                source: p.clone(),
                shapes: Arc::new(shapes),
                bounds,
                used: true,
            },
        );
        #[cfg(test)]
        {
            self.builds += 1;
        }
    }

    pub(super) fn primitive(
        &mut self,
        scene: &Scene<'_>,
        p: &SchPrimitive,
        pointer: Option<Pos2>,
    ) -> (Rect, bool) {
        if !self.contains(p) || matches!(p.kind, K::Image { .. }) {
            self.build(scene, p);
        }
        let entry = self.entries.get_mut(&p.id).unwrap();
        entry.used = true;
        scene.painter.extend(entry.shapes.iter().cloned());
        let bounds = entry.bounds;
        let hit = pointer
            .filter(|at| bounds.expand(6.).contains(*at))
            .is_some_and(|at| {
                let measure = Scene {
                    painter: scene.painter.with_clip_rect(Rect::NOTHING),
                    ..*scene
                };
                measure.primitive(p, Some(at)).1
            });
        (bounds, hit)
    }

    pub fn end(&mut self) {
        self.entries.retain(|_, entry| entry.used);
    }
}
