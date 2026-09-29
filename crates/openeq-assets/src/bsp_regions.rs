//! Shared bounded decoding of WLD BSP trees and authored region declarations.
use crate::{Error, Result, read::Reader, wld::WLD_MAGIC};

const STRING_KEY: [u8; 8] = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub plane: [f64; 4],
    pub region: u32,
    pub children: [u32; 2],
}

#[derive(Debug)]
pub(crate) struct RegionDeclaration {
    /// Region annotations are zero-based; BSP leaf regions are one-based.
    pub indices: Vec<u32>,
    pub declaration: Vec<u8>,
}

pub(crate) struct BspRegions {
    pub nodes: Option<Vec<Node>>,
    pub declarations: Vec<RegionDeclaration>,
    pub region_count: u32,
}

impl BspRegions {
    pub(crate) fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data);
        let magic = reader.u32()?;
        if magic != WLD_MAGIC {
            return Err(Error::BadMagic {
                found: magic,
                expected: WLD_MAGIC,
            });
        }
        reader.u32()?; // Version does not change these two fragment layouts.
        let fragment_count = reader.bounded_count()?;
        reader.skip(8)?;
        let string_size = reader.bounded_count()?;
        reader.skip(4)?;
        let strings = decode(reader.take(string_size)?);
        reader.align4()?;
        if fragment_count > reader.remaining() / 12 {
            return Err(invalid("fragment count exceeds WLD data"));
        }
        let mut nodes = None;
        let mut declarations = Vec::new();
        let mut region_count = 0;
        for _ in 0..fragment_count {
            let size = reader.u32()? as usize;
            let kind = reader.u32()?;
            let mut fragment = Reader::new(reader.take(size)?);
            let name_ref = fragment.i32()?;
            match kind {
                0x21 => {
                    if nodes.is_some() {
                        return Err(invalid("multiple BSP trees in zone"));
                    }
                    let count = fragment.bounded_count()?;
                    if count > fragment.remaining() / 28 {
                        return Err(invalid("BSP node count exceeds fragment"));
                    }
                    let mut tree = Vec::with_capacity(count);
                    for _ in 0..count {
                        let normal = fragment.vec3()?;
                        let distance = fragment.f32()?;
                        let plane = [normal[0], normal[1], normal[2], distance].map(f64::from);
                        if !plane.iter().all(|v| v.is_finite()) {
                            return Err(invalid("non-finite BSP plane"));
                        }
                        tree.push(Node {
                            plane,
                            region: fragment.u32()?,
                            children: [fragment.u32()?, fragment.u32()?],
                        });
                    }
                    nodes = Some(tree);
                }
                0x22 => region_count += 1,
                0x29 => {
                    fragment.u32()?; // Flags.
                    let count = fragment.bounded_count()?;
                    if count > fragment.remaining().saturating_sub(4) / 4 {
                        return Err(invalid("region count exceeds fragment"));
                    }
                    let mut indices = Vec::with_capacity(count);
                    for _ in 0..count {
                        indices.push(fragment.u32()?);
                    }
                    let length = fragment.bounded_count()?;
                    let declaration = if length == 0 {
                        if name_ref > 0 {
                            return Err(invalid("region name is not a string reference"));
                        }
                        let start = name_ref.unsigned_abs() as usize;
                        let tail = strings
                            .get(start..)
                            .ok_or_else(|| invalid("region name outside string table"))?;
                        tail.split(|byte| *byte == 0)
                            .next()
                            .unwrap_or_default()
                            .to_vec()
                    } else {
                        decode(fragment.take(length)?)
                    };
                    declarations.push(RegionDeclaration {
                        indices,
                        declaration,
                    });
                }
                _ => {}
            }
        }
        Ok(Self {
            nodes,
            declarations,
            region_count,
        })
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ STRING_KEY[i % STRING_KEY.len()])
        .collect()
}

fn invalid(detail: &str) -> Error {
    Error::Format(format!("BSP regions: {detail}"))
}
