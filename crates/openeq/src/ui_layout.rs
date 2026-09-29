//! Local presentation preferences, scoped to a world and character. No account
//! credentials, inventory, open commerce sessions or world waypoints are saved.
use crate::{
    gameplay_ui::{MAX_SAVED_WINDOWS, WindowStack, valid_window_id},
    hotbuttons::{HotbuttonBindings, decode_saved_hotbuttons, validate_hotbutton},
    map::MapState,
};
use anyhow::{Context, ensure};
use openeq_net::session::ConnectionConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_LAYOUT_BYTES: usize = 65536;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub windows: BTreeMap<String, [f32; 2]>,
    #[serde(default)]
    pub window_order: Vec<String>,
    #[serde(default)]
    pub hotbuttons: HotbuttonBindings,
    pub map_origin: [f32; 2],
    pub map_zoom: f32,
    pub map_labels: bool,
    pub map_layers: [bool; 4],
}
impl Default for Layout {
    fn default() -> Self {
        Self::capture(
            &BTreeMap::new(),
            &[],
            &MapState::default(),
            &HotbuttonBindings::default(),
        )
    }
}
impl Layout {
    pub fn capture(
        windows: &BTreeMap<String, [f32; 2]>,
        order: &[String],
        map: &MapState,
        hotbuttons: &HotbuttonBindings,
    ) -> Self {
        Self {
            windows: windows.clone(),
            window_order: order.to_vec(),
            hotbuttons: hotbuttons.clone(),
            map_origin: [map.rect.x, map.rect.y],
            map_zoom: map.units_per_pixel,
            map_labels: map.show_labels,
            map_layers: map.layers,
        }
    }
    pub fn apply(
        &self,
        windows: &mut BTreeMap<String, [f32; 2]>,
        stack: &mut WindowStack,
        map: &mut MapState,
        hotbuttons: &mut HotbuttonBindings,
    ) {
        *windows = self.windows.clone();
        *stack = WindowStack::restored(&self.window_order);
        *hotbuttons = self.hotbuttons.clone();
        [map.rect.x, map.rect.y] = self.map_origin;
        map.units_per_pixel = self.map_zoom;
        map.show_labels = self.map_labels;
        map.layers = self.map_layers;
    }
    fn validate(&self) -> anyhow::Result<()> {
        for (index, button) in self.hotbuttons.iter().enumerate() {
            if let Some(button) = button {
                let validated = validate_hotbutton(&button.label, &button.command)
                    .with_context(|| format!("invalid hotbutton slot {}", index + 1))?;
                ensure!(
                    validated.as_ref() == Some(button),
                    "hotbutton slot {} must contain a validated definition",
                    index + 1
                );
            }
        }
        ensure!(
            self.windows.len() <= MAX_SAVED_WINDOWS,
            "too many saved windows"
        );
        ensure!(
            self.window_order.len() <= MAX_SAVED_WINDOWS,
            "too many ordered windows"
        );
        let mut unique = BTreeSet::new();
        for id in &self.window_order {
            ensure!(
                valid_window_id(id) && unique.insert(id),
                "invalid or duplicate window order ID"
            );
        }
        for (key, point) in &self.windows {
            ensure!(
                !key.is_empty() && key.len() <= 128 && !key.chars().any(char::is_control),
                "invalid window key"
            );
            ensure!(
                point.iter().all(|n| n.is_finite() && n.abs() <= 32768.),
                "invalid window position"
            );
        }
        ensure!(
            self.map_origin
                .iter()
                .all(|n| n.is_finite() && n.abs() <= 32768.),
            "invalid map position"
        );
        ensure!(
            self.map_zoom.is_finite() && (0.25..=128.).contains(&self.map_zoom),
            "invalid map zoom"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Identity {
    host: String,
    login_port: u16,
    world_port: u16,
    server_id: Option<u32>,
    character: String,
}
impl Identity {
    fn from_config(config: &ConnectionConfig) -> Self {
        Self {
            host: config.host.to_ascii_lowercase(),
            login_port: config.login_port,
            world_port: config.world_port,
            server_id: config.server_id,
            character: config.character.to_ascii_lowercase(),
        }
    }
    fn filename(&self) -> String {
        // Stable FNV-1a filename avoids invalid/path-traversal characters and
        // filesystem length limits. The full identity is checked on load.
        let bytes = serde_json::to_vec(self).expect("serializable layout identity");
        let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
        format!("{hash:016x}.json")
    }
}
#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    identity: Identity,
    layout: Layout,
}

// Decode/validate identity before touching optional per-slot recovery. Keeping
// layout as JSON here prevents one malformed hotbutton from losing valid
// neighboring buttons or unrelated window/map preferences.
#[derive(Deserialize)]
struct IncomingDocument {
    version: u32,
    identity: Identity,
    layout: serde_json::Value,
}

fn decode_document(bytes: &[u8], identity: &Identity) -> anyhow::Result<Layout> {
    let mut document: IncomingDocument =
        serde_json::from_slice(bytes).context("reading saved UI layout")?;
    ensure!(document.version == 1, "unsupported UI layout version");
    ensure!(
        &document.identity == identity,
        "UI layout identity mismatch"
    );
    let layout = document
        .layout
        .as_object_mut()
        .context("invalid saved UI layout")?;
    let (bindings, diagnostics) = decode_saved_hotbuttons(layout.remove("hotbuttons"));
    layout.insert("hotbuttons".into(), serde_json::to_value(bindings)?);
    let layout: Layout =
        serde_json::from_value(document.layout).context("reading saved UI layout")?;
    layout.validate()?;
    if !diagnostics.is_empty() {
        tracing::warn!(
            discarded_slots = ?diagnostics.discarded_slots,
            stored_slot_count = ?diagnostics.unexpected_count,
            invalid_shape = diagnostics.invalid_shape,
            "recovered saved hotbuttons; invalid entries were cleared"
        );
    }
    Ok(layout)
}

pub struct LayoutStore {
    path: PathBuf,
    identity: Identity,
    saved: Layout,
    observed: Layout,
    changed_at: Instant,
    next_attempt: Instant,
}
impl LayoutStore {
    pub fn open(config: &ConnectionConfig) -> anyhow::Result<Self> {
        Self::open_identity(Identity::from_config(config))
    }
    pub fn open_session(session: &crate::account::SessionIdentity) -> anyhow::Result<Self> {
        Self::open_identity(Identity {
            host: session.endpoint.host.to_ascii_lowercase(),
            login_port: session.endpoint.login_port,
            world_port: session.endpoint.world_port,
            server_id: Some(session.server_id),
            character: session.character.to_ascii_lowercase(),
        })
    }
    fn open_identity(identity: Identity) -> anyhow::Result<Self> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .context("no configuration directory")?;
        Self::open_at(&base.join("openeq/layouts"), identity)
    }
    fn open_at(base: &Path, identity: Identity) -> anyhow::Result<Self> {
        let path = base.join(identity.filename());
        let saved = match fs::metadata(&path) {
            Ok(meta) => {
                ensure!(
                    meta.len() <= MAX_LAYOUT_BYTES as u64,
                    "saved UI layout exceeds size limit"
                );
                let mut bytes = Vec::new();
                fs::File::open(&path)?
                    .take(MAX_LAYOUT_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() <= MAX_LAYOUT_BYTES,
                    "saved UI layout exceeds size limit"
                );
                decode_document(&bytes, &identity)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Layout::default(),
            Err(error) => return Err(error.into()),
        };
        let now = Instant::now();
        Ok(Self {
            path,
            identity,
            observed: saved.clone(),
            saved,
            changed_at: now,
            next_attempt: now,
        })
    }
    pub fn layout(&self) -> &Layout {
        &self.saved
    }
    /// Coalesce dragging/scrolling, but flush on release or clean shutdown.
    /// Failed writes retain the previous file and retry with a bounded delay.
    pub fn update(&mut self, layout: Layout, now: Instant, flush: bool) -> anyhow::Result<bool> {
        if self.observed != layout {
            self.observed = layout;
            self.changed_at = now;
        }
        if self.saved == self.observed
            || (!flush
                && (now < self.next_attempt
                    || now.saturating_duration_since(self.changed_at) < Duration::from_millis(750)))
        {
            return Ok(false);
        }
        self.next_attempt = now + Duration::from_secs(5);
        self.observed.validate()?;
        let document = Document {
            version: 1,
            identity: self.identity.clone(),
            layout: self.observed.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&document)?;
        ensure!(
            bytes.len() <= MAX_LAYOUT_BYTES,
            "saved UI layout exceeds size limit"
        );
        atomic_write(&self.path, &bytes)?;
        self.saved = self.observed.clone();
        Ok(true)
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("layout parent")?;
    fs::create_dir_all(parent)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temporary = parent.join(format!(".layout-{}-{stamp}.tmp", std::process::id()));
    let result = (|| -> anyhow::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity(character: &str) -> Identity {
        Identity {
            host: "fixture.invalid".into(),
            login_port: 5999,
            world_port: 9000,
            server_id: None,
            character: character.into(),
        }
    }
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            Self(std::env::temp_dir().join(format!(
                    "openeq-layout-{}-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn layout_survives_restart_and_isolated_characters_keep_separate_positions() {
        let temp = Temp::new();
        let mut store = LayoutStore::open_at(&temp.0, identity("a")).unwrap();
        let mut layout = Layout::default();
        layout.windows.insert("inventory".into(), [420., 210.]);
        layout.window_order = vec!["chat".into(), "inventory".into(), "map".into()];
        layout.map_origin = [400., 100.];
        layout.map_zoom = 8.;
        for (index, button) in layout.hotbuttons.iter_mut().enumerate() {
            *button = validate_hotbutton(&format!("Button {}", index + 1), "/loc").unwrap();
        }
        assert!(store.update(layout.clone(), Instant::now(), true).unwrap());
        assert_eq!(
            LayoutStore::open_at(&temp.0, identity("a"))
                .unwrap()
                .layout(),
            &layout
        );
        assert_eq!(
            LayoutStore::open_at(&temp.0, identity("b"))
                .unwrap()
                .layout(),
            &Layout::default()
        );
        let mut map = MapState::default();
        let mut windows = BTreeMap::new();
        let mut stack = WindowStack::default();
        let mut hotbuttons = HotbuttonBindings::default();
        layout.apply(&mut windows, &mut stack, &mut map, &mut hotbuttons);
        assert_eq!(
            Layout::capture(&windows, stack.order(), &map, &hotbuttons),
            layout
        );
        windows.insert("skills".into(), [101., 102.]);
        assert_eq!(
            Layout::capture(&windows, stack.order(), &map, &hotbuttons).hotbuttons,
            layout.hotbuttons
        );
        assert!(map.center.is_none() && map.waypoints.is_empty());
    }
    #[test]
    fn dragging_is_coalesced_and_release_flushes_latest_position() {
        let temp = Temp::new();
        let mut store = LayoutStore::open_at(&temp.0, identity("a")).unwrap();
        let now = Instant::now();
        let mut layout = Layout::default();
        layout.windows.insert("loot".into(), [100., 100.]);
        assert!(!store.update(layout.clone(), now, false).unwrap());
        assert!(!store.path.exists());
        layout.windows.insert("loot".into(), [200., 100.]);
        assert!(
            !store
                .update(layout.clone(), now + Duration::from_millis(600), false)
                .unwrap()
        );
        assert!(
            store
                .update(layout.clone(), now + Duration::from_millis(700), true)
                .unwrap()
        );
        assert!(
            !store
                .update(layout, now + Duration::from_secs(2), false)
                .unwrap()
        );
    }

    #[test]
    fn position_only_version_one_loads_and_gains_order_without_losing_preferences() {
        let temp = Temp::new();
        fs::create_dir_all(&temp.0).unwrap();
        let expected = identity("old-layout");
        let document = Document {
            version: 1,
            identity: expected.clone(),
            layout: Layout {
                map_zoom: 4.,
                ..Default::default()
            },
        };
        let mut json = serde_json::to_value(&document).unwrap();
        json["layout"]
            .as_object_mut()
            .unwrap()
            .remove("window_order");
        json["layout"].as_object_mut().unwrap().remove("hotbuttons");
        fs::write(
            temp.0.join(expected.filename()),
            serde_json::to_vec(&json).unwrap(),
        )
        .unwrap();
        let mut store = LayoutStore::open_at(&temp.0, expected.clone()).unwrap();
        assert!(store.layout().window_order.is_empty());
        assert!(store.layout().hotbuttons.iter().all(Option::is_none));
        let mut layout = store.layout().clone();
        layout.window_order = vec!["map".into(), "bag:23".into(), "inventory".into()];
        assert!(store.update(layout.clone(), Instant::now(), true).unwrap());
        assert_eq!(
            LayoutStore::open_at(&temp.0, expected).unwrap().layout(),
            &layout
        );
        assert_eq!(layout.map_zoom, 4.);
    }

    #[test]
    fn invalid_order_does_not_replace_the_last_saved_layout() {
        let temp = Temp::new();
        let mut store = LayoutStore::open_at(&temp.0, identity("order")).unwrap();
        let valid = Layout {
            window_order: vec!["inventory".into()],
            ..Default::default()
        };
        store.update(valid.clone(), Instant::now(), true).unwrap();
        let before = fs::read(&store.path).unwrap();
        for order in [
            vec!["map".into(), "map".into()],
            vec!["../file".into()],
            vec!["bag:-1".into()],
            vec!["bag:023".into()],
            (0..=MAX_SAVED_WINDOWS)
                .map(|id| format!("bag:{id}"))
                .collect(),
        ] {
            let mut invalid = valid.clone();
            invalid.window_order = order;
            assert!(store.update(invalid, Instant::now(), true).is_err());
            assert_eq!(fs::read(&store.path).unwrap(), before);
        }
    }
    #[test]
    fn invalid_save_and_corrupted_load_preserve_existing_file() {
        let temp = Temp::new();
        let mut store = LayoutStore::open_at(&temp.0, identity("a")).unwrap();
        let mut layout = Layout {
            map_zoom: 8.,
            ..Default::default()
        };
        store.update(layout.clone(), Instant::now(), true).unwrap();
        let before = fs::read(&store.path).unwrap();
        layout.windows.insert("inventory".into(), [f32::NAN, 0.]);
        assert!(store.update(layout, Instant::now(), true).is_err());
        assert_eq!(fs::read(&store.path).unwrap(), before);
        fs::write(&store.path, b"{broken").unwrap();
        assert!(LayoutStore::open_at(&temp.0, identity("a")).is_err());
        assert_eq!(fs::read(&store.path).unwrap(), b"{broken");
    }
    #[test]
    fn future_versions_and_identity_collisions_are_not_overwritten() {
        let temp = Temp::new();
        fs::create_dir_all(&temp.0).unwrap();
        let expected = identity("../../a");
        assert_eq!(Path::new(&expected.filename()).components().count(), 1);
        let path = temp.0.join(expected.filename());
        for document in [
            Document {
                version: 2,
                identity: expected.clone(),
                layout: Layout::default(),
            },
            Document {
                version: 1,
                identity: identity("b"),
                layout: Layout::default(),
            },
        ] {
            let bytes = serde_json::to_vec(&document).unwrap();
            fs::write(&path, &bytes).unwrap();
            assert!(LayoutStore::open_at(&temp.0, expected.clone()).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn malformed_hotbutton_slots_recover_neighbors_without_rewriting_on_load() {
        use serde_json::json;
        let temp = Temp::new();
        fs::create_dir_all(&temp.0).unwrap();
        let expected = identity("button-recovery");
        let path = temp.0.join(expected.filename());
        let mut document = serde_json::to_value(Document {
            version: 1,
            identity: expected.clone(),
            layout: Layout {
                map_zoom: 7.,
                window_order: vec!["inventory".into(), "map".into()],
                windows: BTreeMap::from([("inventory".into(), [14., 15.])]),
                ..Default::default()
            },
        })
        .unwrap();
        document["layout"]["hotbuttons"] = json!([
            {"label":" Sit ","command":" /sit "},
            {"label":"Private","command":"/unsupported-private-command"},
            null,
            {"label":"Camp","command":"/camp"},
            {"label":4,"command":"/quit"},
            {"label":"Missing"},
            {"label":"Nested","command":"/hotbutton 1"}
        ]);
        let original = serde_json::to_vec(&document).unwrap();
        fs::write(&path, &original).unwrap();
        let mut store = LayoutStore::open_at(&temp.0, expected.clone()).unwrap();
        let recovered = store.layout().clone();
        assert_eq!(recovered.map_zoom, 7.);
        assert_eq!(recovered.windows["inventory"], [14., 15.]);
        assert_eq!(recovered.window_order, ["inventory", "map"]);
        assert_eq!(
            recovered.hotbuttons[0],
            validate_hotbutton("Sit", "/sit").unwrap()
        );
        assert_eq!(
            recovered.hotbuttons[3],
            validate_hotbutton("Camp", "/camp").unwrap()
        );
        assert!(
            recovered
                .hotbuttons
                .iter()
                .enumerate()
                .all(|(index, value)| [0, 3].contains(&index) || value.is_none())
        );
        assert!(
            !store
                .update(recovered.clone(), Instant::now(), true)
                .unwrap()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        let mut moved = recovered.clone();
        moved.windows.insert("map".into(), [88., 99.]);
        assert!(store.update(moved.clone(), Instant::now(), true).unwrap());
        let written: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            written["layout"]["hotbuttons"].as_array().unwrap().len(),
            12
        );
        assert_eq!(written["layout"]["hotbuttons"][0]["command"], "/sit");
        assert!(written["layout"]["hotbuttons"][1].is_null());
        assert_eq!(
            LayoutStore::open_at(&temp.0, expected).unwrap().layout(),
            &moved
        );
    }

    #[test]
    fn malformed_hotbutton_field_does_not_discard_layout_or_relax_identity() {
        use serde_json::json;
        let expected = identity("shape-recovery");
        let mut document = serde_json::to_value(Document {
            version: 1,
            identity: expected.clone(),
            layout: Layout {
                map_zoom: 9.,
                ..Default::default()
            },
        })
        .unwrap();
        for value in [json!(null), json!("invalid"), json!({"arbitrary":"text"})] {
            document["layout"]["hotbuttons"] = value;
            let bytes = serde_json::to_vec(&document).unwrap();
            let recovered = decode_document(&bytes, &expected).unwrap();
            assert_eq!(recovered.map_zoom, 9.);
            assert!(recovered.hotbuttons.iter().all(Option::is_none));
            assert!(decode_document(&bytes, &identity("different-character")).is_err());
        }
        document["version"] = json!(2);
        assert!(decode_document(&serde_json::to_vec(&document).unwrap(), &expected).is_err());
    }

    #[test]
    fn invalid_hotbutton_save_preserves_last_valid_file() {
        use crate::hotbuttons::SavedHotbutton;
        let temp = Temp::new();
        let mut store = LayoutStore::open_at(&temp.0, identity("validation")).unwrap();
        let mut valid = Layout::default();
        valid.hotbuttons[0] = validate_hotbutton("Sit", "/sit").unwrap();
        store.update(valid.clone(), Instant::now(), true).unwrap();
        let before = fs::read(&store.path).unwrap();
        for (label, command) in [
            ("Bad", "/private-unknown-command"),
            ("", "/sit"),
            ("", ""),
            ("Padded", " /sit "),
            ("Recursive", "/hotbuttons"),
            ("Multiline", "/sit\n/quit"),
        ] {
            let mut invalid = valid.clone();
            invalid.hotbuttons[4] = Some(SavedHotbutton {
                label: label.into(),
                command: command.into(),
            });
            let error = store.update(invalid, Instant::now(), true).unwrap_err();
            assert!(!format!("{error:#}").contains("private-unknown-command"));
            assert_eq!(fs::read(&store.path).unwrap(), before);
            assert_eq!(store.layout(), &valid);
        }
    }

    #[test]
    fn failed_hotbutton_write_retains_pending_change_and_retries() {
        let temp = Temp::new();
        let mut store = LayoutStore::open_at(&temp.0, identity("retry")).unwrap();
        let now = Instant::now();
        let mut changed = Layout::default();
        changed.hotbuttons[11] = validate_hotbutton("Last", "/loc").unwrap();
        // A file at the future parent path produces a real atomic-write error.
        fs::write(&temp.0, b"blocked parent").unwrap();
        assert!(store.update(changed.clone(), now, true).is_err());
        assert_eq!(store.layout(), &Layout::default());
        assert_eq!(store.observed, changed);
        fs::remove_file(&temp.0).unwrap();
        assert!(
            !store
                .update(changed.clone(), now + Duration::from_secs(4), false)
                .unwrap()
        );
        assert!(
            store
                .update(changed.clone(), now + Duration::from_secs(6), false)
                .unwrap()
        );
        assert_eq!(store.layout(), &changed);
        assert_eq!(
            LayoutStore::open_at(&temp.0, identity("retry"))
                .unwrap()
                .layout(),
            &changed
        );
    }

    #[test]
    fn oversized_layout_remains_unread_and_unchanged() {
        let temp = Temp::new();
        fs::create_dir_all(&temp.0).unwrap();
        let expected = identity("large");
        let path = temp.0.join(expected.filename());
        let bytes = vec![b' '; MAX_LAYOUT_BYTES + 1];
        fs::write(&path, &bytes).unwrap();
        assert!(LayoutStore::open_at(&temp.0, expected).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
