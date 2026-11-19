//! Direct numeric input while drawing: type a distance (`3,47`), polar
//! distance and angle (`3,5<45`), angle lock (`<30`), rectangle dimensions
//! (`3,5x4,0`), relative offset (`@2,0;-1,5`) or absolute coordinates
//! (`1,5;2,0`) and press Enter to commit the exact point.

use crate::domain::geometry::{EPSILON, Point, format_meters};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum NumericSpec {
    /// Exact length along the current pointer direction from the draft start.
    Distance(f64),
    /// Exact length at `angle_deg` degrees counter-clockwise from +X (0° right, 90° up).
    Polar { distance: f64, angle_deg: f64 },
    /// Locks direction to `angle_deg` degrees while keeping pointer distance.
    Angle(f64),
    /// Width × height from the draft start in the pointer's quadrant.
    Dimensions { width: f64, height: f64 },
    /// Relative offset `(dx, dy)` from the draft start.
    Relative(f64, f64),
    /// Absolute coordinates `(x, y)` in metres.
    Absolute(f64, f64),
}

fn parse_number(raw: &str) -> Option<f64> {
    let normalized = raw.trim().replace(',', ".");
    if normalized.is_empty() {
        return None;
    }
    let value = normalized.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

fn split_pair(raw: &str) -> Option<(&str, &str)> {
    if let Some(pair) = raw.split_once(';') {
        return Some(pair);
    }
    let mut parts = raw.split_whitespace();
    let first = parts.next()?;
    let second = parts.next()?;
    parts.next().is_none().then_some((first, second))
}

/// Parses typed CAD input, accepting both Brazilian comma and dot decimals.
pub(in crate::app) fn parse_numeric_input(raw: &str) -> Option<NumericSpec> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }

    // Angle-only lock: `<45` or `<-30`.
    if let Some(rest) = text.strip_prefix('<') {
        let angle_deg = parse_number(rest)?;
        return Some(NumericSpec::Angle(angle_deg));
    }

    // Polar distance + angle: `3,5<45` or `@3,5<45`.
    if let Some((dist_part, angle_part)) = text.split_once('<') {
        let dist_str = dist_part.trim().strip_prefix('@').unwrap_or(dist_part);
        let distance = parse_number(dist_str)?;
        let angle_deg = parse_number(angle_part)?;
        return (distance > EPSILON).then_some(NumericSpec::Polar {
            distance,
            angle_deg,
        });
    }

    // Relative coordinates: `@3,0;2,0` or `@3,0 2,0`.
    if let Some(rest) = text.strip_prefix('@') {
        let (dx_str, dy_str) = split_pair(rest.trim())?;
        let dx = parse_number(dx_str)?;
        let dy = parse_number(dy_str)?;
        return Some(NumericSpec::Relative(dx, dy));
    }

    // Dimensions: `3,5x4,0`, `3.5X4`, `3,5×4,0`, `3,5*4,0`.
    for sep in ['x', 'X', '×', '*'] {
        if let Some((w_str, h_str)) = text.split_once(sep) {
            let width = parse_number(w_str)?;
            let height = parse_number(h_str)?;
            return (width > EPSILON && height > EPSILON)
                .then_some(NumericSpec::Dimensions { width, height });
        }
    }

    // Absolute coordinates: `1,5;2,0` or `1,5 2,0`.
    if let Some((x_str, y_str)) = split_pair(text) {
        let x = parse_number(x_str)?;
        let y = parse_number(y_str)?;
        return Some(NumericSpec::Absolute(x, y));
    }

    // Single number: distance along pointer direction (`3,47`).
    let distance = parse_number(text)?;
    (distance > EPSILON).then_some(NumericSpec::Distance(distance))
}

impl NumericSpec {
    /// Resolves this input into a concrete world point given the active
    /// `start` point (if any) and the current `aim` point (snapped pointer).
    pub(in crate::app) fn resolve(self, start: Option<Point>, aim: Option<Point>) -> Option<Point> {
        match self {
            NumericSpec::Absolute(x, y) => Some(Point::new(x, y).rounded()),
            NumericSpec::Relative(dx, dy) => {
                let from = start?;
                Some(Point::new(from.x + dx, from.y + dy).rounded())
            }
            NumericSpec::Distance(distance) => {
                let from = start?;
                let dir = aim
                    .and_then(|target| target.minus(from).normalized())
                    .unwrap_or(Point::new(1.0, 0.0));
                Some(from.translated(dir.scaled(distance)).rounded())
            }
            NumericSpec::Polar {
                distance,
                angle_deg,
            } => {
                let from = start?;
                let rad = angle_deg.to_radians();
                // Counter-clockwise from +X: 0° right, 90° up (-Y on screen).
                let offset = Point::new(distance * rad.cos(), -distance * rad.sin());
                Some(from.translated(offset).rounded())
            }
            NumericSpec::Angle(angle_deg) => {
                let from = start?;
                let distance = aim
                    .map(|target| from.distance(target))
                    .filter(|d| *d > EPSILON)
                    .unwrap_or(1.0);
                let rad = angle_deg.to_radians();
                let offset = Point::new(distance * rad.cos(), -distance * rad.sin());
                Some(from.translated(offset).rounded())
            }
            NumericSpec::Dimensions { width, height } => {
                let from = start?;
                let sx = aim
                    .map(|target| if target.x < from.x { -1.0 } else { 1.0 })
                    .unwrap_or(1.0);
                let sy = aim
                    .map(|target| if target.y < from.y { -1.0 } else { 1.0 })
                    .unwrap_or(1.0);
                Some(Point::new(from.x + sx * width, from.y + sy * height).rounded())
            }
        }
    }

    pub(in crate::app) fn requires_start(self) -> bool {
        !matches!(self, NumericSpec::Absolute(_, _))
    }

    pub(in crate::app) fn describe(self) -> String {
        match self {
            NumericSpec::Distance(d) => format!("Distância: {}", format_meters(d)),
            NumericSpec::Polar {
                distance,
                angle_deg,
            } => format!(
                "Polar: {} ∠ {}°",
                format_meters(distance),
                format_angle(angle_deg)
            ),
            NumericSpec::Angle(angle_deg) => format!("Ângulo: {}°", format_angle(angle_deg)),
            NumericSpec::Dimensions { width, height } => {
                format!(
                    "Medidas: {} × {}",
                    format_meters(width),
                    format_meters(height)
                )
            }
            NumericSpec::Relative(dx, dy) => {
                format!(
                    "Relativo: ΔX {} · ΔY {}",
                    format_meters(dx),
                    format_meters(dy)
                )
            }
            NumericSpec::Absolute(x, y) => {
                format!(
                    "Coordenada: X {} · Y {}",
                    format_meters(x),
                    format_meters(y)
                )
            }
        }
    }
}

fn format_angle(angle_deg: f64) -> String {
    if angle_deg.fract().abs() < 1e-3 {
        format!("{angle_deg:.0}")
    } else {
        format!("{angle_deg:.1}").replace('.', ",")
    }
}

/// Whether `ch` may start or continue direct numeric input on the canvas.
pub(in crate::app) fn accepts_char(ch: char, already_typing: bool) -> bool {
    if already_typing {
        ch.is_ascii_digit()
            || matches!(
                ch,
                ',' | '.' | ';' | '<' | '@' | '-' | '+' | 'x' | 'X' | '×' | '*' | ' '
            )
    } else {
        ch.is_ascii_digit() || matches!(ch, '-' | '@' | '<')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_cad_numeric_input_formats() {
        assert_eq!(
            parse_numeric_input("3,47"),
            Some(NumericSpec::Distance(3.47))
        );
        assert_eq!(
            parse_numeric_input("3.47"),
            Some(NumericSpec::Distance(3.47))
        );
        assert_eq!(
            parse_numeric_input("3,5<45"),
            Some(NumericSpec::Polar {
                distance: 3.5,
                angle_deg: 45.0
            })
        );
        assert_eq!(
            parse_numeric_input("@4<-90"),
            Some(NumericSpec::Polar {
                distance: 4.0,
                angle_deg: -90.0
            })
        );
        assert_eq!(parse_numeric_input("<30"), Some(NumericSpec::Angle(30.0)));
        assert_eq!(
            parse_numeric_input("3,5x4,0"),
            Some(NumericSpec::Dimensions {
                width: 3.5,
                height: 4.0
            })
        );
        assert_eq!(
            parse_numeric_input("@2,5;-1,25"),
            Some(NumericSpec::Relative(2.5, -1.25))
        );
        assert_eq!(
            parse_numeric_input("1,5;2,0"),
            Some(NumericSpec::Absolute(1.5, 2.0))
        );
        assert_eq!(parse_numeric_input("0"), None);
        assert_eq!(parse_numeric_input(""), None);
    }

    #[test]
    fn resolves_distance_polar_dimensions_and_coordinates() {
        let start = Some(Point::new(1.0, 2.0));
        let aim_right = Some(Point::new(5.0, 2.0));
        let p = NumericSpec::Distance(3.47)
            .resolve(start, aim_right)
            .unwrap();
        assert!(p.near(Point::new(4.47, 2.0), 1e-4));

        // 90° goes up (-Y in plan coordinates).
        let up = NumericSpec::Polar {
            distance: 3.0,
            angle_deg: 90.0,
        }
        .resolve(start, None)
        .unwrap();
        assert!(up.near(Point::new(1.0, -1.0), 1e-4));

        // Dimensions respect pointer quadrant (left and down).
        let rect = NumericSpec::Dimensions {
            width: 3.0,
            height: 4.0,
        }
        .resolve(start, Some(Point::new(0.0, 5.0)))
        .unwrap();
        assert!(rect.near(Point::new(-2.0, 6.0), 1e-4));

        // Absolute coordinates work even without a start point.
        let abs = NumericSpec::Absolute(2.5, 3.5).resolve(None, None).unwrap();
        assert!(abs.near(Point::new(2.5, 3.5), 1e-4));
        assert!(NumericSpec::Distance(3.0).resolve(None, None).is_none());
    }

    #[test]
    fn angle_lock_relative_space_pairs_and_char_filtering() {
        let start = Some(Point::new(2.0, 2.0));
        // Angle-only lock keeps pointer distance (5 m from (2,2) to (7,2)) at 90° (up -> -Y).
        let locked = NumericSpec::Angle(90.0)
            .resolve(start, Some(Point::new(7.0, 2.0)))
            .unwrap();
        assert!(locked.near(Point::new(2.0, -3.0), 1e-4));

        // Relative offset and alternative dimension separators.
        let rel = parse_numeric_input("@1,5 -0,5")
            .unwrap()
            .resolve(start, None)
            .unwrap();
        assert!(rel.near(Point::new(3.5, 1.5), 1e-4));
        assert_eq!(
            parse_numeric_input("2,5×3,0"),
            Some(NumericSpec::Dimensions {
                width: 2.5,
                height: 3.0
            })
        );
        assert_eq!(
            parse_numeric_input("2.5*3"),
            Some(NumericSpec::Dimensions {
                width: 2.5,
                height: 3.0
            })
        );
        assert_eq!(
            parse_numeric_input("4,25 1,75"),
            Some(NumericSpec::Absolute(4.25, 1.75))
        );

        // Plain letters must not hijack tool shortcuts before typing starts,
        // but dimension separators are allowed once typing is active.
        assert!(!accepts_char('p', false));
        assert!(!accepts_char('x', false));
        assert!(accepts_char('3', false));
        assert!(accepts_char('@', false));
        assert!(accepts_char('<', false));
        assert!(accepts_char('x', true));
        assert!(accepts_char(',', true));
        assert!(accepts_char(';', true));
        assert!(!accepts_char('p', true));
    }

    #[test]
    fn numeric_input_supports_room_dimensions_circle_radius_and_backspace_editing() {
        use crate::app::{FloorPlanApp, Tool, tools::DrawKind};
        use eframe::egui::{self, Key};

        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);

        // Room by exact dimensions `3,5x4,0`.
        app.set_tool(Tool::Room);
        app.apply_canvas_point(Point::new(0.0, 0.0));
        app.draft_aim = Some(Point::new(2.0, 2.0));
        app.draft_input = "3,5x4,0".to_owned();
        app.commit_numeric_input();
        let corners = app.pending_room.as_ref().expect("pending room corners");
        assert!((crate::domain::geometry::polygon_area(corners) - 14.0).abs() < 1e-4);
        app.pending_room = None;

        // Circle by exact radius `2,5`.
        app.set_tool(Tool::Draw(DrawKind::Circle));
        app.apply_canvas_point(Point::new(5.0, 5.0));
        app.draft_input = "2,5".to_owned();
        app.commit_numeric_input();
        assert_eq!(app.project.drawings.len(), 1);
        match app.project.drawings[0].shape {
            crate::domain::drawing::Shape::Circle { center, radius } => {
                assert!(center.near(Point::new(5.0, 5.0), 1e-4));
                assert!((radius - 2.5).abs() < 1e-4);
            }
            ref other => panic!("expected circle, got {other:?}"),
        }

        // Backspace edits draft_input instead of undoing a RoomPolygon corner.
        app.set_tool(Tool::RoomPolygon);
        app.add_polygon_point(Point::new(0.0, 0.0));
        app.add_polygon_point(Point::new(3.0, 0.0));
        app.draft_input = "2,5".to_owned();
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        context.begin_pass(raw);
        app.handle_shortcuts(&context);
        context.end_pass().textures_delta.clear();
        assert_eq!(app.draft_input, "2,");
        assert_eq!(app.polygon_draft.len(), 2);

        // Switching tools clears draft_input.
        app.set_tool(Tool::Move);
        assert!(app.draft_input.is_empty());
    }
}
