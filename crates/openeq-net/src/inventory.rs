//! RoF2's binary, recursively serialized item instances and typed inventory slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InventorySlot {
    pub kind: u16,
    pub slot: u16,
    pub bag: Option<u16>,
    pub augment: Option<u16>,
}
impl InventorySlot {
    pub const CURSOR: Self = Self::possessions(33);
    pub const DELETE: Self = Self {
        kind: u16::MAX,
        slot: u16::MAX,
        bag: None,
        augment: None,
    };
    pub const fn possessions(slot: u16) -> Self {
        Self {
            kind: 0,
            slot,
            bag: None,
            augment: None,
        }
    }
    pub fn in_bag(self, index: u16) -> Self {
        Self {
            bag: Some(index),
            ..self
        }
    }
    /// Current EQEmu server addressing (200 entries per bag), not Titanium IDs.
    pub fn server_slot(self) -> Option<u32> {
        if self.augment.is_some() {
            return None;
        }
        let slot = u32::from(self.slot);
        match (self.kind, self.bag) {
            (0, None) if slot < 34 => Some(slot),
            (1, None) if slot < 24 => Some(2000 + slot),
            (2, None) if slot < 2 => Some(2500 + slot),
            (3, None) if slot < 8 => Some(3000 + slot),
            (0, Some(b)) if (23..=33).contains(&slot) && b < 200 => {
                Some(4010 + (slot - 23) * 200 + u32::from(b))
            }
            (1, Some(b)) if slot < 24 && b < 200 => Some(6210 + slot * 200 + u32::from(b)),
            (2, Some(b)) if slot < 2 && b < 200 => Some(11010 + slot * 200 + u32::from(b)),
            (3, Some(b)) if slot < 8 && b < 200 => Some(11410 + slot * 200 + u32::from(b)),
            _ => None,
        }
    }
    pub fn from_server_slot(id: u32) -> Option<Self> {
        let (kind, slot, bag) = match id {
            0..=33 => (0, id, None),
            2000..=2023 => (1, id - 2000, None),
            2500..=2501 => (2, id - 2500, None),
            3000..=3007 => (3, id - 3000, None),
            4010..=6209 => (0, 23 + (id - 4010) / 200, Some(((id - 4010) % 200) as u16)),
            6210..=11009 => (1, (id - 6210) / 200, Some(((id - 6210) % 200) as u16)),
            11010..=11409 => (2, (id - 11010) / 200, Some(((id - 11010) % 200) as u16)),
            11410..=13009 => (3, (id - 11410) / 200, Some(((id - 11410) % 200) as u16)),
            _ => return None,
        };
        Some(Self {
            kind,
            slot: slot as u16,
            bag,
            augment: None,
        })
    }
}
#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub slot: InventorySlot,
    pub id: u32,
    pub instance_id: u32,
    pub name: String,
    pub lore: String,
    pub id_file: String,
    pub icon: u32,
    pub count: u32,
    pub charges: i32,
    pub stack_size: u32,
    pub item_class: u8,
    pub item_type: u8,
    pub equip_slots: u32,
    pub classes: u32,
    pub races: u32,
    pub material: u32,
    pub color: u32,
    pub damage: u32,
    pub delay: u8,
    pub ac: i32,
    pub hp: i32,
    pub mana: i32,
    pub endurance: u32,
    pub required_level: u32,
    pub bag_slots: u8,
    pub bag_size: u8,
    pub weight: i32,
    pub size: u8,
    pub children: Vec<InventoryItem>,
}

use crate::wire::{Reader, u16_at, u32_at};
const MAX_ITEMS: usize = 4096;
const MAX_ITEM_DEPTH: usize = 4;

impl InventorySlot {
    pub(crate) fn read(r: &mut Reader<'_>) -> Option<Self> {
        let kind = r.u16()?;
        r.skip(2)?;
        let slot = r.u16()?;
        let bag = r.u16()?;
        let augment = r.u16()?;
        r.skip(2)?;
        Some(Self {
            kind,
            slot,
            bag: (bag != u16::MAX).then_some(bag),
            augment: (augment != u16::MAX).then_some(augment),
        })
    }
    pub(crate) fn write(self, out: &mut Vec<u8>) {
        for v in [
            self.kind,
            0,
            self.slot,
            self.bag.unwrap_or(u16::MAX),
            self.augment.unwrap_or(u16::MAX),
            0,
        ] {
            out.extend(v.to_le_bytes());
        }
    }
    pub(crate) fn movable(self) -> bool {
        self == Self::DELETE || self.server_slot().is_some()
    }
}

pub fn parse_inventory(data: &[u8]) -> Option<Vec<InventoryItem>> {
    let mut r = Reader(data);
    let count = r.u32()? as usize;
    if count > MAX_ITEMS {
        return None;
    }
    let mut budget = MAX_ITEMS;
    let mut items = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        items.push(read_item(&mut r, 0, &mut budget)?);
    }
    r.done().then_some(items)
}
pub fn parse_item_packet(data: &[u8]) -> Option<(u32, InventoryItem)> {
    let mut r = Reader(data);
    let kind = r.u32()?;
    let mut budget = MAX_ITEMS;
    let item = read_item(&mut r, 0, &mut budget)?;
    // Parcels carry extra shipping metadata; expose the item but do not confuse
    // those trailing bytes with a second serialized instance.
    if kind == 0x73 {
        let _sent = r.u32()?;
        r.array(1, 4096)?;
        r.array(1, 16384)?;
    }
    r.done().then_some((kind, item))
}
fn read_item(r: &mut Reader<'_>, depth: usize, budget: &mut usize) -> Option<InventoryItem> {
    if depth > MAX_ITEM_DEPTH || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let h = r.take(77)?;
    let bag = u16_at(h, 28)?;
    let augment = u16_at(h, 30)?;
    let slot = InventorySlot {
        kind: h[25] as u16,
        slot: u16_at(h, 26)?,
        bag: (bag != u16::MAX).then_some(bag),
        augment: (augment != u16::MAX).then_some(augment),
    };
    if h[76] != 0 {
        r.skip(25)?;
    }
    r.string(256)?;
    r.string(256)?; // primary/secondary ornament model names
    let finish = r.take(26)?;
    let name = r.string(256)?;
    let lore = r.string(1024)?;
    let id_file = r.string(256)?;
    r.string(256)?; // additional model field (empty in EQEmu)
    let b = r.take(255)?;
    r.string(256)?; // charm file
    let secondary = r.take(74)?;
    r.string(256)?; // book filename
    let tertiary = r.take(76)?;
    // Click, proc, worn, focus, scroll, bard: each has a fixed header,
    // variable name, and trailing unknown dword.
    for _ in 0..6 {
        r.skip(30)?;
        r.string(1024)?;
        r.skip(4)?;
    }
    r.skip(171)?; // ItemQuaternaryBodyStruct
    let child_count = r.u32()? as usize;
    if child_count > 200 || child_count > *budget {
        return None;
    }
    let mut children = Vec::with_capacity(child_count.min(16));
    let mut child_indices = std::collections::BTreeSet::new();
    for _ in 0..child_count {
        let index = r.u32()?;
        if index >= 200 || !child_indices.insert(index) {
            return None;
        }
        let mut child = read_item(r, depth + 1, budget)?;
        // SerializeItem currently reuses SubSlotNumber for every bag child;
        // its explicit preceding index is the authoritative child address.
        child.slot = slot.in_bag(index as u16);
        children.push(child);
    }
    Some(InventoryItem {
        slot,
        id: u32_at(b, 0)?,
        instance_id: u32_at(h, 44)?,
        name,
        lore,
        id_file,
        icon: u32_at(b, 20)?,
        count: u32_at(h, 17)?,
        charges: u32_at(h, 56)? as i32,
        stack_size: u32_at(tertiary, 50)?,
        item_class: finish[25],
        item_type: b[158],
        equip_slots: u32_at(b, 12)?,
        classes: u32_at(b, 72)?,
        races: u32_at(b, 76)?,
        material: u32_at(b, 159)?,
        color: u32_at(b, 150)?,
        damage: u32_at(b, 146)?,
        delay: b[142],
        ac: u32_at(b, 56)? as i32,
        hp: u32_at(b, 44)? as i32,
        mana: u32_at(b, 48)? as i32,
        endurance: u32_at(b, 52)?,
        required_level: u32_at(b, 121)?,
        bag_slots: secondary[69],
        bag_size: secondary[70],
        weight: u32_at(b, 4)? as i32,
        size: b[11],
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(out: &mut [u8], offset: usize, value: u32) {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn item(id: u32, slot: u16, children: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut h = vec![0; 77];
        put(&mut h, 17, 20);
        h[26..28].copy_from_slice(&slot.to_le_bytes());
        h[28..32].fill(255);
        put(&mut h, 44, id + 1000);
        put(&mut h, 56, 1);
        let mut out = h;
        out.extend([0; 2]);
        out.extend([0; 26]);
        out.extend(b"Test Item\0Lore\0IT123\0\0");
        let mut b = vec![0; 255];
        put(&mut b, 0, id);
        put(&mut b, 20, 123);
        put(&mut b, 12, 1 << 13);
        put(&mut b, 146, 7);
        b[142] = 20;
        out.extend(b);
        out.push(0);
        let mut secondary = vec![0; 74];
        secondary[69] = 8;
        out.extend(secondary);
        out.push(0);
        let mut tertiary = vec![0; 76];
        put(&mut tertiary, 50, 20);
        out.extend(tertiary);
        out.extend([0; 35 * 6]);
        out.extend([0; 171]);
        out.extend((children.len() as u32).to_le_bytes());
        for (index, child) in children {
            out.extend(index.to_le_bytes());
            out.extend(child);
        }
        out
    }
    #[test]
    fn binary_inventory_uses_explicit_bag_child_indices() {
        let children = [(0, item(13006, 23, &[])), (5, item(13005, 23, &[]))];
        let bag = item(17005, 23, &children);
        let mut data = 1u32.to_le_bytes().to_vec();
        data.extend(bag);
        let parsed = parse_inventory(&data).unwrap();
        assert_eq!(parsed[0].id, 17005);
        assert_eq!(parsed[0].bag_slots, 8);
        assert_eq!(
            parsed[0].children[1].slot,
            InventorySlot::possessions(23).in_bag(5)
        );
        assert_eq!(parsed[0].children[1].count, 20);
        assert_eq!(parsed[0].children[1].slot.server_slot(), Some(4015));
        for n in 0..data.len() {
            assert!(parse_inventory(&data[..n]).is_none(), "truncated at{n}");
        }
    }
    #[test]
    fn item_packet_parses_equipment_and_rejects_trailing_or_truncated_data() {
        let mut data = 0x69u32.to_le_bytes().to_vec();
        data.extend(item(5001, 13, &[]));
        let (kind, parsed) = parse_item_packet(&data).unwrap();
        assert_eq!(kind, 0x69);
        assert_eq!(parsed.slot, InventorySlot::possessions(13));
        assert_eq!(parsed.equip_slots, 1 << 13);
        assert_eq!(parsed.id_file, "IT123");
        assert_eq!(parsed.damage, 7);
        assert_eq!(parsed.delay, 20);
        for n in 0..data.len() {
            assert!(parse_item_packet(&data[..n]).is_none(), "truncated at{n}");
        }
        data.push(0);
        assert!(parse_item_packet(&data).is_none());
    }
    #[test]
    fn recursion_and_counts_are_bounded() {
        assert!(parse_inventory(&u32::MAX.to_le_bytes()).is_none());
        let mut nested = item(1, 23, &[]);
        for _ in 0..6 {
            nested = item(1, 23, &[(0, nested)]);
        }
        let mut data = 1u32.to_le_bytes().to_vec();
        data.extend(nested);
        assert!(parse_inventory(&data).is_none());
        let mut data = 1u32.to_le_bytes().to_vec();
        data.extend(item(1, 23, &[(0, item(2, 23, &[])), (0, item(3, 23, &[]))]));
        assert!(parse_inventory(&data).is_none());
    }
    #[test]
    fn current_eqemu_slots_roundtrip_without_legacy_bag_aliases() {
        for id in (0..34)
            .chain(2000..2024)
            .chain(2500..2502)
            .chain(3000..3008)
            .chain(4010..13010)
        {
            let slot = InventorySlot::from_server_slot(id).unwrap();
            assert_eq!(slot.server_slot(), Some(id));
            let mut bytes = Vec::new();
            slot.write(&mut bytes);
            assert_eq!(InventorySlot::read(&mut Reader(&bytes)), Some(slot));
        }
        assert!(InventorySlot::from_server_slot(251).is_none());
        assert_eq!(InventorySlot::CURSOR.server_slot(), Some(33));
        assert_eq!(InventorySlot::CURSOR.in_bag(0).server_slot(), Some(6010));
    }
}
