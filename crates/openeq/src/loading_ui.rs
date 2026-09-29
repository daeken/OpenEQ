//! Asset-independent loading presentation in logical pixels. The caller supplies
//! real progress and keeps advancing `elapsed` while background work runs.
use openeq_ui::{Color, DrawCommand, Rect, TextAlign, UiFrame};

const BACKGROUND: Color = [10, 13, 19, 255];
const GOLD: Color = [199, 166, 103, 255];
const WHITE: Color = [235, 231, 220, 255];
const MUTED: Color = [154, 163, 178, 255];
const TRACK: Color = [39, 44, 54, 255];

/// Draw a startup or zone-transition screen without loading the original skin.
///
/// `viewport` is the logical window size; the renderer applies the display scale.
/// A finite `progress` is clamped to 0–1 and displayed without time-based
/// estimation or a numeric percentage. Activity continues between checkpoints.
/// `None` (or a nonfinite value) draws an indeterminate animation.
/// Error text replaces the progress indicator. Input handling, including Escape,
/// stays with the caller; this frame deliberately contains no hit targets.
pub fn loading_frame(
    viewport: [u32; 2],
    title: &str,
    detail: &str,
    progress: Option<f32>,
    elapsed: f32,
    error: Option<&str>,
) -> UiFrame {
    let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
    let mut frame = UiFrame {
        bounds: screen,
        ..Default::default()
    };
    if screen.is_empty() {
        return frame;
    }
    fill(&mut frame, screen, BACKGROUND);

    // Quiet bands and small corner ornaments give the screen a frame without
    // depending on zone textures or the XML atlas being available yet.
    fill(
        &mut frame,
        Rect::new(0., screen.height * 0.18, screen.width, screen.height * 0.64),
        [12, 16, 23, 255],
    );
    let inset = (screen.width.min(screen.height) * 0.045).clamp(12., 28.);
    for (x, sx) in [(inset, 1.), (screen.width - inset, -1.)] {
        for (y, sy) in [(inset, 1.), (screen.height - inset, -1.)] {
            line(&mut frame, [x, y], [x + sx * 24., y], [91, 79, 57, 255]);
            line(&mut frame, [x, y], [x, y + sy * 24.], [91, 79, 57, 255]);
        }
    }

    let width = (screen.width - 56.).clamp(0., 680.);
    let x = (screen.width - width) * 0.5;
    let compact = screen.height < 480.;
    let top = if compact { 34. } else { screen.height * 0.24 };
    let center = screen.width * 0.5;
    let brand_y = top + 4.;
    // A restrained compass diamond uses only line primitives.
    let diamond = [center - 53., brand_y + 10.];
    for (from, to) in [
        ([-7., 0.], [0., -10.]),
        ([0., -10.], [7., 0.]),
        ([7., 0.], [0., 10.]),
        ([0., 10.], [-7., 0.]),
    ] {
        line(
            &mut frame,
            [diamond[0] + from[0], diamond[1] + from[1]],
            [diamond[0] + to[0], diamond[1] + to[1]],
            GOLD,
        );
    }
    text(
        &mut frame,
        Rect::new(center - 36., brand_y - 2., 112., 25.),
        "OpenEQ",
        5,
        GOLD,
        TextAlign::Left,
    );

    let title_y = top + if compact { 57. } else { 76. };
    text(
        &mut frame,
        Rect::new(x, title_y, width, 68.),
        if title.trim().is_empty() {
            "Norrath"
        } else {
            title
        },
        if compact && title.chars().count() > 40 {
            6
        } else {
            7
        },
        WHITE,
        TextAlign::Center,
    );
    let detail_y = title_y + 72.;
    text(
        &mut frame,
        Rect::new(x + 8., detail_y, (width - 16.).max(0.), 44.),
        detail,
        3,
        MUTED,
        TextAlign::Center,
    );

    let indicator_y = detail_y + 61.;
    if let Some(error) = error {
        let panel_height = (screen.height - indicator_y - 62.).clamp(38., 200.);
        let panel = Rect::new(x, indicator_y - 3., width, panel_height);
        fill(&mut frame, panel, [35, 26, 27, 255]);
        fill(
            &mut frame,
            Rect::new(panel.x, panel.y, 2., panel.height),
            [180, 116, 85, 255],
        );
        text(
            &mut frame,
            Rect::new(
                panel.x + 16.,
                panel.y + 10.,
                (panel.width - 32.).max(0.),
                panel.height - 20.,
            ),
            if error.trim().is_empty() {
                "Loading could not finish. Please try again."
            } else {
                error
            },
            3,
            [237, 193, 170, 255],
            TextAlign::Left,
        );
        text(
            &mut frame,
            Rect::new(x, panel.bottom() + 17., width, 20.),
            "Escape to quit",
            2,
            MUTED,
            TextAlign::Center,
        );
    } else {
        let elapsed = if elapsed.is_finite() {
            elapsed.max(0.)
        } else {
            0.
        };
        let track = Rect::new(x, indicator_y, width, 4.);
        fill(&mut frame, track, TRACK);
        if let Some(progress) = progress.filter(|value| value.is_finite()) {
            let progress = progress.clamp(0., 1.);
            fill(
                &mut frame,
                Rect::new(track.x, track.y, track.width * progress, track.height),
                GOLD,
            );
            // These dots indicate activity, not completion. The checkpoint bar
            // stays exactly where the caller put it while a long stage runs.
            let phase = elapsed.rem_euclid(1.8) / 1.8;
            for index in 0..3 {
                let glow = ((phase - index as f32 / 3.) * std::f32::consts::TAU).cos() * 0.5 + 0.5;
                fill(
                    &mut frame,
                    Rect::new(center - 12. + index as f32 * 10., indicator_y + 24., 4., 4.),
                    [GOLD[0], GOLD[1], GOLD[2], (80. + glow * 150.) as u8],
                );
            }
        } else {
            let phase = elapsed.rem_euclid(2.4) / 2.4;
            let travel = (1. - (phase * std::f32::consts::TAU).cos()) * 0.5;
            let pulse_width = track.width * 0.19;
            let pulse_x = track.x + (track.width - pulse_width) * travel;
            // Fade the ends into the track so the travelling light never looks
            // like a completed fraction or a fabricated percentage.
            for step in 0..16 {
                let glow = (std::f32::consts::PI * (step as f32 + 0.5) / 16.).sin();
                fill(
                    &mut frame,
                    Rect::new(
                        pulse_x + pulse_width * step as f32 / 16.,
                        track.y,
                        pulse_width / 16. + 0.25,
                        track.height,
                    ),
                    [GOLD[0], GOLD[1], GOLD[2], (glow * 230.) as u8],
                );
            }
            text(
                &mut frame,
                Rect::new(x, indicator_y + 19., width, 22.),
                "Loading…",
                2,
                MUTED,
                TextAlign::Center,
            );
        }
    }
    frame
}

fn fill(frame: &mut UiFrame, rect: Rect, color: Color) {
    let clip = rect.intersect(frame.bounds);
    if !clip.is_empty() {
        frame.commands.push(DrawCommand::Fill { rect, clip, color });
    }
}

fn line(frame: &mut UiFrame, from: [f32; 2], to: [f32; 2], color: Color) {
    frame.commands.push(DrawCommand::Line {
        from,
        to,
        width: 1.,
        clip: frame.bounds,
        color,
    });
}

fn text(frame: &mut UiFrame, rect: Rect, value: &str, font: u32, color: Color, align: TextAlign) {
    let clip = rect.intersect(frame.bounds);
    if !clip.is_empty() && !value.is_empty() {
        frame.commands.push(DrawCommand::Text {
            rect,
            clip,
            text: value.into(),
            font,
            color,
            align,
            vertical_center: false,
            wrap: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_text(frame: &UiFrame, value: &str) -> bool {
        frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == value))
    }

    #[test]
    fn progress_is_supplied_by_caller_and_errors_replace_it() {
        for (value, expected) in [(-0.4, 0.), (0.42, 0.42), (2., 1.)] {
            let frame = loading_frame(
                [640, 360],
                "Plane of Knowledge",
                "Loading terrain",
                Some(value),
                30.,
                None,
            );
            let filled = frame
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawCommand::Fill { rect, color, .. } if *color == GOLD => Some(rect.width),
                    _ => None,
                })
                .unwrap_or(0.);
            assert!((filled - 584. * expected).abs() < 0.001);
            assert!(!frame.commands.iter().any(
                |command| matches!(command, DrawCommand::Text { text, .. } if text.contains('%'))
            ));
            assert!(frame.hit_targets.is_empty());
        }
        let frame = loading_frame(
            [640, 360],
            "Plane of Knowledge",
            "Connecting to the zone",
            Some(0.42),
            30.,
            Some("The zone connection closed. Please try again."),
        );
        assert!(has_text(&frame, "Plane of Knowledge"));
        assert!(has_text(
            &frame,
            "The zone connection closed. Please try again."
        ));
        assert!(has_text(&frame, "Escape to quit"));
        assert!(!has_text(&frame, "42%"));
        assert!(frame.hit_targets.is_empty());
        assert!(
            !frame.commands.iter().any(
                |command| matches!(command, DrawCommand::Fill { color, .. } if color[3] < 255)
            )
        );
    }

    #[test]
    fn checkpoint_activity_animates_without_advancing_progress() {
        let frame_at = |elapsed| {
            loading_frame(
                [640, 360],
                "Norrath",
                "Loading terrain",
                Some(0.42),
                elapsed,
                None,
            )
        };
        let bar = |frame: &UiFrame| {
            frame.commands.iter().find_map(|command| match command {
                DrawCommand::Fill { rect, color, .. } if *color == GOLD => Some(*rect),
                _ => None,
            })
        };
        let dots = |frame: &UiFrame| {
            frame
                .commands
                .iter()
                .filter_map(|command| match command {
                    DrawCommand::Fill { color, .. } if color[3] < 255 => Some(*color),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let first = frame_at(0.);
        let later = frame_at(0.6);
        assert_eq!(bar(&first), bar(&later));
        assert_eq!(dots(&first).len(), 3);
        assert_ne!(dots(&first), dots(&later));
    }

    #[test]
    fn indeterminate_animation_and_nonfinite_inputs_stay_bounded() {
        let positions = |elapsed| {
            loading_frame(
                [640, 360],
                "Norrath",
                "Preparing your character",
                None,
                elapsed,
                None,
            )
            .commands
            .into_iter()
            .filter_map(|command| match command {
                DrawCommand::Fill { rect, color, .. } if color[3] < 255 => Some(rect.x),
                _ => None,
            })
            .collect::<Vec<_>>()
        };
        assert_ne!(positions(0.), positions(0.8));
        for viewport in [[0, 0], [640, 360], [1280, 900], [3840, 2160]] {
            let frame = loading_frame(
                viewport,
                "Norrath",
                "Loading terrain",
                Some(f32::NAN),
                f32::INFINITY,
                None,
            );
            assert!(frame.warnings.is_empty() && frame.hit_targets.is_empty());
            for command in &frame.commands {
                let clip = match command {
                    DrawCommand::Fill { rect, clip, .. } | DrawCommand::Text { rect, clip, .. } => {
                        assert!(
                            [rect.x, rect.y, rect.width, rect.height]
                                .iter()
                                .all(|value| value.is_finite())
                        );
                        *clip
                    }
                    DrawCommand::Line { clip, .. } => *clip,
                    _ => panic!("loading presentation must not require assets"),
                };
                assert_eq!(clip, clip.intersect(frame.bounds));
            }
        }
    }

    #[test]
    #[ignore = "requires GPU; writes captures only with OPENEQ_UI_CAPTURE_DIR"]
    fn capture_loading_screens() {
        let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") else {
            return;
        };
        for (name, viewport, scale, progress, error) in [
            ("openeq-loading-small.png", [640, 360], 1., Some(0.42), None),
            ("openeq-loading-retina.png", [1280, 900], 2., None, None),
            (
                "openeq-loading-error.png",
                [640, 360],
                2.,
                None,
                Some("The zone connection closed before your character arrived. Please try again."),
            ),
        ] {
            let frame = loading_frame(
                viewport,
                "Plane of Knowledge",
                "Preparing the world and its inhabitants",
                progress,
                0.8,
                error,
            );
            let mut renderer = openeq_render::Renderer::new_headless(
                (viewport[0] as f32 * scale) as u32,
                (viewport[1] as f32 * scale) as u32,
            )
            .unwrap();
            let scene =
                openeq_assets::Scene::from_geometry("Loading".into(), vec![], vec![], vec![]);
            let gpu = openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene)
                .unwrap();
            renderer.set_scene(&gpu);
            renderer.set_ui_scaled(&frame, scale);
            renderer.render(&gpu, &openeq_render::Camera::default());
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            let path = std::path::PathBuf::from(&directory).join(name);
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
            eprintln!("wrote {}", path.display());
        }
    }
}
