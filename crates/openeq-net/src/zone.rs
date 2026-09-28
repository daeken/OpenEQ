//! RoF2 zone entry and the server-authoritative entity stream.
//!
//! Layouts follow EQEmu's RoF2 encoder, including its variable-length spawn
//! names/equipment and signed, fixed-point position fields. Coordinates here
//! remain in EQ's Z-up coordinate system; conversion belongs in the renderer.
use crate::{AppPacket, EqStream, StreamError, ZoneOp};
use std::net::SocketAddr;

#[derive(Debug, thiserror::Error)]
pub enum ZoneError {
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error("malformed RoF2 {0}")]
    Malformed(&'static str),
    #[error("zone connection closed")]
    Closed,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// EQ heading: 0 = north, 512 = a full turn.
    pub heading: f32,
    /// EQ turn rate; the server advances this by 19 heading units per second.
    pub delta_heading: f32,
    pub velocity: [f32; 3],
    pub animation: i16,
}

#[derive(Debug, Clone)]
pub struct Spawn {
    pub id: u32,
    pub name: String,
    pub last_name: String,
    pub level: u8,
    pub class: u8,
    pub race: u32,
    pub gender: u8,
    pub npc: bool,
    pub size: f32,
    pub hp_percent: u8,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub body_type: u32,
    pub position: Position,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub short_name: String,
    pub long_name: String,
    pub zone_id: u16,
    pub fog_color: [[f32; 3]; 4],
    pub fog_start: [f32; 4],
    pub fog_end: [f32; 4],
    pub fog_density: f32,
    pub min_clip: f32,
    pub max_clip: f32,
    pub sky: u8,
    pub zone_type: u8,
    pub safe_position: [f32; 3],
}

#[derive(Debug, Clone)]
pub enum ZoneEvent {
    Spawn(Spawn),
    Movement { id: u32, position: Position },
    Despawn(u32),
    Environment(Environment),
    Ready,
    Time { hour: u8, minute: u8 },
    Hp { id: u32, percent: u8 },
    Other { opcode: u16, size: usize },
}

pub struct ZoneClient {
    stream: EqStream,
    requested_spawns: bool,
    ready: bool,
    sequence: u16,
}

impl ZoneClient {
    pub async fn connect(address: SocketAddr, character: &str) -> Result<Self, ZoneError> {
        if character.is_empty() || character.len() >= 64 || character.as_bytes().contains(&0) {
            return Err(ZoneError::Malformed("character name"));
        }
        let stream = EqStream::connect(address).await?;
        let mut data = vec![0; 76];
        data[4..4 + character.len()].copy_from_slice(character.as_bytes());
        stream
            .send(&AppPacket::new(ZoneOp::ZoneEntry as u16, data))
            .await?;
        Ok(Self {
            stream,
            requested_spawns: false,
            ready: false,
            sequence: 0,
        })
    }

    pub async fn next_event(&mut self) -> Result<ZoneEvent, ZoneError> {
        let packet = self.stream.recv().await.ok_or(ZoneError::Closed)?;
        let data = &packet.data;
        Ok(match packet.opcode {
            op if op == ZoneOp::ZoneEntry as u16 || op == ZoneOp::NewSpawn as u16 => {
                ZoneEvent::Spawn(parse_spawn(data).ok_or(ZoneError::Malformed("spawn"))?)
            }
            op if op == ZoneOp::NewZone as u16 => {
                let env = parse_environment(data).ok_or(ZoneError::Malformed("environment"))?;
                if !self.requested_spawns {
                    self.stream
                        .send(&AppPacket::empty(ZoneOp::ReqClientSpawn as u16))
                        .await?;
                    self.requested_spawns = true;
                }
                ZoneEvent::Environment(env)
            }
            0x5ae2 | 0x345d if !self.ready => {
                self.stream.send(&AppPacket::empty(0x5ae2)).await?;
                self.stream
                    .send(&AppPacket::empty(ZoneOp::ClientReady as u16))
                    .await?;
                self.ready = true;
                ZoneEvent::Ready
            }
            op if op == ZoneOp::ClientUpdate as u16 || op == 0x2c84 => {
                let mut c = Cursor(data);
                let id = c.u16().ok_or(ZoneError::Malformed("movement id"))? as u32;
                c.skip(2).ok_or(ZoneError::Malformed("movement"))?;
                let position = parse_position(c.take(20).ok_or(ZoneError::Malformed("movement"))?)
                    .ok_or(ZoneError::Malformed("movement"))?;
                ZoneEvent::Movement { id, position }
            }
            0x37b1 if data.len() >= 3 => ZoneEvent::Hp {
                id: u16::from_le_bytes([data[0], data[1]]) as u32,
                percent: data[2].min(100),
            },
            op if op == ZoneOp::HpUpdate as u16 && data.len() >= 10 => {
                let mut c = Cursor(data);
                let id = c.u16().unwrap() as u32;
                let hp = c.u32().unwrap() as f64;
                let max_hp = c.u32().unwrap() as f64;
                ZoneEvent::Hp {
                    id,
                    percent: if max_hp > 0. {
                        (hp / max_hp * 100.).clamp(0., 100.) as u8
                    } else {
                        0
                    },
                }
            }
            op if op == ZoneOp::DeleteSpawn as u16 => {
                let id = Cursor(data).u32().ok_or(ZoneError::Malformed("despawn"))?;
                ZoneEvent::Despawn(id)
            }
            op if op == ZoneOp::TimeOfDay as u16 && data.len() >= 2 => ZoneEvent::Time {
                hour: data[0],
                minute: data[1],
            },
            _ => ZoneEvent::Other {
                opcode: packet.opcode,
                size: data.len(),
            },
        })
    }

    pub async fn send_position(&mut self, id: u32, position: Position) -> Result<(), ZoneError> {
        if ![position.x, position.y, position.z, position.heading]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(ZoneError::Malformed("outgoing position"));
        }
        let mut data = vec![0; 46];
        data[..2].copy_from_slice(&self.sequence.to_le_bytes());
        self.sequence = self.sequence.wrapping_add(1);
        data[2..4].copy_from_slice(&(id as u16).to_le_bytes());
        for (offset, value) in [
            (10, position.velocity[0]),
            (18, position.x),
            (22, position.velocity[2]),
            (26, position.z),
            (30, position.y),
            (38, position.velocity[1]),
        ] {
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let heading = ((position.heading.rem_euclid(512.) * 4.) as u32) & 0xfff;
        data[14..18].copy_from_slice(&heading.to_le_bytes());
        data[34..38].copy_from_slice(&((position.animation as u32) & 0x3ff).to_le_bytes());
        self.stream
            .send(&AppPacket::new(ZoneOp::ClientUpdate as u16, data))
            .await?;
        Ok(())
    }

    pub async fn target(&self, id: u32) -> Result<(), ZoneError> {
        self.stream
            .send(&AppPacket::new(0x075d, id.to_le_bytes().to_vec()))
            .await?;
        Ok(())
    }

    pub async fn logout(&self) -> Result<(), ZoneError> {
        self.stream
            .send(&AppPacket::empty(ZoneOp::Logout as u16))
            .await?;
        Ok(())
    }
}

fn signed(word: u32, shift: u32, bits: u32) -> i32 {
    ((word << (32 - shift - bits)) as i32) >> (32 - bits)
}

pub fn parse_position(data: &[u8]) -> Option<Position> {
    let mut c = Cursor(data);
    let w = [c.u32()?, c.u32()?, c.u32()?, c.u32()?, c.u32()?];
    Some(Position {
        x: signed(w[2], 0, 19) as f32 / 8.,
        y: signed(w[0], 12, 19) as f32 / 8.,
        z: signed(w[3], 10, 19) as f32 / 8.,
        heading: ((w[2] >> 19) & 0xfff) as f32 / 4.,
        delta_heading: signed(w[3], 0, 10) as f32 / 20.,
        velocity: [
            signed(w[1], 13, 13) as f32 / 64.,
            signed(w[4], 10, 13) as f32 / 64.,
            signed(w[1], 0, 13) as f32 / 64.,
        ],
        animation: signed(w[4], 0, 10) as i16,
    })
}

pub fn parse_spawn(data: &[u8]) -> Option<Spawn> {
    let mut c = Cursor(data);
    let name = c.string()?;
    let id = c.u32()?;
    let level = c.u8()?;
    c.skip(4)?;
    let npc = c.u8()? != 0;
    let gender = (c.u32()? & 3) as u8;
    let other = c.u8()?;
    c.skip(8)?;
    if other & 0xe0 == 0xe0 || other & 4 != 0 {
        c.string()?;
        c.string()?;
        c.string()?;
        c.skip(53)?;
    }
    let properties = c.u8()? as usize;
    let mut body_type = 0;
    for i in 0..properties {
        let v = c.u32()?;
        if i == 0 {
            body_type = v;
        }
    }
    let hp_percent = c.u8()?;
    c.skip(6 + 12 + 4)?;
    let size = c.float()?;
    c.skip(1)?;
    let walk_speed = c.float()?;
    let run_speed = c.float()?;
    let race = c.u32()?;
    c.skip(1 + 12)?;
    let class = c.u8()?;
    c.skip(4)?;
    let last_name = c.string()?;
    c.skip(4 + 2 + 4 + 1 + 4 + 20)?;
    c.skip(
        if !npc || race <= 12 || matches!(race, 128 | 130 | 330 | 522) {
            216
        } else {
            60
        },
    )?;
    let position = parse_position(c.take(20)?)?;
    Some(Spawn {
        id,
        name,
        last_name,
        level,
        class,
        race,
        gender,
        npc,
        size,
        hp_percent,
        walk_speed,
        run_speed,
        body_type,
        position,
    })
}

pub fn parse_environment(data: &[u8]) -> Option<Environment> {
    if data.len() < 920 {
        return None;
    }
    let f = |offset| Cursor(&data[offset..]).float();
    let mut fog_color = [[0.; 3]; 4];
    let mut fog_start = [0.; 4];
    let mut fog_end = [0.; 4];
    for i in 0..4 {
        fog_color[i] = [
            data[471 + i] as f32 / 255.,
            data[475 + i] as f32 / 255.,
            data[479 + i] as f32 / 255.,
        ];
        fog_start[i] = f(484 + i * 4)?;
        fog_end[i] = f(500 + i * 4)?;
    }
    Some(Environment {
        short_name: Cursor(&data[64..192]).string()?,
        long_name: Cursor(&data[192..320]).string()?,
        zone_id: Cursor(&data[852..]).u16()?,
        fog_color,
        fog_start,
        fog_end,
        fog_density: f(916)?,
        min_clip: f(612)?,
        max_clip: f(616)?,
        sky: data[570],
        zone_type: data[520],
        safe_position: [f(592)?, f(588)?, f(596)?],
    })
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let bytes = self.0.get(..n)?;
        self.0 = &self.0[n..];
        Some(bytes)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn float(&mut self) -> Option<f32> {
        let f = f32::from_bits(self.u32()?);
        f.is_finite().then_some(f)
    }
    fn string(&mut self) -> Option<String> {
        let n = self.0.iter().position(|b| *b == 0)?;
        let s = String::from_utf8_lossy(self.take(n)?).into_owned();
        self.skip(1)?;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_coordinates_and_heading_match_rof2_bitfields() {
        let words: [u32; 5] = [
            (((-1184i32) as u32) & 0x7ffff) << 12,
            (64 << 13) | ((-128i32 as u32) & 0x1fff),
            ((-2280i32 as u32) & 0x7ffff) | (1024 << 19),
            (((-1272i32 as u32) & 0x7ffff) << 10) | ((-320i32 as u32) & 0x3ff),
            12 | (192 << 10),
        ];
        let data: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        let p = parse_position(&data).unwrap();
        assert_eq!([p.x, p.y, p.z], [-285., -148., -159.]);
        assert_eq!(p.heading, 256.);
        assert_eq!(p.delta_heading, -16.);
        assert_eq!(p.velocity, [1., 3., -2.]);
        assert_eq!(p.animation, 12);
    }
    #[test]
    fn signed_turn_rates_do_not_overlap_height() {
        for rate in [-511i32, -320, -1, 0, 1, 320, 511] {
            let words = [0u32, 0, 0, ((123 * 8) << 10) | (rate as u32 & 0x3ff), 0];
            let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
            let p = parse_position(&bytes).unwrap();
            assert_eq!(p.delta_heading, rate as f32 / 20.);
            assert_eq!(p.z, 123.);
        }
    }
    #[test]
    fn truncated_zone_packets_are_rejected() {
        for size in 0..20 {
            assert!(parse_position(&vec![0; size]).is_none());
        }
        assert!(parse_spawn(&[0; 20]).is_none());
        assert!(parse_environment(&[0; 919]).is_none());
    }
}
