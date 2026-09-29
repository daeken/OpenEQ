//! Local presentation preferences, scoped to a world and character. No account
//! credentials, inventory, open commerce sessions or world waypoints are saved.
use crate::map::MapState;
use anyhow::{Context, ensure};
use openeq_net::session::ConnectionConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub windows: BTreeMap<String, [f32; 2]>,
    pub map_origin: [f32; 2],
    pub map_zoom: f32,
    pub map_labels: bool,
    pub map_layers: [bool; 4],
}
impl Default for Layout {
    fn default() -> Self {
        Self::capture(&BTreeMap::new(), &MapState::default())
    }
}
impl Layout {
    pub fn capture(windows: &BTreeMap<String, [f32; 2]>, map: &MapState) -> Self {
        Self {
            windows: windows.clone(),
            map_origin: [map.rect.x, map.rect.y],
            map_zoom: map.units_per_pixel,
            map_labels: map.show_labels,
            map_layers: map.layers,
        }
    }
    pub fn apply(&self, windows: &mut BTreeMap<String, [f32; 2]>, map: &mut MapState) {
        *windows = self.windows.clone();
        [map.rect.x, map.rect.y] = self.map_origin;
        map.units_per_pixel = self.map_zoom;
        map.show_labels = self.map_labels;
        map.layers = self.map_layers;
    }
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.windows.len() <= 256, "too many saved windows");
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
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .context("no configuration directory")?;
        Self::open_at(&base.join("openeq/layouts"), Identity::from_config(config))
    }
    fn open_at(base: &Path, identity: Identity) -> anyhow::Result<Self> {
        let path = base.join(identity.filename());
        let saved = match fs::metadata(&path) {
            Ok(meta) => {
                ensure!(meta.len() <= 65536, "saved UI layout exceeds size limit");
                let document: Document =
                    serde_json::from_slice(&fs::read(&path)?).context("reading saved UI layout")?;
                ensure!(document.version == 1, "unsupported UI layout version");
                ensure!(document.identity == identity, "UI layout identity mismatch");
                document.layout.validate()?;
                document.layout
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
        atomic_write(&self.path, &serde_json::to_vec_pretty(&document)?)?;
        self.saved = self.observed.clone();
        Ok(true)
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
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
        layout.map_origin = [400., 100.];
        layout.map_zoom = 8.;
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
        layout.apply(&mut windows, &mut map);
        assert_eq!(Layout::capture(&windows, &map), layout);
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
}
