//! Electrical plan symbols (NBR 5444) and tag placement.

use crate::domain::{
    fixtures::{FixtureKind, SymbolShape, Weight},
    geometry::Point,
    model::Project,
};

use super::DeviceRole;

/// practice: triangles for outlets (emptier the lower they are mounted),
/// circles for switches and lights, a hatched box for the panel.
pub(in crate::domain) fn symbol(kind: FixtureKind, width: f64, depth: f64) -> Vec<SymbolShape> {
    use Weight::{Detail, Outline};
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let poly = |points: &[(f64, f64)], closed: bool, weight: Weight| SymbolShape::Polyline {
        points: points.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
        closed,
        dashed: false,
        weight,
    };
    let circle = |x: f64, y: f64, radius: f64, weight: Weight| SymbolShape::Circle {
        center: Point::new(x, y),
        radius,
        weight,
    };
    // An outlet: a triangle with its base on the wall, filled with hatching
    // from the wall up to `fill` (0 empty, 1 full).
    let outlet = |fill: f64| {
        let base = hw * 0.85;
        let mut shapes = vec![poly(&[(-base, -hd), (base, -hd), (0.0, hd)], true, Outline)];
        let rows = 5;
        for row in 1..rows {
            let t = f64::from(row) / f64::from(rows);
            if t > fill {
                break;
            }
            let y = -hd + t * 2.0 * hd;
            let half = base * (1.0 - t);
            shapes.push(poly(&[(-half, y), (half, y)], false, Detail));
        }
        shapes
    };
    let radius = width.min(depth) * 0.42;
    match kind {
        FixtureKind::OutletLow => outlet(0.0),
        FixtureKind::OutletMid => outlet(0.5),
        FixtureKind::OutletHigh => outlet(1.0),
        FixtureKind::OutletSpecific => {
            let mut shapes = outlet(0.0);
            shapes.push(circle(0.0, -hd / 3.0, radius * 0.35, Outline));
            shapes
        }
        FixtureKind::Switch | FixtureKind::SwitchTwoWay | FixtureKind::SwitchIntermediate => {
            let r = radius * 0.8;
            let mut shapes = vec![
                circle(0.0, 0.0, r, Outline),
                poly(&[(0.0, -hd), (0.0, -r)], false, Detail),
            ];
            if kind == FixtureKind::SwitchIntermediate {
                // Four-way (intermediate) switch: crossed strokes inside the circle.
                let d = r * 0.55;
                shapes.push(poly(&[(-d, d), (d, -d)], false, Outline));
                shapes.push(poly(&[(-d, -d), (d, d)], false, Outline));
            } else {
                // One stroke for a one-way switch, two for a two-way (three-way).
                let strokes: &[f64] = if kind == FixtureKind::Switch {
                    &[0.0]
                } else {
                    &[-r * 0.45, r * 0.45]
                };
                for dx in strokes {
                    shapes.push(poly(
                        &[(dx - r * 0.35, r * 0.55), (dx + r * 0.35, -r * 0.55)],
                        false,
                        Outline,
                    ));
                }
            }
            shapes
        }
        FixtureKind::CeilingLight => {
            let d = radius * std::f64::consts::FRAC_1_SQRT_2;
            vec![
                circle(0.0, 0.0, radius, Outline),
                poly(&[(-d, -d), (d, d)], false, Detail),
                poly(&[(-d, d), (d, -d)], false, Detail),
            ]
        }
        FixtureKind::WallLight => {
            let r = hw.min(depth) * 0.9;
            let arc: Vec<(f64, f64)> = (0..=12)
                .map(|i| {
                    let a = std::f64::consts::PI * f64::from(i) / 12.0;
                    (r * a.cos(), -hd + r * a.sin())
                })
                .collect();
            vec![poly(&arc, true, Outline)]
        }
        FixtureKind::Panel => {
            let mut shapes = vec![poly(
                &[(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)],
                true,
                Outline,
            )];
            // Diagonal hatching, clipped to the box.
            let step = width / 6.0;
            let mut x = -hw - depth;
            while x < hw {
                let (x0, y0) = (x.max(-hw), -hd + (x.max(-hw) - x));
                let x1 = (x + depth).min(hw);
                let y1 = -hd + (x1 - x);
                if x1 > x0 && y0 < hd {
                    shapes.push(poly(&[(x0, y0), (x1, y1.min(hd))], false, Detail));
                }
                x += step;
            }
            shapes
        }
        FixtureKind::DataPoint => vec![
            poly(
                &[
                    (-hw * 0.8, -hd),
                    (hw * 0.8, -hd),
                    (hw * 0.8, hd),
                    (-hw * 0.8, hd),
                ],
                true,
                Outline,
            ),
            poly(&[(-hw * 0.8, hd), (hw * 0.8, -hd)], false, Detail),
        ],
        _ => Vec::new(),
    }
}

/// The room a tag takes on the plan, in metres: its text is drawn about
/// 0.13 m high on screen and on paper.
pub fn tag_size(text: &str) -> (f64, f64) {
    (0.075 * text.chars().count() as f64 + 0.06, 0.2)
}

impl Project {
    /// The tag of each device, where it goes (in front of the device, in the
    /// room) and what it says: the circuit, and the power when it is not the
    /// usual 100 VA of a general outlet ("C2 · 600 VA").
    pub fn electrical_tags(&self) -> Vec<(Point, String)> {
        self.electrical_devices()
            .filter_map(|device| {
                let data = device.electrical?;
                let role = device.kind.role()?;
                let circuit = data.circuit.map(|n| format!("C{n}"));
                let power = (data.power_va > 0.0).then(|| format!("{:.0} VA", data.power_va));
                let text = match role {
                    DeviceRole::Panel => "QD".to_owned(),
                    DeviceRole::Data => return None,
                    DeviceRole::Switch => circuit?,
                    DeviceRole::GeneralOutlet if (data.power_va - 100.0).abs() < 1e-6 => circuit?,
                    _ => [circuit, power]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · "),
                };
                if text.is_empty() {
                    return None;
                }
                // The front of a device faces away from its wall.
                let front = match device.quarter_turns % 4 {
                    0 => Point::new(0.0, 1.0),
                    1 => Point::new(-1.0, 0.0),
                    2 => Point::new(0.0, -1.0),
                    _ => Point::new(1.0, 0.0),
                };
                // Facing sideways, the text runs along the front: it starts
                // past the symbol instead of covering it.
                let sideways = front.x.abs() * tag_size(&text).0 / 2.0;
                let reach = if device.kind.mounts_on_wall() {
                    device.depth / 2.0 + 0.12 + sideways
                } else {
                    device.depth / 2.0 + 0.12
                };
                Some((device.center, front, reach, text))
            })
            .fold(
                Vec::new(),
                |mut tags: Vec<(Point, String)>, (center, front, reach, text)| {
                    // Push a tag further out while it would touch an earlier one.
                    let mut anchor = center.translated(front.scaled(reach));
                    for _ in 0..4 {
                        let size = tag_size(&text);
                        let clash = tags.iter().any(|(other, other_text)| {
                            let other_size = tag_size(other_text);
                            (anchor.x - other.x).abs() < (size.0 + other_size.0) / 2.0
                                && (anchor.y - other.y).abs() < (size.1 + other_size.1) / 2.0
                        });
                        if !clash {
                            break;
                        }
                        anchor = anchor.translated(front.scaled(0.2));
                    }
                    tags.push((anchor, text));
                    tags
                },
            )
    }
}
