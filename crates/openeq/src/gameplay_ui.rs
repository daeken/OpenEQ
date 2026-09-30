//! Renderer-neutral live gameplay presentation. All item slots are supplied by
//! the protocol adapter; XML widget numbers are never treated as server slots.

pub use crate::commerce_ui::{CommerceAction, UiBank, UiMerchant, UiMerchantStock, UiMoney};
pub use crate::guild_ui::{
    GuildAction, GuildActionKind, UiGuild, UiGuildMember, UiGuildMotd, UiGuildPage, UiGuildPresence,
};
pub use crate::hotbutton_ui::{UiHotbutton, UiHotbuttonEditor, UiHotbuttons};
use crate::hud::{Hud, HudState};
pub use crate::progression_ui::{
    ProgressionAction, ProgressionActionKind, UiProgression, UiProgressionPage, UiProgressionRow,
};
pub use crate::raid_ui::{
    RaidAction, RaidActionKind, UiRaid, UiRaidInvitation, UiRaidMember, UiRaidPage,
};
pub use crate::social_ui::{SocialAction, UiGroup, UiGroupMember};
pub use crate::trade_ui::{
    ItemUseAction, TradeAction, UiItemUse, UiTrade, UiTradeSlot, UiTradeStage,
};
pub use crate::training_ui::{
    TrainingAction, TrainingActionKind, UiTraining, UiTrainingPage, UiTrainingRow,
};
pub use openeq_ui::TextLink as UiChatLink;
use openeq_ui::{Color, DrawCommand, HitTarget, Rect, TextAlign, TextLine, UiBindings, UiFrame};
use std::collections::{BTreeMap, BTreeSet};
mod spell_inspection;
pub use spell_inspection::SpellInspection;

pub const MAX_SAVED_WINDOWS: usize = 256;

/// Logical presentation IDs, never skin widget names or packet addresses.
pub fn valid_window_id(id: &str) -> bool {
    matches!(
        id,
        "player"
            | "target"
            | "buffs"
            | "spellbar"
            | "group"
            | "raid"
            | "guild"
            | "skills"
            | "training"
            | "hotbuttons"
            | "hotbutton_editor"
            | "chat"
            | "actions"
            | "inventory"
            | "loot"
            | "spellbook"
            | "spell_inspection"
            | "merchant"
            | "bank"
            | "trade"
            | "casting"
            | "map"
    ) || id.strip_prefix("bag:").is_some_and(|slot| {
        slot.parse::<u32>()
            .is_ok_and(|value| value <= i32::MAX as u32 && value.to_string() == slot)
    })
}

/// Back-to-front window preferences plus transient visibility. Initial restore
/// preserves saved order; subsequently opened windows come forward once.
#[derive(Clone, Debug, Default)]
pub struct WindowStack {
    order: Vec<String>,
    visible: BTreeSet<String>,
    initialized: bool,
}
impl WindowStack {
    pub fn restored(order: &[String]) -> Self {
        let mut seen = BTreeSet::new();
        Self {
            order: order
                .iter()
                .filter(|id| valid_window_id(id) && seen.insert((*id).clone()))
                .take(MAX_SAVED_WINDOWS)
                .cloned()
                .collect(),
            ..Default::default()
        }
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }

    pub fn raise_hit(&mut self, hit: &HitTarget) -> bool {
        hit.window_id.as_deref().is_some_and(|id| self.raise(id))
    }

    pub fn raise(&mut self, id: &str) -> bool {
        if !valid_window_id(id) || self.order.last().is_some_and(|last| last == id) {
            return false;
        }
        self.order.retain(|old| old != id);
        if self.order.len() == MAX_SAVED_WINDOWS {
            let oldest_hidden = self
                .order
                .iter()
                .position(|old| !self.visible.contains(old));
            self.order.remove(oldest_hidden.unwrap_or(0));
        }
        self.order.push(id.into());
        true
    }

    fn reconcile(&mut self, visible: &[String]) {
        if self.initialized {
            for id in visible {
                if !self.visible.contains(id) {
                    self.raise(id);
                }
            }
        } else {
            // Missing startup windows go behind restored windows. This keeps
            // an old position-only file's default order, and preserves all
            // saved relative ranks when a skin or character gains a window.
            let mut initial: Vec<_> = visible
                .iter()
                .filter(|id| !self.order.contains(id))
                .cloned()
                .collect();
            initial.append(&mut self.order);
            self.order = initial;
            while self.order.len() > MAX_SAVED_WINDOWS {
                let oldest_hidden = self.order.iter().position(|id| !visible.contains(id));
                self.order.remove(oldest_hidden.unwrap_or(0));
            }
            self.initialized = true;
        }
        self.visible = visible.iter().cloned().collect();
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChatLine {
    pub text: String,
    pub color: Color,
    pub links: Vec<UiChatLink>,
}

#[derive(Clone, Debug, Default)]
pub struct UiItem {
    pub id: u32,
    /// Original EQ icon number (500 is the first cell of dragitem1.dds).
    pub icon: u32,
    pub name: String,
    pub count: u32,
    pub details: Vec<String>,
    pub bag_slots: u8,
}

#[derive(Clone, Debug, Default)]
pub struct UiSlot {
    /// Canonical protocol-adapter slot, including bag interior addresses.
    pub slot: i32,
    pub label: String,
    pub item: Option<UiItem>,
}

#[derive(Clone, Debug, Default)]
pub struct UiBag {
    pub parent_slot: i32,
    pub name: String,
    pub slots: Vec<UiSlot>,
}

#[derive(Clone, Debug, Default)]
pub struct UiLootItem {
    pub slot: u16,
    pub item: UiItem,
}

#[derive(Clone, Debug, Default)]
pub struct UiLoot {
    pub name: String,
    pub items: Vec<UiLootItem>,
}

#[derive(Clone, Debug, Default)]
pub struct UiSpell {
    pub id: u32,
    /// Zero-based original A_SpellIcons cell (the spells file's new icon).
    pub icon: u32,
    pub name: String,
    pub mana: u32,
    pub cast_time: f32,
    pub description: String,
    pub level: u8,
}

#[derive(Clone, Debug)]
pub struct UiSpellGem {
    pub gem: u8,
    pub spell: Option<UiSpell>,
    pub ready: bool,
}
impl Default for UiSpellGem {
    fn default() -> Self {
        Self {
            gem: 0,
            spell: None,
            ready: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UiCasting {
    pub label: String,
    pub progress: f32,
}

#[derive(Clone, Debug, Default)]
pub struct UiBuff {
    pub slot: u32,
    pub spell: UiSpell,
    pub remaining_seconds: Option<u32>,
}

pub const SPELLBOOK_PAGE_SIZE: usize = 12;

#[derive(Clone, Debug, Default)]
pub struct GameHudState {
    pub trade: Option<UiTrade>,
    /// Only set for a persistent inspection of a revalidated owned item.
    pub item_use: Option<UiItemUse>,
    pub group: Option<UiGroup>,
    pub raid: Option<UiRaid>,
    pub guild: Option<UiGuild>,
    pub progression: Option<UiProgression>,
    pub training: Option<UiTraining>,
    pub hotbuttons: Option<UiHotbuttons>,
    pub money: Option<UiMoney>,
    pub merchant: Option<UiMerchant>,
    pub bank: Option<UiBank>,
    pub buffs: Vec<UiBuff>,
    pub known_spells: Vec<UiSpell>,
    pub memorized: Vec<UiSpellGem>,
    pub spellbook_open: bool,
    pub spellbook_page: usize,
    pub selected_gem: Option<u8>,
    pub inspected_spell: Option<UiSpell>,
    pub spell_scroll_rows: usize,
    pub casting: Option<UiCasting>,
    pub chat: Vec<ChatLine>,
    pub chat_input: String,
    /// UTF-8 byte offset; None places the caret at the end.
    pub chat_cursor: Option<usize>,
    pub chat_active: bool,
    /// Wrapped display rows above the newest chat line; zero follows new chat.
    pub chat_scroll: usize,
    pub inventory_open: bool,
    /// Equipment in canonical RoF2 order (power source 21, ammo 22).
    pub equipment: Vec<UiSlot>,
    pub inventory: Vec<UiSlot>,
    /// Only currently opened bags are listed here.
    pub bags: Vec<UiBag>,
    pub loot: Option<UiLoot>,
    pub cursor_item: Option<UiItem>,
    /// Cursor position in the same logical pixels as the frame viewport.
    pub pointer: Option<[f32; 2]>,
    /// An interactive overlay owns the pointer; suppress underlying hover tips.
    /// Cursor items and persistent inspection remain separate presentation.
    pub hover_blocked: bool,
    pub selected_slot: Option<i32>,
    pub attack: bool,
    pub sitting: bool,
    /// Keys: player, target, chat, actions, inventory, loot, bag:<parent_slot>,
    /// spellbar, spellbook, spell_inspection, casting, buffs, merchant, bank, group, raid,
    /// guild, skills, training, trade, hotbuttons, hotbutton_editor.
    /// All coordinates are logical pixels.
    pub window_positions: BTreeMap<String, [f32; 2]>,
    /// Back-to-front logical IDs; used by standalone frame/capture callers.
    pub window_order: Vec<String>,
    /// Optional persistent right-click inspection (hover inspection is automatic).
    pub inspected_item: Option<UiItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiAction {
    Trade(TradeAction),
    ItemUse(ItemUseAction),
    ChatLink(u64),
    Social(SocialAction),
    Raid(RaidAction),
    Guild(GuildAction),
    Progression(ProgressionAction),
    Training(TrainingAction),
    OpenSkills,
    Commerce(CommerceAction),
    /// Only dispatch a removal for a right-click; left-click is inspection.
    RemoveBuff(u32),
    CastGem(u8),
    MemorizeSpell {
        id: u32,
        gem: u8,
    },
    ForgetGem(u8),
    ToggleSpellbook,
    SpellbookPage(i32),
    ScrollSpell(i32),
    InventorySlot(i32),
    LootItem(u16),
    ToggleInventory,
    ToggleAttack,
    ToggleSit,
    Hail,
    LootTarget,
    LootAll,
    CloseWindow(String),
    FocusChat,
    BeginWindowDrag(String),
}

impl UiAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        if let Some(action) = TradeAction::from_hit(hit) {
            return Some(Self::Trade(action));
        }
        if let Some(action) = ItemUseAction::from_hit(hit) {
            return Some(Self::ItemUse(action));
        }
        if let Some(action) = CommerceAction::from_hit(hit) {
            return Some(Self::Commerce(action));
        }
        if let Some(action) = SocialAction::from_hit(hit) {
            return Some(Self::Social(action));
        }
        if let Some(action) = RaidAction::from_hit(hit) {
            return Some(Self::Raid(action));
        }
        if let Some(action) = GuildAction::from_hit(hit) {
            return Some(Self::Guild(action));
        }
        if let Some(action) = ProgressionAction::from_hit(hit) {
            return Some(Self::Progression(action));
        }
        if let Some(action) = TrainingAction::from_hit(hit) {
            return Some(Self::Training(action));
        }
        let item = hit.item.as_str();
        if let Some(rows) = item.strip_prefix("SDW_SpellDescription:scroll:") {
            return rows
                .parse::<i32>()
                .ok()
                .filter(|rows| matches!(rows, -1 | 1))
                .map(Self::ScrollSpell);
        }
        if let Some(id) = item.strip_prefix("chat:link:") {
            return id.parse().ok().map(Self::ChatLink);
        }
        if let Some(slot) = item.strip_prefix("game:buff:") {
            return slot.parse().ok().map(Self::RemoveBuff);
        }

        if let Some(gem) = item.strip_prefix("game:cast:") {
            return gem.parse().ok().map(Self::CastGem);
        }
        if let Some(gem) = item.strip_prefix("game:forget:") {
            return gem.parse().ok().map(Self::ForgetGem);
        }
        if let Some(value) = item.strip_prefix("game:memorize:") {
            let (id, gem) = value.split_once(':')?;
            return Some(Self::MemorizeSpell {
                id: id.parse().ok()?,
                gem: gem.parse().ok()?,
            });
        }
        if let Some(page) = item.strip_prefix("game:spellpage:") {
            return page.parse().ok().map(Self::SpellbookPage);
        }

        if let Some(slot) = item.strip_prefix("game:slot:") {
            return slot.parse().ok().map(Self::InventorySlot);
        }
        if let Some(slot) = item.strip_prefix("game:loot:") {
            return slot.parse().ok().map(Self::LootItem);
        }
        if let Some(window) = item.strip_prefix("game:close:") {
            return Some(Self::CloseWindow(window.to_owned()));
        }
        if let Some(window) = item.strip_prefix("game:drag:") {
            return Some(Self::BeginWindowDrag(window.to_owned()));
        }
        match item {
            "game:spellbook" => Some(Self::ToggleSpellbook),
            "game:inventory" => Some(Self::ToggleInventory),
            "game:skills" if hit.enabled && hit.window_id.as_deref() == Some("inventory") => {
                Some(Self::OpenSkills)
            }
            "game:attack" => Some(Self::ToggleAttack),
            "game:sit" => Some(Self::ToggleSit),
            "game:hail" => Some(Self::Hail),
            "game:loot_target" => Some(Self::LootTarget),
            "game:loot_all" => Some(Self::LootAll),
            "game:chat_input" => Some(Self::FocusChat),
            _ => None,
        }
    }
}

pub(crate) const WHITE: Color = [230, 226, 213, 255];
pub(crate) const GOLD: Color = [224, 194, 121, 255];
pub(crate) const MUTED: Color = [155, 164, 170, 255];

impl Hud {
    /// Composes the original XML skin with live gameplay state. The application
    /// owns interaction/networking and consumes the namespaced hit targets.
    pub fn gameplay_frame(
        &self,
        viewport: [u32; 2],
        resources: &HudState,
        state: &GameHudState,
    ) -> UiFrame {
        let mut stack = WindowStack::restored(&state.window_order);
        self.gameplay_frame_with_windows(viewport, resources, state, Vec::new(), &mut stack)
    }

    /// Additional named windows (such as map) participate before hover lookup.
    /// The stack retains visibility across frames so newly opened windows raise
    /// once without disrupting the order restored on the initial frame.
    pub fn gameplay_frame_with_windows(
        &self,
        viewport: [u32; 2],
        resources: &HudState,
        state: &GameHudState,
        additional: Vec<(String, UiFrame)>,
        stack: &mut WindowStack,
    ) -> UiFrame {
        let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
        let mut draw = Painter::new(self, screen, state.pointer);
        let mut bindings = self.initial_bindings.clone();
        self.bind_resources(&mut bindings, resources);
        for (key, name, fallback) in [
            ("player", "PlayerWindow", Rect::new(12., 12., 240., 95.)),
            ("target", "TargetWindow", Rect::new(264., 12., 260., 52.)),
        ] {
            let rect = position(state, key, fallback, screen);
            draw.begin_window(key, rect);
            bindings.widget_mut(name).rect = Some(rect);
            draw.widget(name, &bindings);
            draw.hit(
                format!("game:drag:{key}"),
                "WindowTitle",
                Rect::new(rect.x, rect.y, rect.width, 16.),
                None,
            );
        }

        if !state.buffs.is_empty() {
            draw.buffs(state);
        }
        if state.memorized.iter().any(|gem| gem.spell.is_some()) || !state.known_spells.is_empty() {
            draw.spellbar(state);
        }
        if let Some(group) = &state.group {
            draw.group(state, group);
        }
        let chat = position(
            state,
            "chat",
            Rect::new(12., screen.height - 220., 550., 208.),
            screen,
        );
        draw.shell(
            "ChatWindow",
            "chat",
            chat,
            if state.chat_scroll == 0 {
                "Chat"
            } else {
                "Chat — scroll to return to newest"
            },
            false,
        );
        let log = Rect::new(
            chat.x + 10.,
            chat.y + 24.,
            chat.width - 20.,
            chat.height - 56.,
        );
        draw.frame.commands.push(DrawCommand::TextLog {
            rect: log,
            clip: screen.intersect(log),
            lines: if state.chat.is_empty() {
                vec![TextLine {
                    text: resources.status.clone(),
                    color: MUTED,
                    links: Vec::new(),
                }]
            } else {
                state
                    .chat
                    .iter()
                    .map(|line| TextLine {
                        text: line.text.clone(),
                        color: line.color,
                        links: line.links.clone(),
                    })
                    .collect()
            },
            font: 2,
            scroll_rows: state.chat_scroll,
        });
        draw.hit("game:chat_log", "ChatLog", log, None);
        let input = Rect::new(chat.x + 8., chat.bottom() - 29., chat.width - 16., 22.);
        draw.fill(input, [8, 10, 15, 220]);
        draw.outline(
            input,
            if state.chat_active {
                GOLD
            } else {
                [70, 77, 87, 255]
            },
        );
        let input_text = if state.chat_active {
            chat_edit_text(
                &state.chat_input,
                state.chat_cursor,
                (input.width / 8.).max(1.) as usize,
            )
        } else if state.chat_input.is_empty() {
            "Press Enter to chat · /say · /tell · /group".to_owned()
        } else {
            state.chat_input.clone()
        };
        draw.text(
            inset(input, 4.),
            input_text,
            if state.chat_active { WHITE } else { MUTED },
            false,
        );
        draw.hit("game:chat_input", "Editbox", input, None);

        let actions = position(
            state,
            "actions",
            Rect::new(12., chat.y - 52., 550., 44.),
            screen,
        );
        draw.shell("ChatWindow", "actions", actions, "Actions", false);
        let width = (actions.width - 20.) / 5.;
        for (index, (id, text, active)) in [
            (
                "game:attack",
                if state.attack { "Attacking" } else { "Attack" },
                state.attack,
            ),
            (
                "game:sit",
                if state.sitting { "Stand" } else { "Sit" },
                state.sitting,
            ),
            ("game:hail", "Hail", false),
            ("game:inventory", "Inventory", state.inventory_open),
            ("game:loot_target", "Loot", state.loot.is_some()),
        ]
        .into_iter()
        .enumerate()
        {
            draw.button(
                "AMP_SitButton",
                id,
                Rect::new(
                    actions.x + 10. + index as f32 * width,
                    actions.y + 18.,
                    width - 4.,
                    22.,
                ),
                text,
                active,
            );
        }

        if state.inventory_open {
            let rect = position(
                state,
                "inventory",
                Rect::new(screen.width - 350., 12., 338., 498.),
                screen,
            );
            draw.shell(
                "InventoryWindow",
                "inventory",
                rect,
                "Inventory & equipment",
                true,
            );
            draw.text(
                Rect::new(rect.x + 12., rect.y + 30., 185., 18.),
                "Equipment",
                GOLD,
                false,
            );
            draw.text(
                Rect::new(rect.x + 220., rect.y + 30., 105., 18.),
                "Inventory",
                GOLD,
                false,
            );
            let mut clipped_slots = false;
            for slot in &state.equipment {
                if let Some((xml_slot, x, y)) = equipment_position(slot.slot) {
                    let bounds = Rect::new(rect.x + x, rect.y + y, 42., 42.);
                    if bounds.bottom() > rect.bottom() - 94. {
                        clipped_slots = true;
                        continue;
                    }
                    draw.slot(
                        &format!("InvSlot{xml_slot}"),
                        bounds,
                        slot,
                        state.selected_slot == Some(slot.slot),
                    );
                }
            }
            for (index, slot) in state.inventory.iter().enumerate() {
                let bounds = Rect::new(
                    rect.x + 222. + (index % 2) as f32 * 46.,
                    rect.y + 58. + (index / 2) as f32 * 46.,
                    42.,
                    42.,
                );
                if bounds.bottom() <= rect.bottom() - 94. {
                    draw.slot(
                        "Container_Slot",
                        bounds,
                        slot,
                        state.selected_slot == Some(slot.slot),
                    );
                } else {
                    clipped_slots = true;
                }
            }
            draw.inventory_progression(rect, state.progression.as_ref());
            draw.text(
                Rect::new(rect.x + 13., rect.bottom() - 24., rect.width - 26., 17.),
                if clipped_slots {
                    "Enlarge the game window to see all slots."
                } else {
                    "Click to pick up/place · Right-click to inspect or open"
                },
                MUTED,
                false,
            );
        }
        for (index, bag) in state.bags.iter().enumerate() {
            let key = format!("bag:{}", bag.parent_slot);
            let columns = bag.slots.len().clamp(1, 5);
            let rows = bag.slots.len().div_ceil(columns).max(1);
            let rect = position(
                state,
                &key,
                Rect::new(
                    screen.width - 590. - index as f32 * 24.,
                    120. + index as f32 * 35.,
                    columns as f32 * 44. + 20.,
                    rows as f32 * 44. + 36.,
                ),
                screen,
            );
            draw.shell("ContainerWindow", &key, rect, &bag.name, true);
            for (index, slot) in bag.slots.iter().enumerate() {
                let bounds = Rect::new(
                    rect.x + 10. + (index % columns) as f32 * 44.,
                    rect.y + 26. + (index / columns) as f32 * 44.,
                    40.,
                    40.,
                );
                draw.slot(
                    "Container_Slot",
                    bounds,
                    slot,
                    state.selected_slot == Some(slot.slot),
                );
            }
        }
        if let Some(loot) = &state.loot {
            let rows = loot.items.len().div_ceil(6).max(1);
            let rect = position(
                state,
                "loot",
                Rect::new(
                    screen.width * 0.5 - 142.,
                    130.,
                    284.,
                    rows as f32 * 44. + 74.,
                ),
                screen,
            );
            draw.shell("LootWnd", "loot", rect, &loot.name, true);
            for (index, item) in loot.items.iter().enumerate() {
                let bounds = Rect::new(
                    rect.x + 12. + (index % 6) as f32 * 44.,
                    rect.y + 28. + (index / 6) as f32 * 44.,
                    40.,
                    40.,
                );
                draw.item_slot("LW_LootSlot0", bounds, Some(&item.item), false);
                draw.hit(
                    format!("game:loot:{}", item.slot),
                    "LootItem",
                    bounds,
                    Some(item.item.name.clone()),
                );
            }
            if loot.items.is_empty() {
                draw.text(
                    Rect::new(rect.x + 16., rect.y + 32., rect.width - 32., 30.),
                    "Nothing left to loot.",
                    MUTED,
                    false,
                );
            }
            draw.button(
                "LW_LootAllButton",
                "game:loot_all",
                Rect::new(rect.x + 12., rect.bottom() - 33., 124., 24.),
                "Loot all",
                false,
            );
            draw.button(
                "LW_DoneButton",
                "game:close:loot",
                Rect::new(rect.x + 148., rect.bottom() - 33., 124., 24.),
                "Done",
                false,
            );
        }

        if state.spellbook_open {
            draw.spellbook(state);
        }
        draw.commerce(state);
        if let Some(trade) = &state.trade {
            draw.trade(state, trade);
        }
        if let Some(casting) = &state.casting {
            draw.casting(state, casting);
        }
        if let Some(spell) = &state.inspected_spell {
            draw.spell_inspection(state, spell);
        }
        if let Some(raid) = &state.raid {
            draw.raid(state, raid);
        }
        if let Some(guild) = &state.guild {
            draw.guild(state, guild);
        }
        if let Some(progression) = &state.progression {
            draw.progression(state, progression);
        }
        if let Some(training) = &state.training {
            draw.training(state, training);
        }
        if let Some(hotbuttons) = &state.hotbuttons {
            draw.hotbuttons(state, hotbuttons);
        }
        draw.compose_windows(additional, stack);
        let hovered = state
            .pointer
            .filter(|_| !state.hover_blocked)
            .and_then(|point| draw.frame.hit_test(point))
            .and_then(|hit| match UiAction::from_hit(hit) {
                Some(UiAction::InventorySlot(id)) => state
                    .equipment
                    .iter()
                    .chain(&state.inventory)
                    .chain(state.bags.iter().flat_map(|bag| &bag.slots))
                    .chain(
                        state
                            .bank
                            .iter()
                            .flat_map(|bank| bank.slots.iter().chain(&bank.shared_slots)),
                    )
                    .find(|slot| slot.slot == id)
                    .and_then(|slot| slot.item.as_ref()),
                Some(UiAction::Commerce(CommerceAction::SelectStock(id))) => state
                    .merchant
                    .as_ref()?
                    .stock
                    .iter()
                    .find(|stock| stock.slot == id)
                    .map(|stock| &stock.item),
                Some(UiAction::LootItem(id)) => state
                    .loot
                    .as_ref()?
                    .items
                    .iter()
                    .find(|item| item.slot == id)
                    .map(|item| &item.item),
                Some(UiAction::Trade(TradeAction::OwnSlot(id))) => state
                    .trade
                    .as_ref()?
                    .own_slots
                    .iter()
                    .find(|slot| slot.slot == id)
                    .and_then(|slot| slot.item.as_ref()),
                Some(UiAction::Trade(TradeAction::InspectPartner(id))) => state
                    .trade
                    .as_ref()?
                    .partner_slots
                    .iter()
                    .find(|slot| slot.slot == id)
                    .and_then(|slot| slot.item.as_ref()),
                _ => None,
            });
        if let Some(item) = state
            .inspected_item
            .as_ref()
            .or(if state.cursor_item.is_none() {
                hovered
            } else {
                None
            })
        {
            let persistent = state.inspected_item.is_some();
            let anchor = if persistent {
                [screen.width * 0.5 - 177., screen.height * 0.35]
            } else {
                state.pointer.unwrap_or([screen.width * 0.5, 190.])
            };
            let item_use = state.item_use.as_ref().filter(|_| persistent);
            let rect = draw.tooltip(item, anchor, if item_use.is_some() { 84. } else { 0. });
            if persistent {
                draw.hit("game:inspect", "ItemInspection", rect, None);
                let close = Rect::new(rect.right() - 20., rect.y + 2., 18., 18.);
                draw.text(close, "×", WHITE, false);
                draw.hit(
                    "game:close:inspect",
                    "Button",
                    close,
                    Some("Close item details".into()),
                );
                if let Some(item_use) = item_use {
                    draw.item_use_controls(item_use, rect);
                }
            }
        }
        if state.cursor_item.is_none()
            && !state.hover_blocked
            && let Some(point) = state.pointer
            && let Some(hit) = draw.frame.hit_test(point)
        {
            let spell = match UiAction::from_hit(hit) {
                Some(UiAction::CastGem(gem)) => state
                    .memorized
                    .iter()
                    .find(|entry| entry.gem == gem)
                    .and_then(|entry| entry.spell.as_ref()),
                Some(UiAction::MemorizeSpell { id, .. }) => {
                    state.known_spells.iter().find(|spell| spell.id == id)
                }
                Some(UiAction::RemoveBuff(slot)) => state
                    .buffs
                    .iter()
                    .find(|buff| buff.slot == slot)
                    .map(|buff| &buff.spell),
                _ => None,
            };
            if let Some(spell) = spell {
                draw.spell_tooltip(spell, point);
            }
        }
        if let (Some(item), Some(point)) = (&state.cursor_item, state.pointer) {
            draw.icon(item, Rect::new(point[0] + 10., point[1] + 10., 40., 40.));
        }
        draw.hotbutton_tooltip(state);
        draw.frame.warnings.sort();
        draw.frame.warnings.dedup();
        draw.frame
    }
}

pub(crate) fn chat_edit_text(input: &str, cursor: Option<usize>, visible: usize) -> String {
    let cursor = cursor.unwrap_or(input.len()).min(input.len());
    let cursor = (0..=cursor)
        .rev()
        .find(|index| input.is_char_boundary(*index))
        .unwrap_or(0);
    let before: Vec<_> = input[..cursor].chars().collect();
    let start = before.len().saturating_sub(visible.saturating_sub(1));
    let mut result: String = before[start..].iter().collect();
    result.push('|');
    result.extend(
        input[cursor..]
            .chars()
            .take(visible.saturating_sub(before.len() - start + 1)),
    );
    result
}

pub(crate) fn position(state: &GameHudState, key: &str, fallback: Rect, screen: Rect) -> Rect {
    let [x, y] = state
        .window_positions
        .get(key)
        .copied()
        .unwrap_or([fallback.x, fallback.y]);
    let width = fallback.width.min(screen.width).max(0.);
    let height = fallback.height.min(screen.height).max(0.);
    Rect::new(
        x.clamp(0., (screen.width - width).max(0.)),
        y.clamp(0., (screen.height - height).max(0.)),
        width,
        height,
    )
}

/// Canonical RoF2 equipment slots match the original XML (power 21, ammo 22).
fn equipment_position(slot: i32) -> Option<(usize, f32, f32)> {
    const LOCATIONS: [(f32, f32); 23] = [
        (98., 228.),
        (11., 12.),
        (54., 12.),
        (98., 12.),
        (141., 12.),
        (141., 55.),
        (141., 141.),
        (11., 99.),
        (141., 98.),
        (11., 185.),
        (141., 185.),
        (98., 316.),
        (54., 228.),
        (11., 316.),
        (54., 316.),
        (54., 272.),
        (98., 272.),
        (11., 55.),
        (11., 228.),
        (141., 228.),
        (11., 142.),
        (141., 272.),
        (141., 316.),
    ];
    let index = match slot {
        0..=22 => slot as usize,
        _ => return None,
    };
    let (x, y) = LOCATIONS[index];
    Some((index, x + 8., y + 46.))
}

fn duration_text(seconds: Option<u32>) -> String {
    match seconds {
        None => "∞".to_owned(),
        Some(value) if value >= 3600 => format!("{}h", value / 3600),
        Some(value) => format!("{}:{:02}", value / 60, value % 60),
    }
}

fn inset(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.x + amount,
        rect.y + amount,
        (rect.width - amount * 2.).max(0.),
        (rect.height - amount * 2.).max(0.),
    )
}

pub(crate) struct Painter<'a> {
    pub(crate) hud: &'a Hud,
    pub(crate) screen: Rect,
    pub(crate) pointer: Option<[f32; 2]>,
    pub(crate) frame: UiFrame,
    active_window: Option<String>,
    windows: Vec<(String, UiFrame)>,
}

impl<'a> Painter<'a> {
    pub(crate) fn new(hud: &'a Hud, screen: Rect, pointer: Option<[f32; 2]>) -> Self {
        Self {
            hud,
            screen,
            pointer,
            frame: UiFrame {
                bounds: screen,
                ..Default::default()
            },
            active_window: None,
            windows: Vec::new(),
        }
    }

    fn finish_window(&mut self) {
        if let Some(id) = self.active_window.take() {
            let frame = std::mem::replace(
                &mut self.frame,
                UiFrame {
                    bounds: self.screen,
                    ..Default::default()
                },
            );
            self.windows.push((id, frame));
        }
    }

    fn begin_window(&mut self, id: &str, rect: Rect) {
        self.finish_window();
        self.active_window = Some(id.to_owned());
        self.hit(format!("game:window:{id}"), "WindowBody", rect, None);
    }

    fn compose_windows(&mut self, additional: Vec<(String, UiFrame)>, stack: &mut WindowStack) {
        self.finish_window();
        self.windows.extend(additional);
        let mut seen = BTreeSet::new();
        self.windows
            .retain(|(id, _)| valid_window_id(id) && seen.insert(id.clone()));
        let visible: Vec<_> = self.windows.iter().map(|(id, _)| id.clone()).collect();
        stack.reconcile(&visible);
        self.windows
            .sort_by_key(|(id, _)| stack.order.iter().position(|old| old == id).unwrap_or(0));
        for (id, mut frame) in self.windows.drain(..) {
            for hit in &mut frame.hit_targets {
                hit.window_id = Some(id.clone());
            }
            self.frame.commands.append(&mut frame.commands);
            self.frame.hit_targets.append(&mut frame.hit_targets);
            self.frame.warnings.append(&mut frame.warnings);
        }
    }

    pub(crate) fn widget(&mut self, name: &str, bindings: &UiBindings) {
        match self.hud.ui.window(name) {
            Ok(window) => {
                let mut frame = window.layout(self.screen, bindings);
                self.frame.commands.append(&mut frame.commands);
                self.frame.hit_targets.append(&mut frame.hit_targets);
                self.frame.warnings.append(&mut frame.warnings);
            }
            Err(error) => self.frame.warnings.push(error.to_string()),
        }
    }

    pub(crate) fn shell(
        &mut self,
        template: &str,
        key: &str,
        rect: Rect,
        title: &str,
        close: bool,
    ) {
        self.begin_window(key, rect);
        let mut bindings = UiBindings::default();
        if let Some(root) = self.hud.ui.definition(template) {
            for child in root.values("Pieces").chain(root.values("Pages")) {
                let name = self
                    .hud
                    .ui
                    .definition(child)
                    .map_or(child, |child| &child.item);
                bindings.widget_mut(name).visible = Some(false);
            }
        }
        bindings.widget_mut(template).rect = Some(rect);
        bindings.widget_mut(template).text = Some(title.to_owned());
        self.widget(template, &bindings);
        self.hit(
            format!("game:drag:{key}"),
            "WindowTitle",
            Rect::new(rect.x, rect.y, rect.width, 20.),
            None,
        );
        if close {
            let bounds = Rect::new(rect.right() - 21., rect.y + 3., 16., 16.);
            let animation = self
                .hud
                .ui
                .definition(template)
                .and_then(|root| root.value("DrawTemplate"))
                .and_then(|name| self.hud.ui.definition(name))
                .and_then(|template| template.child("CloseBox"))
                .and_then(|close| {
                    close.value(if self.pointer.is_some_and(|p| bounds.contains(p)) {
                        "Flyby"
                    } else {
                        "Normal"
                    })
                });
            if let Some(animation) = animation {
                self.animation(animation, None, bounds);
            } else {
                self.text(bounds, "×", WHITE, false);
            }
            self.hit(
                format!("game:close:{key}"),
                "Button",
                bounds,
                Some("Close".to_owned()),
            );
        }
    }

    pub(crate) fn button(
        &mut self,
        template: &str,
        id: &str,
        rect: Rect,
        text: &str,
        active: bool,
    ) {
        let mut bindings = UiBindings::default();
        let state = bindings.widget_mut(template);
        state.rect = Some(rect);
        state.text = Some(text.to_owned());
        state.checked = active;
        state.hovered = self.pointer.is_some_and(|point| rect.contains(point));
        self.widget(template, &bindings);
        if active {
            self.outline(inset(rect, 1.), GOLD);
        }
        self.hit(id, "Button", rect, None);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn button_enabled(
        &mut self,
        template: &str,
        id: &str,
        rect: Rect,
        text: &str,
        active: bool,
        enabled: bool,
    ) {
        let mut bindings = UiBindings::default();
        let state = bindings.widget_mut(template);
        state.rect = Some(rect);
        state.text = Some(text.to_owned());
        state.checked = active;
        state.enabled = Some(enabled);
        state.hovered = enabled && self.pointer.is_some_and(|point| rect.contains(point));
        self.widget(template, &bindings);
        if active {
            self.outline(inset(rect, 1.), GOLD);
        }
        self.hit_enabled(id, "Button", rect, None, enabled);
    }

    pub(crate) fn hit_enabled(
        &mut self,
        id: impl Into<String>,
        kind: &str,
        rect: Rect,
        tooltip: Option<String>,
        enabled: bool,
    ) {
        let rect = rect.intersect(self.screen);
        if rect.is_empty() {
            return;
        }
        let id = id.into();
        self.frame.hit_targets.push(HitTarget {
            window_id: None,
            screen_id: id.clone(),
            item: id,
            kind: kind.into(),
            rect,
            enabled,
            tooltip,
        });
    }

    pub(crate) fn slot(&mut self, template: &str, rect: Rect, slot: &UiSlot, selected: bool) {
        self.item_slot(template, rect, slot.item.as_ref(), selected);
        self.hit(
            format!("game:slot:{}", slot.slot),
            "InvSlot",
            rect,
            Some(
                slot.item
                    .as_ref()
                    .map_or(&slot.label, |item| &item.name)
                    .clone(),
            ),
        );
    }

    pub(crate) fn item_slot(
        &mut self,
        template: &str,
        rect: Rect,
        item: Option<&UiItem>,
        selected: bool,
    ) {
        let mut bindings = UiBindings::default();
        bindings.widget_mut(template).rect = Some(rect);
        self.widget(template, &bindings);
        if let Some(item) = item {
            self.icon(item, inset(rect, 1.));
        }
        if selected || self.pointer.is_some_and(|point| rect.contains(point)) {
            self.outline(rect, if selected { GOLD } else { [176, 187, 207, 255] });
        }
    }

    pub(crate) fn icon(&mut self, item: &UiItem, rect: Rect) {
        if let Some(index) = item.icon.checked_sub(500) {
            self.animation("A_DragItem", Some(index as usize), rect);
        }
        if item.count > 1 {
            let count = item.count.to_string();
            let bounds = Rect::new(rect.x, rect.bottom() - 16., rect.width - 3., 15.);
            self.frame.commands.push(DrawCommand::Text {
                rect: bounds,
                clip: rect.intersect(self.screen),
                text: count,
                font: 2,
                color: [255; 4],
                align: TextAlign::Right,
                vertical_center: false,
                wrap: false,
            });
        }
    }

    pub(crate) fn animation(&mut self, name: &str, index: Option<usize>, rect: Rect) {
        if let Some(frame) = self
            .hud
            .ui
            .animations
            .get(name)
            .and_then(|animation| animation.frame(0, index))
        {
            // Zero-size animation cells are intentional blanks in the skin.
            if frame.source.is_empty() {
                return;
            }
            let texture = self.hud.ui.textures.get(&frame.texture);
            self.frame.commands.push(DrawCommand::Image {
                rect,
                clip: self.screen.intersect(rect),
                texture: texture.map_or_else(
                    || self.hud.ui.texture_path(&frame.texture),
                    |texture| texture.path.clone(),
                ),
                source: frame.source,
                texture_size: texture.map_or([0, 0], |texture| texture.size),
                tint: [255; 4],
            });
        } else {
            self.frame
                .warnings
                .push(format!("missing animation/frame {name} {index:?}"));
        }
    }

    fn buffs(&mut self, state: &GameHudState) {
        let columns = state.buffs.len().clamp(1, 8);
        let rows = state.buffs.len().div_ceil(columns);
        let rect = position(
            state,
            "buffs",
            Rect::new(
                540.,
                12.,
                (columns as f32 * 38. + 16.).max(92.),
                rows as f32 * 50. + 29.,
            ),
            self.screen,
        );
        self.fill(rect, [12, 16, 24, 215]);
        self.shell("BuffWindow", "buffs", rect, "Effects", false);
        for (index, buff) in state.buffs.iter().enumerate() {
            let bounds = Rect::new(
                rect.x + 8. + (index % columns) as f32 * 38.,
                rect.y + 24. + (index / columns) as f32 * 50.,
                34.,
                34.,
            );
            let mut bindings = UiBindings::default();
            let button = bindings.widget_mut("BW_Buff0_Button");
            button.rect = Some(bounds);
            button.hovered = self.pointer.is_some_and(|point| bounds.contains(point));
            self.widget("BW_Buff0_Button", &bindings);
            self.animation(
                "A_SpellIcons",
                Some(buff.spell.icon as usize),
                inset(bounds, 2.),
            );
            self.text(
                Rect::new(bounds.x, bounds.bottom() + 1., 36., 14.),
                duration_text(buff.remaining_seconds),
                WHITE,
                false,
            );
            self.hit(
                format!("game:buff:{}", buff.slot),
                "Buff",
                Rect::new(bounds.x, bounds.y, bounds.width, 48.),
                Some(format!("{} · Right-click to remove", buff.spell.name)),
            );
        }
    }

    fn spellbar(&mut self, state: &GameHudState) {
        let count = state.memorized.len().clamp(1, 12);
        let rect = position(
            state,
            "spellbar",
            Rect::new(12., 120., count as f32 * 42. + 52., 60.),
            self.screen,
        );
        self.shell("ChatWindow", "spellbar", rect, "Spells", false);
        for (index, gem) in state.memorized.iter().take(12).enumerate() {
            let bounds = Rect::new(rect.x + 8. + index as f32 * 42., rect.y + 23., 40., 32.);
            self.animation("A_SpellGemHolder", None, bounds);
            self.animation("A_SpellGemBackground", None, bounds);
            if let Some(spell) = &gem.spell {
                self.animation(
                    "A_SpellIcons",
                    Some(spell.icon as usize),
                    Rect::new(bounds.x + 8., bounds.y + 4., 24., 24.),
                );
                if !gem.ready {
                    self.fill(inset(bounds, 4.), [0, 0, 0, 150]);
                }
            }
            if state.selected_gem == Some(gem.gem)
                || self.pointer.is_some_and(|p| bounds.contains(p))
            {
                self.outline(inset(bounds, 1.), GOLD);
            }
            self.text(
                Rect::new(bounds.x + 1., bounds.y + 17., 13., 14.),
                (gem.gem + 1).to_string(),
                WHITE,
                false,
            );
            self.hit(
                format!("game:cast:{}", gem.gem),
                "SpellGem",
                bounds,
                Some(gem.spell.as_ref().map_or_else(
                    || format!("Empty spell gem {}", gem.gem + 1),
                    |spell| spell.name.clone(),
                )),
            );
        }
        self.button(
            "CSPW_SpellBookH",
            "game:spellbook",
            Rect::new(rect.right() - 32., rect.y + 26., 24., 25.),
            "",
            state.spellbook_open,
        );
    }

    fn spellbook(&mut self, state: &GameHudState) {
        let rect = position(
            state,
            "spellbook",
            Rect::new(self.screen.width * 0.5 - 260., 190., 520., 385.),
            self.screen,
        );
        self.shell("ChatWindow", "spellbook", rect, "Spellbook", true);
        // Use the original four pieces of parchment and binding, while the live
        // book contents are driven by the server's spellbook slots.
        for (name, x, y, width, height) in [
            ("Default_SpellBook1", 8., 26., 252., 256.),
            ("Default_SpellBook2", 260., 26., 252., 256.),
            ("Default_SpellBook3", 8., 282., 252., 60.),
            ("Default_SpellBook4", 260., 282., 252., 60.),
        ] {
            self.animation(name, None, Rect::new(rect.x + x, rect.y + y, width, height));
        }
        // The parchment artwork includes its own close mark.
        self.hit(
            "game:close:spellbook",
            "Button",
            Rect::new(rect.right() - 75., rect.y + 28., 22., 20.),
            Some("Close spellbook".into()),
        );
        let page_count = state
            .known_spells
            .len()
            .div_ceil(SPELLBOOK_PAGE_SIZE)
            .max(1);
        let page = state.spellbook_page.min(page_count - 1);
        let gem = state.selected_gem.unwrap_or(0);
        for (index, spell) in state
            .known_spells
            .iter()
            .skip(page * SPELLBOOK_PAGE_SIZE)
            .take(SPELLBOOK_PAGE_SIZE)
            .enumerate()
        {
            let bounds = Rect::new(
                rect.x + 40. + (index / 6) as f32 * 227.,
                rect.y + 46. + (index % 6) as f32 * 45.,
                204.,
                42.,
            );
            if self.pointer.is_some_and(|p| bounds.contains(p)) {
                self.fill(bounds, [255, 242, 172, 45]);
            }
            self.animation(
                "A_SpellIcons",
                Some(spell.icon as usize),
                Rect::new(bounds.x, bounds.y + 1., 36., 36.),
            );
            self.text(
                Rect::new(bounds.x + 43., bounds.y + 2., bounds.width - 43., 17.),
                &spell.name,
                [43, 28, 16, 255],
                false,
            );
            self.text(
                Rect::new(bounds.x + 43., bounds.y + 20., bounds.width - 43., 17.),
                format!(
                    "Lv {} · {} mana · {:.1}s",
                    spell.level, spell.mana, spell.cast_time
                ),
                [82, 57, 31, 255],
                false,
            );
            self.hit(
                format!("game:memorize:{}:{gem}", spell.id),
                "SpellbookSpell",
                bounds,
                Some(spell.description.clone()),
            );
        }
        if state.known_spells.is_empty() {
            self.text(
                Rect::new(rect.x + 45., rect.y + 76., 410., 40.),
                "Your spellbook is empty.",
                [60, 40, 25, 255],
                false,
            );
        }
        self.button(
            "AMP_SitButton",
            "game:spellpage:-1",
            Rect::new(rect.x + 16., rect.bottom() - 34., 48., 22.),
            "Prev",
            false,
        );
        self.button(
            "AMP_SitButton",
            "game:spellpage:1",
            Rect::new(rect.x + 70., rect.bottom() - 34., 48., 22.),
            "Next",
            false,
        );
        self.text(
            Rect::new(rect.x + 126., rect.bottom() - 31., 230., 18.),
            format!(
                "Page {}/{} · Memorize into gem {}",
                page + 1,
                page_count,
                gem + 1
            ),
            WHITE,
            false,
        );
        self.button(
            "AMP_SitButton",
            &format!("game:forget:{gem}"),
            Rect::new(rect.right() - 133., rect.bottom() - 34., 118., 22.),
            "Clear gem",
            false,
        );
    }

    fn spell_tooltip(&mut self, spell: &UiSpell, point: [f32; 2]) {
        let width = 310_f32.min(self.screen.width);
        let height = if spell.description.is_empty() {
            84.
        } else {
            154.
        };
        let rect = Rect::new(
            (point[0] + 20.).min(self.screen.width - width).max(0.),
            (point[1] + 20.).min(self.screen.height - height).max(0.),
            width,
            height,
        );
        self.fill(rect, [12, 14, 20, 248]);
        self.outline(rect, GOLD);
        self.animation(
            "A_SpellIcons",
            Some(spell.icon as usize),
            Rect::new(rect.x + 10., rect.y + 10., 40., 40.),
        );
        self.text(
            Rect::new(rect.x + 60., rect.y + 10., rect.width - 70., 40.),
            &spell.name,
            GOLD,
            true,
        );
        self.text(
            Rect::new(rect.x + 12., rect.y + 60., rect.width - 24., 18.),
            format!(
                "Level {} · {} mana · {:.1}s cast",
                spell.level, spell.mana, spell.cast_time
            ),
            WHITE,
            false,
        );
        if !spell.description.is_empty() {
            self.text(
                Rect::new(rect.x + 12., rect.y + 83., rect.width - 24., 60.),
                &spell.description,
                WHITE,
                true,
            );
        }
    }

    fn casting(&mut self, state: &GameHudState, casting: &UiCasting) {
        let rect = position(
            state,
            "casting",
            Rect::new(
                self.screen.width * 0.5 - 150.,
                self.screen.height * 0.55,
                300.,
                67.,
            ),
            self.screen,
        );
        self.shell("CastingWindow", "casting", rect, "Casting", false);
        self.text(
            Rect::new(rect.x + 12., rect.y + 23., rect.width - 24., 17.),
            &casting.label,
            WHITE,
            false,
        );
        let mut bindings = UiBindings::default();
        let gauge = bindings.widget_mut("Casting_Gauge");
        gauge.rect = Some(Rect::new(
            rect.x + 12.,
            rect.bottom() - 20.,
            rect.width - 24.,
            12.,
        ));
        gauge.gauge = Some(casting.progress);
        self.widget("Casting_Gauge", &bindings);
    }

    fn tooltip(&mut self, item: &UiItem, point: [f32; 2], extra_height: f32) -> Rect {
        let height = 78. + item.details.len().min(16) as f32 * 17. + extra_height;
        let width = 310_f32.min(self.screen.width);
        let x = if point[0] + 22. + width > self.screen.width {
            point[0] - width - 12.
        } else {
            point[0] + 22.
        };
        let rect = Rect::new(
            x.clamp(0., (self.screen.width - width).max(0.)),
            (point[1] + 20.).clamp(0., (self.screen.height - height).max(0.)),
            width,
            height.min(self.screen.height),
        );
        self.fill(rect, [12, 14, 20, 248]);
        self.outline(rect, GOLD);
        self.icon(item, Rect::new(rect.x + 10., rect.y + 10., 40., 40.));
        self.text(
            Rect::new(rect.x + 60., rect.y + 10., rect.width - 70., 40.),
            &item.name,
            GOLD,
            true,
        );
        for (index, line) in item.details.iter().take(16).enumerate() {
            self.text(
                Rect::new(
                    rect.x + 12.,
                    rect.y + 60. + index as f32 * 17.,
                    rect.width - 24.,
                    17.,
                ),
                line,
                WHITE,
                false,
            );
        }
        rect
    }

    pub(crate) fn hit(
        &mut self,
        id: impl Into<String>,
        kind: &str,
        rect: Rect,
        tooltip: Option<String>,
    ) {
        let rect = rect.intersect(self.screen);
        if !rect.is_empty() {
            let id = id.into();
            self.frame.hit_targets.push(HitTarget {
                window_id: None,
                screen_id: id.clone(),
                item: id,
                kind: kind.to_owned(),
                rect,
                enabled: true,
                tooltip,
            });
        }
    }
    pub(crate) fn fill(&mut self, rect: Rect, color: Color) {
        self.frame.commands.push(DrawCommand::Fill {
            rect,
            clip: rect.intersect(self.screen),
            color,
        });
    }
    pub(crate) fn outline(&mut self, rect: Rect, color: Color) {
        for edge in [
            Rect::new(rect.x, rect.y, rect.width, 1.),
            Rect::new(rect.x, rect.bottom() - 1., rect.width, 1.),
            Rect::new(rect.x, rect.y, 1., rect.height),
            Rect::new(rect.right() - 1., rect.y, 1., rect.height),
        ] {
            self.fill(edge, color);
        }
    }
    pub(crate) fn text(&mut self, rect: Rect, text: impl Into<String>, color: Color, wrap: bool) {
        self.frame.commands.push(DrawCommand::Text {
            rect,
            clip: self.screen.intersect(rect),
            text: text.into(),
            font: 2,
            color,
            align: TextAlign::Left,
            vertical_center: false,
            wrap,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_actions_keep_server_slot_addresses() {
        let hit = |item: &str| HitTarget {
            window_id: None,
            item: item.into(),
            screen_id: String::new(),
            kind: String::new(),
            rect: Rect::default(),
            enabled: true,
            tooltip: None,
        };
        for slot in [0, 21, 22, 23, 32, 33, 4010, 4219, 6010, 6210, 11010] {
            assert_eq!(
                UiAction::from_hit(&hit(&format!("game:slot:{slot}"))),
                Some(UiAction::InventorySlot(slot))
            );
        }
        assert_eq!(
            UiAction::from_hit(&hit("game:loot:17")),
            Some(UiAction::LootItem(17))
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:drag:bag:23")),
            Some(UiAction::BeginWindowDrag("bag:23".into()))
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:attack")),
            Some(UiAction::ToggleAttack)
        );
        assert_eq!(UiAction::from_hit(&hit("InvSlot23")), None);
        assert_eq!(UiAction::from_hit(&hit("game:slot:bad")), None);
        assert_eq!(
            UiAction::from_hit(&hit("game:cast:7")),
            Some(UiAction::CastGem(7))
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:memorize:200:3")),
            Some(UiAction::MemorizeSpell { id: 200, gem: 3 })
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:forget:3")),
            Some(UiAction::ForgetGem(3))
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:spellpage:-1")),
            Some(UiAction::SpellbookPage(-1))
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:buff:4")),
            Some(UiAction::RemoveBuff(4))
        );
        assert_eq!(duration_text(Some(125)), "2:05");
        assert_eq!(duration_text(None), "∞");
        assert_eq!(equipment_position(21).unwrap().0, 21);
        assert_eq!(equipment_position(22).unwrap().0, 22);
    }

    #[test]
    fn input_caret_tracks_unicode_and_visible_tail() {
        assert_eq!(chat_edit_text("héllo", Some(3), 12), "hé|llo");
        assert_eq!(chat_edit_text("héllo", Some(2), 12), "h|éllo");
        assert_eq!(chat_edit_text("0123456789", None, 5), "6789|");
        assert_eq!(chat_edit_text("héllo", Some(999), 12), "héllo|");
    }

    fn minimal_hud() -> Hud {
        Hud {
            ui: openeq_ui::UiDocument::default(),
            initial_bindings: UiBindings::default(),
        }
    }

    fn overlap_frame(hud: &Hud, stack: &mut WindowStack) -> UiFrame {
        let mut draw = Painter::new(hud, Rect::new(0., 0., 240., 160.), None);
        for (id, rect, color, slot) in [
            (
                "inventory",
                Rect::new(0., 0., 140., 140.),
                [200, 0, 0, 255],
                23,
            ),
            (
                "bag:23",
                Rect::new(80., 0., 140., 140.),
                [0, 200, 0, 255],
                4010,
            ),
        ] {
            draw.begin_window(id, rect);
            draw.fill(rect, color);
            draw.hit(format!("game:slot:{slot}"), "InvSlot", rect, None);
        }
        draw.compose_windows(Vec::new(), stack);
        draw.frame
    }

    #[test]
    fn window_stack_moves_pixels_hits_and_original_clicked_action_together() {
        let hud = minimal_hud();
        let mut stack = WindowStack::default();
        let before = overlap_frame(&hud, &mut stack);
        assert_eq!(
            before.hit_test([100., 50.]).unwrap().window_id.as_deref(),
            Some("bag:23")
        );
        assert!(matches!(
            before.commands.last(),
            Some(DrawCommand::Fill {
                color: [0, 200, 0, 255],
                ..
            })
        ));
        // Click the visible part of the lower inventory; keep that exact hit.
        let clicked = before.hit_test([20., 50.]).unwrap();
        assert!(stack.raise_hit(clicked));
        assert_eq!(
            UiAction::from_hit(clicked),
            Some(UiAction::InventorySlot(23))
        );
        assert!(!stack.raise_hit(clicked));
        let after = overlap_frame(&hud, &mut stack);
        assert_eq!(
            after.hit_test([100., 50.]).unwrap().window_id.as_deref(),
            Some("inventory")
        );
        assert!(matches!(
            after.commands.last(),
            Some(DrawCommand::Fill {
                color: [200, 0, 0, 255],
                ..
            })
        ));
        assert_eq!(
            after
                .hit_targets
                .iter()
                .filter_map(|hit| hit.window_id.as_deref())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["inventory", "bag:23"])
        );
    }

    #[test]
    fn window_stack_preserves_restore_then_raises_reopened_windows_once() {
        let ids = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        let mut stack = WindowStack::restored(&ids(&["map", "inventory", "bag:23"]));
        stack.reconcile(&ids(&["chat", "inventory", "map"]));
        assert_eq!(stack.order(), ids(&["chat", "map", "inventory", "bag:23"]));
        stack.raise("chat");
        let saved = stack.order().to_vec();
        stack.reconcile(&ids(&["chat", "inventory", "map"]));
        assert_eq!(stack.order(), saved);
        stack.reconcile(&ids(&["chat", "map"]));
        stack.reconcile(&ids(&["chat", "inventory", "map"]));
        assert_eq!(stack.order().last().unwrap(), "inventory");
        assert!(
            stack.order().contains(&"bag:23".into()),
            "hidden preferences are retained"
        );
        let mut restored = WindowStack::restored(stack.order());
        restored.reconcile(&ids(&["chat", "inventory", "map"]));
        assert_eq!(restored.order(), stack.order());
    }

    #[test]
    fn window_stack_rejects_invalid_ids_and_bounds_hidden_history() {
        let order = [
            "map",
            "unknown",
            "map",
            "bag:23",
            "bag:-1",
            "bag:023",
            "inventory",
        ]
        .map(str::to_owned);
        let mut stack = WindowStack::restored(&order);
        assert_eq!(stack.order(), ["map", "bag:23", "inventory"]);
        stack.reconcile(&["map".into(), "inventory".into()]);
        for id in 0..300 {
            stack.raise(&format!("bag:{id}"));
        }
        assert_eq!(stack.order().len(), MAX_SAVED_WINDOWS);
        assert!(stack.order().contains(&"map".into()));
        assert!(stack.order().contains(&"inventory".into()));
        assert!(!stack.raise("bag:../../file"));
        let history: Vec<_> = (0..MAX_SAVED_WINDOWS)
            .map(|id| format!("bag:{id}"))
            .collect();
        let mut restored = WindowStack::restored(&history);
        restored.reconcile(&["chat".into(), "map".into()]);
        assert_eq!(restored.order().len(), MAX_SAVED_WINDOWS);
        assert!(restored.order().contains(&"map".into()));
        assert!(restored.order().contains(&"chat".into()));
    }

    #[test]
    fn window_body_and_native_hits_keep_ownership_without_replacing_screen_ids() {
        let mut hud = minimal_hud();
        hud.ui.definitions.insert(
            "NativeButton".into(),
            openeq_ui::Element {
                kind: "Button".into(),
                item: "NativeButton".into(),
                ..Default::default()
            },
        );
        let screen = Rect::new(0., 0., 240., 160.);
        let mut draw = Painter::new(&hud, screen, None);
        draw.shell(
            "MissingSkin",
            "bag:23",
            Rect::new(10., 10., 100., 100.),
            "Bag",
            false,
        );
        draw.hit_enabled(
            "disabled",
            "Button",
            Rect::new(20., 50., 30., 30.),
            None,
            false,
        );
        let mut bindings = UiBindings::default();
        bindings.widget_mut("NativeButton").rect = Some(Rect::new(60., 50., 30., 30.));
        draw.widget("NativeButton", &bindings);
        draw.compose_windows(Vec::new(), &mut WindowStack::default());
        let disabled = draw.frame.hit_test([25., 55.]).unwrap();
        assert_eq!(disabled.item, "game:window:bag:23");
        assert_eq!(disabled.window_id.as_deref(), Some("bag:23"));
        assert!(UiAction::from_hit(disabled).is_none());
        let native = draw.frame.hit_test([65., 55.]).unwrap();
        assert_eq!(native.screen_id, "NativeButton");
        assert_eq!(native.window_id.as_deref(), Some("bag:23"));
    }

    #[test]
    fn window_stack_overlays_and_chat_preserve_input_ownership() {
        let hud = minimal_hud();
        let mut stack = WindowStack::default();
        let mut frame = overlap_frame(&hud, &mut stack);
        // An interactive overlay deliberately owns no ordinary window.
        frame.hit_targets.push(HitTarget {
            item: "recovery:panel".into(),
            screen_id: "RecoveryDialog".into(),
            window_id: None,
            kind: "Screen".into(),
            rect: Rect::new(0., 0., 240., 160.),
            enabled: true,
            tooltip: None,
        });
        let before = stack.order().to_vec();
        assert!(!stack.raise_hit(frame.hit_test([100., 50.]).unwrap()));
        assert_eq!(stack.order(), before);
        let mut chat = Painter::new(&hud, frame.bounds, None);
        chat.begin_window("chat", Rect::new(0., 0., 240., 160.));
        chat.hit(
            "game:chat_input",
            "Editbox",
            Rect::new(0., 0., 100., 20.),
            None,
        );
        chat.compose_windows(Vec::new(), &mut stack);
        stack.raise("inventory");
        let clicked = chat.frame.hit_test([5., 5.]).unwrap();
        stack.raise_hit(clicked);
        assert_eq!(UiAction::from_hit(clicked), Some(UiAction::FocusChat));
        assert_eq!(stack.order().last().unwrap(), "chat");
    }

    fn fixture() -> GameHudState {
        let sword = UiItem {
            id: 1,
            icon: 580,
            name: "Short Sword".into(),
            count: 1,
            details: vec![
                "Slot: PRIMARY SECONDARY".into(),
                "Damage: 4   Delay: 25".into(),
                "Weight: 3.5   Size: MEDIUM".into(),
            ],
            ..Default::default()
        };
        let bag = UiItem {
            id: 2,
            icon: 565,
            name: "Backpack".into(),
            count: 1,
            details: vec!["8 container slots".into()],
            bag_slots: 8,
        };
        let mut state = GameHudState {
            chat: vec![ChatLine { text: "Welcome to Norrath!".into(), color: GOLD, links: vec![] }, ChatLine { text: "You say, 'Hail, Guard Alayle'".into(), color: WHITE, links: vec![] }, ChatLine { text: "Guard Alayle says, 'Greetings, adventurer. The road ahead is dangerous; keep your weapon ready and your friends close.'".into(), color: [120, 220, 180, 255], links: vec![] }, ChatLine { text: "You hit a decaying skeleton for 7 points of damage.".into(), color: [240, 145, 145, 255], links: vec![] }],
            chat_input: "/say Ready for adventure".into(), chat_active: true,
            inventory_open: true, attack: true,
            equipment: (0..23).map(|slot| UiSlot { slot, label: format!("Equipment {slot}"), item: (slot == 13).then(|| sword.clone()) }).collect(),
            inventory: (23..33).map(|slot| UiSlot { slot, label: format!("Inventory {}", slot - 22), item: (slot == 23).then(|| bag.clone()) }).collect(),
            bags: vec![UiBag { parent_slot: 23, name: "Backpack".into(), slots: (0..8).map(|index| UiSlot { slot: 4010 + index, label: format!("Bag slot {}", index + 1), item: (index == 0).then(|| UiItem { icon: 570, name: "Iron Ration".into(), count: 12, ..Default::default() }) }).collect() }],
            loot: Some(UiLoot { name: "A decaying skeleton's corpse".into(), items: vec![UiLootItem { slot: 17, item: sword }] }),
            ..Default::default()
        };
        let heal = UiSpell {
            id: 200,
            icon: 99,
            name: "Minor Healing".into(),
            mana: 10,
            cast_time: 1.5,
            description: "Heals your target's wounds.".into(),
            level: 1,
        };
        state.known_spells = vec![heal.clone()];
        state.memorized = (0..8)
            .map(|gem| UiSpellGem {
                gem,
                spell: (gem == 0).then(|| heal.clone()),
                ready: true,
            })
            .collect();
        state.window_positions.insert("loot".into(), [330., 160.]);
        state
    }

    #[test]
    fn map_order_controls_hover_details_and_inspection_stays_above_windows() {
        let hud = minimal_hud();
        let mut state = fixture();
        let resources = HudState::default();
        let initial = hud.gameplay_frame([1280, 720], &resources, &state);
        let slot = initial
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:slot:13")
            .unwrap();
        let point = [slot.rect.x + 4., slot.rect.y + 4.];
        state.pointer = Some(point);
        let map = crate::map::ZoneMap::parse("test", "");
        let map_state = crate::map::MapState {
            rect: Rect::new(point[0] - 30., point[1] - 30., 250., 220.),
            ..Default::default()
        };
        let mut stack = WindowStack::default();
        let frame = |stack: &mut WindowStack, state: &GameHudState| {
            hud.gameplay_frame_with_windows(
                [1280, 720],
                &resources,
                state,
                vec![("map".into(), map.frame([1280, 720], &map_state))],
                stack,
            )
        };
        let covered = frame(&mut stack, &state);
        assert_eq!(
            covered.hit_test(point).unwrap().window_id.as_deref(),
            Some("map")
        );
        let has_details = |frame: &UiFrame| {
            frame.commands.iter().any(|command| {
                matches!(
                    command, DrawCommand::Text { text, .. } if text == "Damage: 4   Delay: 25"
                )
            })
        };
        assert!(!has_details(&covered));
        stack.raise("inventory");
        let raised = frame(&mut stack, &state);
        assert_eq!(raised.hit_test(point).unwrap().item, "game:slot:13");
        assert!(has_details(&raised));
        state.hover_blocked = true;
        assert!(!has_details(&frame(&mut stack, &state)));
        state.hover_blocked = false;
        state.inspected_item = state.equipment.iter().find_map(|slot| slot.item.clone());
        let inspected = frame(&mut stack, &state);
        let close = inspected
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:close:inspect")
            .unwrap();
        let hit = inspected
            .hit_test([close.rect.x + 4., close.rect.y + 4.])
            .unwrap();
        assert!(hit.window_id.is_none());
        assert!(!stack.raise_hit(hit));
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn original_window_stack_pixels_and_hits_match_at_normal_and_retina_scale() {
        let base = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(base).unwrap();
        let mut state = fixture();
        let resources = HudState {
            character: "Adventurer".into(),
            hp: 0.8,
            mana: Some(0.7),
            ..Default::default()
        };
        for (id, point) in [
            ("inventory", [90., 70.]),
            ("bag:23", [140., 130.]),
            ("bag:24", [180., 170.]),
            ("loot", [230., 200.]),
        ] {
            state.window_positions.insert(id.into(), point);
        }
        let mut second = state.bags[0].clone();
        second.parent_slot = 24;
        second.name = "Second backpack".into();
        for slot in &mut second.slots {
            slot.slot += 100;
        }
        state.bags.push(second);
        let viewport = [900, 640];
        let map = crate::map::ZoneMap::parse(
            "Stacking test",
            "L -200,0,0,200,0,0,200,200,200\nP 0,0,0,200,160,80,1,Center",
        );
        let map_state = crate::map::MapState {
            rect: Rect::new(230., 190., 350., 320.),
            ..Default::default()
        };
        let mut stack = WindowStack::default();
        let before = hud.gameplay_frame_with_windows(
            viewport,
            &resources,
            &state,
            vec![("map".into(), map.frame(viewport, &map_state))],
            &mut stack,
        );
        let overlap = [300., 240.];
        assert_eq!(
            before.hit_test(overlap).unwrap().window_id.as_deref(),
            Some("map")
        );
        let clicked = before.hit_test([96., 76.]).unwrap();
        assert_eq!(
            UiAction::from_hit(clicked),
            Some(UiAction::BeginWindowDrag("inventory".into()))
        );
        stack.raise_hit(clicked);
        let after = hud.gameplay_frame_with_windows(
            viewport,
            &resources,
            &state,
            vec![("map".into(), map.frame(viewport, &map_state))],
            &mut stack,
        );
        assert_eq!(
            after.hit_test(overlap).unwrap().window_id.as_deref(),
            Some("inventory")
        );
        for frame in [&before, &after] {
            assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
            for id in ["bag:23", "bag:24", "loot", "map", "inventory"] {
                assert!(
                    frame
                        .hit_targets
                        .iter()
                        .any(|hit| hit.window_id.as_deref() == Some(id))
                );
            }
        }
        for scale in [1., 2.] {
            let mut renderer = openeq_render::Renderer::new_headless(
                (viewport[0] as f32 * scale) as u32,
                (viewport[1] as f32 * scale) as u32,
            )
            .unwrap();
            let mut samples = Vec::new();
            for (name, frame) in [("before", &before), ("after", &after)] {
                renderer.set_ui_scaled(frame, scale);
                renderer.render_ui();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                let offset = (((overlap[1] * scale) as u32 * width + (overlap[0] * scale) as u32)
                    * 4) as usize;
                samples.push(pixels[offset..offset + 4].to_vec());
                if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    let path = std::path::PathBuf::from(directory)
                        .join(format!("window-stack-{name}-{}x.png", scale as u32));
                    image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8)
                        .unwrap();
                    eprintln!("wrote {}", path.display());
                }
            }
            assert_ne!(
                samples[0], samples[1],
                "visible overlap pixel must change with hit ownership"
            );
        }
    }

    #[test]
    #[ignore = "requires original EverQuest UI files and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn actual_gameplay_windows_use_original_art_and_server_hits() {
        let path = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(path).unwrap();
        let mut state = fixture();
        let resources = HudState {
            character: "Adventurer".into(),
            player_level: 7,
            hp: 0.73,
            mana: Some(0.84),
            endurance: Some(1.),
            target: Some(crate::hud::HudTarget {
                name: "Guard Alayle".into(),
                hp: 0.95,
                level: 12,
            }),
            ..Default::default()
        };
        let frame = hud.gameplay_frame([1280, 720], &resources, &state);
        assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
        for id in [
            "game:slot:13",
            "game:slot:22",
            "game:slot:23",
            "game:slot:4010",
            "game:loot:17",
            "game:chat_input",
            "game:drag:inventory",
            "game:close:bag:23",
            "game:cast:0",
            "game:spellbook",
        ] {
            let hit = frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            let point = [hit.rect.x + 4., hit.rect.y + 4.];
            assert_eq!(frame.hit_test(point).unwrap().item, id);
        }
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Image { texture, source, .. } if texture.file_name().unwrap() == "dragitem3.dds" && source.width == 40.)));
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::TextLog { lines, .. } if lines.len() == 4 && lines[2].color == [120,220,180,255])));
        for command in &frame.commands {
            if let DrawCommand::Image { texture, .. } = command {
                assert!(texture.exists(), "{}", texture.display());
            }
        }
        let slot = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:slot:13")
            .unwrap();
        state.pointer = Some([slot.rect.x + 3., slot.rect.y + 3.]);
        let frame = hud.gameplay_frame([1280, 720], &resources, &state);
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Text {text, ..} if text == "Damage: 4   Delay: 25")));
        let mut inspection = state.clone();
        inspection.pointer = None;
        inspection.inspected_item = inspection
            .equipment
            .iter()
            .find_map(|slot| slot.item.clone());
        let inspected = hud.gameplay_frame([1280, 720], &resources, &inspection);
        let close = inspected
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:close:inspect")
            .unwrap();
        assert_eq!(
            UiAction::from_hit(close),
            Some(UiAction::CloseWindow("inspect".into()))
        );
        assert_eq!(
            inspected
                .hit_test([close.rect.x + 4., close.rect.y + 4.])
                .unwrap()
                .item,
            close.item
        );
        if let Some(destination) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
            let mut renderer = openeq_render::Renderer::new_headless(1280, 720).unwrap();
            let scene =
                openeq_assets::Scene::from_geometry("HUD test".into(), vec![], vec![], vec![]);
            let gpu = openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene)
                .unwrap();
            renderer.set_scene(&gpu);
            renderer.set_ui(&frame);
            renderer.render(&gpu, &openeq_render::Camera::default());
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            let path = std::path::PathBuf::from(destination).join("openeq-gameplay-ui.png");
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
            eprintln!("wrote {}", path.display());
        }
    }
    #[test]
    #[ignore = "requires original EverQuest UI files and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn actual_spellbook_icons_gems_and_cast_progress() {
        let path = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(path).unwrap();
        let mut state = fixture();
        state.inventory_open = false;
        state.bags.clear();
        state.loot = None;
        state.spellbook_open = true;
        state.buffs = vec![UiBuff {
            slot: 4,
            spell: UiSpell {
                id: 202,
                icon: 132,
                name: "Courage".into(),
                ..Default::default()
            },
            remaining_seconds: Some(125),
        }];
        state.selected_gem = Some(3);
        state.casting = Some(UiCasting {
            label: "Minor Healing".into(),
            progress: 0.6,
        });
        state
            .window_positions
            .insert("spellbook".into(), [600., 90.]);
        state
            .window_positions
            .insert("casting".into(), [160., 230.]);
        let frame = hud.gameplay_frame([1280, 720], &HudState::default(), &state);
        assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
        let hit = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:memorize:200:3")
            .unwrap();
        assert_eq!(
            UiAction::from_hit(hit),
            Some(UiAction::MemorizeSpell { id: 200, gem: 3 })
        );
        assert_eq!(
            frame
                .hit_test([hit.rect.x + 10., hit.rect.y + 10.])
                .unwrap()
                .item,
            hit.item
        );
        assert!(frame.commands.iter().any(|cmd| matches!(cmd, DrawCommand::Image { texture, .. } if texture.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case("spells03.tga"))));
        let buff = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:buff:4")
            .unwrap();
        assert_eq!(UiAction::from_hit(buff), Some(UiAction::RemoveBuff(4)));
        assert_eq!(
            frame
                .hit_test([buff.rect.x + 5., buff.rect.y + 5.])
                .unwrap()
                .item,
            buff.item
        );
        let icons = &hud.ui.animations["A_SpellIcons"];
        assert_eq!(icons.frames[1].source, Rect::new(40., 0., 40., 40.));
        assert_eq!(icons.frames[6].source, Rect::new(0., 40., 40., 40.));
        let mut empty = state.clone();
        empty.known_spells.clear();
        for gem in &mut empty.memorized {
            gem.spell = None;
        }
        let empty_frame = hud.gameplay_frame([1280, 720], &HudState::default(), &empty);
        assert!(
            !empty_frame
                .hit_targets
                .iter()
                .any(|hit| hit.item.starts_with("game:cast:")),
            "empty gem slots alone must not show a spell bar"
        );
        assert!(
            empty_frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "game:close:spellbook"),
            "the spellbook still opens independently of the bar"
        );

        if let Some(destination) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
            let mut renderer = openeq_render::Renderer::new_headless(1280, 720).unwrap();
            let scene =
                openeq_assets::Scene::from_geometry("Spells test".into(), vec![], vec![], vec![]);
            let gpu = openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene)
                .unwrap();
            renderer.set_scene(&gpu);
            renderer.set_ui(&frame);
            renderer.render(&gpu, &openeq_render::Camera::default());
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            let path = std::path::PathBuf::from(destination).join("openeq-spellbook-ui.png");
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
            eprintln!("wrote {}", path.display());
        }
    }
}
