use crate::{Element, UiDocument, UiError, UiImageFrame};
use std::{collections::HashMap, path::PathBuf};

pub type Color = [u8; 4];
const WHITE: Color = [255, 255, 255, 255];

/// Screen-space logical pixels, with a top-left origin.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn right(self) -> f32 {
        self.x + self.width
    }
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
    pub fn contains(self, point: [f32; 2]) -> bool {
        point[0] >= self.x
            && point[1] >= self.y
            && point[0] < self.right()
            && point[1] < self.bottom()
    }
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self::new(
            x,
            y,
            (self.right().min(other.right()) - x).max(0.),
            (self.bottom().min(other.bottom()) - y).max(0.),
        )
    }
    pub fn is_empty(self) -> bool {
        self.width <= 0. || self.height <= 0.
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Commands are already ordered back-to-front. Apply `clip` to every command.
/// Images carry pixel source rectangles: divide by the decoded texture size for
/// UVs (the authored `texture_size` is available as a hint, and can be zero).
#[derive(Clone, Debug)]
pub enum DrawCommand {
    /// A screen-space line segment, batched as two triangles by the renderer.
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        clip: Rect,
        color: Color,
    },
    /// Colored, word-wrapped rows, anchored to the bottom of a scrollable log.
    TextLog {
        rect: Rect,
        clip: Rect,
        lines: Vec<TextLine>,
        font: u32,
        scroll_rows: usize,
    },
    Fill {
        rect: Rect,
        clip: Rect,
        color: Color,
    },
    Image {
        rect: Rect,
        clip: Rect,
        texture: PathBuf,
        source: Rect,
        texture_size: [u32; 2],
        tint: Color,
    },
    Text {
        rect: Rect,
        clip: Rect,
        text: String,
        font: u32,
        color: Color,
        align: TextAlign,
        vertical_center: bool,
        wrap: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLine {
    pub text: String,
    pub color: Color,
    pub links: Vec<TextLink>,
}

/// Link metadata names a visible UTF-8 range; the application owns its payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLink {
    pub range: std::ops::Range<usize>,
    pub id: u64,
}

#[derive(Clone, Debug)]
pub struct HitTarget {
    pub item: String,
    pub screen_id: String,
    /// Optional application-owned logical window; independent of XML ScreenID.
    pub window_id: Option<String>,
    pub kind: String,
    pub rect: Rect,
    pub enabled: bool,
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct UiFrame {
    pub commands: Vec<DrawCommand>,
    pub hit_targets: Vec<HitTarget>,
    pub bounds: Rect,
    /// Missing optional animation/template references and unsupported widgets.
    pub warnings: Vec<String>,
}
impl UiFrame {
    pub fn hit_test(&self, point: [f32; 2]) -> Option<&HitTarget> {
        self.hit_targets
            .iter()
            .rev()
            .find(|hit| hit.enabled && hit.rect.contains(point))
    }
}

/// Overrides a widget by its `item` name or `ScreenID`. Values are never inferred
/// from a control name: the caller must explicitly mask a password edit control.
#[derive(Clone, Debug, Default)]
pub struct WidgetState {
    pub text: Option<String>,
    /// Fraction in [0, 1]; invalid values are treated as zero.
    pub gauge: Option<f32>,
    pub visible: Option<bool>,
    pub enabled: Option<bool>,
    pub hovered: bool,
    pub pressed: bool,
    pub checked: bool,
    pub password: bool,
    /// Absolute layout override, useful for repositioning/resizing a window.
    pub rect: Option<Rect>,
    /// Explicit animation cell/frame selection for game-controlled indicators.
    pub frame_index: Option<usize>,
}

/// Dynamic values supplied by the application. Numeric EQType values are kept as
/// strings because current UI files also use symbolic values.
#[derive(Clone, Debug, Default)]
pub struct UiBindings {
    pub widgets: HashMap<String, WidgetState>,
    pub eq_text: HashMap<String, String>,
    pub eq_gauges: HashMap<String, f32>,
    pub time_ms: u64,
}
impl UiBindings {
    pub fn widget_mut(&mut self, item_or_screen_id: impl Into<String>) -> &mut WidgetState {
        self.widgets.entry(item_or_screen_id.into()).or_default()
    }
}

#[derive(Debug)]
pub struct UiWindow<'a> {
    document: &'a UiDocument,
    root: &'a Element,
}

impl UiDocument {
    /// Checks that the requested window's pieces exist and have no recursive
    /// references. Other windows may contain unsupported/missing pieces.
    pub fn window(&self, item: &str) -> Result<UiWindow<'_>, UiError> {
        let root = self
            .definition(item)
            .ok_or_else(|| UiError::Missing(item.to_owned()))?;
        validate(self, root, &mut Vec::new())?;
        Ok(UiWindow {
            document: self,
            root,
        })
    }
}

impl UiWindow<'_> {
    pub fn item(&self) -> &str {
        &self.root.item
    }
    pub fn layout(&self, viewport: Rect, bindings: &UiBindings) -> UiFrame {
        let mut frame = UiFrame {
            bounds: widget_rect(
                self.root,
                viewport,
                viewport,
                state(self.root, bindings).and_then(|state| state.rect),
            ),
            ..Default::default()
        };
        self.draw(
            self.root, viewport, viewport, viewport, bindings, &mut frame,
        );
        frame.warnings.sort();
        frame.warnings.dedup();
        frame
    }

    fn draw(
        &self,
        element: &Element,
        parent: Rect,
        viewport: Rect,
        parent_clip: Rect,
        bindings: &UiBindings,
        output: &mut UiFrame,
    ) {
        let default = WidgetState::default();
        let state = state(element, bindings).unwrap_or(&default);
        if !state
            .visible
            .unwrap_or_else(|| element.boolean("AutoDraw", true))
        {
            return;
        }
        let rect = widget_rect(element, parent, viewport, state.rect);
        let clip = parent_clip.intersect(rect);
        if clip.is_empty() {
            return;
        }
        let kind = element.kind.as_str();
        let enabled = state.enabled.unwrap_or(true);
        let is_container = matches!(
            kind,
            "Screen"
                | "Page"
                | "TemplateContainer"
                | "LayoutBox"
                | "HorizontalLayoutBox"
                | "VerticalLayoutBox"
                | "TabBox"
        );
        if is_container
            || matches!(
                kind,
                "Button" | "Editbox" | "Listbox" | "Combobox" | "InvSlot" | "Slider"
            )
        {
            output.hit_targets.push(HitTarget {
                window_id: None,
                item: element.item.clone(),
                screen_id: element
                    .value("ScreenID")
                    .unwrap_or(&element.item)
                    .to_owned(),
                kind: element.kind.clone(),
                rect: clip,
                enabled,
                tooltip: element.value("TooltipReference").map(str::to_owned),
            });
        }
        let tint = color(element.child("BackgroundTextureTint"), WHITE);
        if let Some(template_name) = element.value("DrawTemplate") {
            if let Some(template) = self.document.definition(template_name) {
                if !element.boolean("Style_Transparent", false)
                    && let Some(background) = template.value("Background")
                {
                    self.texture(background, rect, clip, tint, output);
                }
                if element.boolean("Style_Border", false)
                    && let Some(border) = template.child("Border")
                {
                    self.border(border, rect, clip, tint, bindings, output);
                }
                if element.boolean("Style_Titlebar", false)
                    && let Some(titlebar) = template.child("Titlebar")
                {
                    let height = self
                        .part_height(titlebar, "Middle", bindings)
                        .max(self.part_height(titlebar, "Left", bindings))
                        .max(14.);
                    self.border(
                        titlebar,
                        Rect::new(rect.x, rect.y, rect.width, height),
                        clip,
                        tint,
                        bindings,
                        output,
                    );
                }
            } else {
                output
                    .warnings
                    .push(format!("missing window template {template_name}"));
            }
        } else if is_container && !element.boolean("Style_Transparent", false) {
            output.commands.push(DrawCommand::Fill {
                rect,
                clip,
                color: [20, 22, 28, 210],
            });
        }
        match kind {
            "StaticAnimation" => {
                if let Some(animation) = element.value("Animation") {
                    self.image(
                        animation,
                        rect,
                        clip,
                        tint,
                        bindings,
                        state.frame_index,
                        output,
                    );
                }
            }
            "Button" => {
                let template = element.child("ButtonDrawTemplate").or_else(|| {
                    element
                        .value("Template")
                        .and_then(|name| self.document.definition(name))
                });
                let pressed = state.pressed || state.checked;
                let key = match (enabled, pressed, state.hovered) {
                    (false, true, _) => "PressedDisabled",
                    (false, false, _) => "Disabled",
                    (true, true, true) => "PressedFlyby",
                    (true, true, false) => "Pressed",
                    (true, false, true) => "Flyby",
                    _ => "Normal",
                };
                if let Some(animation) = template.and_then(|t| {
                    t.value(key)
                        .or_else(|| if pressed { t.value("Pressed") } else { None })
                        .or_else(|| t.value("Normal"))
                }) {
                    self.image(
                        animation,
                        rect,
                        clip,
                        tint,
                        bindings,
                        state.frame_index,
                        output,
                    );
                } else {
                    output.commands.push(DrawCommand::Fill {
                        rect,
                        clip,
                        color: if pressed {
                            [70, 75, 85, 255]
                        } else {
                            [40, 45, 55, 255]
                        },
                    });
                }
                if let Some(decal) = template.and_then(|t| {
                    t.value(&format!("{key}Decal"))
                        .or_else(|| t.value("NormalDecal"))
                }) {
                    let decal_rect = rect_from_children(element, "DecalOffset", "DecalSize", rect);
                    self.image(
                        decal,
                        decal_rect,
                        clip,
                        WHITE,
                        bindings,
                        state.frame_index,
                        output,
                    );
                }
            }
            "Gauge" => self.gauge(element, rect, clip, bindings, state, output),
            "StaticFrame" => {
                if let Some(template) = element
                    .value("FrameTemplate")
                    .and_then(|name| self.document.definition(name))
                {
                    self.border(template, rect, clip, tint, bindings, output);
                }
            }
            "InvSlot" => {
                if let Some(animation) = element.value("Background") {
                    self.image(
                        animation,
                        rect,
                        clip,
                        tint,
                        bindings,
                        state.frame_index,
                        output,
                    );
                }
            }
            "Editbox" | "Listbox" | "Label" | "StaticText" | "STMLbox" | "Screen" | "Page"
            | "TemplateContainer" => {}
            _ => output.warnings.push(format!(
                "unsupported widget {} ({})",
                element.kind, element.item
            )),
        }
        let text = state
            .text
            .as_deref()
            .or_else(|| {
                element
                    .value("EQType")
                    .and_then(|eq| bindings.eq_text.get(eq).map(String::as_str))
            })
            .or_else(|| element.value("Text"));
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            let titlebar = kind == "Screen" && element.boolean("Style_Titlebar", false);
            if !is_container || titlebar {
                let text = if state.password {
                    "•".repeat(text.chars().count())
                } else {
                    text.to_owned()
                };
                let center = element.boolean(
                    if kind == "Button" {
                        "TextAlignCenter"
                    } else {
                        "AlignCenter"
                    },
                    kind == "Button" || titlebar,
                );
                let right = element.boolean(
                    if kind == "Button" {
                        "TextAlignRight"
                    } else {
                        "AlignRight"
                    },
                    false,
                );
                let mut text_rect = rect;
                text_rect.x += element.number("TextOffsetX", 0.);
                text_rect.y += element.number("TextOffsetY", 0.);
                if titlebar {
                    text_rect.height = 14.;
                }
                let text_color = if enabled {
                    color(element.child("TextColor"), WHITE)
                } else {
                    color(element.child("DisabledColor"), [128, 128, 128, 255])
                };
                output.commands.push(DrawCommand::Text {
                    rect: text_rect,
                    clip,
                    text,
                    font: element.number("Font", 3.) as u32,
                    color: text_color,
                    align: if right {
                        TextAlign::Right
                    } else if center {
                        TextAlign::Center
                    } else {
                        TextAlign::Left
                    },
                    vertical_center: element.boolean(
                        "TextAlignVCenter",
                        kind == "Button" || kind == "Editbox" || titlebar,
                    ),
                    wrap: !element.boolean("NoWrap", kind == "Editbox"),
                });
            }
        }
        // All references were validated when constructing the window. A TabBox
        // draws its first page by default; the caller can select another with
        // explicit visibility overrides on its pages.
        for name in element.values("Pieces") {
            if let Some(child) = self.document.definition(name) {
                self.draw(child, rect, viewport, clip, bindings, output);
            }
        }
        let pages: Vec<_> = element
            .values("Pages")
            .filter_map(|name| self.document.definition(name))
            .collect();
        let selected = pages
            .iter()
            .find(|page| state_for_visible(page, bindings) == Some(true))
            .copied()
            .or_else(|| pages.first().copied());
        if let Some(page) = selected {
            self.draw(page, rect, viewport, clip, bindings, output);
        }
    }

    fn gauge(
        &self,
        element: &Element,
        rect: Rect,
        clip: Rect,
        bindings: &UiBindings,
        state: &WidgetState,
        output: &mut UiFrame,
    ) {
        let value = state
            .gauge
            .or_else(|| {
                element
                    .value("EQType")
                    .and_then(|eq| bindings.eq_gauges.get(eq).copied())
            })
            .unwrap_or(0.);
        let value = if value.is_finite() {
            value.clamp(0., 1.)
        } else {
            0.
        };
        let template = element.child("GaugeDrawTemplate");
        let fill = template.and_then(|t| t.value("Fill"));
        let height = fill
            .and_then(|name| self.animation_frame(name, bindings, None))
            .map_or(10., |frame| frame.source.height);
        let bar = Rect::new(
            rect.x + element.number("GaugeOffsetX", 0.),
            rect.y + element.number("GaugeOffsetY", 16.),
            rect.width,
            height,
        );
        if let Some(background) = template.and_then(|t| t.value("Background")) {
            self.image(background, bar, clip, WHITE, bindings, None, output);
        }
        let fill_clip = clip.intersect(Rect {
            width: bar.width * value,
            ..bar
        });
        let tint = color(element.child("FillTint"), WHITE);
        if let Some(fill) = fill {
            self.image(fill, bar, fill_clip, tint, bindings, None, output);
        } else {
            output.commands.push(DrawCommand::Fill {
                rect: bar,
                clip: fill_clip,
                color: tint,
            });
        }
        if let Some(lines) = template.and_then(|t| t.value("Lines")) {
            self.image(
                lines,
                bar,
                if element.boolean("DrawLinesFill", false) {
                    fill_clip
                } else {
                    clip
                },
                color(element.child("LinesFillTint"), WHITE),
                bindings,
                None,
                output,
            );
        }
        for (key, right) in [("EndCapLeft", false), ("EndCapRight", true)] {
            if let Some(name) = template.and_then(|t| t.value(key))
                && let Some(frame) = self.animation_frame(name, bindings, None)
            {
                let end = Rect::new(
                    if right {
                        bar.right() - frame.source.width
                    } else {
                        bar.x
                    },
                    bar.y,
                    frame.source.width,
                    frame.source.height,
                );
                self.image(name, end, clip, WHITE, bindings, None, output);
            }
        }
    }

    fn animation_frame(
        &self,
        name: &str,
        bindings: &UiBindings,
        index: Option<usize>,
    ) -> Option<&UiImageFrame> {
        self.document
            .animations
            .get(name)?
            .frame(bindings.time_ms, index)
    }
    fn part_height(&self, template: &Element, part: &str, bindings: &UiBindings) -> f32 {
        template
            .value(part)
            .and_then(|name| self.animation_frame(name, bindings, None))
            .map_or(0., |frame| frame.source.height)
    }

    #[allow(clippy::too_many_arguments)]
    fn image(
        &self,
        name: &str,
        rect: Rect,
        clip: Rect,
        tint: Color,
        bindings: &UiBindings,
        index: Option<usize>,
        output: &mut UiFrame,
    ) {
        if clip.is_empty() || rect.is_empty() {
            return;
        }
        if let Some(frame) = self.animation_frame(name, bindings, index) {
            let texture = self.document.textures.get(&frame.texture);
            output.commands.push(DrawCommand::Image {
                rect,
                clip,
                texture: texture.map_or_else(
                    || self.document.texture_path(&frame.texture),
                    |texture| texture.path.clone(),
                ),
                source: frame.source,
                texture_size: texture.map_or([0, 0], |texture| texture.size),
                tint,
            });
        } else {
            output
                .warnings
                .push(format!("missing animation/frame {name}"));
        }
    }

    fn texture(&self, name: &str, rect: Rect, clip: Rect, tint: Color, output: &mut UiFrame) {
        let texture = self.document.textures.get(name);
        let size = texture.map_or([0, 0], |texture| texture.size);
        output.commands.push(DrawCommand::Image {
            rect,
            clip,
            texture: texture.map_or_else(
                || self.document.texture_path(name),
                |texture| texture.path.clone(),
            ),
            source: Rect::new(0., 0., size[0] as f32, size[1] as f32),
            texture_size: size,
            tint,
        });
    }

    fn border(
        &self,
        template: &Element,
        rect: Rect,
        clip: Rect,
        tint: Color,
        bindings: &UiBindings,
        output: &mut UiFrame,
    ) {
        let part = |name: &str| {
            template
                .value(name)
                .and_then(|name| self.animation_frame(name, bindings, None))
        };
        let width = |name| part(name).map_or(0., |frame| frame.source.width);
        let height = |name| part(name).map_or(0., |frame| frame.source.height);
        let left = width("Left")
            .max(width("TopLeft"))
            .max(width("BottomLeft"))
            .min(rect.width / 2.);
        let right = width("Right")
            .max(width("TopRight"))
            .max(width("BottomRight"))
            .min(rect.width / 2.);
        let top = height("Top")
            .max(height("TopLeft"))
            .max(height("TopRight"))
            .min(rect.height / 2.);
        let bottom = height("Bottom")
            .max(height("BottomLeft"))
            .max(height("BottomRight"))
            .min(rect.height / 2.);
        let parts = [
            (
                "Middle",
                Rect::new(
                    rect.x + left,
                    rect.y + top,
                    rect.width - left - right,
                    rect.height - top - bottom,
                ),
            ),
            (
                "Top",
                Rect::new(
                    rect.x + width("TopLeft"),
                    rect.y,
                    rect.width - width("TopLeft") - width("TopRight"),
                    height("Top"),
                ),
            ),
            (
                "Bottom",
                Rect::new(
                    rect.x + width("BottomLeft"),
                    rect.bottom() - height("Bottom"),
                    rect.width - width("BottomLeft") - width("BottomRight"),
                    height("Bottom"),
                ),
            ),
            (
                "Left",
                Rect::new(
                    rect.x,
                    rect.y + height("LeftTop"),
                    width("Left"),
                    rect.height - height("LeftTop") - height("LeftBottom"),
                ),
            ),
            (
                "Right",
                Rect::new(
                    rect.right() - width("Right"),
                    rect.y + height("RightTop"),
                    width("Right"),
                    rect.height - height("RightTop") - height("RightBottom"),
                ),
            ),
            (
                "TopLeft",
                Rect::new(rect.x, rect.y, width("TopLeft"), height("TopLeft")),
            ),
            (
                "TopRight",
                Rect::new(
                    rect.right() - width("TopRight"),
                    rect.y,
                    width("TopRight"),
                    height("TopRight"),
                ),
            ),
            (
                "BottomLeft",
                Rect::new(
                    rect.x,
                    rect.bottom() - height("BottomLeft"),
                    width("BottomLeft"),
                    height("BottomLeft"),
                ),
            ),
            (
                "BottomRight",
                Rect::new(
                    rect.right() - width("BottomRight"),
                    rect.bottom() - height("BottomRight"),
                    width("BottomRight"),
                    height("BottomRight"),
                ),
            ),
            (
                "LeftTop",
                Rect::new(rect.x, rect.y, width("LeftTop"), height("LeftTop")),
            ),
            (
                "RightTop",
                Rect::new(
                    rect.right() - width("RightTop"),
                    rect.y,
                    width("RightTop"),
                    height("RightTop"),
                ),
            ),
            (
                "LeftBottom",
                Rect::new(
                    rect.x,
                    rect.bottom() - height("LeftBottom"),
                    width("LeftBottom"),
                    height("LeftBottom"),
                ),
            ),
            (
                "RightBottom",
                Rect::new(
                    rect.right() - width("RightBottom"),
                    rect.bottom() - height("RightBottom"),
                    width("RightBottom"),
                    height("RightBottom"),
                ),
            ),
        ];
        for (part, bounds) in parts {
            if let Some(name) = template.value(part) {
                self.image(name, bounds, clip, tint, bindings, None, output);
            }
        }
    }
}

fn validate(
    document: &UiDocument,
    element: &Element,
    active: &mut Vec<String>,
) -> Result<(), UiError> {
    if active.contains(&element.item) || active.len() >= 128 {
        return Err(UiError::Cycle(element.item.clone()));
    }
    active.push(element.item.clone());
    for name in element.values("Pieces").chain(element.values("Pages")) {
        let child = document
            .definition(name)
            .ok_or_else(|| UiError::Missing(name.to_owned()))?;
        validate(document, child, active)?;
    }
    active.pop();
    Ok(())
}
fn state<'a>(element: &Element, bindings: &'a UiBindings) -> Option<&'a WidgetState> {
    bindings.widgets.get(&element.item).or_else(|| {
        element
            .value("ScreenID")
            .and_then(|id| bindings.widgets.get(id))
    })
}
fn state_for_visible(element: &Element, bindings: &UiBindings) -> Option<bool> {
    state(element, bindings).and_then(|state| state.visible)
}
fn color(element: Option<&Element>, default: Color) -> Color {
    element.map_or(default, |element| {
        [
            element.number("R", default[0] as f32).clamp(0., 255.) as u8,
            element.number("G", default[1] as f32).clamp(0., 255.) as u8,
            element.number("B", default[2] as f32).clamp(0., 255.) as u8,
            element.number("Alpha", default[3] as f32).clamp(0., 255.) as u8,
        ]
    })
}
fn rect_from_children(
    element: &Element,
    position_name: &str,
    size_name: &str,
    parent: Rect,
) -> Rect {
    let location = element.child(position_name);
    let size = element.child(size_name);
    Rect::new(
        parent.x + location.map_or(0., |p| p.number("X", 0.)),
        parent.y + location.map_or(0., |p| p.number("Y", 0.)),
        size.map_or(parent.width, |s| s.number("CX", parent.width)),
        size.map_or(parent.height, |s| s.number("CY", parent.height)),
    )
}
fn widget_rect(
    element: &Element,
    parent: Rect,
    viewport: Rect,
    override_rect: Option<Rect>,
) -> Rect {
    if let Some(rect) = override_rect {
        return rect;
    }
    let base = if element.boolean("RelativePosition", true) {
        parent
    } else {
        viewport
    };
    let mut rect = rect_from_children(
        element,
        "Location",
        "Size",
        Rect {
            width: 0.,
            height: 0.,
            ..base
        },
    );
    let all = element.boolean("AutoStretch", false);
    let coordinate =
        |offset, near: bool, origin, length| origin + if near { offset } else { length - offset };
    if all || element.boolean("AutoStretchHorizontal", false) {
        rect.x = coordinate(
            element.number("LeftAnchorOffset", 0.),
            element.boolean("LeftAnchorToLeft", true),
            base.x,
            base.width,
        );
        let right = coordinate(
            element.number("RightAnchorOffset", 0.),
            element.boolean("RightAnchorToLeft", true),
            base.x,
            base.width,
        );
        rect.width = (right - rect.x).max(0.);
    }
    if all || element.boolean("AutoStretchVertical", false) {
        rect.y = coordinate(
            element.number("TopAnchorOffset", 0.),
            element.boolean("TopAnchorToTop", true),
            base.y,
            base.height,
        );
        let bottom = coordinate(
            element.number("BottomAnchorOffset", 0.),
            element.boolean("BottomAnchorToTop", true),
            base.y,
            base.height,
        );
        rect.height = (bottom - rect.y).max(0.);
    }
    for (dimension, min_name, max_name) in [
        (&mut rect.width, "MinHSize", "MaxHSize"),
        (&mut rect.height, "MinVSize", "MaxVSize"),
    ] {
        *dimension = dimension.max(element.number(min_name, 0.));
        let maximum = element.number(max_name, 0.);
        if maximum > 0. {
            *dimension = dimension.min(maximum);
        }
    }
    rect
}
