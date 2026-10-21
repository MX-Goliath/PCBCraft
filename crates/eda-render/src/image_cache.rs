//! Textures are retained only while used by the current/previous egui pass.
//! In particular closing documents, hiding images or replacing image contents
//! must release old TextureHandles rather than leave them in egui temp data.
use base64::{Engine, engine::general_purpose::STANDARD};
use egui::{Context, Id, TextureHandle};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};
use uuid::Uuid;

struct Entry {
    used: bool,
    texture: Option<TextureHandle>,
}
#[derive(Default)]
struct ImageCache {
    entries: HashMap<Id, Entry>,
}
impl egui::cache::CacheTrait for ImageCache {
    fn update(&mut self) {
        self.entries.retain(|_, entry| {
            let used = entry.used;
            entry.used = false;
            used
        });
    }
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub(super) fn texture(
    ctx: &Context,
    document: Uuid,
    filename: &str,
    encoded: Option<&str>,
) -> Option<TextureHandle> {
    let mut fingerprint = std::collections::hash_map::DefaultHasher::new();
    encoded.hash(&mut fingerprint);
    let key = Id::new((document, filename, fingerprint.finish()));
    let cached = ctx.memory_mut(|memory| {
        memory
            .caches
            .cache::<ImageCache>()
            .entries
            .get_mut(&key)
            .map(|entry| {
                entry.used = true;
                entry.texture.clone()
            })
    });
    if let Some(texture) = cached {
        return texture;
    }
    // Decoding and load_texture must run outside the egui memory lock.
    let texture = encoded
        .and_then(decode)
        .map(|pixels| ctx.load_texture(filename, pixels, egui::TextureOptions::LINEAR));
    ctx.memory_mut(|memory| {
        memory.caches.cache::<ImageCache>().entries.insert(
            key,
            Entry {
                used: true,
                texture: texture.clone(),
            },
        );
    });
    texture
}

fn decode(encoded: &str) -> Option<egui::ColorImage> {
    let bytes = STANDARD.decode(encoded).ok()?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.into_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png() -> String {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        STANDARD.encode(bytes.into_inner())
    }
    fn allocated(ctx: &Context) -> usize {
        ctx.tex_manager()
            .read()
            .allocated()
            .filter(|(_, meta)| meta.name == "probe.png")
            .count()
    }
    #[test]
    fn textures_survive_repaint_but_not_closed_documents() {
        let ctx = Context::default();
        let encoded = png();
        for _ in 0..50 {
            let document = Uuid::new_v4();
            let mut previous = None;
            for _ in 0..3 {
                let _ = ctx.run(Default::default(), |ctx| {
                    let handle = texture(ctx, document, "probe.png", Some(&encoded)).unwrap();
                    if let Some(id) = previous {
                        assert_eq!(handle.id(), id);
                    }
                    previous = Some(handle.id());
                });
                assert_eq!(allocated(&ctx), 1);
            }
            // No image is drawn once the document/panel is closed.
            let _ = ctx.run(Default::default(), |_| {});
            assert_eq!(allocated(&ctx), 0);
        }
    }
    #[test]
    fn changed_image_contents_replace_cached_texture_and_hidden_images_expire() {
        let ctx = Context::default();
        let document = Uuid::new_v4();
        let encoded = png();
        let mut old = None;
        let _ = ctx.run(Default::default(), |ctx| {
            old = Some(
                texture(ctx, document, "probe.png", Some(&encoded))
                    .unwrap()
                    .id(),
            );
        });
        let _ = ctx.run(Default::default(), |ctx| {
            assert!(texture(ctx, document, "probe.png", Some("invalid image")).is_none());
        });
        assert!(ctx.tex_manager().read().meta(old.unwrap()).is_none());
        let _ = ctx.run(Default::default(), |_| {});
        ctx.memory_mut(|memory| assert!(memory.caches.cache::<ImageCache>().entries.is_empty()));
    }
}
