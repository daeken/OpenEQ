//! Short, bounded combat annotations from authoritative damage packets. Negative
//! damage codes are avoidance outcomes, never healing; HP deltas are not used.
use openeq_net::gameplay::Damage;
use openeq_render::{Camera, actors::ActorBounds};
use openeq_ui::{Color, DrawCommand, Rect, TextAlign, UiFrame};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

const MAX_EVENTS: usize = 48;
const MAX_PER_TARGET: usize = 4;
const MAX_DISTANCE: f32 = 320.;
const INCOMING: Color = [255, 120, 108, 255];
const OUTGOING: Color = [255, 226, 157, 255];
const OTHER: Color = [222, 229, 237, 255];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Hit(u32),
    Miss,
    Blocked,
    Parried,
    Riposted,
    Dodged,
    Invulnerable,
    Absorbed,
}

impl Kind {
    fn from_damage(damage: &Damage) -> Option<Self> {
        // Zero-damage spell packets include buffs. They do not indicate misses
        // or resists; the latter have their own authoritative server messages.
        match damage.amount {
            amount if amount > 0 => Some(Self::Hit(amount as u32)),
            0 if damage.spell_id == 0 || damage.spell_id >= 0xffff => Some(Self::Miss),
            -1 => Some(Self::Blocked),
            -2 => Some(Self::Parried),
            -3 => Some(Self::Riposted),
            -4 => Some(Self::Dodged),
            -5 => Some(Self::Invulnerable),
            -6 => Some(Self::Absorbed),
            _ => None,
        }
    }

    fn lifetime(self) -> Duration {
        Duration::from_millis(if matches!(self, Self::Hit(_)) {
            1700
        } else {
            1200
        })
    }

    fn label(self, incoming: bool) -> String {
        match self {
            Self::Hit(amount) if incoming => format!("−{amount}"),
            Self::Hit(amount) => amount.to_string(),
            Self::Miss => "Miss".into(),
            Self::Blocked => "Blocked".into(),
            Self::Parried => "Parried".into(),
            Self::Riposted => "Riposted".into(),
            Self::Dodged => "Dodged".into(),
            Self::Invulnerable => "Invulnerable".into(),
            Self::Absorbed => "Absorbed".into(),
        }
    }
}

#[derive(Debug)]
struct Event {
    source: u32,
    target: u32,
    kind: Kind,
    at: Instant,
}

/// At most 48 recent packets and four annotations per target are retained.
/// Each displayed number is one server hit; numbers are never combined or
/// inferred. Repeated misses/avoidance within 180 ms are visually suppressed.
#[derive(Default, Debug)]
pub struct CombatFeedback {
    events: VecDeque<Event>,
}

impl CombatFeedback {
    /// Records only fights involving the local player or the selected target.
    /// Returns whether a visible event was queued. Call `clear` when leaving a
    /// zone or disconnecting so reused spawn IDs cannot inherit old feedback.
    pub fn record_damage(
        &mut self,
        damage: &Damage,
        own: Option<u32>,
        selected: Option<u32>,
        now: Instant,
    ) -> bool {
        self.prune(now);
        let relevant = [own, selected]
            .into_iter()
            .flatten()
            .any(|id| id != 0 && (id == damage.source_id || id == damage.target_id));
        let Some(kind) = Kind::from_damage(damage).filter(|_| relevant && damage.target_id != 0)
        else {
            return false;
        };
        if !matches!(kind, Kind::Hit(_))
            && self.events.iter().rev().any(|event| {
                event.target == damage.target_id
                    && event.kind == kind
                    && now.saturating_duration_since(event.at) < Duration::from_millis(180)
            })
        {
            return false;
        }
        if self
            .events
            .iter()
            .filter(|event| event.target == damage.target_id)
            .count()
            >= MAX_PER_TARGET
            && let Some(index) = self
                .events
                .iter()
                .position(|event| event.target == damage.target_id)
        {
            self.events.remove(index);
        }
        if self.events.len() == MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(Event {
            source: damage.source_id,
            target: damage.target_id,
            kind,
            at: now,
        });
        true
    }

    pub fn prune(&mut self, now: Instant) {
        self.events
            .retain(|event| now.saturating_duration_since(event.at) < event.kind.lifetime());
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Appends overlay commands in logical pixels; expired events are ignored
    /// without mutating the queue. Compose this frame behind gameplay windows.
    /// Actor bounds are the renderer's animated scene-space bounds. This checks
    /// the view and distance, but does not perform world-depth occlusion tests.
    /// Incoming events use a screen-side anchor when first-person rendering has
    /// no local-player bounds. No interaction targets are added.
    pub fn append(
        &self,
        frame: &mut UiFrame,
        camera: &Camera,
        viewport: [u32; 2],
        actors: &BTreeMap<u32, ActorBounds>,
        own: Option<u32>,
        now: Instant,
    ) {
        let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
        if screen.is_empty() {
            return;
        }
        if frame.bounds.is_empty() {
            frame.bounds = screen;
        }
        let mut rows = BTreeMap::<u32, usize>::new();
        for event in self.events.iter().rev() {
            let Some(age) = now.checked_duration_since(event.at) else {
                continue;
            };
            if age >= event.kind.lifetime() {
                continue;
            }
            let incoming = Some(event.target) == own;
            let anchor = match actors.get(&event.target) {
                Some(bounds) => actor_anchor(camera, viewport, bounds),
                None if incoming => Some([screen.width * 0.72, screen.height * 0.56]),
                None => None,
            };
            let Some([x, y]) = anchor else {
                continue;
            };
            let row = rows.entry(event.target).or_default();
            let y = y - *row as f32 * 26. - age.as_secs_f32() * 22.;
            *row += 1;
            let text_width = 152_f32.min(screen.width);
            let rect = Rect::new(
                (x - text_width * 0.5).clamp(0., (screen.width - text_width).max(0.)),
                y,
                text_width,
                26.,
            );
            if rect.intersect(screen).is_empty() {
                continue;
            }
            let hit = matches!(event.kind, Kind::Hit(_));
            let mut color = if incoming {
                if hit { INCOMING } else { [224, 172, 166, 255] }
            } else if Some(event.source) == own {
                if hit { OUTGOING } else { [198, 187, 163, 255] }
            } else {
                OTHER
            };
            let remaining = 1. - age.as_secs_f32() / event.kind.lifetime().as_secs_f32();
            color[3] = ((remaining / 0.35).clamp(0., 1.) * if hit { 255. } else { 215. }) as u8;
            let label = event.kind.label(incoming);
            let font = if hit { 5 } else { 3 };
            for [dx, dy] in [[-1., 0.], [1., 0.], [0., -1.], [0., 1.]] {
                draw_text(
                    frame,
                    Rect::new(rect.x + dx, rect.y + dy, rect.width, rect.height),
                    screen,
                    &label,
                    font,
                    [5, 7, 10, (f32::from(color[3]) * 0.85) as u8],
                );
            }
            draw_text(frame, rect, screen, &label, font, color);
        }
    }
}

fn actor_anchor(camera: &Camera, viewport: [u32; 2], bounds: &ActorBounds) -> Option<[f32; 2]> {
    if !bounds
        .min
        .iter()
        .zip(bounds.max)
        .all(|(&min, max)| min.is_finite() && max.is_finite() && min <= max)
    {
        return None;
    }
    let distance_squared: f32 = bounds
        .center()
        .iter()
        .zip(camera.position)
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    if !distance_squared.is_finite() || distance_squared > MAX_DISTANCE * MAX_DISTANCE {
        return None;
    }
    let [left, top, right, bottom] =
        crate::targeting::screen_bounds(camera, viewport.map(|v| v as f32), bounds)?;
    if right < 0. || left > viewport[0] as f32 || bottom < 0. || top > viewport[1] as f32 {
        return None;
    }
    // Leave a row below the numbers for the actor's nameplate.
    Some([(left + right) * 0.5, (top - 49.).max(8.)])
}

fn draw_text(frame: &mut UiFrame, rect: Rect, screen: Rect, label: &str, font: u32, color: Color) {
    frame.commands.push(DrawCommand::Text {
        rect,
        clip: rect.intersect(screen),
        text: label.into(),
        font,
        color,
        align: TextAlign::Center,
        vertical_center: true,
        wrap: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn damage(source: u32, target: u32, amount: i32) -> Damage {
        Damage {
            source_id: source,
            target_id: target,
            amount,
            skill: 0,
            spell_id: 0xffff,
            secondary: false,
            special: 0,
        }
    }

    fn actors() -> BTreeMap<u32, ActorBounds> {
        BTreeMap::from([(
            2,
            ActorBounds {
                min: [-3., 29., 0.],
                max: [3., 33., 8.],
            },
        )])
    }

    fn camera() -> Camera {
        Camera {
            position: [0., 0., 6.],
            pitch: 0.,
            ..Default::default()
        }
    }

    #[test]
    fn packet_semantics_exclude_spell_effects_unknown_codes_and_unrelated_fights() {
        let now = Instant::now();
        let mut feedback = CombatFeedback::default();
        let mut buff = damage(1, 1, 0);
        buff.spell_id = 288;
        assert!(!feedback.record_damage(&buff, Some(1), Some(2), now));
        assert!(!feedback.record_damage(&damage(3, 4, 12), Some(1), Some(2), now));
        assert!(!feedback.record_damage(&damage(1, 2, -99), Some(1), Some(2), now));
        for (amount, expected) in [
            (0, Kind::Miss),
            (-1, Kind::Blocked),
            (-2, Kind::Parried),
            (-3, Kind::Riposted),
            (-4, Kind::Dodged),
            (-5, Kind::Invulnerable),
            (-6, Kind::Absorbed),
            (42, Kind::Hit(42)),
        ] {
            let packet = damage(1, 2, amount);
            assert_eq!(Kind::from_damage(&packet), Some(expected));
            assert!(feedback.record_damage(&packet, Some(1), Some(2), now));
        }
        assert!(feedback.record_damage(&damage(3, 2, 12), Some(1), Some(2), now));
        assert!(!feedback.record_damage(&damage(0, 0, 12), None, None, now));
    }

    #[test]
    fn bursts_are_bounded_and_expiry_does_not_require_mutable_render_access() {
        let now = Instant::now();
        let mut feedback = CombatFeedback::default();
        for amount in 1..10 {
            assert!(feedback.record_damage(&damage(1, 2, amount), Some(1), None, now));
        }
        assert_eq!(feedback.events.len(), MAX_PER_TARGET);
        assert_eq!(feedback.events.front().unwrap().kind, Kind::Hit(6));
        for target in 3..100 {
            feedback.record_damage(&damage(1, target, 1), Some(1), None, now);
        }
        assert_eq!(feedback.events.len(), MAX_EVENTS);
        feedback.clear();
        assert!(feedback.record_damage(&damage(2, 1, 0), Some(1), None, now));
        assert!(!feedback.record_damage(
            &damage(2, 1, 0),
            Some(1),
            None,
            now + Duration::from_millis(100)
        ));
        assert!(feedback.record_damage(
            &damage(2, 1, 0),
            Some(1),
            None,
            now + Duration::from_millis(200)
        ));
        let mut frame = UiFrame::default();
        feedback.append(
            &mut frame,
            &camera(),
            [640, 360],
            &actors(),
            Some(1),
            now + Duration::from_secs(3),
        );
        assert!(frame.commands.is_empty());
        assert_eq!(feedback.events.len(), 2);
        feedback.prune(now + Duration::from_secs(3));
        assert!(feedback.events.is_empty());
    }

    #[test]
    fn projection_stacks_hits_and_uses_red_incoming_feedback_without_local_bounds() {
        let now = Instant::now();
        let mut feedback = CombatFeedback::default();
        feedback.record_damage(&damage(1, 2, 42), Some(1), None, now);
        feedback.record_damage(&damage(1, 2, 11), Some(1), None, now);
        feedback.record_damage(&damage(2, 1, 7), Some(1), None, now);
        let mut frame = UiFrame::default();
        feedback.append(&mut frame, &camera(), [640, 360], &actors(), Some(1), now);
        let foreground: Vec<_> = frame
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text {
                    rect, text, color, ..
                } if *color == INCOMING || *color == OUTGOING => {
                    Some((rect, text.as_str(), *color))
                }
                _ => None,
            })
            .collect();
        assert_eq!(foreground.len(), 3);
        assert!(
            foreground
                .iter()
                .any(|(_, text, color)| *text == "−7" && *color == INCOMING)
        );
        let y = |value| {
            foreground
                .iter()
                .find(|(_, text, _)| *text == value)
                .unwrap()
                .0
                .y
        };
        assert!(y("42") + 25. <= y("11"));
        assert!(frame.hit_targets.is_empty());
        feedback.clear();
        let mut frame = UiFrame::default();
        feedback.append(&mut frame, &camera(), [640, 360], &actors(), Some(1), now);
        assert!(frame.commands.is_empty());
    }

    #[test]
    fn offscreen_behind_far_missing_and_invalid_actors_are_not_annotated() {
        let now = Instant::now();
        let mut feedback = CombatFeedback::default();
        feedback.record_damage(&damage(1, 2, 42), Some(1), None, now);
        for bounds in [
            ActorBounds {
                min: [-2., -30., 0.],
                max: [2., -25., 8.],
            },
            ActorBounds {
                min: [100., 10., 0.],
                max: [105., 15., 8.],
            },
            ActorBounds {
                min: [-2., 500., 0.],
                max: [2., 505., 8.],
            },
            ActorBounds {
                min: [f32::NAN, 29., 0.],
                max: [2., 33., 8.],
            },
        ] {
            let mut frame = UiFrame::default();
            feedback.append(
                &mut frame,
                &camera(),
                [640, 360],
                &BTreeMap::from([(2, bounds)]),
                Some(1),
                now,
            );
            assert!(frame.commands.is_empty());
        }
        let mut frame = UiFrame::default();
        feedback.append(
            &mut frame,
            &camera(),
            [640, 360],
            &BTreeMap::new(),
            Some(1),
            now,
        );
        assert!(frame.commands.is_empty());
        feedback.append(&mut frame, &camera(), [0, 0], &actors(), Some(1), now);
        assert!(frame.commands.is_empty());
    }

    #[test]
    #[ignore = "requires GPU; writes captures with OPENEQ_UI_CAPTURE_DIR"]
    fn capture_combat_annotations_at_both_pixel_densities() {
        let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") else {
            return;
        };
        let now = Instant::now();
        let mut feedback = CombatFeedback::default();
        feedback.record_damage(&damage(1, 2, 84), Some(1), None, now);
        feedback.record_damage(
            &damage(1, 2, 0),
            Some(1),
            None,
            now + Duration::from_millis(180),
        );
        feedback.record_damage(
            &damage(1, 2, 17),
            Some(1),
            None,
            now + Duration::from_millis(360),
        );
        feedback.record_damage(
            &damage(2, 1, 12),
            Some(1),
            None,
            now + Duration::from_millis(360),
        );
        let screen = Rect::new(0., 0., 960., 600.);
        let mut frame = UiFrame {
            bounds: screen,
            ..Default::default()
        };
        frame.commands.push(DrawCommand::Fill {
            rect: screen,
            clip: screen,
            color: [24, 30, 35, 255],
        });
        // A neutral marker locates the actor bounds in this asset-free capture.
        let [left, top, right, bottom] =
            crate::targeting::screen_bounds(&camera(), [960., 600.], &actors()[&2]).unwrap();
        frame.commands.push(DrawCommand::Fill {
            rect: Rect::new(left, top, right - left, bottom - top),
            clip: screen,
            color: [43, 55, 57, 255],
        });
        draw_text(
            &mut frame,
            Rect::new(left - 40., top - 22., right - left + 80., 20.),
            screen,
            "Training opponent",
            2,
            [188, 212, 215, 255],
        );
        feedback.append(
            &mut frame,
            &camera(),
            [960, 600],
            &actors(),
            Some(1),
            now + Duration::from_millis(450),
        );
        for scale in [1., 2.] {
            let mut renderer =
                openeq_render::Renderer::new_headless((960. * scale) as u32, (600. * scale) as u32)
                    .unwrap();
            let scene = openeq_assets::Scene::from_geometry(
                "Combat feedback".into(),
                vec![],
                vec![],
                vec![],
            );
            let gpu = openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene)
                .unwrap();
            renderer.set_scene(&gpu);
            renderer.set_ui_scaled(&frame, scale);
            renderer.render(&gpu, &camera());
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            let path = std::path::PathBuf::from(&directory)
                .join(format!("openeq-combat-feedback-{}x.png", scale as u32));
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
            eprintln!("wrote {}", path.display());
        }
    }
}
