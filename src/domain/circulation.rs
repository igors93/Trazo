//! Vertical circulation and level transitions: straight/L/U stairs (with
//! Blondel ergonomics check), accessible ramps (ABNT NBR 9050 slope check),
//! landings, railings (guarda-corpos) and floor level changes (desníveis).

use serde::{Deserialize, Serialize};

use super::{geometry::Point, model::Project, notes::format_elevation};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StairShape {
    #[default]
    Straight,
    LShaped,
    UShaped,
}

impl StairShape {
    pub const ALL: [StairShape; 3] = [
        StairShape::Straight,
        StairShape::LShaped,
        StairShape::UShaped,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Straight => "Lance reto",
            Self::LShaped => "Em L (com patamar)",
            Self::UShaped => "Em U (com patamar)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RailingSide {
    None,
    Left,
    Right,
    #[default]
    Both,
}

impl RailingSide {
    pub const ALL: [RailingSide; 4] = [
        RailingSide::None,
        RailingSide::Left,
        RailingSide::Right,
        RailingSide::Both,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Sem guarda-corpo",
            Self::Left => "Esquerda",
            Self::Right => "Direita",
            Self::Both => "Ambos os lados",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CirculationKind {
    Stair {
        #[serde(default)]
        shape: StairShape,
        width_m: f64,
        total_rise_m: f64,
        tread_m: f64,
        #[serde(default = "default_landing")]
        landing_depth_m: f64,
    },
    Ramp {
        width_m: f64,
        length_m: f64,
        total_rise_m: f64,
        #[serde(default = "default_landing")]
        landing_depth_m: f64,
    },
    Landing {
        width_m: f64,
        depth_m: f64,
        elevation_m: f64,
    },
    Railing {
        end: Point,
        #[serde(default = "default_railing_height")]
        height_m: f64,
    },
    LevelChange {
        end: Point,
        from_elev_m: f64,
        to_elev_m: f64,
    },
}

fn default_landing() -> f64 {
    1.20
}

fn default_railing_height() -> f64 {
    1.10
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CirculationElement {
    pub id: u64,
    pub name: String,
    pub origin: Point,
    #[serde(default)]
    pub rotation_deg: f64,
    #[serde(default)]
    pub railing: RailingSide,
    pub kind: CirculationKind,
}

#[derive(Clone, Debug, Default)]
pub struct CirculationGeometry {
    pub polygons: Vec<Vec<Point>>,
    pub lines: Vec<(Point, Point)>,
    pub railing_lines: Vec<(Point, Point)>,
    pub arrow: Option<(Point, Point)>,
    pub label_pos: Point,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StairMetrics {
    pub risers: usize,
    pub riser_height_m: f64,
    pub tread_m: f64,
    /// Blondel formula `2e + p` (ideal 0.63–0.65 m, comfortable 0.60–0.66 m).
    pub blondel_m: f64,
    pub blondel_ok: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RampMetrics {
    pub slope_percent: f64,
    /// ABNT NBR 9050 recommends <= 8.33% for accessible ramps.
    pub nbr_9050_ok: bool,
}

impl CirculationElement {
    pub fn stair_metrics(&self) -> Option<StairMetrics> {
        let CirculationKind::Stair {
            total_rise_m,
            tread_m,
            ..
        } = self.kind
        else {
            return None;
        };
        let risers = (total_rise_m / 0.175).round().clamp(2.0, 40.0) as usize;
        let riser_height_m = total_rise_m / risers as f64;
        let blondel_m = 2.0 * riser_height_m + tread_m;
        let blondel_ok =
            (0.60..=0.66).contains(&blondel_m) && (0.15..=0.19).contains(&riser_height_m);
        Some(StairMetrics {
            risers,
            riser_height_m,
            tread_m,
            blondel_m,
            blondel_ok,
        })
    }

    pub fn ramp_metrics(&self) -> Option<RampMetrics> {
        let CirculationKind::Ramp {
            length_m,
            total_rise_m,
            ..
        } = self.kind
        else {
            return None;
        };
        let slope_percent = (total_rise_m / length_m.max(0.1)) * 100.0;
        Some(RampMetrics {
            slope_percent,
            nbr_9050_ok: slope_percent <= 8.34,
        })
    }

    pub fn summary(&self) -> String {
        match &self.kind {
            CirculationKind::Stair { shape, .. } => {
                let Some(m) = self.stair_metrics() else {
                    return shape.label().to_owned();
                };
                let status = if m.blondel_ok {
                    "OK"
                } else {
                    "Ajustar Blondel"
                };
                format!(
                    "{} · {} degraus (e={:.1} / p={:.0} cm · Blondel {:.0} cm · {})",
                    shape.label(),
                    m.risers,
                    m.riser_height_m * 100.0,
                    m.tread_m * 100.0,
                    m.blondel_m * 100.0,
                    status
                )
                .replace('.', ",")
            }
            CirculationKind::Ramp { .. } => {
                let Some(m) = self.ramp_metrics() else {
                    return "Rampa".to_owned();
                };
                let status = if m.nbr_9050_ok {
                    "NBR 9050 OK"
                } else {
                    "Acima de 8,33%"
                };
                format!("Rampa · i = {:.1}% ({status})", m.slope_percent).replace('.', ",")
            }
            CirculationKind::Landing {
                width_m,
                depth_m,
                elevation_m,
            } => format!(
                "Patamar {:.2} × {:.2} m · nível {}",
                width_m,
                depth_m,
                format_elevation(*elevation_m)
            )
            .replace('.', ","),
            CirculationKind::Railing { end, height_m } => format!(
                "Guarda-corpo {:.2} m · h = {:.2} m",
                self.origin.distance(*end),
                height_m
            )
            .replace('.', ","),
            CirculationKind::LevelChange {
                from_elev_m,
                to_elev_m,
                ..
            } => format!(
                "Desnível {} → {}",
                format_elevation(*from_elev_m),
                format_elevation(*to_elev_m)
            ),
        }
    }

    fn to_world(&self, local: Point) -> Point {
        let rad = self.rotation_deg.to_radians();
        let (c, s) = (rad.cos(), rad.sin());
        Point::new(
            self.origin.x + local.x * c - local.y * s,
            self.origin.y + local.x * s + local.y * c,
        )
    }

    pub fn plan_geometry(&self) -> CirculationGeometry {
        let mut geom = CirculationGeometry {
            label_pos: self.origin,
            ..Default::default()
        };
        match &self.kind {
            CirculationKind::Stair {
                shape,
                width_m,
                tread_m,
                landing_depth_m,
                ..
            } => {
                let Some(m) = self.stair_metrics() else {
                    return geom;
                };
                let treads = m.risers.saturating_sub(1).max(1);
                let w = width_m.max(0.6);
                let p = tread_m.max(0.22);
                let ld = landing_depth_m.max(w);

                let add_flight = |geom: &mut CirculationGeometry,
                                  x0: f64,
                                  y0: f64,
                                  dx: f64,
                                  dy: f64,
                                  nx: f64,
                                  ny: f64,
                                  steps: usize,
                                  railing: RailingSide| {
                    let len = steps as f64 * p;
                    let a = self.to_world(Point::new(x0, y0));
                    let b = self.to_world(Point::new(x0 + dx * len, y0 + dy * len));
                    let c =
                        self.to_world(Point::new(x0 + dx * len + nx * w, y0 + dy * len + ny * w));
                    let d = self.to_world(Point::new(x0 + nx * w, y0 + ny * w));
                    geom.polygons.push(vec![a, b, c, d]);
                    for i in 1..steps {
                        let s = i as f64 * p;
                        let p0 = self.to_world(Point::new(x0 + dx * s, y0 + dy * s));
                        let p1 =
                            self.to_world(Point::new(x0 + dx * s + nx * w, y0 + dy * s + ny * w));
                        geom.lines.push((p0, p1));
                    }
                    if matches!(railing, RailingSide::Left | RailingSide::Both) {
                        geom.railing_lines.push((a, b));
                    }
                    if matches!(railing, RailingSide::Right | RailingSide::Both) {
                        geom.railing_lines.push((d, c));
                    }
                };

                match shape {
                    StairShape::Straight => {
                        let len = treads as f64 * p;
                        add_flight(
                            &mut geom,
                            0.0,
                            0.0,
                            1.0,
                            0.0,
                            0.0,
                            1.0,
                            treads,
                            self.railing,
                        );
                        geom.arrow = Some((
                            self.to_world(Point::new(0.25, w * 0.5)),
                            self.to_world(Point::new((len - 0.25).max(0.4), w * 0.5)),
                        ));
                        geom.label_pos = self.to_world(Point::new(len * 0.5, -0.25));
                    }
                    StairShape::LShaped => {
                        let first = treads / 2;
                        let second = treads - first;
                        let l1 = first as f64 * p;
                        add_flight(&mut geom, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, first, self.railing);
                        // Corner landing
                        geom.polygons.push(vec![
                            self.to_world(Point::new(l1, 0.0)),
                            self.to_world(Point::new(l1 + ld, 0.0)),
                            self.to_world(Point::new(l1 + ld, w)),
                            self.to_world(Point::new(l1, w)),
                        ]);
                        add_flight(&mut geom, l1, w, 0.0, 1.0, 1.0, 0.0, second, self.railing);
                        geom.arrow = Some((
                            self.to_world(Point::new(0.25, w * 0.5)),
                            self.to_world(Point::new(l1 + ld * 0.5, w * 0.5)),
                        ));
                        geom.label_pos = self.to_world(Point::new(l1 * 0.5, -0.25));
                    }
                    StairShape::UShaped => {
                        let first = treads / 2;
                        let second = treads - first;
                        let l1 = first as f64 * p;
                        let gap = 0.15;
                        add_flight(&mut geom, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, first, self.railing);
                        // Return landing spanning both flights + gap
                        geom.polygons.push(vec![
                            self.to_world(Point::new(l1, 0.0)),
                            self.to_world(Point::new(l1 + ld, 0.0)),
                            self.to_world(Point::new(l1 + ld, w * 2.0 + gap)),
                            self.to_world(Point::new(l1, w * 2.0 + gap)),
                        ]);
                        add_flight(
                            &mut geom,
                            l1,
                            w + gap,
                            -1.0,
                            0.0,
                            0.0,
                            1.0,
                            second,
                            self.railing,
                        );
                        geom.arrow = Some((
                            self.to_world(Point::new(0.25, w * 0.5)),
                            self.to_world(Point::new((l1 - 0.2).max(0.4), w * 0.5)),
                        ));
                        geom.label_pos = self.to_world(Point::new(l1 * 0.5, -0.25));
                    }
                }
                geom.label = format!("SOBE ({} deg.)", m.risers);
            }
            CirculationKind::Ramp {
                width_m,
                length_m,
                landing_depth_m,
                ..
            } => {
                let w = width_m.max(0.8);
                let l = length_m.max(0.8);
                let ld = landing_depth_m.max(0.0);
                let a = self.to_world(Point::new(0.0, 0.0));
                let b = self.to_world(Point::new(l, 0.0));
                let c = self.to_world(Point::new(l, w));
                let d = self.to_world(Point::new(0.0, w));
                geom.polygons.push(vec![a, b, c, d]);
                if ld > 0.1 {
                    geom.polygons.push(vec![
                        self.to_world(Point::new(l, 0.0)),
                        self.to_world(Point::new(l + ld, 0.0)),
                        self.to_world(Point::new(l + ld, w)),
                        self.to_world(Point::new(l, w)),
                    ]);
                }
                if matches!(self.railing, RailingSide::Left | RailingSide::Both) {
                    geom.railing_lines.push((a, b));
                }
                if matches!(self.railing, RailingSide::Right | RailingSide::Both) {
                    geom.railing_lines.push((d, c));
                }
                geom.arrow = Some((
                    self.to_world(Point::new(0.25, w * 0.5)),
                    self.to_world(Point::new((l - 0.25).max(0.5), w * 0.5)),
                ));
                geom.label_pos = self.to_world(Point::new(l * 0.5, -0.25));
                if let Some(m) = self.ramp_metrics() {
                    geom.label = format!("RAMPA i={:.1}%", m.slope_percent).replace('.', ",");
                }
            }
            CirculationKind::Landing {
                width_m,
                depth_m,
                elevation_m,
            } => {
                let a = self.to_world(Point::new(0.0, 0.0));
                let b = self.to_world(Point::new(*width_m, 0.0));
                let c = self.to_world(Point::new(*width_m, *depth_m));
                let d = self.to_world(Point::new(0.0, *depth_m));
                geom.polygons.push(vec![a, b, c, d]);
                geom.lines.push((a, c));
                geom.label_pos = self.to_world(Point::new(width_m * 0.5, depth_m * 0.5));
                geom.label = format!("PATAMAR {}", format_elevation(*elevation_m));
            }
            CirculationKind::Railing { end, height_m } => {
                let dir = end.minus(self.origin);
                let normal = dir
                    .normalized()
                    .map(|u| Point::new(-u.y, u.x).scaled(0.03))
                    .unwrap_or(Point::new(0.0, 0.03));
                geom.railing_lines
                    .push((self.origin.translated(normal), end.translated(normal)));
                geom.railing_lines
                    .push((self.origin.minus(normal), end.minus(normal)));
                geom.label_pos = self.origin.midpoint(*end);
                geom.label = format!("GC h={:.2}m", height_m).replace('.', ",");
            }
            CirculationKind::LevelChange {
                end,
                from_elev_m,
                to_elev_m,
            } => {
                let dir = end.minus(self.origin);
                let normal = dir
                    .normalized()
                    .map(|u| Point::new(-u.y, u.x).scaled(0.05))
                    .unwrap_or(Point::new(0.0, 0.05));
                geom.lines.push((self.origin, *end));
                geom.lines
                    .push((self.origin.translated(normal), end.translated(normal)));
                geom.label_pos = self.origin.midpoint(*end);
                geom.label = format!(
                    "{} / {}",
                    format_elevation(*from_elev_m),
                    format_elevation(*to_elev_m)
                );
            }
        }
        geom
    }
}

impl Project {
    fn next_circulation_id(&self) -> u64 {
        self.circulations.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    pub fn add_stair(
        &mut self,
        origin: Point,
        shape: StairShape,
        width_m: f64,
        total_rise_m: f64,
        tread_m: f64,
    ) -> Result<u64, String> {
        if !origin.is_finite()
            || !(0.60..=4.0).contains(&width_m)
            || !(0.30..=6.0).contains(&total_rise_m)
            || !(0.22..=0.45).contains(&tread_m)
        {
            return Err("Medidas de escada fora dos limites técnicos.".to_owned());
        }
        let id = self.next_circulation_id();
        self.circulations.push(CirculationElement {
            id,
            name: format!("Escada {id}"),
            origin: origin.rounded(),
            rotation_deg: 0.0,
            railing: RailingSide::Both,
            kind: CirculationKind::Stair {
                shape,
                width_m,
                total_rise_m,
                tread_m,
                landing_depth_m: width_m.max(1.20),
            },
        });
        Ok(id)
    }

    pub fn add_ramp(
        &mut self,
        origin: Point,
        width_m: f64,
        length_m: f64,
        total_rise_m: f64,
    ) -> Result<u64, String> {
        if !origin.is_finite()
            || !(0.80..=5.0).contains(&width_m)
            || !(0.80..=30.0).contains(&length_m)
            || !(0.05..=4.0).contains(&total_rise_m)
        {
            return Err("Medidas de rampa fora dos limites técnicos.".to_owned());
        }
        let id = self.next_circulation_id();
        self.circulations.push(CirculationElement {
            id,
            name: format!("Rampa {id}"),
            origin: origin.rounded(),
            rotation_deg: 0.0,
            railing: RailingSide::Both,
            kind: CirculationKind::Ramp {
                width_m,
                length_m,
                total_rise_m,
                landing_depth_m: 1.20,
            },
        });
        Ok(id)
    }

    pub fn add_landing(
        &mut self,
        origin: Point,
        width_m: f64,
        depth_m: f64,
        elevation_m: f64,
    ) -> Result<u64, String> {
        if !origin.is_finite()
            || !(0.5..=10.0).contains(&width_m)
            || !(0.5..=10.0).contains(&depth_m)
        {
            return Err("Medidas de patamar inválidas.".to_owned());
        }
        let id = self.next_circulation_id();
        self.circulations.push(CirculationElement {
            id,
            name: format!("Patamar {id}"),
            origin: origin.rounded(),
            rotation_deg: 0.0,
            railing: RailingSide::None,
            kind: CirculationKind::Landing {
                width_m,
                depth_m,
                elevation_m,
            },
        });
        Ok(id)
    }

    pub fn add_railing(&mut self, start: Point, end: Point, height_m: f64) -> Result<u64, String> {
        if !start.is_finite() || !end.is_finite() || start.distance(end) < 0.25 {
            return Err("O guarda-corpo precisa ter pelo menos 25 cm.".to_owned());
        }
        let id = self.next_circulation_id();
        self.circulations.push(CirculationElement {
            id,
            name: format!("Guarda-corpo {id}"),
            origin: start.rounded(),
            rotation_deg: 0.0,
            railing: RailingSide::Both,
            kind: CirculationKind::Railing {
                end: end.rounded(),
                height_m: height_m.clamp(0.80, 1.80),
            },
        });
        Ok(id)
    }

    pub fn add_level_change(
        &mut self,
        start: Point,
        end: Point,
        from_elev_m: f64,
        to_elev_m: f64,
    ) -> Result<u64, String> {
        if !start.is_finite() || !end.is_finite() || start.distance(end) < 0.25 {
            return Err("A linha de desnível precisa ter pelo menos 25 cm.".to_owned());
        }
        let id = self.next_circulation_id();
        self.circulations.push(CirculationElement {
            id,
            name: format!("Desnível {id}"),
            origin: start.rounded(),
            rotation_deg: 0.0,
            railing: RailingSide::None,
            kind: CirculationKind::LevelChange {
                end: end.rounded(),
                from_elev_m,
                to_elev_m,
            },
        });
        Ok(id)
    }

    pub fn remove_circulation(&mut self, id: u64) -> bool {
        let before = self.circulations.len();
        self.circulations.retain(|c| c.id != id);
        self.circulations.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stairs_ramps_landings_railings_and_level_changes_compute_metrics_and_geometry() {
        let mut project = Project::default();
        // 2.80 m floor-to-floor rise -> 16 risers of 17.5 cm + 28 cm tread => Blondel = 63 cm (OK!).
        let s1 = project
            .add_stair(Point::new(0.0, 0.0), StairShape::Straight, 1.20, 2.80, 0.28)
            .unwrap();
        let stair = &project.circulations[0];
        let m = stair.stair_metrics().unwrap();
        assert_eq!(m.risers, 16);
        assert!((m.riser_height_m - 0.175).abs() < 1e-6);
        assert!((m.blondel_m - 0.63).abs() < 1e-6);
        assert!(m.blondel_ok);
        let geom = stair.plan_geometry();
        assert_eq!(geom.polygons.len(), 1);
        assert_eq!(geom.lines.len(), 14); // 15 treads -> 14 internal riser lines
        assert_eq!(geom.railing_lines.len(), 2);
        assert!(geom.arrow.is_some());

        // U-shaped stair has 2 flights + 1 return landing = 3 polygons.
        let s_u = project
            .add_stair(Point::new(5.0, 0.0), StairShape::UShaped, 1.20, 2.80, 0.28)
            .unwrap();
        let u_geom = project
            .circulations
            .iter()
            .find(|c| c.id == s_u)
            .unwrap()
            .plan_geometry();
        assert_eq!(u_geom.polygons.len(), 3);

        // Accessible ramp: 0.50 m rise over 6.00 m => 8.33% (NBR 9050 OK!).
        let r1 = project
            .add_ramp(Point::new(0.0, 4.0), 1.20, 6.00, 0.50)
            .unwrap();
        let ramp = project.circulations.iter().find(|c| c.id == r1).unwrap();
        let rm = ramp.ramp_metrics().unwrap();
        assert!((rm.slope_percent - 8.3333).abs() < 0.05);
        assert!(rm.nbr_9050_ok);

        // Landing, railing, and level change.
        let _l1 = project
            .add_landing(Point::new(2.0, 2.0), 1.50, 1.50, 0.45)
            .unwrap();
        let _gc = project
            .add_railing(Point::new(0.0, 6.0), Point::new(3.0, 6.0), 1.10)
            .unwrap();
        let _dc = project
            .add_level_change(Point::new(0.0, 7.0), Point::new(3.0, 7.0), 0.0, -0.15)
            .unwrap();

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.circulations.len(), 6);
        assert!(project.remove_circulation(s1));
        assert_eq!(project.circulations.len(), 5);
    }
}
