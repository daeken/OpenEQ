//! Nonsecret endpoint/selection preferences. No username, password, session key
//! or account ID is ever serialized here.
use super::{Endpoint, SessionIdentity};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub endpoint: Endpoint,
    pub last_server: Option<u32>,
    pub last_character: Option<String>,
}
impl Preferences {
    pub fn for_session(identity: &SessionIdentity) -> Self {
        Self {
            endpoint: identity.endpoint.clone(),
            last_server: Some(identity.server_id),
            last_character: Some(identity.character.clone()),
        }
    }
    fn validate(&self) -> Result<()> {
        self.endpoint.validate()?;
        if let Some(name) = &self.last_character {
            ensure!(
                !name.is_empty() && name.len() <= 64 && !name.chars().any(char::is_control),
                "invalid saved character selection"
            );
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    preferences: Preferences,
}
pub struct PreferenceStore {
    path: PathBuf,
    pub preferences: Preferences,
}
impl PreferenceStore {
    pub fn open() -> Result<Self> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".config")))
            .context("no configuration directory")?;
        Self::open_at(base.join("openeq/connection.json"))
    }
    fn open_at(path: PathBuf) -> Result<Self> {
        let preferences = match std::fs::File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(8193).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() <= 8192,
                    "connection preferences exceed size limit"
                );
                let doc: Document = serde_json::from_slice(&bytes)?;
                ensure!(
                    doc.version == 1,
                    "unsupported connection preferences version"
                );
                doc.preferences.validate()?;
                doc.preferences
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, preferences })
    }
    pub fn save_session(&mut self, identity: &SessionIdentity) -> Result<()> {
        let preferences = Preferences::for_session(identity);
        if preferences == self.preferences {
            return Ok(());
        }
        save(&self.path, &preferences)?;
        self.preferences = preferences;
        Ok(())
    }
}
fn save(path: &Path, preferences: &Preferences) -> Result<()> {
    preferences.validate()?;
    crate::ui_layout::atomic_write(
        path,
        &serde_json::to_vec_pretty(&Document {
            version: 1,
            preferences: preferences.clone(),
        })?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_roundtrip_only_public_identity_and_preserve_future_data() {
        let path = std::env::temp_dir().join(format!(
            "openeq-connection-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = PreferenceStore::open_at(path.clone()).unwrap();
        let identity = SessionIdentity {
            endpoint: Endpoint {
                host: "eq.example".into(),
                ..Default::default()
            },
            server_id: 42,
            character: "Fixture".into(),
        };
        store.save_session(&identity).unwrap();
        assert_eq!(
            PreferenceStore::open_at(path.clone()).unwrap().preferences,
            Preferences::for_session(&identity)
        );
        let doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(doc["preferences"].as_object().unwrap().len(), 3);
        let future = br#"{"version":2,"preferences":{}}"#;
        std::fs::write(&path, future).unwrap();
        assert!(PreferenceStore::open_at(path.clone()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), future);
        std::fs::remove_file(path).unwrap();
    }
}
