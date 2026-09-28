//! Texture decoding.
//!
//! Textures inside `S3D`/`EQG` archives are `DDS` (usually `DXT1`/`DXT3`/
//! `DXT5`) or 8-bit palettised `BMP`. Both are decoded to straight RGBA8 here.
//! A file's extension is not a reliable guide to its contents: classic zones
//! store `DDS` data under `.bmp` names, so decoding sniffs the magic instead.

use crate::{Error, Result};

/// A decoded, tightly packed RGBA8 texture.
#[derive(Clone)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Row-major, top-left origin, 4 bytes per pixel.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("name", &self.name)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.rgba.len())
            .finish()
    }
}

impl Texture {
    /// Decodes a texture from raw bytes, sniffing the container format.
    pub fn decode(name: &str, data: &[u8]) -> Result<Self> {
        let image = image::load_from_memory(data).map_err(|source| Error::Image {
            name: name.to_owned(),
            source,
        })?;
        let rgba = image.to_rgba8();
        Ok(Self {
            name: name.to_owned(),
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }

    /// Decodes a texture, falling back to a magenta placeholder on failure.
    ///
    /// Asset files in the wild occasionally contain truncated or exotic
    /// textures; a placeholder keeps a zone loadable rather than fatal.
    pub fn decode_or_placeholder(name: &str, data: &[u8]) -> Self {
        match Self::decode(name, data) {
            Ok(texture) => texture,
            Err(error) => {
                tracing::warn!(%error, name, "using placeholder texture");
                Self {
                    name: name.to_owned(),
                    width: 1,
                    height: 1,
                    rgba: vec![255, 0, 255, 255],
                }
            }
        }
    }

    /// A 1x1 placeholder, used when a material references a missing texture.
    pub fn placeholder(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            width: 1,
            height: 1,
            rgba: vec![255, 0, 255, 255],
        }
    }
}
