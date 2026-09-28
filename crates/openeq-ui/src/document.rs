use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::Rect;

#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("cannot read UI file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid UI XML in {path}: {message}")]
    Xml { path: PathBuf, message: String },
    #[error("UI reference {0:?} was not found")]
    Missing(String),
    #[error("cyclic UI reference: {0}")]
    Cycle(String),
    #[error("UI path must stay inside the skin directory: {0}")]
    InvalidPath(String),
}

/// A SIDL element. Unrecognized properties are retained for future widget types.
#[derive(Clone, Debug, Default)]
pub struct Element {
    pub kind: String,
    pub item: String,
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.kind == name)
    }
    pub fn value(&self, name: &str) -> Option<&str> {
        self.child(name).map(|child| child.text.as_str())
    }
    pub fn values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.children
            .iter()
            .filter(move |child| child.kind == name)
            .map(|child| child.text.as_str())
    }
    pub fn number(&self, name: &str, default: f32) -> f32 {
        self.value(name)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(default)
    }
    pub fn boolean(&self, name: &str, default: bool) -> bool {
        match self.value(name) {
            Some(value) if value.eq_ignore_ascii_case("true") || value == "1" => true,
            Some(value) if value.eq_ignore_ascii_case("false") || value == "0" => false,
            _ => default,
        }
    }
    fn from_xml(node: roxmltree::Node<'_, '_>) -> Self {
        Self {
            kind: node.tag_name().name().to_owned(),
            item: node.attribute("item").unwrap_or_default().to_owned(),
            text: node.text().unwrap_or_default().trim().to_owned(),
            children: node
                .children()
                .filter(|child| child.is_element())
                .map(Self::from_xml)
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextureInfo {
    pub name: String,
    pub path: PathBuf,
    /// Authored atlas dimensions (not inferred from the image decoder).
    pub size: [u32; 2],
}

#[derive(Clone, Debug)]
pub struct UiImageFrame {
    pub texture: String,
    pub source: Rect,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct UiAnimation {
    pub frames: Vec<UiImageFrame>,
    pub cycle: bool,
}

impl UiAnimation {
    pub fn frame(&self, time_ms: u64, index: Option<usize>) -> Option<&UiImageFrame> {
        if let Some(index) = index {
            return self.frames.get(index);
        }
        let total: u64 = self.frames.iter().map(|frame| frame.duration_ms).sum();
        if total == 0 {
            return self.frames.first();
        }
        let mut time = if self.cycle {
            time_ms % total
        } else {
            time_ms.min(total - 1)
        };
        for frame in &self.frames {
            if time < frame.duration_ms {
                return Some(frame);
            }
            time -= frame.duration_ms;
        }
        self.frames.last()
    }
}

/// A named definition registry assembled in include order. Later definitions win,
/// matching custom skins that replace a shared definition.
#[derive(Debug, Default)]
pub struct UiDocument {
    pub definitions: BTreeMap<String, Element>,
    pub textures: BTreeMap<String, TextureInfo>,
    pub animations: BTreeMap<String, UiAnimation>,
    pub source_files: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub(crate) directory: PathBuf,
}

impl UiDocument {
    /// Loads a composite such as `EQUI.xml` or `EQLSUI.xml`, resolving includes
    /// relative to the skin directory, with case-insensitive filename fallback.
    pub fn load(directory: impl AsRef<Path>, entry: impl AsRef<Path>) -> Result<Self, UiError> {
        let directory = directory
            .as_ref()
            .canonicalize()
            .map_err(|source| UiError::Io {
                path: directory.as_ref().to_owned(),
                source,
            })?;
        let mut document = Self {
            directory,
            ..Default::default()
        };
        document.load_file(entry.as_ref(), &mut BTreeSet::new(), &mut BTreeSet::new())?;
        document.compile();
        Ok(document)
    }

    /// Parses an in-memory document without loading includes. Useful for generated
    /// overlays and tests. An Include requires `load` to provide its file context.
    pub fn from_xml(xml: &str) -> Result<Self, UiError> {
        let path = PathBuf::from("<memory>");
        let nodes = parse(xml, &path)?;
        let mut document = Self::default();
        for node in nodes {
            if node.kind == "Composite" && node.child("Include").is_some() {
                return Err(UiError::Missing(
                    "includes require UiDocument::load".to_owned(),
                ));
            }
            document.insert(node);
        }
        document.compile();
        Ok(document)
    }

    pub fn definition(&self, item: &str) -> Option<&Element> {
        self.definitions.get(item).or_else(|| {
            // SIDL permits a concrete class prefix in references, for example
            // `TileLayoutBox:Target_Buttons`, while the item stays unqualified.
            let (kind, name) = item.split_once(':')?;
            self.definitions
                .get(name)
                .filter(|element| element.kind == kind)
        })
    }

    /// Returns an existing case-correct path when possible; missing image paths
    /// are retained so a renderer can report a missing texture accurately.
    pub fn texture_path(&self, name: &str) -> PathBuf {
        self.resolve(Path::new(name))
            .unwrap_or_else(|_| self.directory.join(name))
    }

    pub fn screen_names(&self) -> impl Iterator<Item = &str> {
        self.definitions
            .values()
            .filter(|element| element.kind == "Screen")
            .map(|element| element.item.as_str())
    }

    fn load_file(
        &mut self,
        name: &Path,
        active: &mut BTreeSet<PathBuf>,
        loaded: &mut BTreeSet<PathBuf>,
    ) -> Result<(), UiError> {
        let path = self.resolve(name)?;
        if active.contains(&path) {
            return Err(UiError::Cycle(path.display().to_string()));
        }
        if loaded.contains(&path) {
            return Ok(());
        }
        if active.len() >= 128 {
            return Err(UiError::Cycle(
                "include nesting exceeds 128 files".to_owned(),
            ));
        }
        active.insert(path.clone());
        let bytes = fs::read(&path).map_err(|source| UiError::Io {
            path: path.clone(),
            source,
        })?;
        // Historical skins are often Windows-1252 despite an ASCII declaration.
        let xml = match std::str::from_utf8(&bytes) {
            Ok(xml) => std::borrow::Cow::Borrowed(xml),
            Err(_) => encoding_rs::WINDOWS_1252.decode(&bytes).0,
        };
        for element in parse(xml.trim_start_matches('\u{feff}'), &path)? {
            if element.kind == "Composite" {
                for include in element.values("Include") {
                    self.load_file(Path::new(include), active, loaded)?;
                }
            } else {
                self.insert(element);
            }
        }
        active.remove(&path);
        loaded.insert(path.clone());
        self.source_files.push(path);
        Ok(())
    }

    fn resolve(&self, name: &Path) -> Result<PathBuf, UiError> {
        let name = PathBuf::from(name.to_string_lossy().replace('\\', "/"));
        if name.is_absolute()
            || name
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(UiError::InvalidPath(name.display().to_string()));
        }
        let mut current = self.directory.clone();
        for part in name.components() {
            let exact = current.join(part.as_os_str());
            if exact.exists() {
                current = exact;
                continue;
            }
            let wanted = part.as_os_str().to_string_lossy();
            let found = fs::read_dir(&current)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&wanted)
                });
            current = found.map(|entry| entry.path()).unwrap_or(exact);
        }
        if let Ok(canonical) = current.canonicalize() {
            if !canonical.starts_with(&self.directory) {
                return Err(UiError::InvalidPath(name.display().to_string()));
            }
            return Ok(canonical);
        }
        Ok(current)
    }

    fn insert(&mut self, element: Element) {
        if element.item.is_empty() {
            return;
        }
        if let Some(previous) = self
            .definitions
            .insert(element.item.clone(), element.clone())
            && previous.kind != element.kind
        {
            self.warnings.push(format!(
                "{} changed type from {} to {}",
                element.item, previous.kind, element.kind
            ));
        }
    }

    fn compile(&mut self) {
        for (name, element) in &self.definitions {
            match element.kind.as_str() {
                "TextureInfo" => {
                    let size = element.child("Size");
                    self.textures.insert(
                        name.clone(),
                        TextureInfo {
                            name: name.clone(),
                            path: self.texture_path(name),
                            size: [
                                size.map_or(0., |s| s.number("CX", 0.)).max(0.) as u32,
                                size.map_or(0., |s| s.number("CY", 0.)).max(0.) as u32,
                            ],
                        },
                    );
                }
                "Ui2DAnimation" => {
                    let frames = element
                        .children
                        .iter()
                        .filter(|child| child.kind == "Frames")
                        .filter_map(|frame| {
                            let texture = frame.value("Texture")?.to_owned();
                            let location = frame.child("Location");
                            let size = frame.child("Size");
                            Some(UiImageFrame {
                                texture,
                                source: Rect::new(
                                    location.map_or(0., |p| p.number("X", 0.)),
                                    location.map_or(0., |p| p.number("Y", 0.)),
                                    size.map_or(0., |s| s.number("CX", 0.)).max(0.),
                                    size.map_or(0., |s| s.number("CY", 0.)).max(0.),
                                ),
                                duration_ms: frame.number("Duration", 1000.).max(1.) as u64,
                            })
                        })
                        .collect();
                    self.animations.insert(
                        name.clone(),
                        UiAnimation {
                            frames,
                            cycle: element.boolean("Cycle", false),
                        },
                    );
                }
                _ => {}
            }
        }
    }
}

fn parse(xml: &str, path: &Path) -> Result<Vec<Element>, UiError> {
    let document = roxmltree::Document::parse(xml).map_err(|error| UiError::Xml {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    Ok(document
        .root_element()
        .children()
        .filter(|node| node.is_element())
        .map(Element::from_xml)
        .collect())
}
