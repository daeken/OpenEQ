//! RoF2 character creation wire data, audited against EQEmu 4aceae18b94ff.
//! ApproveName reserves a durable row. It is not a read-only availability check.
use crate::{
    AppPacket,
    wire::{Reader, u32_at},
};
use std::collections::{BTreeMap, BTreeSet};

pub const OP_CATALOG: u16 = 0x6773;
pub const OP_APPROVE_NAME: u16 = 0x56a2;
pub const OP_CREATE: u16 = 0x6bbf;
pub const OP_EXPANSIONS: u16 = 0x590d;
pub const OP_MAX_CHARACTERS: u16 = 0x5475;
pub const OP_MEMBERSHIP: u16 = 0x7acc;
pub const ROF2_CHARACTER_LIMIT: u32 = 12;
const MAX_ALLOCATIONS: usize = 4096;
const MAX_COMBINATIONS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationError {
    #[error("invalid character creation data: {0}")]
    Invalid(&'static str),
    #[error("missing character creation capability: {0}")]
    Missing(&'static str),
    #[error("character choice is unavailable: {0}")]
    Unavailable(&'static str),
}
type Result<T> = std::result::Result<T, CreationError>;

/// Names avoid copying the catalog's order into the different create layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub strength: u32,
    pub dexterity: u32,
    pub agility: u32,
    pub stamina: u32,
    pub intelligence: u32,
    pub wisdom: u32,
    pub charisma: u32,
}
impl Stats {
    pub fn from_catalog_order(values: [u32; 7]) -> Self {
        let [
            strength,
            dexterity,
            agility,
            stamina,
            intelligence,
            wisdom,
            charisma,
        ] = values;
        Self {
            strength,
            dexterity,
            agility,
            stamina,
            intelligence,
            wisdom,
            charisma,
        }
    }
    pub fn catalog_order(self) -> [u32; 7] {
        [
            self.strength,
            self.dexterity,
            self.agility,
            self.stamina,
            self.intelligence,
            self.wisdom,
            self.charisma,
        ]
    }
    fn checked_sum(self) -> Result<u32> {
        self.catalog_order()
            .into_iter()
            .try_fold(0u32, |sum, value| sum.checked_add(value))
            .ok_or(CreationError::Invalid("stat sum overflow"))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    pub index: u32,
    pub base: Stats,
    pub default_points: Stats,
}
impl Allocation {
    pub fn point_budget(&self) -> Result<u32> {
        self.default_points.checked_sum()
    }
    pub fn default_stats(&self) -> Result<Stats> {
        let mut values = self.base.catalog_order();
        for (value, extra) in values.iter_mut().zip(self.default_points.catalog_order()) {
            *value = value
                .checked_add(extra)
                .ok_or(CreationError::Invalid("stat addition overflow"))?;
        }
        let stats = Stats::from_catalog_order(values);
        self.validate_stats(stats)?;
        Ok(stats)
    }
    /// The stock server permits unspent points, but never negative allocation.
    pub fn validate_stats(&self, stats: Stats) -> Result<u32> {
        let budget = self.point_budget()?;
        let mut spent = 0u32;
        for (value, base) in stats
            .catalog_order()
            .into_iter()
            .zip(self.base.catalog_order())
        {
            let maximum = base
                .checked_add(budget)
                .ok_or(CreationError::Invalid("stat limit overflow"))?;
            if value < base || value > maximum {
                return Err(CreationError::Unavailable("stat bounds"));
            }
            spent = spent
                .checked_add(value - base)
                .ok_or(CreationError::Invalid("spent points overflow"))?;
        }
        budget
            .checked_sub(spent)
            .ok_or(CreationError::Unavailable("too many stat points"))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Choice {
    pub race: u32,
    pub class: u32,
    pub deity: u32,
    pub start_zone: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combination {
    pub choice: Choice,
    pub expansion_mask: u32,
    pub allocation_index: u32,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    allocations: BTreeMap<u32, Allocation>,
    combinations: Vec<Combination>,
}
impl Catalog {
    pub fn request() -> AppPacket {
        AppPacket::empty(OP_CATALOG)
    }
    pub fn parse(data: &[u8]) -> Result<Self> {
        let bad = || CreationError::Invalid("creation catalog");
        let mut reader = Reader(data);
        if reader.u8() != Some(0) {
            return Err(bad());
        }
        let count = reader.u32().ok_or_else(bad)? as usize;
        if count > MAX_ALLOCATIONS
            || reader.0.len()
                < count
                    .checked_mul(60)
                    .and_then(|n| n.checked_add(4))
                    .ok_or_else(bad)?
        {
            return Err(bad());
        }
        let mut allocations = BTreeMap::new();
        for _ in 0..count {
            let index = reader.u32().ok_or_else(bad)?;
            let mut values = [0u32; 14];
            for value in &mut values {
                *value = reader.u32().ok_or_else(bad)?;
            }
            let allocation = Allocation {
                index,
                base: Stats::from_catalog_order(values[..7].try_into().unwrap()),
                default_points: Stats::from_catalog_order(values[7..].try_into().unwrap()),
            };
            allocation.default_stats()?;
            if allocations.insert(index, allocation).is_some() {
                return Err(CreationError::Invalid("duplicate allocation ID"));
            }
        }
        let count = reader.u32().ok_or_else(bad)? as usize;
        if count > MAX_COMBINATIONS || reader.0.len() != count.checked_mul(24).ok_or_else(bad)? {
            return Err(bad());
        }
        let mut combinations = Vec::with_capacity(count);
        let mut choices = BTreeSet::new();
        for _ in 0..count {
            let expansion_mask = reader.u32().ok_or_else(bad)?;
            let race = reader.u32().ok_or_else(bad)?;
            let class = reader.u32().ok_or_else(bad)?;
            let deity = reader.u32().ok_or_else(bad)?;
            let allocation_index = reader.u32().ok_or_else(bad)?;
            let start_zone = reader.u32().ok_or_else(bad)?;
            let choice = Choice {
                race,
                class,
                deity,
                start_zone,
            };
            if !allocations.contains_key(&allocation_index) {
                return Err(CreationError::Invalid("missing referenced allocation"));
            }
            if !choices.insert(choice) {
                return Err(CreationError::Invalid("ambiguous creation combination"));
            }
            combinations.push(Combination {
                choice,
                expansion_mask,
                allocation_index,
            });
        }
        Ok(Self {
            allocations,
            combinations,
        })
    }
    pub fn combinations(&self) -> &[Combination] {
        &self.combinations
    }
    pub fn allocation(&self, index: u32) -> Option<&Allocation> {
        self.allocations.get(&index)
    }
    pub fn resolve(&self, choice: Choice) -> Result<(&Combination, &Allocation)> {
        let combination = self
            .combinations
            .iter()
            .find(|combination| combination.choice == choice)
            .ok_or(CreationError::Unavailable(
                "race/class/deity/start zone combination",
            ))?;
        Ok((
            combination,
            &self.allocations[&combination.allocation_index],
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub tier: u32,
    pub race_mask: u32,
    pub class_mask: u32,
    /// Unknown membership settings are retained, never interpreted as capacity.
    pub settings: [i32; 25],
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub expansion_mask: Option<u32>,
    pub maximum_characters: Option<u32>,
    pub membership: Option<Membership>,
}
impl Capabilities {
    pub fn observe(&mut self, opcode: u16, data: &[u8]) -> Option<Result<()>> {
        let result = match opcode {
            OP_EXPANSIONS => decode_expansions(data).map(|value| self.expansion_mask = Some(value)),
            OP_MAX_CHARACTERS => {
                decode_maximum(data).map(|value| self.maximum_characters = Some(value))
            }
            OP_MEMBERSHIP => decode_membership(data).map(|value| self.membership = Some(value)),
            _ => return None,
        };
        // A malformed replacement cannot leave an older permissive value live.
        if result.is_err() {
            match opcode {
                OP_EXPANSIONS => self.expansion_mask = None,
                OP_MAX_CHARACTERS => self.maximum_characters = None,
                OP_MEMBERSHIP => self.membership = None,
                _ => {}
            }
        }
        Some(result)
    }
    pub fn permits(&self, combination: &Combination, roster_count: usize) -> Result<()> {
        let expansions = self
            .expansion_mask
            .ok_or(CreationError::Missing("expansions"))?;
        let maximum = self
            .maximum_characters
            .ok_or(CreationError::Missing("character capacity"))?;
        let membership = self
            .membership
            .as_ref()
            .ok_or(CreationError::Missing("membership"))?;
        if roster_count >= maximum.min(ROF2_CHARACTER_LIMIT) as usize {
            return Err(CreationError::Unavailable("character slots full"));
        }
        if combination.expansion_mask & expansions != combination.expansion_mask {
            return Err(CreationError::Unavailable("required expansion"));
        }
        let race = player_race_bit(combination.choice.race)
            .ok_or(CreationError::Unavailable("unsupported player race"))?;
        let class = class_bit(combination.choice.class)
            .ok_or(CreationError::Unavailable("unsupported player class"))?;
        if membership.race_mask & race == 0 || membership.class_mask & class == 0 {
            return Err(CreationError::Unavailable("membership race/class"));
        }
        Ok(())
    }
}
pub fn decode_expansions(data: &[u8]) -> Result<u32> {
    if data.len() != 68 {
        return Err(CreationError::Invalid("RoF2 expansion record"));
    }
    Ok(u32_at(data, 64).unwrap())
}
pub fn decode_maximum(data: &[u8]) -> Result<u32> {
    if data.len() != 12 {
        return Err(CreationError::Invalid("maximum characters record"));
    }
    Ok(u32_at(data, 0).unwrap())
}
pub fn decode_membership(data: &[u8]) -> Result<Membership> {
    if data.len() != 116 || u32_at(data, 12) != Some(25) {
        return Err(CreationError::Invalid("RoF2 membership record"));
    }
    Ok(Membership {
        tier: u32_at(data, 0).unwrap(),
        race_mask: u32_at(data, 4).unwrap(),
        class_mask: u32_at(data, 8).unwrap(),
        settings: std::array::from_fn(|index| u32_at(data, 16 + index * 4).unwrap() as i32),
    })
}
/// EQEmu GetPlayerRaceBit, including its noncontiguous player race IDs.
pub fn player_race_bit(race: u32) -> Option<u32> {
    let index = match race {
        1..=12 => race - 1,
        128 => 12,
        130 => 13,
        330 => 14,
        522 => 15,
        _ => return None,
    };
    Some(1 << index)
}
pub fn class_bit(class: u32) -> Option<u32> {
    (1..=16).contains(&class).then(|| 1 << (class - 1))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Appearance {
    pub hair_color: u8,
    pub beard: u8,
    pub beard_color: u8,
    pub hair_style: u8,
    pub face: u8,
    pub eye_color_1: u8,
    pub eye_color_2: u8,
    pub heritage: u32,
    pub tattoo: u32,
    pub details: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creation {
    pub choice: Choice,
    pub gender: u8,
    pub appearance: Appearance,
    pub stats: Stats,
}
impl Creation {
    /// Encoding enforces structural bounds. Catalog/capability/preview validation
    /// must precede name reservation in the account transaction layer.
    pub fn packet(&self) -> Result<AppPacket> {
        if self.gender > 1
            || player_race_bit(self.choice.race).is_none()
            || class_bit(self.choice.class).is_none()
        {
            return Err(CreationError::Invalid("player race/class/gender"));
        }
        let a = self.appearance;
        if [
            a.hair_color,
            a.beard,
            a.beard_color,
            a.hair_style,
            a.face,
            a.eye_color_1,
            a.eye_color_2,
        ]
        .contains(&255)
            || [a.heritage, a.tattoo, a.details].contains(&u32::MAX)
        {
            return Err(CreationError::Invalid("appearance sentinel"));
        }
        let s = self.stats;
        let words = [
            u32::from(self.gender),
            self.choice.race,
            self.choice.class,
            self.choice.deity,
            self.choice.start_zone,
            u32::from(a.hair_color),
            u32::from(a.beard),
            u32::from(a.beard_color),
            u32::from(a.hair_style),
            u32::from(a.face),
            u32::from(a.eye_color_1),
            u32::from(a.eye_color_2),
            a.heritage,
            a.tattoo,
            a.details,
            s.strength,
            s.stamina,
            s.agility,
            s.dexterity,
            s.wisdom,
            s.intelligence,
            s.charisma,
            0,
            0,
        ]; // Tutorial unavailable in this slice; reserved word must be zero.
        Ok(AppPacket::new(
            OP_CREATE,
            words.into_iter().flat_map(u32::to_le_bytes).collect(),
        ))
    }
}
pub fn validate_name(name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if !(4..=15).contains(&bytes.len())
        || !bytes[0].is_ascii_uppercase()
        || !bytes[1..].iter().all(u8::is_ascii_lowercase)
        || bytes
            .windows(3)
            .any(|part| part[0] == part[1] && part[1] == part[2])
    {
        return Err(CreationError::Invalid(
            "name must use 4–15 letters, initial capital, no triple repeated letter",
        ));
    }
    Ok(())
}
pub fn approve_name(name: &str, choice: Choice) -> Result<AppPacket> {
    validate_name(name)?;
    if player_race_bit(choice.race).is_none() || class_bit(choice.class).is_none() {
        return Err(CreationError::Invalid("name approval race/class"));
    }
    let mut data = vec![0; 72];
    data[..name.len()].copy_from_slice(name.as_bytes());
    data[64..68].copy_from_slice(&choice.race.to_le_bytes());
    data[68..72].copy_from_slice(&choice.class.to_le_bytes());
    Ok(AppPacket::new(OP_APPROVE_NAME, data))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    Rejected,
    Approved,
    Unknown(u8),
}
pub fn decode_approval(data: &[u8]) -> Result<Approval> {
    match data {
        [0] => Ok(Approval::Rejected),
        [1] => Ok(Approval::Approved),
        [other] => Ok(Approval::Unknown(*other)),
        _ => Err(CreationError::Invalid("name approval reply")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog_packet() -> Vec<u8> {
        let mut data = vec![0];
        data.extend_from_slice(&2u32.to_le_bytes());
        for id in [17u32, 91] {
            for word in [id, 71, 72, 73, 74, 75, 76, 77, 1, 2, 3, 4, 5, 6, 7] {
                data.extend_from_slice(&word.to_le_bytes());
            }
        }
        data.extend_from_slice(&2u32.to_le_bytes());
        for words in [[0u32, 1, 2, 201, 17, 77], [0x800, 522, 1, 216, 91, 394]] {
            for word in words {
                data.extend_from_slice(&word.to_le_bytes());
            }
        }
        data
    }
    fn membership_packet() -> Vec<u8> {
        [2u32, 0xffff, 0xffff, 25]
            .into_iter()
            .chain((0..25).map(|i| (i - 10i32) as u32))
            .flat_map(u32::to_le_bytes)
            .collect()
    }
    fn capabilities() -> Capabilities {
        Capabilities {
            expansion_mask: Some(0x800),
            maximum_characters: Some(12),
            membership: Some(decode_membership(&membership_packet()).unwrap()),
        }
    }
    #[test]
    fn exact_catalog_rejects_every_prefix_trailer_count_and_ambiguous_reference() {
        let bytes = catalog_packet();
        let catalog = Catalog::parse(&bytes).unwrap();
        assert_eq!(catalog.combinations().len(), 2);
        assert_eq!(
            catalog
                .allocation(17)
                .unwrap()
                .default_stats()
                .unwrap()
                .catalog_order(),
            [72, 74, 76, 78, 80, 82, 84]
        );
        for end in 0..bytes.len() {
            assert!(Catalog::parse(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(Catalog::parse(&extra).is_err());
        for offset in [1, 125] {
            let mut bad = bytes.clone();
            bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(Catalog::parse(&bad).is_err());
        }
        let mut duplicate = bytes.clone();
        duplicate[65..69].copy_from_slice(&17u32.to_le_bytes());
        assert!(Catalog::parse(&duplicate).is_err());
        let mut missing = bytes.clone();
        missing[145..149].copy_from_slice(&92u32.to_le_bytes());
        assert!(Catalog::parse(&missing).is_err());
        let mut ambiguous = bytes.clone();
        ambiguous[153..177].copy_from_slice(&bytes[129..153]);
        assert!(Catalog::parse(&ambiguous).is_err());
        let mut overflow = bytes;
        overflow[9..13].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Catalog::parse(&overflow).is_err());
    }
    #[test]
    fn capabilities_require_exact_rof2_shapes_and_clear_bad_replacements() {
        let mut expansion = vec![0; 68];
        expansion[64..].copy_from_slice(&0x804u32.to_le_bytes());
        let maximum: Vec<_> = [12u32, 99, 100]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let membership = membership_packet();
        assert_eq!(decode_membership(&membership).unwrap().settings[0], -10);
        let mut caps = Capabilities::default();
        for (opcode, packet) in [
            (OP_EXPANSIONS, expansion),
            (OP_MAX_CHARACTERS, maximum),
            (OP_MEMBERSHIP, membership),
        ] {
            for end in 0..packet.len() {
                assert!(caps.observe(opcode, &packet[..end]).unwrap().is_err());
            }
            caps.observe(opcode, &packet).unwrap().unwrap();
            let mut extra = packet.clone();
            extra.push(0);
            assert!(caps.observe(opcode, &extra).unwrap().is_err());
            match opcode {
                OP_EXPANSIONS => assert!(caps.expansion_mask.is_none()),
                OP_MAX_CHARACTERS => assert!(caps.maximum_characters.is_none()),
                _ => assert!(caps.membership.is_none()),
            }
            caps.observe(opcode, &packet).unwrap().unwrap();
        }
        assert_eq!(caps.expansion_mask, Some(0x804));
        assert_eq!(caps.maximum_characters, Some(12));
        let mut wrong_count = membership_packet();
        wrong_count[12..16].copy_from_slice(&21u32.to_le_bytes());
        assert!(decode_membership(&wrong_count).is_err());
        assert!(caps.observe(0x1234, &[]).is_none());
    }
    #[test]
    fn advertised_eligibility_does_not_infer_absent_membership_or_capacity() {
        let catalog = Catalog::parse(&catalog_packet()).unwrap();
        let combo = catalog.combinations()[1];
        assert!(capabilities().permits(&combo, 11).is_ok());
        for change in 0..6 {
            let mut caps = capabilities();
            match change {
                0 => caps.expansion_mask = None,
                1 => caps.membership = None,
                2 => caps.maximum_characters = None,
                3 => caps.expansion_mask = Some(0x400),
                4 => caps.membership.as_mut().unwrap().race_mask = 0x7fff,
                _ => caps.membership.as_mut().unwrap().class_mask = 2,
            }
            assert!(caps.permits(&combo, 0).is_err());
        }
        assert!(capabilities().permits(&combo, 12).is_err());
        let mut caps = capabilities();
        caps.maximum_characters = Some(u32::MAX);
        assert!(caps.permits(&combo, 12).is_err());
        let bits: BTreeSet<_> = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 128, 130, 330, 522]
            .into_iter()
            .map(|race| player_race_bit(race).unwrap())
            .collect();
        assert_eq!(bits.len(), 16);
        assert_eq!(bits.iter().copied().sum::<u32>(), 0xffff);
        assert_eq!(player_race_bit(128), Some(0x1000));
        assert_eq!(player_race_bit(522), Some(0x8000));
        assert_eq!(player_race_bit(13), None);
        assert_eq!(class_bit(0), None);
        assert_eq!(class_bit(17), None);
    }
    #[test]
    fn named_stats_allow_unspent_points_and_check_individual_total_and_overflow() {
        let catalog = Catalog::parse(&catalog_packet()).unwrap();
        let allocation = catalog.allocation(17).unwrap();
        assert_eq!(allocation.validate_stats(allocation.base), Ok(28));
        let mut stats = allocation.base;
        stats.strength += 28;
        assert_eq!(allocation.validate_stats(stats), Ok(0));
        stats.dexterity += 1;
        assert!(allocation.validate_stats(stats).is_err());
        stats = allocation.base;
        stats.stamina -= 1;
        assert!(allocation.validate_stats(stats).is_err());
        let overflow = Allocation {
            index: 0,
            base: Stats::default(),
            default_points: Stats::from_catalog_order([u32::MAX; 7]),
        };
        assert!(overflow.point_budget().is_err());
    }
    #[test]
    fn create_packet_preserves_stat_order_cosmetics_and_zero_tutorial_reserved() {
        let creation = Creation {
            choice: Choice {
                race: 522,
                class: 9,
                deity: 216,
                start_zone: 394,
            },
            gender: 1,
            appearance: Appearance {
                hair_color: 1,
                beard: 2,
                beard_color: 3,
                hair_style: 4,
                face: 5,
                eye_color_1: 6,
                eye_color_2: 7,
                heritage: 8,
                tattoo: 9,
                details: 10,
            },
            stats: Stats::from_catalog_order([101, 102, 103, 104, 105, 106, 107]),
        };
        let packet = creation.packet().unwrap();
        assert_eq!(packet.opcode, OP_CREATE);
        assert_eq!(packet.data.len(), 96);
        let words: Vec<_> = (0..24)
            .map(|i| u32_at(&packet.data, i * 4).unwrap())
            .collect();
        assert_eq!(
            words,
            vec![
                1, 522, 9, 216, 394, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 101, 104, 103, 102, 106, 105,
                107, 0, 0
            ]
        );
        let approval = approve_name("Asteria", creation.choice).unwrap();
        assert_eq!(approval.data.len(), 72);
        assert_eq!(&approval.data[..8], b"Asteria\0");
        assert!(approval.data[8..64].iter().all(|v| *v == 0));
        assert_eq!(u32_at(&approval.data, 64), Some(522));
        assert_eq!(u32_at(&approval.data, 68), Some(9));
    }
    #[test]
    fn name_rules_and_approval_unknown_are_not_success() {
        for name in [
            "",
            "Abc",
            "asteria",
            "AsTeria",
            "Asteria1",
            "Ásteria",
            "Abbbcd",
            "Abcdefghijklmnop",
            "Asteria\0",
        ] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        for name in ["Asteria", "Abcd", "Abcdefghijklmno"] {
            validate_name(name).unwrap();
        }
        assert_eq!(decode_approval(&[0]), Ok(Approval::Rejected));
        assert_eq!(decode_approval(&[1]), Ok(Approval::Approved));
        assert_eq!(decode_approval(&[2]), Ok(Approval::Unknown(2)));
        assert!(decode_approval(&[]).is_err());
        assert!(decode_approval(&[1, 0]).is_err());
    }
}
