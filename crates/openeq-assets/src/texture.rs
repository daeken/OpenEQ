//! Texture decoding.
//!
//! Textures inside `S3D`/`EQG` archives are `DDS` (usually `DXT1`/`DXT3`/
//! `DXT5`, or packed RGB such as ARGB4444) or 8-bit palettised `BMP`. Both are
//! decoded to straight RGBA8 here. DXT cubemaps can also be decoded by face.
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
    /// Decodes the six faces of a DXT-compressed DDS cubemap in DDS order
    /// (+X, -X, +Y, -Y, +Z, -Z), skipping each face's mip chain.
    pub fn decode_cube(name: &str, data: &[u8]) -> Result<Vec<Self>> {
        if data.len() < 128 || !data.starts_with(b"DDS ") {
            return Err(Error::Format(format!("not a DDS cubemap: {name}")));
        }
        let word = |offset| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        if word(112) & 0xFE00 != 0xFE00 {
            return Err(Error::Format(format!(
                "DDS cubemap needs all six faces: {name}"
            )));
        }
        let block_size = match &data[84..88] {
            b"DXT1" => 8usize,
            b"DXT3" | b"DXT5" => 16,
            _ => {
                return Err(Error::Format(format!(
                    "unsupported DDS cubemap format: {name}"
                )));
            }
        };
        let (width, height, mips) = (word(16), word(12), word(28).max(1));
        if width == 0 || width != height || mips > 32 {
            return Err(Error::Format(format!(
                "invalid DDS cubemap dimensions: {name}"
            )));
        }
        let mut face_size = 0usize;
        for mip in 0..mips {
            let w = (width >> mip).max(1).div_ceil(4) as usize;
            let h = (height >> mip).max(1).div_ceil(4) as usize;
            face_size = w
                .checked_mul(h)
                .and_then(|n| n.checked_mul(block_size))
                .and_then(|n| face_size.checked_add(n))
                .ok_or_else(|| Error::Format(format!("DDS cubemap size overflow: {name}")))?;
        }
        let mut faces = Vec::with_capacity(6);
        let mut offset = 128usize;
        for _ in 0..6 {
            let end = offset
                .checked_add(face_size)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| Error::Format(format!("truncated DDS cubemap: {name}")))?;
            let mut face = data[..128].to_vec();
            face[112..116].fill(0);
            face.extend_from_slice(&data[offset..end]);
            faces.push(Self::decode(name, &face)?);
            offset = end;
        }
        Ok(faces)
    }

    /// Decodes a texture from raw bytes, sniffing the container format.
    pub fn decode(name: &str, data: &[u8]) -> Result<Self> {
        // image's DDS reader handles DXT compression but not the packed RGB
        // format used by the client's shared water normal map.
        if data.starts_with(b"DDS ") && data.len() >= 128 {
            let word = |offset| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            if word(76) == 32 && word(80) & 0x40 != 0 && word(80) & 4 == 0 {
                return Self::decode_rgb_dds(name, data);
            }
        }
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

    fn decode_rgb_dds(name: &str, data: &[u8]) -> Result<Self> {
        let word = |offset| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        let (width, height, bits) = (word(16), word(12), word(88));
        if word(4) != 124 || width == 0 || height == 0 || ![16, 24, 32].contains(&bits) {
            return Err(Error::Format(format!("invalid RGB DDS header: {name}")));
        }
        let bytes_per_pixel = (bits / 8) as usize;
        let row_size = width as usize * bytes_per_pixel;
        let pitch = if word(8) & 8 != 0 {
            word(20) as usize
        } else {
            row_size
        };
        let size = pitch
            .checked_mul(height as usize)
            .filter(|size| *size <= data.len() - 128 && pitch >= row_size)
            .ok_or_else(|| Error::Format(format!("truncated RGB DDS pixels: {name}")))?;
        let masks = [word(92), word(96), word(100), word(104)];
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in data[128..128 + size].chunks_exact(pitch) {
            for pixel in row[..row_size].chunks_exact(bytes_per_pixel) {
                let mut bytes = [0; 4];
                bytes[..bytes_per_pixel].copy_from_slice(pixel);
                let pixel = u32::from_le_bytes(bytes);
                for (channel, mask) in masks.into_iter().enumerate() {
                    let value = if mask == 0 {
                        if channel == 3 { 255 } else { 0 }
                    } else {
                        let shift = mask.trailing_zeros();
                        let maximum = (mask >> shift) as u64;
                        ((((pixel & mask) >> shift) as u64 * 255 + maximum / 2) / maximum) as u8
                    };
                    rgba.push(value);
                }
            }
        }
        Ok(Self {
            name: name.to_owned(),
            width,
            height,
            rgba,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn header(width: u32, height: u32) -> Vec<u8> {
        let mut data = vec![0; 128];
        data[..4].copy_from_slice(b"DDS ");
        for (offset, value) in [
            (4, 124u32),
            (8, 0x1007),
            (12, height),
            (16, width),
            (76, 32),
            (108, 0x1000),
        ] {
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        data
    }

    #[test]
    fn packed_water_normals_decode_channels_and_row_pitch() {
        let mut data = header(2, 2);
        for (offset, value) in [
            (8, 0x100Fu32),
            (20, 6),
            (80, 0x41),
            (88, 16),
            (92, 0x0F00),
            (96, 0x00F0),
            (100, 0x000F),
            (104, 0xF000),
        ] {
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for pixel in [0xF123u16, 0x8456, 0xFFFF, 0x0789, 0x1ABC, 0xFFFF] {
            data.extend_from_slice(&pixel.to_le_bytes());
        }
        let texture = Texture::decode("normal.dds", &data).unwrap();
        assert_eq!(
            texture.rgba,
            [
                17, 34, 51, 255, 68, 85, 102, 136, 119, 136, 153, 0, 170, 187, 204, 17
            ]
        );
        data.pop();
        assert!(Texture::decode("truncated.dds", &data).is_err());
    }

    #[test]
    fn cubemap_faces_skip_mips_and_preserve_face_order() {
        let mut data = header(4, 4);
        for (offset, value) in [(28, 2u32), (80, 4), (112, 0xFE00)] {
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        data[84..88].copy_from_slice(b"DXT1");
        for color in [0xF800u16, 0x07E0, 0x001F, 0xFFE0, 0x07FF, 0xF81F] {
            data.extend_from_slice(&color.to_le_bytes());
            data.extend_from_slice(&[0; 6]);
            // A white 2x2 mip follows each face, not all six base levels.
            data.extend_from_slice(&[255, 255, 0, 0, 0, 0, 0, 0]);
        }
        let faces = Texture::decode_cube("environment.dds", &data).unwrap();
        assert_eq!(faces.len(), 6);
        let colors = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [0, 255, 255, 255],
            [255, 0, 255, 255],
        ];
        for (face, color) in faces.iter().zip(colors) {
            assert_eq!((face.width, face.height), (4, 4));
            assert!(face.rgba.chunks_exact(4).all(|pixel| pixel == color));
        }
        data.pop();
        assert!(Texture::decode_cube("truncated.dds", &data).is_err());
    }
}
