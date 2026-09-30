//! Structured RoF2 gameplay messages. All identifiers and inventory addresses are
//! authoritative server values; sending a command does not imply acceptance.
use crate::inventory::{InventoryItem, InventorySlot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ChatChannel {
    Guild = 0,
    Group = 2,
    Shout = 3,
    Auction = 4,
    Ooc = 5,
    Broadcast = 6,
    Tell = 7,
    Say = 8,
    GmSay = 11,
    Raid = 15,
}

/// Coin move locations deliberately exclude cursor/trade/destruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CoinLocation {
    Carried = 1,
    Bank = 2,
    SharedBank = 4,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CoinType {
    Copper = 0,
    Silver = 1,
    Gold = 2,
    Platinum = 3,
}

#[derive(Debug, Clone)]
pub enum Command {
    Training(crate::training::TrainingCommand),
    Death(crate::death::DeathCommand),
    Social(crate::social::SocialCommand),
    Raid(crate::raid::RaidCommand),
    Trade(crate::trade::TradeCommand),
    ItemUse(crate::item_use::ItemUseCommand),
    MerchantOpen {
        merchant_id: u32,
        player_id: u32,
    },
    MerchantBuy {
        merchant_id: u32,
        player_id: u32,
        slot: u32,
        quantity: u32,
        price: u32,
    },
    MerchantSell {
        merchant_id: u32,
        slot: InventorySlot,
        quantity: u32,
    },
    MerchantClose,
    /// Requires a nearby banker. Shared bank supports platinum only and must be enabled by server policy.
    MoveCoin {
        from: CoinLocation,
        to: CoinLocation,
        coin: CoinType,
        amount: u32,
    },
    /// Consolidates carried/bank denominations and returns authoritative balances; not a bank-open handshake.
    BankerChange,
    MemorizeSpell {
        slot: u32,
        spell_id: u32,
    },
    UnmemorizeSpell {
        slot: u32,
        spell_id: u32,
    },
    CastSpell {
        slot: u32,
        spell_id: u32,
        target_id: u32,
    },
    InterruptSpell,
    /// Uses the RoF2 buff slot from a server event, not a canonical server slot.
    RemoveBuff {
        slot: u32,
        player_id: u32,
    },
    ClickDoor {
        door_id: u8,
        player_id: u32,
    },
    /// Requests a zone change; the server must accept before the client leaves.
    /// For a natural border crossing, use the destination zone/instance from
    /// its zone point and reason 0. The server resolves the exit from the last
    /// reported player position, so send movement before this request.
    ZoneChange {
        character: String,
        zone_id: u16,
        instance_id: u16,
        position: [f32; 3],
        reason: u32,
    },
    Emote(String),
    Chat {
        channel: ChatChannel,
        target: String,
        text: String,
        language: u32,
    },
    MoveItem {
        from: InventorySlot,
        to: InventorySlot,
        count: u32,
    },
    DeleteItem {
        slot: InventorySlot,
        count: u32,
    },
    AutoAttack(bool),
    Assist(u32),
    Consider {
        player_id: u32,
        target_id: u32,
    },
    LootRequest(u32),
    LootItem {
        corpse_id: u32,
        player_id: u32,
        slot: u16,
        auto_loot: bool,
    },
    EndLoot(u32),
    /// EQ appearance 0=standing, 1=sitting, 2=crouching, 3=dead, 4=looting.
    Posture {
        player_id: u32,
        posture: u32,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ChatMessage {
    pub sender: String,
    pub target: String,
    pub channel: u32,
    pub language: u32,
    pub text: String,
}
#[derive(Debug, Clone)]
pub struct ServerMessage {
    pub color: u32,
    pub string_id: Option<u32>,
    pub arguments: Vec<String>,
    pub text: Option<String>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Currency {
    pub platinum: u32,
    pub gold: u32,
    pub silver: u32,
    pub copper: u32,
}
#[derive(Debug, Clone)]
pub struct PlayerProfile {
    pub name: String,
    pub last_name: String,
    /// Independently establishes local membership; only GUILD_NONE maps to None.
    pub guild_id: Option<u32>,
    /// Server's modern rank value, not a permission grant.
    pub guild_rank: u8,
    pub race: u32,
    pub class: u8,
    pub level: u8,
    pub hp: u32,
    pub mana: u32,
    pub endurance: u32,
    pub stats: [u32; 7],
    pub currency: Currency,
    pub bank_currency: Currency,
    pub cursor_currency: Currency,
    pub shared_platinum: u32,
    /// Last profile snapshot of unspent training points, not a live balance.
    pub training_points: u32,
    /// Raw base skills; additional slots do not establish named capabilities.
    pub skills: Vec<u32>,
    /// Raw language bytes, including any extra profile padding slots.
    pub languages: Vec<u8>,
    /// Absolute XP at profile time, distinct from subsequent XP bar updates.
    pub experience_total: u64,
    /// Full-width unspent AA points at profile time; AA stats updates use u16.
    pub aa_unspent_points: u32,
    pub spell_book: Vec<u32>,
    pub memorized_spells: Vec<u32>,
    pub spell_refresh: Vec<u32>,
    pub buffs: Vec<Buff>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct EquipmentAppearance {
    pub material: u32,
    pub elite_material: u32,
    pub hero_forge_model: u32,
    pub color: u32,
}
#[derive(Debug, Clone, Default)]
pub struct CharacterAppearance {
    pub texture: u8,
    pub helm_texture: u8,
    pub face: u8,
    pub hair_color: u8,
    pub beard_color: u8,
    pub eye_color_1: u8,
    pub eye_color_2: u8,
    pub hair_style: u8,
    pub beard: u8,
    pub drakkin_heritage: u32,
    pub drakkin_tattoo: u32,
    pub drakkin_details: u32,
    pub equipment: [EquipmentAppearance; 9],
}
/// RoF2 OP_Illusion. Eye colors are absent from this wire format and must
/// remain unchanged, as must equipment until separate wear packets arrive.
#[derive(Debug, Clone, PartialEq)]
pub struct Illusion {
    pub id: u32,
    pub race: u16,
    pub gender: u8,
    pub texture: u8,
    pub helm_texture: u8,
    /// The illusion field is wider than the spawn/face-update field. Retain
    /// its full value rather than truncating an unsupported value or sentinel.
    pub face: u32,
    pub hair_style: u8,
    pub hair_color: u8,
    pub beard: u8,
    pub beard_color: u8,
    pub size: f32,
    pub drakkin_heritage: u32,
    pub drakkin_tattoo: u32,
    pub drakkin_details: u32,
}
/// Server OP_SetFace (not the client OP_FaceChange request). Replaces facial
/// features only; does not carry race, gender, size, texture, or equipment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceChange {
    pub id: u32,
    pub hair_color: u8,
    pub beard_color: u8,
    pub eye_color_1: u8,
    pub eye_color_2: u8,
    pub hair_style: u8,
    pub beard: u8,
    pub face: u8,
    pub drakkin_heritage: u32,
    pub drakkin_tattoo: u32,
    pub drakkin_details: u32,
}
#[derive(Debug, Clone)]
pub struct WearChange {
    pub id: u32,
    pub slot: u8,
    pub appearance: EquipmentAppearance,
}
#[derive(Debug, Clone)]
pub struct Damage {
    pub target_id: u32,
    pub source_id: u32,
    pub skill: u8,
    pub spell_id: u32,
    pub amount: i32,
    pub secondary: bool,
    pub special: u32,
}
#[derive(Debug, Clone)]
pub struct Death {
    pub id: u32,
    pub killer_id: u32,
    pub corpse_id: u32,
    pub skill: u32,
    pub spell_id: u32,
    pub damage: u32,
}
#[derive(Debug, Clone)]
pub struct Consider {
    pub player_id: u32,
    pub target_id: u32,
    pub faction: u32,
    pub level: u32,
    pub pvp: bool,
}

#[derive(Debug, Clone)]
pub struct Buff {
    pub slot: u32,
    pub spell_id: u32,
    pub ticks_remaining: u32,
    pub num_hits: u32,
    pub caster: String,
}

/// Explicit particle definition, not a spells_us spell ID. RoF2 forwards the
/// common SpellEffect_Struct unchanged, including its millisecond timing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellEffect {
    pub effect_id: u32,
    pub source_id: u32,
    pub target_id: u32,
    pub duration_ms: u32,
    pub finish_delay_ms: u32,
    pub unknown_020: u32,
}

/// Authored item projectile. Position and heading remain in server space,
/// matching other network events. Velocity units and trajectory are client
/// presentation parameters, not an authoritative damage/arrival timestamp.
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub source_id: u32,
    pub target_id: u32,
    pub position: [f32; 3],
    pub velocity: f32,
    pub launch_angle: f32,
    pub tilt: f32,
    pub arc: f32,
    pub item_id: u32,
    pub skill: u8,
    pub item_type: u8,
    pub model_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NimbusEffect {
    pub id: u32,
    pub effect_id: u32,
    pub removed: bool,
}

#[derive(Debug, Clone)]
pub struct Door {
    pub id: u8,
    pub name: String,
    pub position: [f32; 3],
    pub heading: f32,
    pub incline: u32,
    pub size: u32,
    pub open_type: u8,
    pub state: u8,
    pub inverted: bool,
    pub parameter: u32,
}
#[derive(Debug, Clone)]
pub struct ZoneDestination {
    pub zone_id: u16,
    pub instance_id: u16,
    pub position: [f32; 3],
    pub heading: f32,
}

/// A server-provided destination for an asset-defined zone-line trigger.
/// This packet does not include source coordinates or trigger geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonePoint {
    /// Exact `zone_points.number` (the wire calls this `iterator`), not the
    /// record's ordinal in the packet and not a destination zone ID.
    pub number: u32,
    pub zone_id: u16,
    pub instance_id: u16,
    /// Destination XYZ in server coordinates. A component of 999999 means
    /// retain that source coordinate; the server resolves this on acceptance.
    pub position: [f32; 3],
    /// Destination EQ heading; 999 means retain the source heading.
    pub heading: f32,
}

#[derive(Debug, Clone)]
pub enum GameplayEvent {
    Training(crate::training::TrainingEvent),
    Recovery(crate::death::DeathEvent),
    Social(crate::social::SocialEvent),
    Raid(crate::raid::RaidEvent),
    Guild(crate::guild::GuildEvent),
    Progression(crate::progression::ProgressionEvent),
    Trade(crate::trade::TradeEvent),
    ItemUse(crate::item_use::ItemUseEvent),
    MerchantOpened {
        merchant_id: u32,
        command: u32,
        rate: f32,
        tabs: u32,
    },
    MerchantBought {
        merchant_id: u32,
        player_id: u32,
        slot: u32,
        quantity: u32,
        price: u32,
    },
    MerchantSold {
        merchant_id: u32,
        slot: InventorySlot,
        quantity: u32,
        price: u32,
        rejected: bool,
    },
    MerchantItemRemoved {
        merchant_id: u32,
        player_id: u32,
        slot: u32,
    },
    MerchantClosed,
    BankerBalances {
        carried: Currency,
        bank: Currency,
    },
    SpellMemorized {
        slot: u32,
        spell_id: u32,
        action: u32,
        reduction: u32,
    },
    BeginCast {
        caster_id: u32,
        spell_id: u32,
        cast_time_ms: u32,
    },
    CastInterrupted {
        id: u32,
        string_id: u32,
        message: String,
    },
    SpellBarEnabled {
        spell_id: u32,
        slot: i32,
        keep_casting: bool,
        mana: u32,
        endurance: u32,
    },
    SpellAction {
        source_id: u32,
        target_id: u32,
        spell_id: u32,
        level: u16,
        /// 231 is a spell; other values are combat skill/action types.
        action_type: u8,
        spell_level: u8,
        instrument_modifier: f32,
        effect_flag: u8,
    },
    SpellEffect(SpellEffect),
    Projectile(Projectile),
    NimbusEffect(NimbusEffect),
    Buffs {
        id: u32,
        all: bool,
        tick_timer: u32,
        kind: u8,
        buffs: Vec<Buff>,
    },
    BuffChanged {
        id: u32,
        buff: Buff,
        removed: bool,
    },
    Doors(Vec<Door>),
    /// Replaces the destination table for the current zone. Source trigger
    /// containment must be determined from zone assets, not these positions.
    ZonePoints(Vec<ZonePoint>),
    DoorMoved {
        id: u8,
        action: u8,
    },
    ZoneChangeRequested(ZoneDestination),
    ZoneChangeResult {
        zone_id: u16,
        instance_id: u16,
        position: [f32; 3],
        success: i32,
    },
    /// The accepted world handoff has started; discard entities from the old zone.
    ZoneTransition {
        zone_id: u16,
        instance_id: u16,
    },
    Inventory(Vec<InventoryItem>),
    /// Packet type is retained: 0=inspection, 0x64=merchant, 0x66=loot,
    /// 0x67=inventory delivery, 0x69=inventory, 0x6a=limbo/cursor delivery.
    Item {
        packet_type: u32,
        item: InventoryItem,
    },
    ItemMoved {
        from: InventorySlot,
        to: InventorySlot,
        count: u32,
    },
    ItemDeleted {
        from: InventorySlot,
        /// Units consumed; the wire's 0xffffffff sentinel means one unit.
        count: u32,
    },
    ItemChargeUsed {
        from: InventorySlot,
        count: u32,
    },
    Chat(ChatMessage),
    Message(ServerMessage),
    Profile(PlayerProfile),
    Health {
        id: u32,
        current: i32,
        maximum: i32,
    },
    Mana {
        current: u32,
        maximum: Option<u32>,
    },
    Endurance {
        current: u32,
        maximum: Option<u32>,
    },
    ManaEndurance {
        mana: u32,
        endurance: u32,
    },
    Currency(Currency),
    Hunger {
        food: u32,
        water: u32,
    },
    Assist(u32),
    Consider(Consider),
    Damage(Damage),
    Death(Death),
    Animation {
        id: u32,
        action: u8,
        speed: u8,
    },
    WearChange(WearChange),
    Illusion(Illusion),
    FaceChange(FaceChange),
    SpawnAppearance {
        id: u32,
        kind: u16,
        parameter: u32,
    },
    LootOpened {
        response: u8,
        currency: Currency,
    },
    LootItemAcknowledged {
        corpse_id: u32,
        player_id: u32,
        slot: u16,
        /// A quest or dynamic-zone lockout can reject the item without closing loot.
        rejected: bool,
    },
    LootComplete,
}

use crate::{
    AppPacket,
    inventory::{parse_inventory, parse_item_packet},
    wire::{Reader, u32_at},
    zone::ZoneError,
};
fn currency(r: &mut Reader<'_>) -> Option<Currency> {
    Some(Currency {
        platinum: r.u32()?,
        gold: r.u32()?,
        silver: r.u32()?,
        copper: r.u32()?,
    })
}
fn checked_text(text: &str, max: usize) -> Result<(), ZoneError> {
    if text.len() > max || text.as_bytes().contains(&0) {
        return Err(ZoneError::Malformed("outgoing text"));
    }
    Ok(())
}
fn text(out: &mut Vec<u8>, value: &str) {
    out.extend(value.as_bytes());
    out.push(0);
}

pub fn encode_command(command: Command) -> Result<AppPacket, ZoneError> {
    let mut out = Vec::new();
    let opcode = match command {
        Command::Training(command) => return crate::training::encode_command(command),
        Command::Death(command) => return crate::death::encode_command(command),
        Command::Social(command) => return crate::social::encode_command(command),
        Command::Raid(command) => return crate::raid::encode_command(command),
        Command::Trade(command) => return crate::trade::encode_command(command),
        Command::ItemUse(command) => return crate::item_use::encode_command(command),
        Command::MerchantOpen {
            merchant_id,
            player_id,
        } => {
            if merchant_id == 0 || player_id == 0 {
                return Err(ZoneError::Malformed("merchant id"));
            }
            for value in [merchant_id, player_id, 1, 1f32.to_bits(), 1, u32::MAX] {
                out.extend(value.to_le_bytes());
            }
            0x4fed
        }
        Command::MerchantBuy {
            merchant_id,
            player_id,
            slot,
            quantity,
            price,
        } => {
            if merchant_id == 0
                || player_id == 0
                || slot == 0
                || slot > u16::MAX as u32
                || quantity == 0
                || quantity > i16::MAX as u32
            {
                return Err(ZoneError::Malformed("merchant purchase"));
            }
            for value in [merchant_id, player_id, slot, 0, quantity, 0, price, 0] {
                out.extend(value.to_le_bytes());
            }
            0x0ddd
        }
        Command::MerchantSell {
            merchant_id,
            slot,
            quantity,
        } => {
            if merchant_id == 0
                || slot.kind != 0
                || slot.augment.is_some()
                || slot.server_slot().is_none()
                || quantity == 0
                || quantity > i16::MAX as u32
            {
                return Err(ZoneError::Malformed("merchant sale"));
            }
            out.extend(merchant_id.to_le_bytes());
            for value in [slot.slot, slot.bag.unwrap_or(u16::MAX), u16::MAX, 0] {
                out.extend(value.to_le_bytes());
            }
            out.extend(quantity.to_le_bytes());
            out.extend(0u32.to_le_bytes());
            0x791b
        }
        Command::MerchantClose => 0x30a8,
        Command::BankerChange => {
            out.extend(0u32.to_le_bytes());
            0x791e
        }
        Command::MoveCoin {
            from,
            to,
            coin,
            amount,
        } => {
            if from == to
                || amount == 0
                || amount > i32::MAX as u32
                || ((from == CoinLocation::SharedBank || to == CoinLocation::SharedBank)
                    && coin != CoinType::Platinum)
            {
                return Err(ZoneError::Malformed("coin move"));
            }
            for value in [from as u32, to as u32, coin as u32, coin as u32, amount] {
                out.extend(value.to_le_bytes());
            }
            0x0bcf
        }
        Command::MemorizeSpell { slot, spell_id } | Command::UnmemorizeSpell { slot, spell_id } => {
            if slot >= 12 || spell_id == 0 || spell_id > 45000 {
                return Err(ZoneError::Malformed("spell gem"));
            }
            let action = if matches!(command, Command::UnmemorizeSpell { .. }) {
                2u32
            } else {
                1
            };
            for value in [slot, spell_id, action, 0] {
                out.extend(value.to_le_bytes());
            }
            0x217c
        }
        Command::CastSpell {
            slot,
            spell_id,
            target_id,
        } => {
            if slot >= 12 || spell_id == 0 || spell_id > 45000 {
                return Err(ZoneError::Malformed("spell cast"));
            }
            out.extend(slot.to_le_bytes());
            out.extend(spell_id.to_le_bytes());
            InventorySlot::DELETE.write(&mut out);
            out.extend(target_id.to_le_bytes());
            out.extend([0; 20]);
            0x1287
        }
        Command::InterruptSpell => 0x5467,
        Command::RemoveBuff { slot, player_id } => {
            if slot >= 128 {
                return Err(ZoneError::Malformed("buff slot"));
            }
            out.extend(slot.to_le_bytes());
            out.extend(player_id.to_le_bytes());
            0x64f2
        }
        Command::ClickDoor { door_id, player_id } => {
            let id = u16::try_from(player_id).map_err(|_| ZoneError::Malformed("spawn id"))?;
            out.resize(16, 0);
            out[0] = door_id;
            out[12..14].copy_from_slice(&id.to_le_bytes());
            0x3a8f
        }
        Command::ZoneChange {
            character,
            zone_id,
            instance_id,
            position,
            reason,
        } => {
            checked_text(&character, 63)?;
            if character.is_empty() || !position.iter().all(|v| v.is_finite()) {
                return Err(ZoneError::Malformed("zone request"));
            }
            out.resize(100, 0);
            out[..character.len()].copy_from_slice(character.as_bytes());
            out[64..66].copy_from_slice(&zone_id.to_le_bytes());
            out[66..68].copy_from_slice(&instance_id.to_le_bytes());
            for (offset, value) in [(76, position[1]), (80, position[0]), (84, position[2])] {
                out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            out[88..92].copy_from_slice(&reason.to_le_bytes());
            0x2d18
        }
        Command::Emote(message) => {
            checked_text(&message, 1000)?;
            out.extend(0u32.to_le_bytes());
            text(&mut out, &message);
            0x373b
        }
        Command::Chat {
            channel,
            target,
            text: message,
            language,
        } => {
            checked_text(&target, 63)?;
            checked_text(&message, 1000)?;
            if language >= 32 {
                return Err(ZoneError::Malformed("chat language"));
            }
            text(&mut out, "");
            text(&mut out, &target);
            for value in [0, language, channel as u32, 0] {
                out.extend(value.to_le_bytes());
            }
            out.push(0);
            out.extend(100u32.to_le_bytes());
            text(&mut out, &message);
            out.extend([0; 15]);
            0x2b2d
        }
        Command::MoveItem { from, to, count } => {
            if !from.movable() || from == InventorySlot::DELETE || !to.movable() {
                return Err(ZoneError::Malformed("inventory slot"));
            }
            from.write(&mut out);
            to.write(&mut out);
            out.extend(count.to_le_bytes());
            0x32ee
        }
        Command::DeleteItem { slot, count } => {
            if !slot.movable() || slot == InventorySlot::DELETE {
                return Err(ZoneError::Malformed("inventory slot"));
            }
            slot.write(&mut out);
            InventorySlot::DELETE.write(&mut out);
            out.extend(count.to_le_bytes());
            0x32ee
        }
        Command::AutoAttack(enabled) => {
            out.extend(u32::from(enabled).to_le_bytes());
            0x109d
        }
        Command::Assist(id) => {
            out.extend(id.to_le_bytes());
            0x4478
        }
        Command::Consider {
            player_id,
            target_id,
        } => {
            out.extend(player_id.to_le_bytes());
            out.extend(target_id.to_le_bytes());
            out.extend([0; 12]);
            0x742b
        }
        Command::LootRequest(id) => {
            out.extend(id.to_le_bytes());
            0x0adf
        }
        Command::LootItem {
            corpse_id,
            player_id,
            slot,
            auto_loot,
        } => {
            out.extend(corpse_id.to_le_bytes());
            out.extend(player_id.to_le_bytes());
            out.extend(slot.to_le_bytes());
            out.extend([0; 2]);
            out.extend(u32::from(auto_loot).to_le_bytes());
            out.extend([0; 4]);
            0x4dc9
        }
        Command::EndLoot(id) => {
            out.extend(id.to_le_bytes());
            0x30f7
        }
        Command::Posture { player_id, posture } => {
            let parameter = match posture {
                0 => 100u32,
                1 => 110,
                2 => 111,
                3 => 115,
                4 => 105,
                _ => return Err(ZoneError::Malformed("posture")),
            };
            let id = u16::try_from(player_id).map_err(|_| ZoneError::Malformed("spawn id"))?;
            out.extend(id.to_le_bytes());
            out.extend(14u16.to_le_bytes());
            out.extend(parameter.to_le_bytes());
            0x0971
        }
    };
    Ok(AppPacket::new(opcode, out))
}

/// None means an opcode belongs to another subsystem; a recognized truncated
/// gameplay packet is an error, never a fabricated empty inventory or message.
pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<GameplayEvent, ZoneError>> {
    if let Some(event) = crate::death::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Recovery));
    }
    if let Some(event) = crate::trade::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Trade));
    }
    if let Some(event) = crate::item_use::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::ItemUse));
    }
    if let Some(event) = crate::social::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Social));
    }
    if let Some(event) = crate::raid::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Raid));
    }
    if let Some(event) = crate::guild::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Guild));
    }
    if let Some(event) = crate::training::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Training));
    }
    if let Some(event) = crate::progression::parse_packet(opcode, data) {
        return Some(event.map(GameplayEvent::Progression));
    }
    if !matches!(
        opcode,
        0x4fed
            | 0x0ddd
            | 0x791b
            | 0x724f
            | 0x3196
            | 0x791e
            | 0x217c
            | 0x318f
            | 0x048c
            | 0x744c
            | 0x5936
            | 0x747c
            | 0xc693
            | 0x7b1e
            | 0x3377
            | 0x4f4b
            | 0x659c
            | 0x7291
            | 0x69a4
            | 0x08e8
            | 0x3fcf
            | 0x2d18
            | 0x5ca6
            | 0x368e
            | 0x32ee
            | 0x18ad
            | 0x01b8
            | 0x2b2d
            | 0x1024
            | 0x213f
            | 0x0083
            | 0x373b
            | 0x6506
            | 0x2828
            | 0x3791
            | 0x5f42
            | 0x5467
            | 0x640c
            | 0x2a79
            | 0x4478
            | 0x742b
            | 0x6f15
            | 0x6517
            | 0x7177
            | 0x7994
            | 0x312a
            | 0x1af3
            | 0x0971
            | 0x5f44
            | 0x4dc9
            | 0x55c4
    ) {
        return None;
    }
    Some(parse_known(opcode, data).ok_or(ZoneError::Malformed("gameplay packet")))
}
fn parse_known(opcode: u16, data: &[u8]) -> Option<GameplayEvent> {
    let mut r = Reader(data);
    Some(match opcode {
        0x4fed => {
            let merchant_id = r.u32()?;
            r.skip(4)?;
            let command = r.u32()?;
            let rate = r.float()?;
            let tabs = r.u32()?;
            r.skip(4)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::MerchantOpened {
                merchant_id,
                command,
                rate,
                tabs,
            }
        }
        0x0ddd => {
            let merchant_id = r.u32()?;
            let player_id = r.u32()?;
            let slot = r.u32()?;
            r.skip(4)?;
            let quantity = r.u32()?;
            r.skip(4)?;
            let price = r.u32()?;
            r.skip(4)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::MerchantBought {
                merchant_id,
                player_id,
                slot,
                quantity,
                price,
            }
        }
        0x791b => {
            let merchant_id = r.u32()?;
            let main = r.u16()?;
            let bag = r.u16()?;
            let augment = r.u16()?;
            r.skip(2)?;
            let quantity = r.u32()?;
            let price = r.u32()?;
            if !r.done() {
                return None;
            }
            let slot = InventorySlot {
                kind: 0,
                slot: main,
                bag: (bag != u16::MAX).then_some(bag),
                augment: (augment != u16::MAX).then_some(augment),
            };
            GameplayEvent::MerchantSold {
                merchant_id,
                slot,
                quantity,
                price,
                rejected: main == u16::MAX || quantity == 0,
            }
        }
        0x724f => {
            let merchant_id = r.u32()?;
            let player_id = r.u32()?;
            let slot = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::MerchantItemRemoved {
                merchant_id,
                player_id,
                slot,
            }
        }
        0x3196 => {
            if !r.done() {
                return None;
            }
            GameplayEvent::MerchantClosed
        }
        0x791e => {
            let carried = currency(&mut r)?;
            let bank = currency(&mut r)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::BankerBalances { carried, bank }
        }
        0x217c => {
            let slot = r.u32()?;
            let spell_id = r.u32()?;
            let action = r.u32()?;
            let reduction = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::SpellMemorized {
                slot,
                spell_id,
                action,
                reduction,
            }
        }
        0x318f => {
            let spell_id = r.u32()?;
            let caster_id = r.u16()? as u32;
            let cast_time_ms = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::BeginCast {
                caster_id,
                spell_id,
                cast_time_ms,
            }
        }
        0x048c => {
            let id = r.u32()?;
            let string_id = r.u32()?;
            let message = if r.done() {
                String::new()
            } else {
                r.string(4096)?
            };
            if !r.done() {
                return None;
            }
            GameplayEvent::CastInterrupted {
                id,
                string_id,
                message,
            }
        }
        0x744c => {
            let target_id = r.u16()? as u32;
            let source_id = r.u16()? as u32;
            let level = r.u16()?;
            r.skip(4)?;
            let instrument_modifier = r.float()?;
            r.skip(12)?; // force, hit_heading and hit_pitch are combat push data
            let action_type = r.u8()?;
            r.skip(6)?;
            let spell_id = r.u32()?;
            let spell_level = r.u8()?;
            let effect_flag = r.u8()?;
            r.skip(17)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::SpellAction {
                source_id,
                target_id,
                spell_id,
                level,
                action_type,
                spell_level,
                instrument_modifier,
                effect_flag,
            }
        }
        0x5936 => {
            let effect = SpellEffect {
                effect_id: r.u32()?,
                source_id: r.u32()?,
                target_id: r.u32()?,
                duration_ms: r.u32()?,
                finish_delay_ms: r.u32()?,
                unknown_020: r.u32()?,
            };
            r.skip(4)?; // unknown024/025/026, encoded as 1/1/0 by EQEmu
            if !r.done() {
                return None;
            }
            GameplayEvent::SpellEffect(effect)
        }
        0xc693 | 0x7b1e => {
            let effect = NimbusEffect {
                id: r.u32()?,
                effect_id: r.u32()?,
                removed: opcode == 0x7b1e,
            };
            if !r.done() {
                return None;
            }
            GameplayEvent::NimbusEffect(effect)
        }
        0x747c => {
            let y = r.float()?;
            let x = r.float()?;
            let z = r.float()?;
            r.skip(12)?;
            let velocity = r.float()?;
            let launch_angle = r.float()?;
            let tilt = r.float()?;
            r.skip(8)?;
            let arc = r.float()?;
            let source_id = r.u32()?;
            let target_id = r.u32()?;
            let item_id = r.u32()?;
            r.skip(13)?;
            let skill = r.u8()?;
            let item_type = r.u8()?;
            r.skip(14)?;
            let model = r.take(27)?;
            // The RoF2 encoder uses strncpy and can fill the entire fixed field.
            let end = model.iter().position(|b| *b == 0).unwrap_or(model.len());
            let model_name = String::from_utf8_lossy(&model[..end]).into_owned();
            if !r.done() {
                return None;
            }
            GameplayEvent::Projectile(Projectile {
                source_id,
                target_id,
                position: [x, y, z],
                velocity,
                launch_angle,
                tilt,
                arc,
                item_id,
                skill,
                item_type,
                model_name,
            })
        }
        0x3377 | 0x4f4b => {
            let id = r.u32()?;
            let tick_timer = r.u32()?;
            let all = r.u8()? != 0;
            let count = r.u16()? as usize;
            if count > 128 {
                return None;
            }
            let mut buffs = Vec::with_capacity(count.min(32));
            for _ in 0..count {
                buffs.push(Buff {
                    slot: r.u32()?,
                    spell_id: r.u32()?,
                    ticks_remaining: r.u32()?,
                    num_hits: r.u32()?,
                    caster: r.string(63)?,
                });
            }
            let kind = r.u8()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Buffs {
                id,
                all,
                tick_timer,
                kind,
                buffs,
            }
        }
        0x659c => {
            let id = r.u32()?;
            let b = r.take(88)?;
            let slot = r.u32()?;
            let removed = r.u32()? == 1;
            if !r.done() {
                return None;
            }
            GameplayEvent::BuffChanged {
                id,
                buff: Buff {
                    slot,
                    spell_id: u32_at(b, 8)?,
                    ticks_remaining: u32_at(b, 12)?,
                    num_hits: u32_at(b, 20)?,
                    caster: String::new(),
                },
                removed,
            }
        }
        0x7291 => {
            if !data.len().is_multiple_of(100) || data.len() / 100 > 256 {
                return None;
            }
            let mut doors = Vec::with_capacity(data.len() / 100);
            while !r.done() {
                let record = r.take(100)?;
                let mut d = Reader(record);
                let name = Reader(d.take(32)?).string(31)?;
                let y = d.float()?;
                let x = d.float()?;
                let z = d.float()?;
                let heading = d.float()?;
                let incline = d.u32()?;
                let size = d.u32()?;
                d.skip(4)?;
                let id = d.u8()?;
                let open_type = d.u8()?;
                let state = d.u8()?;
                let inverted = d.u8()? != 0;
                let parameter = d.u32()?;
                doors.push(Door {
                    id,
                    name,
                    position: [x, y, z],
                    heading,
                    incline,
                    size,
                    open_type,
                    state,
                    inverted,
                    parameter,
                });
            }
            GameplayEvent::Doors(doors)
        }
        0x69a4 => {
            let count = r.u32()? as usize;
            // RoF2 always includes one extra, unused 32-byte record. Validate
            // the full size before allocating, including count overflow.
            let size = count.checked_add(1)?.checked_mul(32)?.checked_add(4)?;
            if data.len() != size {
                return None;
            }
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                let number = r.u32()?;
                let y = r.float()?;
                let x = r.float()?;
                let z = r.float()?;
                let heading = r.float()?;
                let zone_id = r.u16()?;
                let instance_id = r.u16()?;
                r.skip(8)?;
                points.push(ZonePoint {
                    number,
                    zone_id,
                    instance_id,
                    position: [x, y, z],
                    heading,
                });
            }
            r.skip(32)?; // unused trailer, not another zone point
            GameplayEvent::ZonePoints(points)
        }
        0x08e8 => {
            let id = r.u8()?;
            let action = r.u8()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::DoorMoved { id, action }
        }
        0x3fcf => {
            let zone_id = r.u16()?;
            let instance_id = r.u16()?;
            r.skip(4)?;
            let y = r.float()?;
            let x = r.float()?;
            let z = r.float()?;
            let heading = r.float()?;
            r.skip(152)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::ZoneChangeRequested(ZoneDestination {
                zone_id,
                instance_id,
                position: [x, y, z],
                heading,
            })
        }
        0x2d18 => {
            r.skip(64)?;
            let zone_id = r.u16()?;
            let instance_id = r.u16()?;
            r.skip(8)?;
            let y = r.float()?;
            let x = r.float()?;
            let z = r.float()?;
            r.skip(4)?;
            let success = r.i32()?;
            r.skip(4)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::ZoneChangeResult {
                zone_id,
                instance_id,
                position: [x, y, z],
                success,
            }
        }
        0x5ca6 => GameplayEvent::Inventory(parse_inventory(data)?),
        0x368e => {
            let (packet_type, item) = parse_item_packet(data)?;
            GameplayEvent::Item { packet_type, item }
        }
        0x32ee | 0x18ad | 0x01b8 => {
            let from = InventorySlot::read(&mut r)?;
            let to = InventorySlot::read(&mut r)?;
            let count = r.u32()?;
            if !r.done() {
                return None;
            }
            if opcode == 0x32ee {
                GameplayEvent::ItemMoved { from, to, count }
            } else {
                // EQEmu sends one packet per consumed unit/charge with this
                // sentinel. Whole-item removal uses OP_MoveItem to DELETE.
                let count = if count == u32::MAX { 1 } else { count };
                if opcode == 0x01b8 {
                    GameplayEvent::ItemChargeUsed { from, count }
                } else {
                    GameplayEvent::ItemDeleted { from, count }
                }
            }
        }
        0x2b2d => {
            let sender = r.string(63)?;
            let target = r.string(63)?;
            r.skip(4)?;
            let language = r.u32()?;
            let channel = r.u32()?;
            r.skip(9)?;
            let text = r.string(4096)?;
            r.skip(15)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Chat(ChatMessage {
                sender,
                target,
                channel,
                language,
                text,
            })
        }
        0x1024 => {
            r.skip(4)?;
            let string_id = r.u32()?;
            let color = r.u32()?;
            let mut arguments = Vec::new();
            loop {
                let arg = r.string(4096)?;
                if arg.is_empty() {
                    break;
                }
                if arguments.len() == 9 {
                    return None;
                }
                arguments.push(arg);
            }
            if !r.done() {
                return None;
            }
            GameplayEvent::Message(ServerMessage {
                color,
                string_id: Some(string_id),
                arguments,
                text: None,
            })
        }
        0x213f => {
            let string_id = r.u32()?;
            let color = r.u32()?;
            r.skip(4)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Message(ServerMessage {
                color,
                string_id: Some(string_id),
                arguments: Vec::new(),
                text: None,
            })
        }
        0x0083 => {
            r.skip(3)?;
            let color = r.u32()?;
            r.skip(4)?;
            let _speaker = r.string(63)?;
            r.skip(12)?;
            let text = r.string(16384)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Message(ServerMessage {
                color,
                string_id: None,
                arguments: Vec::new(),
                text: Some(text),
            })
        }
        0x373b => {
            let color = r.u32()?;
            let text = r.string(4096)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Message(ServerMessage {
                color,
                string_id: None,
                arguments: Vec::new(),
                text: Some(text),
            })
        }
        0x6506 => GameplayEvent::Profile(parse_profile(data)?),
        0x2828 => {
            let id = r.u16()? as u32;
            let current = r.i32()?;
            let maximum = r.i32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Health {
                id,
                current,
                maximum,
            }
        }
        0x3791 | 0x5f42 => {
            let current = r.u32()?;
            let maximum = Some(r.u32()?);
            r.skip(2)?;
            if !r.done() {
                return None;
            }
            if opcode == 0x3791 {
                GameplayEvent::Mana { current, maximum }
            } else {
                GameplayEvent::Endurance { current, maximum }
            }
        }
        0x5467 => {
            let mana = r.u32()?;
            let endurance = r.u32()?;
            let spell_id = r.u32()?;
            let keep_casting = r.u8()? != 0;
            r.skip(3)?;
            let slot = r.i32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::SpellBarEnabled {
                spell_id,
                slot,
                keep_casting,
                mana,
                endurance,
            }
        }
        0x640c => {
            let money = currency(&mut r)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Currency(money)
        }
        0x2a79 => {
            let food = r.u32()?;
            let water = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Hunger { food, water }
        }
        0x4478 => {
            let id = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Assist(id)
        }
        0x742b => {
            let player_id = r.u32()?;
            let target_id = r.u32()?;
            let faction = r.u32()?;
            let level = r.u32()?;
            let pvp = r.u8()? != 0;
            r.skip(3)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Consider(Consider {
                player_id,
                target_id,
                faction,
                level,
                pvp,
            })
        }
        0x6f15 => {
            let target_id = r.u16()? as u32;
            let source_id = r.u16()? as u32;
            let skill = r.u8()?;
            let spell_id = r.u32()?;
            let amount = r.i32()?;
            r.skip(12)?;
            let secondary = r.u8()? != 0;
            let special = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Damage(Damage {
                target_id,
                source_id,
                skill,
                spell_id,
                amount,
                secondary,
                special,
            })
        }
        0x6517 => {
            let id = r.u32()?;
            let killer_id = r.u32()?;
            let corpse_id = r.u32()?;
            let skill = r.u32()?;
            let spell_id = r.u32()?;
            r.skip(4)?;
            let damage = r.u32()?;
            r.skip(4)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Death(Death {
                id,
                killer_id,
                corpse_id,
                skill,
                spell_id,
                damage,
            })
        }
        0x7177 => {
            let id = r.u16()? as u32;
            let action = r.u8()?;
            let speed = r.u8()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Animation { id, action, speed }
        }
        0x7994 => {
            let id = r.u16()? as u32;
            let material = r.u32()?;
            r.skip(4)?;
            let elite_material = r.u32()?;
            let hero_forge_model = r.u32()?;
            r.skip(4)?;
            let color = r.u32()?;
            let slot = r.u8()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::WearChange(WearChange {
                id,
                slot,
                appearance: EquipmentAppearance {
                    material,
                    elite_material,
                    hero_forge_model,
                    color,
                },
            })
        }
        0x312a => {
            // EQEmu common/patches/rof2.cpp ENCODE(OP_Illusion), 336 bytes.
            // Drakkin features moved from common-struct offset 92 to RoF2 244.
            let id = r.u32()?;
            r.skip(64)?; // display name; the entity is identified by spawn ID
            let race = r.u16()?;
            r.skip(2)?;
            let gender = r.u8()?;
            let texture = r.u8()?;
            r.skip(2)?;
            let helm_texture = r.u8()?;
            r.skip(3)?;
            let face = r.u32()?;
            let hair_style = r.u8()?;
            let hair_color = r.u8()?;
            let beard = r.u8()?;
            let beard_color = r.u8()?;
            let size = r.float()?;
            r.skip(152)?;
            let drakkin_heritage = r.u32()?;
            let drakkin_tattoo = r.u32()?;
            let drakkin_details = r.u32()?;
            r.skip(80)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::Illusion(Illusion {
                id,
                race,
                gender,
                texture,
                helm_texture,
                face,
                hair_style,
                hair_color,
                beard,
                beard_color,
                size,
                drakkin_heritage,
                drakkin_tattoo,
                drakkin_details,
            })
        }
        0x1af3 => {
            // Mob::SetFaceAppearance broadcasts the shared 24-byte struct as
            // OP_SetFace; OP_FaceChange is the separate client request opcode.
            let hair_color = r.u8()?;
            let beard_color = r.u8()?;
            let eye_color_1 = r.u8()?;
            let eye_color_2 = r.u8()?;
            let hair_style = r.u8()?;
            let beard = r.u8()?;
            let face = r.u8()?;
            r.skip(1)?;
            let drakkin_heritage = r.u32()?;
            let drakkin_tattoo = r.u32()?;
            let drakkin_details = r.u32()?;
            let id = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::FaceChange(FaceChange {
                id,
                hair_color,
                beard_color,
                eye_color_1,
                eye_color_2,
                hair_style,
                beard,
                face,
                drakkin_heritage,
                drakkin_tattoo,
                drakkin_details,
            })
        }
        0x0971 => {
            let id = r.u16()? as u32;
            let kind = r.u16()?;
            let parameter = r.u32()?;
            if !r.done() {
                return None;
            }
            GameplayEvent::SpawnAppearance {
                id,
                kind,
                parameter,
            }
        }
        0x5f44 => {
            let response = r.u8()?;
            r.skip(3)?;
            let money = currency(&mut r)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::LootOpened {
                response,
                currency: money,
            }
        }
        0x4dc9 => {
            let corpse_id = r.u32()?;
            let player_id = r.u32()?;
            let slot = r.u16()?;
            r.skip(2)?;
            let rejected = r.i32()? == -1;
            r.skip(4)?;
            if !r.done() {
                return None;
            }
            GameplayEvent::LootItemAcknowledged {
                corpse_id,
                player_id,
                slot,
                rejected,
            }
        }
        0x55c4 => {
            if !r.done() {
                return None;
            }
            GameplayEvent::LootComplete
        }
        _ => return None,
    })
}
fn array_u32(r: &mut Reader<'_>, max: usize) -> Option<Vec<u32>> {
    Some(
        r.array(4, max)?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect(),
    )
}
fn parse_profile(data: &[u8]) -> Option<PlayerProfile> {
    // The encoder stamps its complete variable-length payload size at offset4.
    // This catches all truncation, including fields after the subset we expose.
    if u32_at(data, 4)? as usize != data.len().checked_sub(9)? {
        return None;
    }
    let mut r = Reader(data);
    r.skip(16)?;
    r.skip(1)?;
    let race = r.u32()?;
    let class = r.u8()?;
    let level = r.u8()?;
    r.skip(1)?;
    r.array(20, 5)?;
    r.skip(8)?;
    r.array(4, 100)?;
    r.array(20, 64)?;
    r.array(20, 64)?;
    r.array(4, 64)?;
    r.array(4, 64)?;
    r.skip(11 + 12 + 5 + 16 + 8)?;
    let training_points = r.u32()?;
    let mana = r.u32()?;
    let hp = r.u32()?;
    let mut stats = [0; 7];
    for stat in &mut stats {
        *stat = r.u32()?;
    }
    r.skip(28)?;
    r.array(12, 1000)?;
    let skills = array_u32(&mut r, 256)?;
    r.array(4, 128)?;
    r.array(4, 1000)?;
    for _ in 0..3 {
        r.array(4, 1024)?;
    }
    let spell_book = array_u32(&mut r, 2048)?;
    let memorized_spells = array_u32(&mut r, 64)?;
    let spell_refresh = array_u32(&mut r, 64)?;
    r.skip(1)?;
    let buff_data = r.array(80, 128)?;
    let mut buffs = Vec::new();
    for (slot, b) in buff_data.chunks_exact(80).enumerate() {
        let spell_id = u32_at(b, 19)?;
        if spell_id != 0 && spell_id != 0xffff && spell_id != u32::MAX {
            buffs.push(Buff {
                slot: slot as u32,
                spell_id,
                ticks_remaining: u32_at(b, 14)?,
                num_hits: u32_at(b, 24)?,
                caster: String::new(),
            });
        }
    }
    let carried = currency(&mut r)?;
    let cursor_currency = currency(&mut r)?;
    r.skip(12 + 8 + 4)?;
    r.array(4, 32)?;
    let aa_unspent_points = r.u32()?;
    r.skip(2)?;
    let bandoliers = r.u32()?;
    if bandoliers > 100 {
        return None;
    }
    for _ in 0..bandoliers {
        r.string(64)?;
        for _ in 0..4 {
            r.string(256)?;
            r.skip(8)?;
        }
    }
    let belt = r.u32()?;
    if belt > 100 {
        return None;
    }
    for _ in 0..belt {
        r.string(256)?;
        r.skip(8)?;
    }
    // The three totals here are encoder placeholders, not real resource maxima.
    r.skip(16 + 48 + 4 + 16)?;
    let endurance = r.u32()?;
    r.skip(8)?;
    let name = Reader(r.array(1, 64)?).string(63)?;
    let last_name = Reader(r.array(1, 64)?).string(63)?;
    r.skip(24)?;
    let languages = r.array(1, 64)?.to_vec();
    r.skip(4 + 16 + 4)?; // zone/instance, position, flags
    let guild_id = r.u32()?;
    let guild_id = (guild_id != crate::guild::GUILD_NONE).then_some(guild_id);
    let guild_rank = r.u8()?;
    r.skip(9)?;
    let experience_total = u64::from_le_bytes(r.take(8)?.try_into().ok()?);
    r.skip(1)?; // eye height
    let bank_currency = currency(&mut r)?;
    let shared_platinum = r.u32()?;
    Some(PlayerProfile {
        name,
        last_name,
        guild_id,
        guild_rank,
        race,
        class,
        level,
        hp,
        mana,
        endurance,
        stats,
        currency: carried,
        bank_currency,
        cursor_currency,
        shared_platinum,
        training_points,
        skills,
        languages,
        experience_total,
        aa_unspent_points,
        spell_book,
        memorized_spells,
        spell_refresh,
        buffs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compact variable-count profile fixture following RoF2's PlayerProfile
    // writer. Distinct progression, guild and currency data catches cursor drift
    // without relying on a local client installation or private packet capture.
    fn guild_profile_fixture(guild_id: u32, guild_rank: u8, language_count: usize) -> Vec<u8> {
        fn zeros(out: &mut Vec<u8>, size: usize) {
            out.resize(out.len() + size, 0);
        }
        fn array(out: &mut Vec<u8>, count: usize, stride: usize) {
            out.extend_from_slice(&(count as u32).to_le_bytes());
            zeros(out, count * stride);
        }
        let mut data = vec![0; 17];
        data.extend_from_slice(&522u32.to_le_bytes());
        data.extend_from_slice(&[12, 85, 0]);
        array(&mut data, 1, 20); // bind points
        zeros(&mut data, 8);
        for stride in [4, 20, 20, 4, 4] {
            array(&mut data, 1, stride);
        }
        zeros(&mut data, 11 + 12 + 5 + 16 + 8);
        data.extend_from_slice(&u32::MAX.to_le_bytes()); // training points
        for value in [101u32, 102, 10, 11, 12, 13, 14, 15, 16] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        zeros(&mut data, 28);
        array(&mut data, 1, 12); // AA
        data.extend_from_slice(&100u32.to_le_bytes()); // skills
        for value in [0u32, 254, 255, 70_001, u32::MAX].into_iter().chain(5..100) {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for _ in 0..8 {
            // disciplines, timers, spellbook, gems, refresh
            array(&mut data, 1, 4);
        }
        zeros(&mut data, 1);
        array(&mut data, 0, 80); // buffs
        for value in [20u32, 21, 22, 23, 30, 31, 32, 33] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        zeros(&mut data, 12 + 8 + 4);
        array(&mut data, 1, 4);
        data.extend_from_slice(&0xf123_4567u32.to_le_bytes()); // unspent AA
        zeros(&mut data, 2);
        data.extend_from_slice(&0u32.to_le_bytes()); // bandoliers
        data.extend_from_slice(&0u32.to_le_bytes()); // potion belt
        zeros(&mut data, 16 + 48 + 4 + 16);
        data.extend_from_slice(&103u32.to_le_bytes());
        zeros(&mut data, 8);
        for value in [b"Fellowship\0".as_slice(), b"Example\0".as_slice()] {
            data.extend_from_slice(&(value.len() as u32).to_le_bytes());
            data.extend_from_slice(value);
        }
        zeros(&mut data, 24);
        data.extend_from_slice(&(language_count as u32).to_le_bytes());
        data.extend((0..language_count).map(|index| (index as u8).wrapping_mul(255)));
        data.extend_from_slice(&202u16.to_le_bytes());
        data.extend_from_slice(&7u16.to_le_bytes());
        zeros(&mut data, 16); // position
        data.extend_from_slice(&[0, 1, 0, 0]); // flags
        data.extend_from_slice(&guild_id.to_le_bytes());
        data.push(guild_rank);
        zeros(&mut data, 9);
        data.extend_from_slice(&0xf123_4567_89ab_cdefu64.to_le_bytes());
        data.push(5); // eye height
        for value in [40u32, 41, 42, 43, 50] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        let encoded_size = (data.len() - 9) as u32;
        data[4..8].copy_from_slice(&encoded_size.to_le_bytes());
        data
    }

    #[test]
    fn profile_guild_identity_preserves_progression_and_currency_alignment() {
        for language_count in [0, 1, 28, 32, 64] {
            for (wire_id, expected_id, rank) in [
                (crate::guild::GUILD_NONE, None, 0),
                (0, Some(0), 8), // only the documented sentinel means unguilded
                (70_001, Some(70_001), 255),
            ] {
                let data = guild_profile_fixture(wire_id, rank, language_count);
                let GameplayEvent::Profile(profile) = parse_packet(0x6506, &data).unwrap().unwrap()
                else {
                    panic!()
                };
                assert_eq!(profile.name, "Fellowship");
                assert_eq!(profile.last_name, "Example");
                assert_eq!((profile.guild_id, profile.guild_rank), (expected_id, rank));
                assert_eq!(profile.training_points, u32::MAX);
                assert_eq!(
                    profile.skills,
                    [0u32, 254, 255, 70_001, u32::MAX]
                        .into_iter()
                        .chain(5..100)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    profile.languages,
                    (0..language_count)
                        .map(|index| (index as u8).wrapping_mul(255))
                        .collect::<Vec<_>>()
                );
                assert_eq!(profile.experience_total, 0xf123_4567_89ab_cdef);
                assert_eq!(profile.aa_unspent_points, 0xf123_4567);
                assert_eq!((profile.race, profile.class, profile.level), (522, 12, 85));
                assert_eq!(profile.stats, [10, 11, 12, 13, 14, 15, 16]);
                assert_eq!(
                    (profile.hp, profile.mana, profile.endurance),
                    (102, 101, 103)
                );
                assert_eq!(
                    (profile.currency.platinum, profile.cursor_currency.platinum),
                    (20, 30)
                );
                assert_eq!(
                    (
                        profile.bank_currency.platinum,
                        profile.bank_currency.gold,
                        profile.bank_currency.silver,
                        profile.bank_currency.copper,
                        profile.shared_platinum
                    ),
                    (40, 41, 42, 43, 50)
                );
                for length in 0..data.len() {
                    assert!(parse_packet(0x6506, &data[..length]).unwrap().is_err());
                    // Also verify the field reads, independently of the total
                    // size stamp, reject a truncated profile at every offset.
                    if length >= 9 {
                        let mut truncated = data[..length].to_vec();
                        truncated[4..8].copy_from_slice(&((length - 9) as u32).to_le_bytes());
                        assert!(parse_packet(0x6506, &truncated).unwrap().is_err());
                    }
                }
            }
        }
    }

    #[test]
    fn profile_rejects_language_count_beyond_existing_resource_bound() {
        let data = guild_profile_fixture(70_001, 3, 65);
        assert!(parse_packet(0x6506, &data).unwrap().is_err());
    }

    #[test]
    fn progression_dispatch_preserves_events_and_malformed_packet_errors() {
        use crate::progression::{
            OP_AA_EXP_UPDATE, OP_EXP_UPDATE, OP_LEVEL_UPDATE, OP_SKILL_UPDATE, ProgressionEvent,
        };
        let data: Vec<_> = [127u32, 70_001, 0xffff_ffff]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert!(matches!(
            parse_packet(OP_SKILL_UPDATE, &data),
            Some(Ok(GameplayEvent::Progression(
                ProgressionEvent::SkillValue {
                    wire_skill_id: 127,
                    value: 70_001,
                }
            )))
        ));
        assert!(matches!(
            parse_packet(OP_AA_EXP_UPDATE, &data),
            Some(Ok(GameplayEvent::Progression(
                ProgressionEvent::AlternateAdvancement {
                    bar_units: 127,
                    unspent_points: 4465,
                    allocation_percent: 255,
                }
            )))
        ));
        for opcode in [
            OP_AA_EXP_UPDATE,
            OP_EXP_UPDATE,
            OP_LEVEL_UPDATE,
            OP_SKILL_UPDATE,
        ] {
            assert!(parse_packet(opcode, &[]).unwrap().is_err());
        }
    }

    #[test]
    fn illusion_uses_rof2_feature_offsets_and_preserves_wide_values() {
        let mut packet = vec![0xcc; 336];
        packet[..4].copy_from_slice(&70001u32.to_le_bytes());
        packet[4..68].fill(0);
        packet[4..11].copy_from_slice(b"Drakkin");
        packet[68..70].copy_from_slice(&522u16.to_le_bytes());
        packet[72] = 1;
        packet[73] = 0xff; // preserve the original texture sentinel
        packet[76] = 9;
        packet[80..84].copy_from_slice(&0x12345678u32.to_le_bytes());
        packet[84..88].copy_from_slice(&[11, 22, 33, 44]);
        packet[88..92].copy_from_slice(&6.5f32.to_le_bytes());
        // Offset 92 is padding in RoF2, not the common struct's heritage.
        packet[92..104].copy_from_slice(&[0xee; 12]);
        packet[244..248].copy_from_slice(&0x11223344u32.to_le_bytes());
        packet[248..252].copy_from_slice(&0x55667788u32.to_le_bytes());
        packet[252..256].copy_from_slice(&0xaabbccddu32.to_le_bytes());
        packet[316..320].copy_from_slice(&(-1i32).to_le_bytes());
        let GameplayEvent::Illusion(illusion) = parse_packet(0x312a, &packet).unwrap().unwrap()
        else {
            panic!("illusion")
        };
        assert_eq!(
            illusion,
            Illusion {
                id: 70001,
                race: 522,
                gender: 1,
                texture: 255,
                helm_texture: 9,
                face: 0x12345678,
                hair_style: 11,
                hair_color: 22,
                beard: 33,
                beard_color: 44,
                size: 6.5,
                drakkin_heritage: 0x11223344,
                drakkin_tattoo: 0x55667788,
                drakkin_details: 0xaabbccdd,
            }
        );
        for length in 0..packet.len() {
            assert!(parse_packet(0x312a, &packet[..length]).unwrap().is_err());
        }
        packet.push(0);
        assert!(parse_packet(0x312a, &packet).unwrap().is_err());
        packet.pop();
        for size in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            packet[88..92].copy_from_slice(&size.to_le_bytes());
            assert!(parse_packet(0x312a, &packet).unwrap().is_err());
        }
    }

    #[test]
    fn server_set_face_keeps_both_eye_colors_and_full_drakkin_selections() {
        let mut packet = vec![11, 22, 33, 44, 55, 66, 77, 0xab];
        packet.extend_from_slice(&0x11223344u32.to_le_bytes());
        packet.extend_from_slice(&0x55667788u32.to_le_bytes());
        packet.extend_from_slice(&0xaabbccddu32.to_le_bytes());
        packet.extend_from_slice(&70001u32.to_le_bytes());
        let GameplayEvent::FaceChange(face) = parse_packet(0x1af3, &packet).unwrap().unwrap()
        else {
            panic!("face update")
        };
        assert_eq!(
            face,
            FaceChange {
                id: 70001,
                hair_color: 11,
                beard_color: 22,
                eye_color_1: 33,
                eye_color_2: 44,
                hair_style: 55,
                beard: 66,
                face: 77,
                drakkin_heritage: 0x11223344,
                drakkin_tattoo: 0x55667788,
                drakkin_details: 0xaabbccdd,
            }
        );
        for length in 0..packet.len() {
            assert!(parse_packet(0x1af3, &packet[..length]).unwrap().is_err());
        }
        packet.push(0);
        assert!(parse_packet(0x1af3, &packet).unwrap().is_err());
        assert!(parse_packet(0x5578, &packet[..24]).is_none());
    }

    #[test]
    fn commerce_packets_preserve_actual_slots_prices_and_rejections() {
        let open = encode_command(Command::MerchantOpen {
            merchant_id: 45,
            player_id: 7,
        })
        .unwrap();
        assert_eq!(open.data.len(), 24);
        assert!(matches!(
            parse_packet(open.opcode, &open.data).unwrap().unwrap(),
            GameplayEvent::MerchantOpened {
                merchant_id: 45,
                command: 1,
                rate: 1.,
                tabs: 1
            }
        ));
        let buy = encode_command(Command::MerchantBuy {
            merchant_id: 45,
            player_id: 7,
            slot: 5,
            quantity: 2,
            price: 302,
        })
        .unwrap();
        assert_eq!(buy.data.len(), 32);
        assert!(matches!(
            parse_packet(buy.opcode, &buy.data).unwrap().unwrap(),
            GameplayEvent::MerchantBought {
                merchant_id: 45,
                player_id: 7,
                slot: 5,
                quantity: 2,
                price: 302
            }
        ));
        let slot = InventorySlot::possessions(25).in_bag(4);
        let mut sell = encode_command(Command::MerchantSell {
            merchant_id: 45,
            slot,
            quantity: 2,
        })
        .unwrap();
        sell.data[16..20].copy_from_slice(&294u32.to_le_bytes());
        let GameplayEvent::MerchantSold {
            slot: actual,
            quantity,
            price,
            rejected,
            ..
        } = parse_packet(sell.opcode, &sell.data).unwrap().unwrap()
        else {
            panic!("sale")
        };
        assert_eq!(actual, slot);
        assert_eq!((quantity, price, rejected), (2, 294, false));
        sell.data[4..6].copy_from_slice(&u16::MAX.to_le_bytes());
        sell.data[12..20].fill(0);
        assert!(matches!(
            parse_packet(sell.opcode, &sell.data).unwrap().unwrap(),
            GameplayEvent::MerchantSold { rejected: true, .. }
        ));
        for p in [open, buy, sell] {
            for n in 0..p.data.len() {
                assert!(parse_packet(p.opcode, &p.data[..n]).unwrap().is_err());
            }
        }
        let mut invalid_rate = vec![0; 24];
        invalid_rate[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(parse_packet(0x4fed, &invalid_rate).unwrap().is_err());
        assert!(
            encode_command(Command::MerchantSell {
                merchant_id: 1,
                slot: InventorySlot::bank(0),
                quantity: 1
            })
            .is_err()
        );
        assert!(
            encode_command(Command::MerchantBuy {
                merchant_id: 1,
                player_id: 7,
                slot: 1,
                quantity: 0,
                price: 0
            })
            .is_err()
        );
        assert_eq!(
            encode_command(Command::MerchantClose).unwrap().opcode,
            0x30a8
        );
        assert!(matches!(
            parse_packet(0x3196, &[]).unwrap().unwrap(),
            GameplayEvent::MerchantClosed
        ));
        assert!(parse_packet(0x3196, &[0]).unwrap().is_err());
    }
    #[test]
    fn bank_coins_only_encode_supported_locations_and_denominations() {
        let p = encode_command(Command::MoveCoin {
            from: CoinLocation::Carried,
            to: CoinLocation::Bank,
            coin: CoinType::Gold,
            amount: 12,
        })
        .unwrap();
        assert_eq!(p.opcode, 0x0bcf);
        assert_eq!(
            p.data,
            [1u32, 2, 2, 2, 12]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert!(
            encode_command(Command::MoveCoin {
                from: CoinLocation::Carried,
                to: CoinLocation::SharedBank,
                coin: CoinType::Copper,
                amount: 1
            })
            .is_err()
        );
        assert!(
            encode_command(Command::MoveCoin {
                from: CoinLocation::Carried,
                to: CoinLocation::Bank,
                coin: CoinType::Gold,
                amount: u32::MAX
            })
            .is_err()
        );
        let p = encode_command(Command::BankerChange).unwrap();
        assert_eq!(p.data.len(), 4);
        assert_eq!(p.opcode, 0x791e);
        let values = [9u32, 8, 7, 6, 5, 4, 3, 2];
        let bytes = values
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let GameplayEvent::BankerBalances { carried, bank } =
            parse_packet(0x791e, &bytes).unwrap().unwrap()
        else {
            panic!("bank")
        };
        assert_eq!(
            (carried.platinum, carried.copper, bank.platinum, bank.copper),
            (9, 6, 5, 2)
        );
        for n in 0..bytes.len() {
            assert!(parse_packet(0x791e, &bytes[..n]).unwrap().is_err());
        }
        for n in 0..12 {
            assert!(parse_packet(0x724f, &[0; 12][..n]).unwrap().is_err());
        }
    }
    #[test]
    fn consumption_opcodes_preserve_unit_charge_and_whole_item_meanings() {
        let from = InventorySlot::possessions(23);
        let p = encode_command(Command::DeleteItem {
            slot: from,
            count: u32::MAX,
        })
        .unwrap();
        assert!(matches!(
            parse_packet(0x18ad, &p.data).unwrap().unwrap(),
            GameplayEvent::ItemDeleted { count: 1, .. }
        ));
        assert!(matches!(
            parse_packet(0x01b8, &p.data).unwrap().unwrap(),
            GameplayEvent::ItemChargeUsed { count: 1, .. }
        ));
        assert!(matches!(
            parse_packet(0x32ee, &p.data).unwrap().unwrap(),
            GameplayEvent::ItemMoved {
                to: InventorySlot::DELETE,
                count: u32::MAX,
                ..
            }
        ));
        for opcode in [0x18ad, 0x01b8, 0x32ee] {
            for n in 0..p.data.len() {
                assert!(parse_packet(opcode, &p.data[..n]).unwrap().is_err());
            }
        }
    }
    #[test]
    fn loot_acknowledgment_preserves_quest_rejection() {
        let mut p = encode_command(Command::LootItem {
            corpse_id: 5,
            player_id: 7,
            slot: 23,
            auto_loot: true,
        })
        .unwrap();
        assert!(matches!(
            parse_packet(p.opcode, &p.data).unwrap().unwrap(),
            GameplayEvent::LootItemAcknowledged {
                rejected: false,
                ..
            }
        ));
        p.data[12..16].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(matches!(
            parse_packet(p.opcode, &p.data).unwrap().unwrap(),
            GameplayEvent::LootItemAcknowledged {
                corpse_id: 5,
                player_id: 7,
                slot: 23,
                rejected: true
            }
        ));
    }
    #[test]
    fn spells_use_rof2_layouts_and_preserve_server_feedback() {
        let cast = encode_command(Command::CastSpell {
            slot: 3,
            spell_id: 45000,
            target_id: 0x1234,
        })
        .unwrap();
        assert_eq!(cast.opcode, 0x1287);
        assert_eq!(cast.data.len(), 44);
        assert_eq!(u32_at(&cast.data, 20), Some(0x1234));
        assert!(
            encode_command(Command::CastSpell {
                slot: 12,
                spell_id: 54,
                target_id: 1
            })
            .is_err()
        );
        assert!(
            encode_command(Command::MemorizeSpell {
                slot: 0,
                spell_id: 45001
            })
            .is_err()
        );
        let mem = encode_command(Command::UnmemorizeSpell {
            slot: 1,
            spell_id: 288,
        })
        .unwrap();
        assert!(matches!(
            parse_packet(mem.opcode, &mem.data).unwrap().unwrap(),
            GameplayEvent::SpellMemorized {
                slot: 1,
                spell_id: 288,
                action: 2,
                ..
            }
        ));
        let interrupt = encode_command(Command::InterruptSpell).unwrap();
        assert_eq!(interrupt.opcode, 0x5467);
        assert!(interrupt.data.is_empty());
        let mut begin = 288u32.to_le_bytes().to_vec();
        begin.extend(42u16.to_le_bytes());
        begin.extend(2500u32.to_le_bytes());
        assert!(matches!(
            parse_packet(0x318f, &begin).unwrap().unwrap(),
            GameplayEvent::BeginCast {
                caster_id: 42,
                spell_id: 288,
                cast_time_ms: 2500
            }
        ));
        let mut action = vec![0; 56];
        action[..2].copy_from_slice(&42u16.to_le_bytes());
        action[2..4].copy_from_slice(&9u16.to_le_bytes());
        action[33..37].copy_from_slice(&288u32.to_le_bytes());
        action[38] = 4;
        assert!(matches!(
            parse_packet(0x744c, &action).unwrap().unwrap(),
            GameplayEvent::SpellAction {
                source_id: 9,
                target_id: 42,
                spell_id: 288,
                effect_flag: 4,
                ..
            }
        ));
        assert!(parse_packet(0x048c, &[0; 8]).unwrap().is_ok());
        assert!(parse_packet(0x048c, &[0; 7]).unwrap().is_err());
    }
    #[test]
    fn rof2_explicit_particle_and_nimbus_packets_keep_effect_ids_and_timing() {
        // common/eq_packet_structs.h SpellEffect_Struct and
        // zone/mob.cpp SendSpellEffect; RoF2 has no translating encoder.
        let mut bytes = Vec::new();
        for value in [278u32, 0x10002, 0x30004, 2250, 375, 3000] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([1, 1, 0, 0]);
        assert_eq!(bytes.len(), 28);
        let GameplayEvent::SpellEffect(effect) = parse_packet(0x5936, &bytes).unwrap().unwrap()
        else {
            panic!("spell effect");
        };
        assert_eq!(
            effect,
            SpellEffect {
                effect_id: 278,
                source_id: 0x10002,
                target_id: 0x30004,
                duration_ms: 2250,
                finish_delay_ms: 375,
                unknown_020: 3000,
            }
        );
        for n in 0..bytes.len() {
            assert!(parse_packet(0x5936, &bytes[..n]).unwrap().is_err());
        }
        bytes.push(0);
        assert!(parse_packet(0x5936, &bytes).unwrap().is_err());

        // AddNimbusEffect and RemoveNimbusEffect share an 8-byte payload.
        let bytes = [0x10002u32.to_le_bytes(), 278u32.to_le_bytes()].concat();
        for (opcode, removed) in [(0xc693, false), (0x7b1e, true)] {
            let GameplayEvent::NimbusEffect(effect) =
                parse_packet(opcode, &bytes).unwrap().unwrap()
            else {
                panic!("nimbus effect");
            };
            assert_eq!(
                effect,
                NimbusEffect {
                    id: 0x10002,
                    effect_id: 278,
                    removed
                }
            );
            for n in 0..bytes.len() {
                assert!(parse_packet(opcode, &bytes[..n]).unwrap().is_err());
            }
            let mut extra = bytes.clone();
            extra.push(0);
            assert!(parse_packet(opcode, &extra).unwrap().is_err());
        }
    }

    #[test]
    fn rof2_item_projectile_uses_translated_offsets_and_server_xyz() {
        // rof2_structs.h Arrow_Struct is 116 bytes, not the common packet's
        // layout. ENCODE(OP_SomeItemPacketMaybe) also inserts byte070=175.
        let mut bytes = vec![0; 116];
        for (offset, value) in [
            (0, -17.25f32),
            (4, 90.5),
            (8, 42.125),
            (24, 4.),
            (28, 192.),
            (32, 125.),
            (44, 50.),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (offset, value) in [(48, 0x10002u32), (52, 0x30004), (56, 8005)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[70] = 175;
        bytes[73] = 7;
        bytes[74] = 27;
        bytes[89..95].copy_from_slice(b"IT600\0");
        let GameplayEvent::Projectile(projectile) = parse_packet(0x747c, &bytes).unwrap().unwrap()
        else {
            panic!("projectile");
        };
        assert_eq!(
            projectile,
            Projectile {
                source_id: 0x10002,
                target_id: 0x30004,
                position: [90.5, -17.25, 42.125],
                velocity: 4.,
                launch_angle: 192.,
                tilt: 125.,
                arc: 50.,
                item_id: 8005,
                skill: 7,
                item_type: 27,
                model_name: "IT600".into(),
            }
        );
        for n in 0..bytes.len() {
            assert!(parse_packet(0x747c, &bytes[..n]).unwrap().is_err());
        }
        for offset in [0, 4, 8, 24, 28, 32, 44] {
            let mut invalid = bytes.clone();
            invalid[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
            assert!(parse_packet(0x747c, &invalid).unwrap().is_err());
        }
        // strncpy can fill the whole fixed-width model name without a NUL.
        bytes[89..].fill(b'X');
        let GameplayEvent::Projectile(projectile) = parse_packet(0x747c, &bytes).unwrap().unwrap()
        else {
            panic!("projectile");
        };
        assert_eq!(projectile.model_name, "X".repeat(27));
        bytes.push(0);
        assert!(parse_packet(0x747c, &bytes).unwrap().is_err());
    }

    #[test]
    fn rof2_spell_action_preserves_initial_success_and_nonspell_types() {
        // ENCODE(OP_Action) writes ActionAlt_Struct (56 bytes). An initial
        // flag0 reaches observers; flag4 success may follow for caster/target.
        let mut bytes = vec![0; 56];
        bytes[0..2].copy_from_slice(&42u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&9u16.to_le_bytes());
        bytes[4..6].copy_from_slice(&65u16.to_le_bytes());
        bytes[10..14].copy_from_slice(&1.5f32.to_le_bytes());
        bytes[33..37].copy_from_slice(&288u32.to_le_bytes());
        bytes[37] = 63;
        for action_type in [231, 1] {
            bytes[26] = action_type;
            for effect_flag in [0, 4] {
                bytes[38] = effect_flag;
                let GameplayEvent::SpellAction {
                    source_id,
                    target_id,
                    spell_id,
                    level,
                    action_type: parsed_type,
                    spell_level,
                    instrument_modifier,
                    effect_flag: parsed_flag,
                } = parse_packet(0x744c, &bytes).unwrap().unwrap()
                else {
                    panic!("action");
                };
                assert_eq!((source_id, target_id, spell_id, level), (9, 42, 288, 65));
                assert_eq!(
                    (parsed_type, parsed_flag, spell_level),
                    (action_type, effect_flag, 63)
                );
                assert_eq!(instrument_modifier, 1.5);
            }
        }
        bytes[10..14].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert!(parse_packet(0x744c, &bytes).unwrap().is_err());
    }

    #[test]
    fn buff_lists_are_bounded_and_require_complete_strings() {
        let mut data = 42u32.to_le_bytes().to_vec();
        data.extend(6000u32.to_le_bytes());
        data.push(1);
        data.extend(1u16.to_le_bytes());
        for value in [2u32, 288, 270, 0] {
            data.extend(value.to_le_bytes());
        }
        data.extend(b"Arcanist\0");
        data.push(0);
        for opcode in [0x3377, 0x4f4b] {
            let GameplayEvent::Buffs { id, all, buffs, .. } =
                parse_packet(opcode, &data).unwrap().unwrap()
            else {
                panic!("buffs")
            };
            assert_eq!(id, 42);
            assert!(all);
            assert_eq!(buffs.len(), 1);
            assert_eq!(buffs[0].spell_id, 288);
            assert_eq!(buffs[0].caster, "Arcanist");
            for n in 0..data.len() {
                assert!(parse_packet(opcode, &data[..n]).unwrap().is_err());
            }
        }
        data[9..11].copy_from_slice(&129u16.to_le_bytes());
        assert!(parse_packet(0x3377, &data).unwrap().is_err());
    }
    #[test]
    fn chat_is_variable_length_and_channels_keep_their_wire_values() {
        for channel in [
            ChatChannel::Say,
            ChatChannel::Tell,
            ChatChannel::Group,
            ChatChannel::Guild,
            ChatChannel::Raid,
            ChatChannel::Ooc,
            ChatChannel::Shout,
        ] {
            let packet = encode_command(Command::Chat {
                channel,
                target: "Mechanic".into(),
                text: "Hello from OpenEQ".into(),
                language: 0,
            })
            .unwrap();
            let GameplayEvent::Chat(chat) =
                parse_packet(packet.opcode, &packet.data).unwrap().unwrap()
            else {
                panic!("chat")
            };
            assert_eq!(chat.channel, channel as u32);
            assert_eq!(chat.target, "Mechanic");
            assert_eq!(chat.text, "Hello from OpenEQ");
            for n in 0..packet.data.len() {
                assert!(
                    parse_packet(packet.opcode, &packet.data[..n])
                        .unwrap()
                        .is_err()
                );
            }
        }
        assert!(encode_command(Command::Emote("invalid\0text".into())).is_err());
        assert_eq!(
            encode_command(Command::Emote("waves".into()))
                .unwrap()
                .opcode,
            0x373b
        );
    }
    #[test]
    fn formatted_messages_keep_arguments_instead_of_guessing_strings() {
        let mut data = vec![0; 4];
        data.extend(123u32.to_le_bytes());
        data.extend(15u32.to_le_bytes());
        data.extend(b"Mechanic\0a rat\0\0");
        let GameplayEvent::Message(message) = parse_packet(0x1024, &data).unwrap().unwrap() else {
            panic!("message")
        };
        assert_eq!(message.string_id, Some(123));
        assert_eq!(message.arguments, ["Mechanic", "a rat"]);
        assert_eq!(message.color, 15);
        for n in 0..data.len() {
            assert!(parse_packet(0x1024, &data[..n]).unwrap().is_err());
        }
    }
    #[test]
    fn command_layouts_match_rof2_including_bags_loot_and_posture() {
        let from = InventorySlot::possessions(23).in_bag(5);
        let to = InventorySlot::CURSOR;
        let p = encode_command(Command::MoveItem { from, to, count: 3 }).unwrap();
        assert_eq!(p.data.len(), 28);
        let GameplayEvent::ItemMoved {
            from: a,
            to: b,
            count,
        } = parse_packet(p.opcode, &p.data).unwrap().unwrap()
        else {
            panic!("move")
        };
        assert_eq!((a, b, count), (from, to, 3));
        let p = encode_command(Command::LootItem {
            corpse_id: 55,
            player_id: 12,
            slot: 23,
            auto_loot: true,
        })
        .unwrap();
        assert_eq!(p.data.len(), 20);
        assert_eq!(u32_at(&p.data, 12), Some(1));
        let p = encode_command(Command::Posture {
            player_id: 12,
            posture: 1,
        })
        .unwrap();
        assert_eq!(p.data, [12, 0, 14, 0, 110, 0, 0, 0]);
        assert_eq!(
            encode_command(Command::AutoAttack(true)).unwrap().data,
            [1, 0, 0, 0]
        );
    }
    #[test]
    fn signed_damage_and_authoritative_resources_are_preserved() {
        let mut d = vec![0; 30];
        d[0..2].copy_from_slice(&8u16.to_le_bytes());
        d[2..4].copy_from_slice(&12u16.to_le_bytes());
        d[9..13].copy_from_slice(&(-3i32).to_le_bytes());
        let GameplayEvent::Damage(hit) = parse_packet(0x6f15, &d).unwrap().unwrap() else {
            panic!("damage")
        };
        assert_eq!((hit.source_id, hit.target_id, hit.amount), (12, 8, -3));
        for n in 0..d.len() {
            assert!(parse_packet(0x6f15, &d[..n]).unwrap().is_err());
        }
        let mut hp = 8u16.to_le_bytes().to_vec();
        hp.extend(10i32.to_le_bytes());
        hp.extend(100i32.to_le_bytes());
        assert!(matches!(
            parse_packet(0x2828, &hp).unwrap().unwrap(),
            GameplayEvent::Health {
                id: 8,
                current: 10,
                maximum: 100
            }
        ));
    }
    #[test]
    fn fixed_packet_truncations_never_turn_into_events() {
        for (op, size) in [
            (0x217c, 16),
            (0x318f, 10),
            (0x744c, 56),
            (0x659c, 100),
            (0x213f, 12),
            (0x2828, 10),
            (0x3791, 10),
            (0x5f42, 10),
            (0x5467, 20),
            (0x640c, 16),
            (0x2a79, 8),
            (0x4478, 4),
            (0x742b, 20),
            (0x6517, 32),
            (0x7177, 4),
            (0x7994, 27),
            (0x0971, 8),
            (0x5f44, 20),
            (0x4dc9, 20),
        ] {
            let data = vec![0; size];
            assert!(parse_packet(op, &data).unwrap().is_ok(), "valid{op:#x}");
            for n in 0..size {
                assert!(
                    parse_packet(op, &data[..n]).unwrap().is_err(),
                    "truncated{op:#x}@{n}"
                );
            }
        }
    }
    #[test]
    fn zone_points_preserve_destination_numbers_coordinates_and_sentinels() {
        // EQEmu sends these destination fields from zone_points.target_*.
        // The final 32-byte record is outside count and must be ignored.
        let mut data = vec![0; 4 + 3 * 32];
        data[..4].copy_from_slice(&2u32.to_le_bytes());
        data[4..8].copy_from_slice(&177u32.to_le_bytes());
        for (offset, value) in [(8, 838f32), (12, 882.), (16, -157.), (20, 2.)] {
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        data[24..26].copy_from_slice(&202u16.to_le_bytes());
        data[26..28].copy_from_slice(&7u16.to_le_bytes());
        data[36..40].copy_from_slice(&3u32.to_le_bytes());
        for (offset, value) in [(40, 999999f32), (44, -3082.), (48, 3.13), (52, 999.)] {
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        data[56..58].copy_from_slice(&68u16.to_le_bytes());
        data[68..].fill(0xff); // not a record, even if its floats are NaNs
        let GameplayEvent::ZonePoints(points) = parse_packet(0x69a4, &data).unwrap().unwrap()
        else {
            panic!("zone points")
        };
        assert_eq!(
            points,
            vec![
                ZonePoint {
                    number: 177,
                    zone_id: 202,
                    instance_id: 7,
                    position: [882., 838., -157.],
                    heading: 2.,
                },
                ZonePoint {
                    number: 3,
                    zone_id: 68,
                    instance_id: 0,
                    position: [-3082., 999999., 3.13],
                    heading: 999.,
                },
            ]
        );
        for length in 0..data.len() {
            assert!(parse_packet(0x69a4, &data[..length]).unwrap().is_err());
        }
        data.push(0);
        assert!(parse_packet(0x69a4, &data).unwrap().is_err());
    }

    #[test]
    fn zone_points_reject_invalid_count_and_nonfinite_destinations() {
        assert!(matches!(
            parse_packet(0x69a4, &[0; 36]).unwrap().unwrap(),
            GameplayEvent::ZonePoints(points) if points.is_empty()
        ));
        let mut data = vec![0; 68];
        for count in [0u32, 2, u32::MAX] {
            data[..4].copy_from_slice(&count.to_le_bytes());
            assert!(parse_packet(0x69a4, &data).unwrap().is_err());
        }
        data[..4].copy_from_slice(&1u32.to_le_bytes());
        for offset in [8, 12, 16, 20] {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                assert!(parse_packet(0x69a4, &data).unwrap().is_err());
            }
            data[offset..offset + 4].fill(0);
        }
        assert!(parse_packet(0x69a4, &data).unwrap().is_ok());
    }

    #[test]
    fn door_records_and_zone_changes_use_rof2_offsets() {
        let mut door = vec![0; 100];
        door[..10].copy_from_slice(b"POKDOOR500");
        for (offset, value) in [(32, -852f32), (36, 242.), (40, -160.), (44, 128.)] {
            door[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        door[52..56].copy_from_slice(&100u32.to_le_bytes());
        door[60] = 40;
        door[61] = 5;
        let GameplayEvent::Doors(doors) = parse_packet(0x7291, &door).unwrap().unwrap() else {
            panic!("doors")
        };
        assert_eq!(doors[0].position, [242., -852., -160.]);
        assert_eq!(
            (doors[0].id, doors[0].open_type, doors[0].size),
            (40, 5, 100)
        );
        for n in 1..100 {
            assert!(parse_packet(0x7291, &door[..n]).unwrap().is_err());
        }
        let click = encode_command(Command::ClickDoor {
            door_id: 40,
            player_id: 555,
        })
        .unwrap();
        assert_eq!(click.data.len(), 16);
        assert_eq!(&click.data[12..14], &555u16.to_le_bytes());
        let packet = encode_command(Command::ZoneChange {
            character: "Mechanic".into(),
            zone_id: 202,
            instance_id: 7,
            position: [1., 2., 3.],
            reason: 0,
        })
        .unwrap();
        assert_eq!(packet.data.len(), 100);
        assert_eq!(&packet.data[64..68], &[202, 0, 7, 0]);
        assert_eq!(&packet.data[76..80], &2f32.to_le_bytes()); // wire Y first
        assert_eq!(&packet.data[80..84], &1f32.to_le_bytes());
        assert_eq!(&packet.data[84..88], &3f32.to_le_bytes());
        assert_eq!(&packet.data[88..100], &[0; 12]); // natural reason, success, unknown
        assert!(matches!(
            parse_packet(packet.opcode, &packet.data).unwrap().unwrap(),
            GameplayEvent::ZoneChangeResult {
                zone_id: 202,
                instance_id: 7,
                position: [1., 2., 3.],
                success: 0
            }
        ));
        for n in 0..100 {
            assert!(
                parse_packet(packet.opcode, &packet.data[..n])
                    .unwrap()
                    .is_err()
            );
        }
        let request = vec![0; 176];
        assert!(parse_packet(0x3fcf, &request).unwrap().is_ok());
        for n in 0..176 {
            assert!(parse_packet(0x3fcf, &request[..n]).unwrap().is_err());
        }
    }
    #[test]
    fn corpse_removal_preserves_unknown_wear_slot_without_disconnect() {
        let mut data = vec![0; 27];
        data[26] = 255;
        let GameplayEvent::WearChange(change) = parse_packet(0x7994, &data).unwrap().unwrap()
        else {
            panic!("wear")
        };
        assert_eq!(change.slot, 255);
    }
}
