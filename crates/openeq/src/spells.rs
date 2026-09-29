//! Presentation metadata from the original client's spells_us.txt. The server
//! remains authoritative for learned spells, cast timing, costs and effects.
use crate::gameplay_ui::UiSpell;
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug)]
pub struct Spell {
    pub id: u32,
    pub name: String,
    pub projectile_model: String,
    pub icon: u32,
    pub mana: u32,
    pub cast_time_ms: u32,
    pub recovery_ms: u32,
    pub recast_ms: u32,
    pub range: f32,
    pub target_type: u32,
    pub beneficial: bool,
    pub effect_id: u32,
    pub casting_animation: u8,
    pub travel_type: u32,
    pub persistent_particles: bool,
    pub levels: [u8; 16],
    pub description: String,
}
impl Spell {
    pub fn view(&self, class: u8) -> UiSpell {
        UiSpell {
            id: self.id,
            icon: self.icon,
            name: self.name.clone(),
            mana: self.mana,
            cast_time: self.cast_time_ms as f32 / 1000.,
            description: format!(
                "{}\nRange {:.0} • Recast {:.1}s",
                self.description,
                self.range,
                self.recast_ms as f32 / 1000.
            ),
            level: self
                .levels
                .get(class.wrapping_sub(1) as usize)
                .copied()
                .unwrap_or(255),
        }
    }
}

#[derive(Default)]
pub struct SpellCatalog {
    pub spells: BTreeMap<u32, Spell>,
    pub malformed_records: usize,
}
impl SpellCatalog {
    pub fn load(base: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(base.join("spells_us.txt"))?;
        Ok(Self::parse(&encoding_rs::WINDOWS_1252.decode(&bytes).0))
    }
    pub fn parse(text: &str) -> Self {
        let mut result = Self::default();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<_> = line.trim_end_matches('\r').split('^').collect();
            if let Some(spell) = parse_spell(&fields) {
                result.spells.insert(spell.id, spell);
            } else {
                result.malformed_records += 1;
            }
        }
        result
    }
    pub fn view(&self, id: u32, class: u8) -> UiSpell {
        self.spells.get(&id).map_or_else(
            || UiSpell {
                id,
                name: format!("Spell {id}"),
                ..Default::default()
            },
            |spell| spell.view(class),
        )
    }
}
fn parse_spell(fields: &[&str]) -> Option<Spell> {
    // The 2016 client moves effect arrays into a final pipe-separated field.
    // Through mana (19) both formats agree; after components the compact form
    // is 60 fields shorter. Original RoF2/EQEmu exports retain fixed arrays.
    let compact = fields.len() >= 100 && fields.len() < 200 && fields.last()?.contains('|');
    let (classes, icon, target, beneficial) = if compact {
        (44, 84, 38, 35)
    } else {
        (104, 144, 98, 83)
    };
    if fields.len() <= icon || fields.get(1)?.is_empty() {
        return None;
    }
    let number = |index: usize| {
        fields
            .get(index)
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0) as u32
    };
    let id = fields[0].parse::<u32>().ok()?;
    let range = fields.get(9)?.parse::<f32>().unwrap_or(0.);
    if !range.is_finite() {
        return None;
    }
    let levels = std::array::from_fn(|i| {
        fields
            .get(classes + i)
            .and_then(|value| value.parse().ok())
            .unwrap_or(255)
    });
    Some(Spell {
        id,
        name: fields[1].into(),
        projectile_model: fields.get(2).copied().unwrap_or("").to_owned(),
        icon: number(icon),
        mana: number(19),
        cast_time_ms: number(13),
        recovery_ms: number(14),
        recast_ms: number(15),
        range: range.max(0.),
        target_type: number(target),
        beneficial: number(beneficial) > 0,
        effect_id: number(if compact { 85 } else { 145 }),
        casting_animation: number(if compact { 60 } else { 120 }).min(255) as u8,
        travel_type: number(if compact { 62 } else { 122 }),
        persistent_particles: number(if compact { 93 } else { 153 }) != 0,
        levels,
        description: fields.get(6).copied().unwrap_or("").to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_and_compact_spell_layouts_agree() {
        for compact in [false, true] {
            let mut fields = vec!["0"; if compact { 174 } else { 220 }];
            fields[0] = "200";
            fields[1] = "Minor Healing";
            fields[6] = "You feel a little better.";
            fields[9] = "100";
            fields[13] = "1500";
            fields[14] = "1500";
            fields[15] = "1500";
            fields[19] = "10";
            fields[if compact { 45 } else { 105 }] = "1";
            fields[if compact { 84 } else { 144 }] = "99";
            fields[if compact { 38 } else { 98 }] = "5";
            fields[if compact { 35 } else { 83 }] = "1";
            fields[if compact { 85 } else { 145 }] = "278";
            fields[if compact { 60 } else { 120 }] = "43";
            fields[if compact { 62 } else { 122 }] = "3";
            fields[if compact { 93 } else { 153 }] = "1";
            if compact {
                fields[173] = "1|0|10|0|2|20";
            }
            let catalog = SpellCatalog::parse(&fields.join("^"));
            let spell = &catalog.spells[&200];
            assert_eq!(spell.name, "Minor Healing");
            assert_eq!(spell.levels[1], 1);
            assert_eq!(spell.mana, 10);
            assert_eq!(spell.icon, 99);
            assert_eq!(spell.cast_time_ms, 1500);
            assert!(spell.beneficial);
            assert_eq!(spell.target_type, 5);
            assert_eq!(spell.effect_id, 278);
            assert_eq!(spell.casting_animation, 43);
            assert_eq!(spell.travel_type, 3);
            assert!(spell.persistent_particles);
        }
    }
    #[test]
    fn malformed_and_nonfinite_records_are_rejected() {
        let catalog = SpellCatalog::parse("not a spell\n1^truncated");
        assert_eq!(catalog.malformed_records, 2);
        assert!(catalog.spells.is_empty());
    }
    #[test]
    fn actual_client_spell_metadata_is_consistent() {
        let Some(base) = openeq_assets::loader::default_client_dir() else {
            return;
        };
        if !base.join("spells_us.txt").is_file() {
            return;
        }
        let catalog = SpellCatalog::load(&base).unwrap();
        assert!(catalog.spells.len() > 1000);
        assert_eq!(catalog.spells[&200].name, "Minor Healing");
        assert_eq!(catalog.spells[&200].levels[1], 1);
        assert_eq!(catalog.spells[&36].target_type, 6);
        assert_eq!(catalog.spells[&36].name, "Gate");
        assert_eq!(catalog.spells[&200].effect_id, 278);
        assert_eq!(catalog.spells[&54].effect_id, 179);
        assert_eq!(catalog.spells[&54].travel_type, 3);
        assert_eq!(catalog.spells[&288].effect_id, 220);
    }
}
