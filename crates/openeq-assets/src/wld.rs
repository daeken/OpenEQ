//! Parser for the `WLD` fragment format.
//!
//! A `WLD` file is a flat list of typed *fragments*. Fragments refer to each
//! other, and to strings, through signed 32-bit references:
//!
//! * a positive value `n` refers to fragment `n - 1`;
//! * a negative value `-n` refers to the string at byte offset `n` in the
//!   file's decoded string table.
//!
//! The string table is stored XOR-obfuscated with a fixed 8-byte key, which is
//! decoded once up front.
//!
//! Note that references may point *forward*, so this parser stores references
//! verbatim and resolves them after the whole file has been read. The original
//! C# implementation resolved eagerly and therefore silently dropped forward
//! references.

use std::collections::HashMap;

use crate::pfs::Archive;
use crate::read::Reader;
use crate::{Error, Result};

/// Magic at the start of a `WLD`: the bytes `02 3D 54 50`.
pub const WLD_MAGIC: u32 = 0x5450_3D02;

/// The version tag used by the original (pre-`EQG`) fragment layout.
const LEGACY_VERSION: u32 = 0x0001_5500;

const STRING_HASH_KEY: [u8; 8] = [0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A];

/// An unresolved fragment reference. See the module docs for the encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ref(pub i32);

impl Ref {
    /// A positive reference addresses fragment `index - 1`.
    pub fn fragment_index(self) -> Option<usize> {
        (self.0 > 0).then(|| (self.0 - 1) as usize)
    }

    /// A negative reference names a string at offset `-value`.
    pub fn string_offset(self) -> Option<usize> {
        (self.0 < 0).then(|| self.0.unsigned_abs() as usize)
    }
}

/// A single animation frame: local rotation, translation and uniform scale.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: f32,
}

#[derive(Debug, Clone)]
pub struct TextureList {
    /// Texture layers for a single frame: diffuse first, then optional detail
    /// maps. Animation frames are the references in [`AnimationRef::textures`].
    pub filenames: Vec<String>,
    /// Remaining declared bytes, including authored record padding.
    pub tail: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AnimationRef {
    pub flags: u32,
    /// Raw optional word selected by flag bit 2; no meaning inferred here.
    pub parameter: Option<u32>,
    pub frame_time: u32,
    pub textures: Vec<Ref>,
    pub tail: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SkeletonRef {
    pub animation: Ref,
    pub flags: u32,
    pub tail: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub name: String,
    pub flags: u32,
    pub piece_track: Ref,
    pub mesh: Ref,
    pub children: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub tracks: Vec<Track>,
    pub meshes: Vec<Ref>,
}

#[derive(Debug, Clone)]
pub struct SkeletonRef2 {
    pub skeleton: Ref,
}

#[derive(Debug, Clone)]
pub struct PieceTrack {
    pub flags: u32,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone)]
pub struct PieceTrackRef {
    pub track: Ref,
    /// Original fragment 0x13 flags, including the optional timing-word bit.
    pub flags: u32,
    pub speed: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ActorDef {
    pub magic: Ref,
    pub references: Vec<Ref>,
}

#[derive(Debug, Clone)]
pub struct ActorInstance {
    pub actor: Ref,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct LightSource {
    pub attenuation: Option<u32>,
    pub color: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct LightSourceRef {
    pub source: Ref,
}

#[derive(Debug, Clone)]
pub struct Light {
    pub source: Ref,
    pub flags: u32,
    pub position: [f32; 3],
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct MeshRef {
    pub mesh: Ref,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub animation: Ref,
    pub flags: u32,
}

#[derive(Debug, Clone)]
pub struct MaterialList {
    pub materials: Vec<Ref>,
}

/// A `0x26` texture binding used by native particle texture resolution.
/// The native resolver uses fixed child/material offsets, without interpreting
/// flags. This preserves source identity, not named-cache or low-byte behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleTexture {
    pub flags: u32,
    pub texture: Ref,
    /// Raw material handle; negative aliases require native renderer context.
    pub material: u32,
    pub tail: Vec<u8>,
}

/// A `0x34` particle-cloud definition, retained without inventing playback.
///
/// Offsets in `fixed_words` start after the common name reference; word zero
/// contains the optional-field flags. Raw words preserve integer fields,
/// float bits and packed color bytes exactly. See `WLD_PARTICLE_ACTORS.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleCloud {
    pub fixed_words: [u32; 20],
    /// Flag 1: six float bit patterns, in source order (two triples).
    pub optional_vectors: Option<[u32; 6]>,
    /// Flag 2: the native reader skips these 24 bytes without interpreting them.
    pub optional_block: Option<[u8; 24]>,
    /// Flag 4: the full file reference, not the native reader's low-byte read.
    /// Native named-resource reuse and texture selection are not emulated.
    pub texture_reference: Option<Ref>,
    /// Uninterpreted bytes remaining inside this fragment's declared extent.
    pub tail: Vec<u8>,
}

impl ParticleCloud {
    pub fn flags(&self) -> u32 {
        self.fixed_words[0]
    }
}

#[derive(Debug, Clone)]
pub struct Polygon {
    pub collidable: bool,
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

/// A `0x36` fragment: the actual triangle geometry of a model.
#[derive(Debug, Clone)]
pub struct Mesh {
    pub materials: Ref,
    pub animation: Ref,
    pub center: [f32; 3],
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tex_coords: Vec<[f32; 2]>,
    pub colors: Vec<u32>,
    pub polygons: Vec<Polygon>,
    /// `(vertex_count, track_index)` runs describing which bone drives vertices.
    pub vertex_pieces: Vec<(u16, u16)>,
    /// `(polygon_count, material_index)` runs into [`Mesh::polygons`].
    pub polygon_textures: Vec<(u16, u16)>,
}

/// The parsed payload of a single fragment.
#[derive(Debug, Clone)]
pub enum Fragment {
    TextureList(TextureList),
    Animation(AnimationRef),
    AnimationRef(SkeletonRef),
    Skeleton(Skeleton),
    SkeletonRef(SkeletonRef2),
    PieceTrack(PieceTrack),
    PieceTrackRef(PieceTrackRef),
    ActorDef(ActorDef),
    ActorInstance(ActorInstance),
    LightSource(LightSource),
    LightSourceRef(LightSourceRef),
    Light(Light),
    MeshRef(MeshRef),
    Material(Material),
    MaterialList(MaterialList),
    ParticleTexture(ParticleTexture),
    ParticleCloud(ParticleCloud),
    Mesh(Mesh),
    /// A fragment type this reader does not model. It is skipped but retained
    /// so that reference indices still line up.
    Ignored(u32),
}

impl Fragment {
    /// The fragment's numeric type code, useful when dumping files.
    pub fn type_code(&self) -> u32 {
        match self {
            Fragment::TextureList(_) => 0x03,
            Fragment::Animation(_) => 0x04,
            Fragment::AnimationRef(_) => 0x05,
            Fragment::Skeleton(_) => 0x10,
            Fragment::SkeletonRef(_) => 0x11,
            Fragment::PieceTrack(_) => 0x12,
            Fragment::PieceTrackRef(_) => 0x13,
            Fragment::ActorDef(_) => 0x14,
            Fragment::ActorInstance(_) => 0x15,
            Fragment::LightSource(_) => 0x1B,
            Fragment::LightSourceRef(_) => 0x1C,
            Fragment::Light(_) => 0x28,
            Fragment::MeshRef(_) => 0x2D,
            Fragment::Material(_) => 0x30,
            Fragment::MaterialList(_) => 0x31,
            Fragment::ParticleTexture(_) => 0x26,
            Fragment::ParticleCloud(_) => 0x34,
            Fragment::Mesh(_) => 0x36,
            Fragment::Ignored(code) => *code,
        }
    }
}

/// A named fragment.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub name: String,
    pub fragment: Fragment,
}

/// A parsed `WLD` file.
#[derive(Debug, Clone)]
pub struct Wld {
    /// The archive-relative path this file was read from.
    pub filename: String,
    /// Whether the file uses the newer fragment layout.
    pub new_format: bool,
    chunks: Vec<Chunk>,
    name_index: HashMap<String, usize>,
    strings: StringTable,
}

impl Wld {
    /// Reads and parses a `WLD` stored inside `archive`.
    pub fn open(archive: &Archive, filename: &str) -> Result<Self> {
        let data = archive.read(filename)?;
        Self::parse(filename.to_owned(), &data)
    }

    /// Parses a `WLD` from a byte buffer.
    pub fn parse(filename: String, data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data);

        let magic = reader.u32()?;
        if magic != WLD_MAGIC {
            return Err(Error::BadMagic {
                found: magic,
                expected: WLD_MAGIC,
            });
        }
        let version = reader.u32()?;
        let new_format = version != LEGACY_VERSION;
        let fragment_count = reader.u32()? as usize;
        reader.skip(8)?;
        let string_size = reader.u32()? as usize;
        reader.skip(4)?;

        let encoded = reader.take(string_size)?;
        let strings = StringTable::decode(encoded);
        reader.align4()?;

        let mut chunks = Vec::with_capacity(fragment_count);
        let mut name_index = HashMap::new();

        for index in 0..fragment_count {
            let size = reader.u32()? as usize;
            let type_code = reader.u32()?;
            // References remain global, but a malformed fragment cannot read
            // fields from the next fragment's header or payload.
            let mut payload = reader.window(size)?;
            let name_ref = payload.i32()?;
            let name = if name_ref <= 0 && type_code != 0x35 {
                string_at(&strings, name_ref.unsigned_abs() as usize).to_owned()
            } else {
                String::new()
            };
            let fragment = read_fragment(&mut payload, type_code, new_format, &strings)?;

            name_index.insert(name.clone(), index);
            chunks.push(Chunk { name, fragment });
        }

        Ok(Self {
            filename,
            new_format,
            chunks,
            name_index,
            strings,
        })
    }

    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Iterates over every fragment of a particular kind.
    pub fn iter<T>(&self) -> impl Iterator<Item = (&Chunk, &T)> + '_
    where
        T: FragmentKind + 'static,
    {
        self.chunks
            .iter()
            .filter_map(|chunk| T::of(&chunk.fragment).map(|value| (chunk, value)))
    }

    /// Looks up a fragment by name (as stored in the fragment header).
    pub fn by_name(&self, name: &str) -> Option<&Chunk> {
        self.name_index
            .get(name)
            .and_then(|index| self.chunks.get(*index))
    }

    /// Resolves a fragment reference.
    pub fn resolve(&self, reference: Ref) -> Option<&Chunk> {
        if let Some(index) = reference.fragment_index() {
            return self.chunks.get(index);
        }
        self.by_name(self.resolve_str(reference)?)
    }

    /// Resolves a string reference, returning fragment-backed or inline strings.
    pub fn resolve_str(&self, reference: Ref) -> Option<&str> {
        if let Some(index) = reference.fragment_index() {
            return self.chunks.get(index).map(|chunk| chunk.name.as_str());
        }
        let offset = reference.string_offset()?;
        // Invalid or empty name offsets must not alias an unnamed fragment.
        // Nonempty substrings remain valid names, as in the existing reader.
        self.strings.at(offset).filter(|name| !name.is_empty())
    }

    /// Returns the fragment name for a reference, regardless of its sign.
    pub fn reference_name(&self, reference: Ref) -> Option<&str> {
        if let Some(index) = reference.fragment_index() {
            return self.chunks.get(index).map(|chunk| chunk.name.as_str());
        }
        self.resolve_str(reference)
    }
}

/// Helper for narrowing a [`Fragment`] to a concrete payload type.
pub trait FragmentKind: Sized {
    fn of(fragment: &Fragment) -> Option<&Self>;
}

macro_rules! fragment_kind {
    ($ty:ty, $variant:ident) => {
        impl FragmentKind for $ty {
            fn of(fragment: &Fragment) -> Option<&Self> {
                match fragment {
                    Fragment::$variant(inner) => Some(inner),
                    _ => None,
                }
            }
        }
    };
}

fragment_kind!(TextureList, TextureList);
fragment_kind!(AnimationRef, Animation);
fragment_kind!(SkeletonRef, AnimationRef);
fragment_kind!(Skeleton, Skeleton);
fragment_kind!(SkeletonRef2, SkeletonRef);
fragment_kind!(PieceTrack, PieceTrack);
fragment_kind!(PieceTrackRef, PieceTrackRef);
fragment_kind!(ActorDef, ActorDef);
fragment_kind!(ActorInstance, ActorInstance);
fragment_kind!(LightSource, LightSource);
fragment_kind!(LightSourceRef, LightSourceRef);
fragment_kind!(Light, Light);
fragment_kind!(MeshRef, MeshRef);
fragment_kind!(Material, Material);
fragment_kind!(MaterialList, MaterialList);
fragment_kind!(ParticleTexture, ParticleTexture);
fragment_kind!(ParticleCloud, ParticleCloud);
fragment_kind!(Mesh, Mesh);

fn read_fragment(
    reader: &mut Reader<'_>,
    type_code: u32,
    new_format: bool,
    strings: &StringTable,
) -> Result<Fragment> {
    Ok(match type_code {
        0x03 => {
            let filenames = read_texture_list(reader)?;
            let tail = reader.take(reader.remaining())?.to_vec();
            Fragment::TextureList(TextureList { filenames, tail })
        }
        0x04 => {
            let flags = reader.u32()?;
            let reference_count = reader.bounded_count()?;
            let parameter = if flags & (1 << 2) != 0 {
                Some(reader.u32()?)
            } else {
                None
            };
            let frame_time = if flags & (1 << 3) != 0 {
                reader.u32()?
            } else {
                0
            };
            let mut textures = Vec::with_capacity(reference_count);
            for _ in 0..reference_count {
                textures.push(reader.reference()?);
            }
            let tail = reader.take(reader.remaining())?.to_vec();
            Fragment::Animation(AnimationRef {
                flags,
                parameter,
                frame_time,
                textures,
                tail,
            })
        }
        0x05 => {
            let animation = reader.reference()?;
            let flags = reader.u32()?;
            let tail = reader.take(reader.remaining())?.to_vec();
            Fragment::AnimationRef(SkeletonRef {
                animation,
                flags,
                tail,
            })
        }
        0x10 => Fragment::Skeleton(read_skeleton(reader, strings)?),
        0x11 => Fragment::SkeletonRef(SkeletonRef2 {
            skeleton: reader.reference()?,
        }),
        0x12 => {
            let flags = reader.u32()?;
            let frame_count = reader.bounded_count()?;
            let mut frames = Vec::with_capacity(frame_count);
            for _ in 0..frame_count {
                let frame = if flags & 8 != 0 {
                    let rot_w = reader.i16()? as f32;
                    let rot_x = reader.i16()? as f32;
                    let rot_y = reader.i16()? as f32;
                    let rot_z = reader.i16()? as f32;
                    let shift_x = reader.i16()? as f32;
                    let shift_y = reader.i16()? as f32;
                    let shift_z = reader.i16()? as f32;
                    // Native packed scale is zero-extended; the preceding
                    // rotation/translation words remain signed.
                    let scale = reader.u16()? as f32 / 256.0;
                    // The last word is a scale, not a translation divisor.
                    // Rotation is normalized by the animation sampler.
                    Frame {
                        rotation: [
                            rot_x / 16384.0,
                            rot_y / 16384.0,
                            rot_z / 16384.0,
                            rot_w / 16384.0,
                        ],
                        translation: [shift_x / 256.0, shift_y / 256.0, shift_z / 256.0],
                        scale,
                    }
                } else {
                    let scale = reader.f32()?;
                    let translation = reader.vec3()?;
                    let rot_w = reader.f32()?;
                    let rot_x = reader.f32()?;
                    let rot_y = reader.f32()?;
                    let rot_z = reader.f32()?;
                    Frame {
                        rotation: [rot_x, rot_y, rot_z, rot_w],
                        translation,
                        scale,
                    }
                };
                frames.push(frame);
            }
            Fragment::PieceTrack(PieceTrack { flags, frames })
        }
        0x13 => {
            let track = reader.reference()?;
            let flags = reader.u32()?;
            let speed = if flags & 1 != 0 {
                Some(reader.u32()?)
            } else {
                None
            };
            Fragment::PieceTrackRef(PieceTrackRef {
                track,
                flags,
                speed,
            })
        }
        0x14 => {
            let flags = reader.u32()?;
            let magic = reader.reference()?;
            let size = reader.bounded_count()?;
            let reference_count = reader.bounded_count()?;
            reader.u32()?;
            if flags & (1 << 0) != 0 {
                reader.u32()?;
            }
            if flags & (1 << 1) != 0 {
                reader.skip(28)?;
            }
            for _ in 0..size {
                let count = reader.bounded_count()?;
                for _ in 0..count {
                    reader.u32()?;
                    reader.f32()?;
                }
            }
            let mut references = Vec::with_capacity(reference_count);
            for _ in 0..reference_count {
                references.push(reader.reference()?);
            }
            Fragment::ActorDef(ActorDef { magic, references })
        }
        0x15 => {
            let actor = reader.reference()?;
            reader.u32()?;
            reader.u32()?;
            let position = reader.vec3()?;
            let raw_rotation = reader.vec3()?;
            let raw_scale = reader.vec3()?;

            // A scale of zero means "unspecified, use 1".
            let scale = if raw_scale[2] > 0.0001 {
                [raw_scale[2]; 3]
            } else {
                [1.0; 3]
            };
            // The file stores placement angles as (around Z, around Y, around
            // X) in 1/256 pi units; normalise to a plain (X, Y, Z) triple so
            // every consumer sees the same convention.
            let rotation = [
                raw_rotation[2] / 256.0 * std::f32::consts::PI,
                raw_rotation[1] / 256.0 * std::f32::consts::PI,
                raw_rotation[0] / 256.0 * std::f32::consts::PI,
            ];

            Fragment::ActorInstance(ActorInstance {
                actor,
                position,
                rotation,
                scale,
            })
        }
        0x1B => {
            let flags = reader.u32()?;
            reader.u32()?;
            let (attenuation, color) = if flags & (1 << 4) != 0 {
                let attenuation = if flags & (1 << 3) != 0 {
                    Some(reader.u32()?)
                } else {
                    None
                };
                reader.f32()?;
                (attenuation, reader.vec3()?)
            } else {
                let value = reader.f32()?;
                (None, [value; 3])
            };
            Fragment::LightSource(LightSource { attenuation, color })
        }
        0x1C => {
            let source = reader.reference()?;
            reader.u32()?;
            Fragment::LightSourceRef(LightSourceRef { source })
        }
        0x28 => {
            let source = reader.reference()?;
            let flags = reader.u32()?;
            let position = reader.vec3()?;
            let radius = reader.f32()?;
            Fragment::Light(Light {
                source,
                flags,
                position,
                radius,
            })
        }
        0x26 => {
            let flags = reader.u32()?;
            let texture = reader.reference()?;
            let material = reader.u32()?;
            let tail = reader.take(reader.remaining())?.to_vec();
            Fragment::ParticleTexture(ParticleTexture {
                flags,
                texture,
                material,
                tail,
            })
        }
        0x2D => Fragment::MeshRef(MeshRef {
            mesh: reader.reference()?,
        }),
        0x30 => {
            // Layout observed in client data: the texture reference precedes
            // the optional trailing pair, which is the opposite of what the
            // original reader assumed.
            let existence_flags = reader.u32()?;
            let flags = reader.u32()?;
            reader.u32()?;
            reader.f32()?;
            reader.f32()?;
            let animation = reader.reference()?;
            if existence_flags & (1 << 1) != 0 {
                reader.u32()?;
                reader.f32()?;
            }
            Fragment::Material(Material { animation, flags })
        }
        0x31 => {
            reader.u32()?;
            let count = reader.bounded_count()?;
            let mut materials = Vec::with_capacity(count);
            for _ in 0..count {
                materials.push(reader.reference()?);
            }
            Fragment::MaterialList(MaterialList { materials })
        }
        0x34 => {
            let mut fixed_words = [0; 20];
            for word in &mut fixed_words {
                *word = reader.u32()?;
            }
            let flags = fixed_words[0];
            let optional_vectors = if flags & 1 != 0 {
                let mut words = [0; 6];
                for word in &mut words {
                    *word = reader.u32()?;
                }
                Some(words)
            } else {
                None
            };
            let optional_block = if flags & 2 != 0 {
                let mut block = [0; 24];
                block.copy_from_slice(reader.take(24)?);
                Some(block)
            } else {
                None
            };
            let texture_reference = if flags & 4 != 0 {
                Some(reader.reference()?)
            } else {
                None
            };
            let tail = reader.take(reader.remaining())?.to_vec();
            Fragment::ParticleCloud(ParticleCloud {
                fixed_words,
                optional_vectors,
                optional_block,
                texture_reference,
                tail,
            })
        }
        0x36 => Fragment::Mesh(read_mesh(reader, new_format)?),
        other => Fragment::Ignored(other),
    })
}

fn read_texture_list(reader: &mut Reader<'_>) -> Result<Vec<String>> {
    let count = reader.bounded_count()?;
    // The stored count is one less than the number of names.
    let count = count.saturating_add(1);
    let mut filenames = Vec::with_capacity(count);
    for _ in 0..count {
        let len = reader.u16()? as usize;
        let bytes = reader.take(len)?;
        // Names are obfuscated with the same key as the string table, but the
        // XOR counter restarts for each name.
        filenames.push(
            bytes
                .iter()
                .enumerate()
                .map(|(i, b)| *b ^ STRING_HASH_KEY[i % 8])
                .take_while(|b| *b != 0)
                .map(|b| b as char)
                .collect(),
        );
    }
    Ok(filenames)
}

fn read_skeleton(reader: &mut Reader<'_>, strings: &StringTable) -> Result<Skeleton> {
    let flags = reader.u32()?;
    let track_count = reader.bounded_count()?;
    reader.reference()?; // Legacy polygon-animation reference; unused.
    if flags & (1 << 0) != 0 {
        reader.skip(12)?;
    }
    if flags & (1 << 1) != 0 {
        reader.f32()?;
    }

    let mut tracks = Vec::with_capacity(track_count);
    for _ in 0..track_count {
        let name = strings_at(strings, reader.i32()?);
        let flags = reader.u32()?;
        let piece_track = reader.reference()?;
        let mesh = reader.reference()?;
        let child_count = reader.bounded_count()?;
        let mut children = Vec::with_capacity(child_count);
        for _ in 0..child_count {
            children.push(reader.i32()?);
        }
        tracks.push(Track {
            name,
            flags,
            piece_track,
            mesh,
            children,
        });
    }

    let meshes = if flags & (1 << 9) != 0 {
        let count = reader.bounded_count()?;
        let mut meshes = Vec::with_capacity(count);
        for _ in 0..count {
            meshes.push(reader.reference()?);
        }
        meshes
    } else {
        tracks.iter().map(|track| track.mesh).collect()
    };

    Ok(Skeleton { tracks, meshes })
}

fn read_mesh(reader: &mut Reader<'_>, new_format: bool) -> Result<Mesh> {
    reader.u32()?;
    let materials = reader.reference()?;
    let animation = reader.reference()?;
    reader.u32()?;
    reader.i32()?;
    let center = reader.vec3()?;
    reader.skip(12)?;
    let _max_distance = reader.f32()?;
    let bounds_min = reader.vec3()?;
    let bounds_max = reader.vec3()?;

    let vertex_count = reader.u16()? as usize;
    let tex_coord_count = reader.u16()? as usize;
    let normal_count = reader.u16()? as usize;
    let color_count = reader.u16()? as usize;
    let polygon_count = reader.u16()? as usize;
    let vertex_piece_count = reader.u16()? as usize;
    let polygon_texture_count = reader.u16()? as usize;
    let _vertex_texture_count = reader.u16()?;
    reader.u16()?;
    let scale_bits = reader.u16()?;
    let scale = (1u32.checked_shl(scale_bits as u32).unwrap_or(1)) as f32;

    let mut vertices = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        vertices.push([
            reader.i16()? as f32 / scale + center[0],
            reader.i16()? as f32 / scale + center[1],
            reader.i16()? as f32 / scale + center[2],
        ]);
    }

    let mut tex_coords = Vec::with_capacity(vertex_count);
    for _ in 0..tex_coord_count {
        if new_format {
            tex_coords.push(reader.vec2()?);
        } else {
            tex_coords.push([reader.i16()? as f32 / 256.0, reader.i16()? as f32 / 256.0]);
        }
    }
    tex_coords.resize(vertex_count, [0.0, 0.0]);

    let mut normals = Vec::with_capacity(vertex_count);
    for _ in 0..normal_count {
        normals.push([
            reader.i8()? as f32 / 127.0,
            reader.i8()? as f32 / 127.0,
            reader.i8()? as f32 / 127.0,
        ]);
    }
    normals.resize(vertex_count, [1.0, 1.0, 1.0]);

    let mut colors = Vec::with_capacity(color_count);
    for _ in 0..color_count {
        colors.push(reader.u32()?);
    }

    let mut polygons = Vec::with_capacity(polygon_count);
    for _ in 0..polygon_count {
        let collidable = reader.u16()? == 0;
        polygons.push(Polygon {
            collidable,
            a: reader.u16()? as u32,
            b: reader.u16()? as u32,
            c: reader.u16()? as u32,
        });
    }

    let mut vertex_pieces = Vec::with_capacity(vertex_piece_count);
    for _ in 0..vertex_piece_count {
        vertex_pieces.push((reader.u16()?, reader.u16()?));
    }

    let mut polygon_textures = Vec::with_capacity(polygon_texture_count);
    for _ in 0..polygon_texture_count {
        polygon_textures.push((reader.u16()?, reader.u16()?));
    }

    Ok(Mesh {
        materials,
        animation,
        center,
        bounds_min,
        bounds_max,
        vertices,
        normals,
        tex_coords,
        colors,
        polygons,
        vertex_pieces,
        polygon_textures,
    })
}

/// File references count XOR-decoded source bytes. The existing display
/// conversion maps each byte to U+0000..U+00FF, which can expand in UTF-8.
/// Keep those two domains separate, including references inside a name.
#[derive(Debug, Clone)]
struct StringTable {
    text: String,
    source_len: usize,
    // Each high byte becomes exactly two UTF-8 bytes under the retained
    // conversion. Sparse offsets avoid duplicating large mostly-ASCII tables.
    expanded_at: Vec<usize>,
}

impl StringTable {
    fn decode(encoded: &[u8]) -> Self {
        let mut text = String::with_capacity(encoded.len());
        let mut expanded_at = Vec::new();
        for (offset, byte) in encoded.iter().enumerate() {
            let decoded = byte ^ STRING_HASH_KEY[offset % 8];
            if !decoded.is_ascii() {
                expanded_at.push(offset);
            }
            text.push(char::from(decoded));
        }
        Self {
            text,
            source_len: encoded.len(),
            expanded_at,
        }
    }

    fn at(&self, source_offset: usize) -> Option<&str> {
        if source_offset > self.source_len {
            return None;
        }
        let offset = source_offset
            + self
                .expanded_at
                .partition_point(|offset| *offset < source_offset);
        self.text.get(offset..)?.split('\0').next()
    }
}

fn string_at(strings: &StringTable, offset: usize) -> &str {
    strings.at(offset).unwrap_or("")
}

fn strings_at(strings: &StringTable, reference: i32) -> String {
    if reference >= 0 {
        String::new()
    } else {
        string_at(strings, reference.unsigned_abs() as usize).to_owned()
    }
}

trait ReferenceReader {
    fn reference(&mut self) -> Result<Ref>;
}

impl ReferenceReader for Reader<'_> {
    fn reference(&mut self) -> Result<Ref> {
        Ok(Ref(self.i32()?))
    }
}
