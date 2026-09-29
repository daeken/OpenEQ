//! Client map-file decoding and a renderer-neutral, north-up minimap.
//!
//! EQ map text stores (-server_x, -server_y, server_z). Public positions use
//! scene/asset coordinates: (server_y, server_x, server_z). North is +scene_x.

use anyhow::{Context, bail};
use openeq_ui::{Color, DrawCommand, HitTarget, Rect, TextAlign, UiFrame};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct MapLine {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub color: Color,
    pub layer: u8,
}

#[derive(Clone, Debug)]
pub struct MapLabel {
    pub position: [f32; 3],
    pub text: String,
    pub color: Color,
    pub size: u8,
    pub layer: u8,
}

#[derive(Clone, Debug, Default)]
pub struct ZoneMap {
    pub zone: String,
    pub lines: Vec<MapLine>,
    pub labels: Vec<MapLabel>,
    pub source_files: Vec<PathBuf>,
    pub malformed_records: usize,
}

#[derive(Clone, Debug)]
pub struct MapMarker {
    pub position: [f32; 3],
    pub label: String,
    pub color: Color,
}
impl Default for MapMarker {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            label: String::new(),
            color: [255, 205, 85, 255],
        }
    }
}

#[derive(Clone, Debug)]
pub struct MapState {
    pub rect: Rect,
    pub player_position: [f32; 3],
    /// Camera yaw in radians: zero points +scene_y; north (+scene_x) is pi/2.
    pub heading: f32,
    pub units_per_pixel: f32,
    /// None follows the player; Some fixes the map center in world coordinates.
    pub center: Option<[f32; 2]>,
    pub target: Option<MapMarker>,
    pub waypoints: Vec<MapMarker>,
    pub pointer: Option<[f32; 2]>,
    pub show_labels: bool,
    pub layers: [bool; 4],
    /// Optional height slice either side of the player's Z coordinate.
    pub height_range: Option<f32>,
}
impl Default for MapState {
    fn default() -> Self {
        Self {
            rect: Rect::new(12., 120., 340., 340.),
            player_position: [0.; 3],
            heading: 0.,
            units_per_pixel: 4.,
            center: None,
            target: None,
            waypoints: Vec::new(),
            pointer: None,
            show_labels: true,
            layers: [true; 4],
            height_range: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapAction {
    Close,
    ZoomIn,
    ZoomOut,
    Recenter,
    BeginDrag,
    Canvas,
    Label(usize),
}
impl MapAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        match hit.item.as_str() {
            "map:close" => Some(Self::Close),
            "map:zoom_in" => Some(Self::ZoomIn),
            "map:zoom_out" => Some(Self::ZoomOut),
            "map:recenter" => Some(Self::Recenter),
            "map:drag" => Some(Self::BeginDrag),
            "map:canvas" => Some(Self::Canvas),
            id => id.strip_prefix("map:label:")?.parse().ok().map(Self::Label),
        }
    }
}

impl MapState {
    /// Keep saved/dragged controls reachable after a window or display resize.
    /// Retain the preferred 340px map size when growing the viewport again.
    pub fn fit_viewport(&mut self, viewport: [u32; 2]) {
        self.rect.width = 340_f32.min(viewport[0] as f32);
        self.rect.height = 340_f32.min(viewport[1] as f32);
        self.rect.x = self
            .rect
            .x
            .clamp(0., (viewport[0] as f32 - self.rect.width).max(0.));
        self.rect.y = self
            .rect
            .y
            .clamp(0., (viewport[1] as f32 - self.rect.height).max(0.));
    }

    pub fn canvas(&self) -> Rect {
        Rect::new(
            self.rect.x + 8.,
            self.rect.y + 28.,
            (self.rect.width - 16.).max(0.),
            (self.rect.height - 66.).max(0.),
        )
    }

    fn scale(&self) -> f32 {
        if self.units_per_pixel.is_finite() {
            self.units_per_pixel.clamp(0.05, 1000.)
        } else {
            4.
        }
    }

    fn center(&self) -> [f32; 2] {
        self.center
            .unwrap_or([self.player_position[0], self.player_position[1]])
    }

    pub fn screen_at(&self, position: [f32; 3]) -> [f32; 2] {
        let canvas = self.canvas();
        let center = self.center();
        [
            canvas.x + canvas.width * 0.5 + (position[1] - center[1]) / self.scale(),
            canvas.y + canvas.height * 0.5 - (position[0] - center[0]) / self.scale(),
        ]
    }

    /// Converts a canvas click into a scene waypoint at the player's height.
    pub fn world_at(&self, point: [f32; 2]) -> Option<[f32; 3]> {
        let canvas = self.canvas();
        if !canvas.contains(point) {
            return None;
        }
        let center = self.center();
        Some([
            center[0] - (point[1] - canvas.y - canvas.height * 0.5) * self.scale(),
            center[1] + (point[0] - canvas.x - canvas.width * 0.5) * self.scale(),
            self.player_position[2],
        ])
    }
}

impl ZoneMap {
    /// Accepts the client installation or its maps directory. Missing layers are
    /// normal; an entirely missing map returns a useful error for the caller.
    pub fn load(base: impl AsRef<Path>, zone: &str) -> anyhow::Result<Self> {
        if zone.is_empty() || !zone.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            bail!("invalid map zone name");
        }
        let base = base.as_ref();
        let directory = if base.join("maps").is_dir() {
            base.join("maps")
        } else {
            base.to_owned()
        };
        let files: HashMap<_, _> = std::fs::read_dir(&directory)
            .with_context(|| format!("reading map directory {}", directory.display()))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .map(|entry| {
                (
                    entry.file_name().to_string_lossy().to_ascii_lowercase(),
                    entry.path(),
                )
            })
            .collect();
        let mut map = Self {
            zone: zone.to_owned(),
            ..Default::default()
        };
        for layer in 0..4 {
            let filename = if layer == 0 {
                format!("{zone}.txt")
            } else {
                format!("{zone}_{layer}.txt")
            };
            let Some(path) = files.get(&filename.to_ascii_lowercase()) else {
                continue;
            };
            let bytes =
                std::fs::read(path).with_context(|| format!("reading map {}", path.display()))?;
            let text = match std::str::from_utf8(&bytes) {
                Ok(text) => std::borrow::Cow::Borrowed(text),
                Err(_) => encoding_rs::WINDOWS_1252.decode(&bytes).0,
            };
            map.add_layer(&text, layer);
            map.source_files.push(path.clone());
        }
        if map.source_files.is_empty() {
            bail!("no client map found for {zone} in {}", directory.display());
        }
        Ok(map)
    }

    pub fn parse(zone: impl Into<String>, text: &str) -> Self {
        let mut map = Self {
            zone: zone.into(),
            ..Default::default()
        };
        map.add_layer(text, 0);
        map
    }

    pub fn add_layer(&mut self, text: &str, layer: u8) {
        for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let Some((kind, fields)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            match kind {
                "L" => {
                    let fields: Vec<_> = fields.split(',').map(str::trim).collect();
                    let parsed = (|| {
                        if fields.len() != 9 {
                            return None;
                        }
                        Some(MapLine {
                            from: point(&fields[..3])?,
                            to: point(&fields[3..6])?,
                            color: color(&fields[6..9])?,
                            layer,
                        })
                    })();
                    if let Some(line) = parsed {
                        self.lines.push(line);
                    } else {
                        self.malformed_records += 1;
                    }
                }
                "P" => {
                    // A label may contain commas; only the seven numeric fields split.
                    let fields: Vec<_> = fields.splitn(8, ',').map(str::trim).collect();
                    let parsed = (|| {
                        if fields.len() != 8 {
                            return None;
                        }
                        let text = fields[7].replace('_', " ");
                        if text.is_empty() {
                            return None;
                        }
                        Some(MapLabel {
                            position: point(&fields[..3])?,
                            color: color(&fields[3..6])?,
                            size: fields[6].parse::<u8>().ok()?.clamp(1, 3),
                            text,
                            layer,
                        })
                    })();
                    if let Some(label) = parsed {
                        self.labels.push(label);
                    } else {
                        self.malformed_records += 1;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut points = self
            .lines
            .iter()
            .flat_map(|line| [line.from, line.to])
            .chain(self.labels.iter().map(|label| label.position));
        let first = points.next()?;
        let mut min = first;
        let mut max = first;
        for point in points {
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        Some((min, max))
    }

    /// Draws a standalone overlay; append it after the gameplay HUD so the map's
    /// controls receive the same topmost hit-test order as its visible pixels.
    pub fn frame(&self, viewport: [u32; 2], state: &MapState) -> UiFrame {
        let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
        let rect = state.rect;
        let canvas = state.canvas();
        let clip = canvas.intersect(screen);
        let mut frame = UiFrame {
            bounds: rect,
            ..Default::default()
        };
        fill(&mut frame, rect, screen, [12, 16, 24, 238]);
        outline(&mut frame, rect, screen, [176, 153, 100, 255]);
        hit(
            &mut frame,
            "map:window",
            "Map",
            rect.intersect(screen),
            None,
        );
        hit(
            &mut frame,
            "map:drag",
            "WindowTitle",
            Rect::new(rect.x, rect.y, rect.width, 25.).intersect(screen),
            None,
        );
        text(
            &mut frame,
            Rect::new(rect.x + 10., rect.y + 6., (rect.width - 112.).max(0.), 18.),
            screen,
            format!("Map · {}", self.zone),
            [228, 210, 160, 255],
            2,
        );
        for (index, (id, label, tip)) in [
            ("map:zoom_out", "−", "Zoom out"),
            ("map:zoom_in", "+", "Zoom in"),
            ("map:recenter", "N", "Follow player, north up"),
            ("map:close", "×", "Close map (M)"),
        ]
        .into_iter()
        .enumerate()
        {
            let bounds = Rect::new(
                rect.right() - 97. + index as f32 * 23.,
                rect.y + 4.,
                20.,
                18.,
            );
            let hovered = state.pointer.is_some_and(|p| bounds.contains(p));
            fill(
                &mut frame,
                bounds,
                screen,
                if hovered {
                    [68, 73, 83, 255]
                } else {
                    [32, 39, 51, 255]
                },
            );
            text(
                &mut frame,
                Rect::new(
                    bounds.x + 5.,
                    bounds.y + 1.,
                    bounds.width - 4.,
                    bounds.height,
                ),
                screen,
                label,
                [233, 228, 210, 255],
                2,
            );
            hit(
                &mut frame,
                id,
                "Button",
                bounds.intersect(screen),
                Some(tip.into()),
            );
        }
        fill(&mut frame, canvas, clip, [17, 25, 33, 255]);
        hit(&mut frame, "map:canvas", "MapCanvas", clip, None);
        for line in &self.lines {
            if !state
                .layers
                .get(line.layer as usize)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(range) = state.height_range
                && (line.from[2].min(line.to[2]) > state.player_position[2] + range
                    || line.from[2].max(line.to[2]) < state.player_position[2] - range)
            {
                continue;
            }
            let from = state.screen_at(line.from);
            let to = state.screen_at(line.to);
            if !segment_visible(from, to, clip) {
                continue;
            }
            let mut color = visible_color(line.color);
            let height = ((line.from[2] + line.to[2]) * 0.5 - state.player_position[2]).abs();
            color[3] = if height > 80. {
                70
            } else if height > 35. {
                125
            } else {
                225
            };
            line_command(&mut frame, from, to, 1., clip, color);
        }
        if self.lines.is_empty() {
            text(
                &mut frame,
                Rect::new(
                    canvas.x + 12.,
                    canvas.y + canvas.height * 0.5 - 18.,
                    canvas.width - 24.,
                    36.,
                ),
                clip,
                "No map lines available for this zone.",
                [177, 186, 196, 255],
                2,
            );
        }
        let mut occupied = Vec::<Rect>::new();
        for (index, label) in self.labels.iter().enumerate() {
            if !state
                .layers
                .get(label.layer as usize)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            if state
                .height_range
                .is_some_and(|range| (label.position[2] - state.player_position[2]).abs() > range)
            {
                continue;
            }
            let point = state.screen_at(label.position);
            if !clip.contains(point) {
                continue;
            }
            let label_color = visible_color(label.color);
            fill(
                &mut frame,
                Rect::new(point[0] - 1., point[1] - 1., 3., 3.),
                clip,
                label_color,
            );
            let mut target = Rect::new(point[0] - 4., point[1] - 4., 9., 9.);
            if state.show_labels {
                let bounds = Rect::new(
                    point[0] + 5.,
                    point[1] - 6.,
                    (label.text.chars().count() as f32 * 6.2).min(150.),
                    14.,
                );
                if bounds.right() <= clip.right()
                    && bounds.bottom() <= clip.bottom()
                    && occupied
                        .iter()
                        .all(|other| other.intersect(bounds).is_empty())
                {
                    fill(&mut frame, bounds, clip, [17, 25, 33, 220]);
                    text(&mut frame, bounds, clip, &label.text, label_color, 1);
                    occupied.push(bounds);
                    target.width += bounds.width + 5.;
                    target.height = 16.;
                }
            }
            hit(
                &mut frame,
                format!("map:label:{index}"),
                "MapLabel",
                target.intersect(clip),
                Some(label.text.clone()),
            );
        }
        for marker in &state.waypoints {
            draw_marker(&mut frame, state, marker, clip, true);
        }
        if let Some(target) = &state.target {
            draw_marker(&mut frame, state, target, clip, false);
        }
        let player = state.screen_at(state.player_position);
        if clip.contains(player) {
            let (sin, cos) = state.heading.sin_cos();
            let forward = [cos, -sin];
            let right = [sin, cos];
            let tip = [player[0] + forward[0] * 9., player[1] + forward[1] * 9.];
            let left = [
                player[0] - forward[0] * 5. - right[0] * 5.,
                player[1] - forward[1] * 5. - right[1] * 5.,
            ];
            let right = [
                player[0] - forward[0] * 5. + right[0] * 5.,
                player[1] - forward[1] * 5. + right[1] * 5.,
            ];
            for (a, b) in [(tip, left), (left, right), (right, tip)] {
                line_command(&mut frame, a, b, 2., clip, [100, 226, 255, 255]);
            }
            fill(
                &mut frame,
                Rect::new(player[0] - 1., player[1] - 1., 3., 3.),
                clip,
                [230, 255, 255, 255],
            );
        }
        text(
            &mut frame,
            Rect::new(canvas.x + canvas.width * 0.5 - 4., canvas.y + 3., 16., 16.),
            clip,
            "N",
            [215, 222, 225, 255],
            2,
        );
        let status = if let Some(waypoint) = state.waypoints.last() {
            let distance = (waypoint.position[0] - state.player_position[0])
                .hypot(waypoint.position[1] - state.player_position[1]);
            format!("{} · {:.0} units", waypoint.label, distance)
        } else {
            format!(
                "X {:.0}  Y {:.0}  Z {:.0}",
                state.player_position[1], state.player_position[0], state.player_position[2]
            )
        };
        text(
            &mut frame,
            Rect::new(rect.x + 10., rect.bottom() - 33., rect.width - 20., 15.),
            screen,
            status,
            [223, 210, 175, 255],
            1,
        );
        text(
            &mut frame,
            Rect::new(rect.x + 10., rect.bottom() - 18., rect.width - 20., 14.),
            screen,
            "Click to mark · Wheel to zoom · M to close",
            [151, 162, 174, 255],
            1,
        );
        frame
    }
}

fn point(fields: &[&str]) -> Option<[f32; 3]> {
    let point = [
        fields.first()?.parse::<f32>().ok()?,
        fields.get(1)?.parse::<f32>().ok()?,
        fields.get(2)?.parse::<f32>().ok()?,
    ];
    point
        .iter()
        .all(|n| n.is_finite())
        .then_some(crate::coordinates::server_point_to_scene([
            -point[0], -point[1], point[2],
        ]))
}
fn color(fields: &[&str]) -> Option<Color> {
    Some([
        fields.first()?.parse().ok()?,
        fields.get(1)?.parse().ok()?,
        fields.get(2)?.parse().ok()?,
        255,
    ])
}
fn visible_color(mut color: Color) -> Color {
    if color[0].max(color[1]).max(color[2]) < 110 {
        color = [155, 169, 180, color[3]];
    }
    color
}
fn segment_visible(from: [f32; 2], to: [f32; 2], clip: Rect) -> bool {
    from.iter().chain(&to).all(|n| n.is_finite())
        && from[0].max(to[0]) >= clip.x
        && from[0].min(to[0]) <= clip.right()
        && from[1].max(to[1]) >= clip.y
        && from[1].min(to[1]) <= clip.bottom()
}
fn draw_marker(
    frame: &mut UiFrame,
    state: &MapState,
    marker: &MapMarker,
    clip: Rect,
    waypoint: bool,
) {
    if clip.width < 16. || clip.height < 16. {
        return;
    }
    let raw = state.screen_at(marker.position);
    if !raw.iter().all(|n| n.is_finite()) {
        return;
    }
    let point = [
        raw[0].clamp(clip.x + 7., clip.right() - 7.),
        raw[1].clamp(clip.y + 7., clip.bottom() - 7.),
    ];
    if waypoint {
        let mut color = marker.color;
        color[3] = 100;
        line_command(
            frame,
            state.screen_at(state.player_position),
            point,
            1.,
            clip,
            color,
        );
    }
    let points = [
        [point[0], point[1] - 5.],
        [point[0] + 5., point[1]],
        [point[0], point[1] + 5.],
        [point[0] - 5., point[1]],
    ];
    for index in 0..4 {
        line_command(
            frame,
            points[index],
            points[(index + 1) % 4],
            2.,
            clip,
            marker.color,
        );
    }
    let label = Rect::new(
        (point[0] + 9.).min(clip.right() - 110.).max(clip.x),
        (point[1] + 8.).min(clip.bottom() - 14.),
        110.,
        14.,
    );
    text(frame, label, clip, &marker.label, marker.color, 1);
}
fn line_command(
    frame: &mut UiFrame,
    from: [f32; 2],
    to: [f32; 2],
    width: f32,
    clip: Rect,
    color: Color,
) {
    frame.commands.push(DrawCommand::Line {
        from,
        to,
        width,
        clip,
        color,
    });
}
fn fill(frame: &mut UiFrame, rect: Rect, clip: Rect, color: Color) {
    frame.commands.push(DrawCommand::Fill {
        rect,
        clip: rect.intersect(clip),
        color,
    });
}
fn outline(frame: &mut UiFrame, rect: Rect, clip: Rect, color: Color) {
    for (from, to) in [
        ([rect.x, rect.y], [rect.right(), rect.y]),
        ([rect.right(), rect.y], [rect.right(), rect.bottom()]),
        ([rect.right(), rect.bottom()], [rect.x, rect.bottom()]),
        ([rect.x, rect.bottom()], [rect.x, rect.y]),
    ] {
        line_command(frame, from, to, 1., clip, color);
    }
}
fn text(
    frame: &mut UiFrame,
    rect: Rect,
    clip: Rect,
    text: impl Into<String>,
    color: Color,
    font: u32,
) {
    frame.commands.push(DrawCommand::Text {
        rect,
        clip: rect.intersect(clip),
        text: text.into(),
        font,
        color,
        align: TextAlign::Left,
        vertical_center: false,
        wrap: false,
    });
}
fn hit(
    frame: &mut UiFrame,
    id: impl Into<String>,
    kind: &str,
    rect: Rect,
    tooltip: Option<String>,
) {
    if rect.is_empty() {
        return;
    }
    let id = id.into();
    frame.hit_targets.push(HitTarget {
        item: id.clone(),
        screen_id: id,
        kind: kind.to_owned(),
        rect,
        enabled: true,
        tooltip,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resized_saved_map_keeps_controls_reachable_and_waypoints_aligned() {
        let map = ZoneMap::default();
        let mut state = MapState {
            rect: Rect::new(2400., 1400., 340., 340.),
            ..Default::default()
        };
        for viewport in [[320, 240], [1280, 720]] {
            state.fit_viewport(viewport);
            let frame = map.frame(viewport, &state);
            for name in ["map:close", "map:zoom_in", "map:zoom_out", "map:recenter"] {
                let hit = frame
                    .hit_targets
                    .iter()
                    .find(|hit| hit.item == name)
                    .unwrap();
                assert!(!hit.rect.is_empty(), "invisible {name} at {viewport:?}");
                let point = [
                    hit.rect.x + hit.rect.width / 2.,
                    hit.rect.y + hit.rect.height / 2.,
                ];
                assert_eq!(frame.hit_test(point).unwrap().item, name);
            }
            let point = state.screen_at([12., 24., 0.]);
            assert_eq!(state.world_at(point), Some([12., 24., 0.]));
        }
        assert_eq!(state.rect.width, 340.);
    }

    #[test]
    fn parses_lines_labels_layers_and_rejects_invalid_numbers() {
        let mut map = ZoneMap::parse(
            "test",
            "\u{feff}# comment\r\nL 10,20,-3,30,40,5,0,128,255\r\nP 25,-50,7,255,0,0,3,Inn,_north_door\nL NaN,0,0,1,1,1,0,0,0\nP 0,0,0,256,0,0,1,Invalid\n",
        );
        assert_eq!(map.lines.len(), 1);
        assert_eq!(map.lines[0].from, [-20., -10., -3.]);
        assert_eq!(map.lines[0].to, [-40., -30., 5.]);
        assert_eq!(map.lines[0].color, [0, 128, 255, 255]);
        assert_eq!(map.labels[0].position, [50., -25., 7.]);
        assert_eq!(map.labels[0].text, "Inn, north door");
        assert_eq!(map.malformed_records, 2);
        map.add_layer("P 0,0,0,0,0,0,1,Layer_two", 2);
        assert_eq!(map.labels[1].layer, 2);
        assert_eq!(map.bounds().unwrap(), ([-40., -30., -3.], [50., -0., 7.]));
    }

    #[test]
    fn north_up_projection_round_trips_world_waypoints() {
        let mut state = MapState {
            rect: Rect::new(20., 30., 340., 340.),
            player_position: [100., 200., 30.],
            units_per_pixel: 2.,
            ..Default::default()
        };
        let center = state.screen_at(state.player_position);
        assert_eq!(state.world_at(center), Some(state.player_position));
        assert_eq!(
            state.screen_at([120., 200., 30.]),
            [center[0], center[1] - 10.]
        );
        assert_eq!(
            state.screen_at([100., 240., 30.]),
            [center[0] + 20., center[1]]
        );
        let point = [center[0] - 23., center[1] + 16.];
        assert_eq!(state.screen_at(state.world_at(point).unwrap()), point);
        assert!(
            state.world_at([21., 31.]).is_none(),
            "title must not create waypoints"
        );
        state.center = Some([0., 0.]);
        assert_eq!(state.world_at(center), Some([0., 0., 30.]));
        state.units_per_pixel = f32::NAN;
        assert!(state.screen_at([1., 2., 3.]).iter().all(|v| v.is_finite()));
    }

    #[test]
    fn player_arrow_uses_scene_yaw_on_a_north_up_map() {
        let map = ZoneMap::default();
        for (heading, direction) in [
            (std::f32::consts::FRAC_PI_2, [0., -1.]),
            (0., [1., 0.]),
            (std::f32::consts::PI, [-1., 0.]),
        ] {
            let state = MapState {
                player_position: [-315., 944., -93.625],
                heading,
                ..Default::default()
            };
            let player = state.screen_at(state.player_position);
            let frame = map.frame([640, 480], &state);
            let tip = frame
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawCommand::Line { from, color, .. } if *color == [100, 226, 255, 255] => {
                        Some(*from)
                    }
                    _ => None,
                })
                .unwrap();
            for axis in 0..2 {
                assert!((tip[axis] - player[axis] - direction[axis] * 9.).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn loads_case_insensitive_layers_and_client_encoding() {
        let directory =
            std::env::temp_dir().join(format!("openeq-map-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("TEST.txt"), "L 0,0,0,10,10,0,0,0,0").unwrap();
        std::fs::write(directory.join("test_2.TXT"), b"P 0,0,0,0,0,0,1,Caf\xe9").unwrap();
        let map = ZoneMap::load(&directory, "test").unwrap();
        assert_eq!(map.source_files.len(), 2);
        assert_eq!(map.labels[0].text, "Café");
        assert_eq!(map.labels[0].layer, 2);
        assert!(ZoneMap::load(&directory, "missing").is_err());
        assert!(ZoneMap::load(&directory, "../test").is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn frame_clips_map_and_controls_take_precedence() {
        let map = ZoneMap::parse(
            "test",
            "L -100,-100,0,100,100,0,0,0,0\nP 0,0,0,255,255,0,1,Center",
        );
        let state = MapState {
            rect: Rect::new(10., 10., 300., 300.),
            ..Default::default()
        };
        let frame = map.frame([320, 320], &state);
        for hit in frame.hit_targets.iter().filter(|hit| {
            hit.item.starts_with("map:zoom_")
                || hit.item == "map:close"
                || hit.item == "map:recenter"
        }) {
            assert_eq!(
                frame
                    .hit_test([hit.rect.x + 5., hit.rect.y + 5.])
                    .unwrap()
                    .item,
                hit.item
            );
            assert!(MapAction::from_hit(hit).is_some());
        }
        let point = state.screen_at([0.; 3]);
        assert_eq!(
            MapAction::from_hit(frame.hit_test(point).unwrap()),
            Some(MapAction::Label(0))
        );
        assert!(frame.commands.iter().any(|cmd| matches!(cmd, DrawCommand::Line { color, clip, .. } if color[0] == 155 && *clip == state.canvas())));
        let filtered = MapState {
            layers: [false; 4],
            ..state
        };
        let frame = map.frame([320, 320], &filtered);
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item.starts_with("map:label:"))
        );
    }

    #[test]
    #[ignore = "requires original client maps and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn actual_poknowledge_landmarks_and_map_capture() {
        let base = std::env::var_os("EQ_CLIENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest"));
        let map = ZoneMap::load(&base, "poknowledge").unwrap();
        assert!(map.lines.len() > 7000);
        assert!(map.labels.len() > 30);
        assert_eq!(map.malformed_records, 0);
        let crescent = map
            .labels
            .iter()
            .find(|label| label.text == "Crescent Reach")
            .unwrap();
        assert_eq!(crescent.position, [59.1857, -157.5488, -156.8613]);
        let bank = map
            .labels
            .iter()
            .find(|label| label.text == "Dogle (Bank)")
            .unwrap();
        assert_eq!(bank.position, [-305., 944., -91.624]);
        let state = MapState {
            rect: Rect::new(20., 20., 600., 650.),
            player_position: [-100., -100., -156.],
            heading: 0.45,
            units_per_pixel: 3.6,
            center: Some([0., 576.]),
            waypoints: vec![MapMarker {
                position: crescent.position,
                label: "Crescent Reach".into(),
                ..Default::default()
            }],
            target: Some(MapMarker {
                position: bank.position,
                label: "Dogle (Bank)".into(),
                color: [255, 115, 115, 255],
            }),
            ..Default::default()
        };
        let frame = map.frame([640, 690], &state);
        assert!(frame.commands.len() > 6000);
        assert!(frame.warnings.is_empty());
        if let Some(destination) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
            let mut renderer = openeq_render::Renderer::new_headless(640, 690).unwrap();
            let scene =
                openeq_assets::Scene::from_geometry("Map test".into(), vec![], vec![], vec![]);
            let gpu = openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene)
                .unwrap();
            renderer.set_scene(&gpu);
            renderer.set_ui(&frame);
            renderer.render(&gpu, &openeq_render::Camera::default());
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            let path = PathBuf::from(destination).join("openeq-poknowledge-map.png");
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
            eprintln!("wrote {}", path.display());
        }
    }
}
