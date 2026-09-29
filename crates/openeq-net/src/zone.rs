//! RoF2 zone entry and the server-authoritative entity stream.
//!
//! Layouts follow EQEmu's RoF2 encoder, including its variable-length spawn
//! names/equipment and signed, fixed-point position fields. Coordinates here
//! remain in EQ's Z-up coordinate system; conversion belongs in the renderer.
use crate::gameplay::{self, CharacterAppearance, Command, GameplayEvent};
use crate::{AppPacket, EqStream, StreamError, ZoneOp};
use std::{collections::VecDeque, future::Future, net::SocketAddr, pin::Pin};

#[derive(Debug, thiserror::Error)]
pub enum ZoneError {
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error("malformed RoF2 {0}")]
    Malformed(&'static str),
    #[error("zone connection closed")]
    Closed,
    #[error(transparent)]
    World(#[from] crate::world::WorldError),
    #[error("a zone handoff is in progress")]
    Zoning,
    #[error("this direct zone connection has no authenticated world session for zoning")]
    MissingWorldSession,
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
    pub appearance: CharacterAppearance,
    pub is_corpse: bool,
    pub stand_state: u8,
    /// EQ gravity behavior: 0 ground, 1 flying, 2 levitating, 3 water,
    /// 4 floating, 5 levitating while running. Mode 3 also walks on land
    /// and is EQEmu's default for ordinary NPCs.
    pub fly_mode: u8,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub short_name: String,
    pub long_name: String,
    pub zone_id: u16,
    pub instance_id: u16,
    pub fog_color: [[f32; 3]; 4],
    pub fog_start: [f32; 4],
    pub fog_end: [f32; 4],
    pub fog_density: f32,
    pub min_clip: f32,
    pub max_clip: f32,
    pub sky: u8,
    pub zone_type: u8,
    pub safe_position: [f32; 3],
    /// Raw zone gravity coefficient from OP_NewZone (normally 0.4).
    pub gravity: f32,
    pub underworld: f32,
    pub underworld_teleport_index: u32,
    pub lava_damage: u32,
    pub min_lava_damage: u32,
    /// Wire flag; EQEmu's RoF2 encoder currently always sends false.
    pub fall_damage_disabled: bool,
    /// Wire flag; EQEmu's RoF2 encoder currently always sends false,
    /// even when the zone database forbids levitation.
    pub levitation_disabled: bool,
}

#[derive(Debug, Clone)]
pub enum ZoneEvent {
    Gameplay(GameplayEvent),
    Spawn(Spawn),
    Movement { id: u32, position: Position },
    Despawn(u32),
    Environment(Environment),
    Ready,
    Time { hour: u8, minute: u8 },
    Hp { id: u32, percent: u8 },
    Other { opcode: u16, size: usize },
}

// Contains a login handoff key: deliberately no Debug implementation.
#[derive(Clone)]
struct WorldHandoff {
    address: SocketAddr,
    account_id: u32,
    key: String,
}
type PendingHandoff = Pin<Box<dyn Future<Output = Result<ZoneClient, ZoneError>> + Send>>;

#[derive(Default)]
struct RecoveryTransfer {
    requested: Option<(u16, u16)>,
}

impl RecoveryTransfer {
    fn request(&mut self, current: Option<(u16, u16)>, target: (u16, u16)) -> bool {
        if target.0 != 0 && current == Some(target) {
            return false; // Hover revival stays on the current zone socket.
        }
        if self.requested == Some(target) {
            return false;
        }
        self.requested = Some(target);
        true
    }

    fn accepted(&mut self, destination: (u16, u16), success: i32) -> bool {
        let Some(requested) = self.requested else {
            return false;
        };
        if requested.0 != 0 && requested != destination {
            return false;
        }
        self.requested = None;
        success == 1 && requested.0 == 0
    }
}

pub struct ZoneClient {
    stream: EqStream,
    requested_spawns: bool,
    ready: bool,
    sequence: u16,
    pending: VecDeque<ZoneEvent>,
    control: VecDeque<AppPacket>,
    character: String,
    current_zone: Option<(u16, u16)>,
    world: Option<WorldHandoff>,
    handoff: Option<PendingHandoff>,
    recovery_transfer: RecoveryTransfer,
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
            pending: VecDeque::new(),
            control: VecDeque::new(),
            character: character.into(),
            current_zone: None,
            world: None,
            handoff: None,
            recovery_transfer: RecoveryTransfer::default(),
        })
    }

    pub(crate) fn enable_zoning(&mut self, address: SocketAddr, account_id: u32, key: String) {
        self.world = Some(WorldHandoff {
            address,
            account_id,
            key,
        });
    }

    pub fn is_zoning(&self) -> bool {
        self.handoff.is_some()
    }

    pub fn current_zone(&self) -> Option<(u16, u16)> {
        self.current_zone
    }

    fn begin_handoff(&mut self) -> Result<(), ZoneError> {
        let context = self.world.clone().ok_or(ZoneError::MissingWorldSession)?;
        let character = self.character.clone();
        self.handoff = Some(Box::pin(async move {
            let mut world = crate::world::WorldClient::connect_zoning(
                context.address,
                context.account_id,
                &context.key,
            )
            .await?;
            let address = world.enter_world(&character).await?;
            let mut zone = ZoneClient::connect(address, &character).await?;
            zone.world = Some(context);
            Ok(zone)
        }));
        Ok(())
    }

    pub async fn next_event(&mut self) -> Result<ZoneEvent, ZoneError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(event);
        }
        // Store the future on self: a caller's heartbeat/select may cancel this
        // method repeatedly without restarting the world/zone handshakes.
        if let Some(handoff) = &mut self.handoff {
            let next = handoff.await;
            self.handoff = None;
            *self = next?;
        }
        while let Some(packet) = self.control.front() {
            self.stream.send(packet).await?;
            self.control.pop_front();
        }
        let packet = self.stream.recv().await.ok_or(ZoneError::Closed)?;
        let data = &packet.data;
        if let Some(event) = gameplay::parse_packet(packet.opcode, data) {
            let event = event.inspect_err(|_| {
                tracing::warn!(
                    opcode = format!("{:#06x}", packet.opcode),
                    size = data.len(),
                    "malformed gameplay packet"
                );
            })?;
            if matches!(
                &event,
                GameplayEvent::BeginCast { .. }
                    | GameplayEvent::CastInterrupted { .. }
                    | GameplayEvent::SpellAction { .. }
                    | GameplayEvent::SpellEffect(_)
                    | GameplayEvent::Projectile(_)
                    | GameplayEvent::NimbusEffect(_)
                    | GameplayEvent::Buffs { .. }
                    | GameplayEvent::BuffChanged { .. }
                    | GameplayEvent::SpellBarEnabled { .. }
                    | GameplayEvent::Damage(_)
            ) {
                tracing::debug!(
                    target: "openeq_net::spell_effects",
                    opcode = format!("{:#06x}", packet.opcode),
                    ?event,
                    "spell presentation event"
                );
            }
            if let GameplayEvent::SpellBarEnabled {
                mana, endurance, ..
            } = event
            {
                self.pending
                    .push_back(ZoneEvent::Gameplay(GameplayEvent::ManaEndurance {
                        mana,
                        endurance,
                    }));
            }
            match &event {
                GameplayEvent::Recovery(crate::death::DeathEvent::BindTransfer(destination)) => {
                    tracing::info!(target: "openeq_net::recovery", zone_id = destination.zone_id,
                        instance_id = destination.instance_id, "received bind transfer");
                    if self.recovery_transfer.request(
                        self.current_zone,
                        (destination.zone_id, destination.instance_id),
                    ) {
                        self.control
                            .push_back(gameplay::encode_command(Command::ZoneChange {
                                character: self.character.clone(),
                                zone_id: destination.zone_id,
                                instance_id: destination.instance_id,
                                position: destination.position,
                                reason: 0,
                            })?);
                    }
                }
                GameplayEvent::ZoneChangeRequested(destination) => {
                    if self.current_zone != Some((destination.zone_id, destination.instance_id)) {
                        self.control
                            .push_back(gameplay::encode_command(Command::ZoneChange {
                                character: self.character.clone(),
                                zone_id: destination.zone_id,
                                instance_id: destination.instance_id,
                                position: destination.position,
                                reason: 0,
                            })?);
                    }
                }
                GameplayEvent::ZoneChangeResult {
                    zone_id,
                    instance_id,
                    success,
                    ..
                } => {
                    let destination = (*zone_id, *instance_id);
                    let forced = self.recovery_transfer.accepted(destination, *success);
                    if *success == 1 && (forced || self.current_zone != Some(destination)) {
                        tracing::info!(target: "openeq_net::recovery", zone_id, instance_id,
                            forced_reentry = forced, "starting authenticated zone handoff");
                        let transition = GameplayEvent::ZoneTransition {
                            zone_id: *zone_id,
                            instance_id: *instance_id,
                        };
                        self.begin_handoff()?;
                        self.pending.push_back(ZoneEvent::Gameplay(event));
                        return Ok(ZoneEvent::Gameplay(transition));
                    }
                }
                _ => {}
            }
            if let GameplayEvent::Health {
                id,
                current,
                maximum,
            } = event
            {
                self.pending.push_back(ZoneEvent::Hp {
                    id,
                    percent: if maximum > 0 {
                        ((current.max(0) as f64 / maximum as f64) * 100.).clamp(0., 100.) as u8
                    } else {
                        0
                    },
                });
            }
            return Ok(ZoneEvent::Gameplay(event));
        }
        Ok(match packet.opcode {
            op if op == ZoneOp::ZoneEntry as u16 || op == ZoneOp::NewSpawn as u16 => {
                ZoneEvent::Spawn(parse_spawn(data).ok_or(ZoneError::Malformed("spawn"))?)
            }
            op if op == ZoneOp::NewZone as u16 => {
                let env = parse_environment(data).ok_or(ZoneError::Malformed("environment"))?;
                self.current_zone = Some((env.zone_id, env.instance_id));
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
        // Heartbeats from the old scene must not move the character during a
        // transfer. Fresh spawn state supplies the next zone's position.
        if self.is_zoning() {
            return Ok(());
        }
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

    pub async fn command(&self, command: Command) -> Result<(), ZoneError> {
        if self.is_zoning() {
            return Err(ZoneError::Zoning);
        }
        self.stream
            .send(&gameplay::encode_command(command)?)
            .await?;
        Ok(())
    }

    pub async fn target(&self, id: u32) -> Result<(), ZoneError> {
        if self.is_zoning() {
            return Err(ZoneError::Zoning);
        }
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
    let spawn_kind = c.u8()?;
    let npc = spawn_kind != 0;
    let is_corpse = matches!(spawn_kind, 2 | 3);
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
    // RoF2 OP_ZoneSpawns encodes these before textures and size. The eye
    // colors are independent and Drakkin selections are full 32-bit values.
    let hair_color = c.u8()?;
    let beard_color = c.u8()?;
    let eye_color_1 = c.u8()?;
    let eye_color_2 = c.u8()?;
    let hair_style = c.u8()?;
    let beard = c.u8()?;
    let drakkin_heritage = c.u32()?;
    let drakkin_tattoo = c.u32()?;
    let drakkin_details = c.u32()?;
    let texture = c.u8()?;
    c.skip(2)?;
    let helm_texture = c.u8()?;
    let size = c.float()?;
    let face = c.u8()?;
    let walk_speed = c.float()?;
    let run_speed = c.float()?;
    let race = c.u32()?;
    c.skip(1 + 12)?;
    let class = c.u8()?;
    c.skip(1)?;
    let stand_state = c.u8()?;
    c.skip(1)?; // light
    let fly_mode = c.u8()?;
    let last_name = c.string()?;
    c.skip(4 + 2 + 4 + 1 + 4 + 20)?;
    let mut appearance = CharacterAppearance {
        texture,
        helm_texture,
        face,
        hair_color,
        beard_color,
        eye_color_1,
        eye_color_2,
        hair_style,
        beard,
        drakkin_heritage,
        drakkin_tattoo,
        drakkin_details,
        ..CharacterAppearance::default()
    };
    if !npc || race <= 12 || matches!(race, 128 | 130 | 330 | 522) {
        for part in &mut appearance.equipment {
            part.color = c.u32()?;
        }
        for part in &mut appearance.equipment {
            part.material = c.u32()?;
            c.skip(4)?;
            part.elite_material = c.u32()?;
            part.hero_forge_model = c.u32()?;
            c.skip(4)?;
        }
    } else {
        c.skip(20)?;
        appearance.equipment[7].material = c.u32()?;
        c.skip(16)?;
        appearance.equipment[8].material = c.u32()?;
        c.skip(16)?;
    }
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
        appearance,
        is_corpse,
        stand_state,
        fly_mode,
    })
}

pub fn parse_environment(data: &[u8]) -> Option<Environment> {
    if data.len() < 948 {
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
        instance_id: Cursor(&data[854..]).u16()?,
        fog_color,
        fog_start,
        fog_end,
        fog_density: f(916)?,
        min_clip: f(612)?,
        max_clip: f(616)?,
        sky: data[570],
        zone_type: data[520],
        safe_position: [f(592)?, f(588)?, f(596)?],
        gravity: f(516)?,
        underworld: f(608)?,
        underworld_teleport_index: Cursor(&data[868..]).u32()?,
        lava_damage: Cursor(&data[880..]).u32()?,
        min_lava_damage: Cursor(&data[884..]).u32()?,
        fall_damage_disabled: data[894] != 0,
        levitation_disabled: Cursor(&data[940..]).u32()? != 0,
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
    fn forced_bind_zero_reenters_same_zone_once_but_hover_does_not() {
        let mut recovery = RecoveryTransfer::default();
        assert!(!recovery.accepted((202, 0), 1)); // Normal cancelled border.
        assert!(!recovery.request(Some((202, 0)), (202, 0))); // Local hover.
        assert!(recovery.request(Some((202, 0)), (0, 0)));
        assert!(!recovery.request(Some((202, 0)), (0, 0))); // Duplicate packet.
        assert!(recovery.accepted((202, 0), 1));
        assert!(!recovery.accepted((202, 0), 1));
        assert!(recovery.request(Some((202, 0)), (0, 0)));
        assert!(!recovery.accepted((202, 0), -1));
        assert!(!recovery.accepted((202, 0), 1)); // Failure consumes the intent.
    }

    #[test]
    fn recovery_transfer_compares_instances_and_retains_unmatched_request() {
        let mut recovery = RecoveryTransfer::default();
        assert!(recovery.request(Some((202, 0)), (202, 9)));
        assert!(!recovery.accepted((202, 0), 1));
        assert_eq!(recovery.requested, Some((202, 9)));
        assert!(!recovery.accepted((202, 9), 1)); // Ordinary changed-instance handoff.
        assert!(recovery.requested.is_none());
    }

    // Field order from EQEmu common/patches/rof2.cpp's OP_ZoneSpawns encoder.
    // Nonzero adjacent fields and a position after equipment catch accidental
    // byte shifts when retaining the gravity mode.
    fn spawn_packet(fly_mode: u8, race: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"Guard_Briarstorm\0");
        data.extend_from_slice(&123u32.to_le_bytes());
        data.push(20); // level
        data.extend_from_slice(&4.3f32.to_le_bytes()); // bounding radius
        data.push(1); // NPC
        data.extend_from_slice(&2u32.to_le_bytes()); // gender bitfield
        data.push(0); // other data
        data.extend_from_slice(&(-1f32).to_le_bytes()); // emitter radius
        data.extend_from_slice(&0f32.to_le_bytes()); // emitter ID
        data.push(1); // character property count
        data.extend_from_slice(&1u32.to_le_bytes()); // body type
        data.push(100); // HP
        data.extend_from_slice(&[11, 22, 33, 44, 55, 66]); // hair/color, eyes, styles
        data.extend_from_slice(&0x11223344u32.to_le_bytes()); // Drakkin heritage
        data.extend_from_slice(&0x55667788u32.to_le_bytes()); // Drakkin tattoo
        data.extend_from_slice(&0xaabbccddu32.to_le_bytes()); // Drakkin details
        data.extend_from_slice(&[3, 0, 0, 4]); // chest, material, variation, helm
        data.extend_from_slice(&5f32.to_le_bytes()); // size
        data.push(6); // face
        data.extend_from_slice(&0.5f32.to_le_bytes()); // walk speed
        data.extend_from_slice(&1.25f32.to_le_bytes()); // run speed
        data.extend_from_slice(&race.to_le_bytes());
        data.push(0); // holding
        data.extend_from_slice(&[0; 12]); // deity, guild ID, guild rank
        data.extend_from_slice(&[1, 0, 100, 9, fly_mode]); // class, PvP, stand, light, gravity
        data.extend_from_slice(b"Sentinel\0");
        data.extend_from_slice(&[0; 4 + 2 + 4 + 1 + 4 + 20]);
        let equipment_bytes = if race <= 12 || matches!(race, 128 | 130 | 330 | 522) {
            216
        } else {
            60
        };
        data.resize(data.len() + equipment_bytes, 0);
        let position_words = [0u32, 0, 0, (77 * 8) << 10, 0];
        data.extend(position_words.into_iter().flat_map(u32::to_le_bytes));
        data
    }

    #[test]
    fn spawn_preserves_all_facial_features_without_shifting_following_fields() {
        for race in [4, 112, 522] {
            let packet = spawn_packet(3, race);
            let spawn = parse_spawn(&packet).unwrap();
            let a = spawn.appearance;
            assert_eq!(a.hair_color, 11);
            assert_eq!(a.beard_color, 22);
            assert_eq!(a.eye_color_1, 33);
            assert_eq!(a.eye_color_2, 44);
            assert_eq!(a.hair_style, 55);
            assert_eq!(a.beard, 66);
            assert_eq!(a.drakkin_heritage, 0x11223344);
            assert_eq!(a.drakkin_tattoo, 0x55667788);
            assert_eq!(a.drakkin_details, 0xaabbccdd);
            assert_eq!((a.texture, a.helm_texture, a.face), (3, 4, 6));
            assert_eq!(spawn.size, 5.);
            assert_eq!(spawn.walk_speed, 0.5);
            assert_eq!(spawn.run_speed, 1.25);
            assert_eq!(spawn.race, race);
            assert_eq!(spawn.position.z, 77.);
            for length in 0..packet.len() {
                assert!(parse_spawn(&packet[..length]).is_none());
            }
        }
    }

    #[test]
    fn spawn_gravity_mode_follows_light_for_both_equipment_layouts() {
        for race in [4, 112] {
            for fly_mode in [0, 1, 2, 3, 4, 5, 255] {
                let spawn = parse_spawn(&spawn_packet(fly_mode, race)).unwrap();
                assert_eq!(spawn.fly_mode, fly_mode);
                assert_eq!(spawn.stand_state, 100);
                assert_eq!(spawn.last_name, "Sentinel");
                assert_eq!(spawn.gender, 2);
                assert_eq!(spawn.race, race);
                assert_eq!(spawn.position.z, 77.);
            }
        }
    }

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
        assert!(parse_environment(&[0; 947]).is_none());
    }

    #[test]
    fn zone_motion_fields_follow_the_rof2_new_zone_encoder() {
        let mut data = vec![0; 948];
        data[516..520].copy_from_slice(&0.4f32.to_le_bytes());
        data[608..612].copy_from_slice(&(-3000f32).to_le_bytes());
        data[868..872].copy_from_slice(&u32::MAX.to_le_bytes());
        data[880..884].copy_from_slice(&50u32.to_le_bytes());
        data[884..888].copy_from_slice(&10u32.to_le_bytes());
        data[894] = 1;
        data[940..944].copy_from_slice(&1u32.to_le_bytes());
        let environment = parse_environment(&data).unwrap();
        assert_eq!(environment.gravity, 0.4);
        assert_eq!(environment.underworld, -3000.);
        assert_eq!(environment.underworld_teleport_index, u32::MAX);
        assert_eq!(environment.lava_damage, 50);
        assert_eq!(environment.min_lava_damage, 10);
        assert!(environment.fall_damage_disabled);
        assert!(environment.levitation_disabled);
        data[516..520].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(parse_environment(&data).is_none());
    }
}
