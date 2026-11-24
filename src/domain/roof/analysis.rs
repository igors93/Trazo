//! Roof CRUD on `Project`, wall-height fitting and piece-by-piece roof analysis.

use crate::domain::{fixtures::FixtureKind, geometry::Point, model::Project};

use super::{
    Roof, RoofElement, RoofElementResult, RoofReaction, RoofResult, RoofStyle, RoofUpliftSummary,
    sizing::{
        CEILING_KN_M2, FRAMING_KN_M2, LIVE_KN_M2, Piece, beam_positions, size_howe_truss,
        size_piece,
    },
};

impl Project {
    /// Adds a roof, returning its id.
    pub fn add_roof(&mut self, mut roof: Roof) -> Result<u64, String> {
        roof.check()?;
        roof.id = self.id();
        let id = roof.id;
        self.structure.roofs.push(roof);
        Ok(id)
    }

    /// Replaces roof `id` with `roof` (its id kept).
    pub fn update_roof(&mut self, id: u64, mut roof: Roof) -> Result<(), String> {
        roof.check()?;
        roof.id = id;
        let slot = self
            .structure
            .roofs
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Telhado não encontrado.")?;
        *slot = roof;
        Ok(())
    }

    pub fn remove_roof(&mut self, id: u64) -> bool {
        let before = self.structure.roofs.len();
        self.structure.roofs.retain(|r| r.id != id);
        self.structure.roofs.len() != before
    }

    /// Gives the walls under roof `id` the roof's height at their ends
    /// (sloped tops for the gables). Returns how many changed.
    pub fn fit_walls_to_roof(&mut self, id: u64) -> Result<usize, String> {
        let roof = self
            .structure
            .roofs
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or("Telhado não encontrado.")?;
        let inside = |p: Point| {
            p.x >= roof.min.x - 0.3
                && p.x <= roof.max.x + 0.3
                && p.y >= roof.min.y - 0.3
                && p.y <= roof.max.y + 0.3
        };
        let walls: Vec<(u64, Point, Point)> = self
            .walls
            .iter()
            .filter(|w| inside(w.start) && inside(w.end))
            .map(|w| (w.id, w.start, w.end))
            .collect();
        let mut changed = 0;
        for (wall, start, end) in walls {
            let (a, b) = (roof.height_at(start), roof.height_at(end));
            if self.set_wall_height(wall, Some(a), Some(b)).is_ok() {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// The labels of the roofs, T1, T2… in reading order.
    fn roof_label(&self, id: u64) -> String {
        let mut ids: Vec<(u64, Point)> =
            self.structure.roofs.iter().map(|r| (r.id, r.min)).collect();
        ids.sort_by(|a, b| a.1.y.total_cmp(&b.1.y).then(a.1.x.total_cmp(&b.1.x)));
        let n = ids.iter().position(|(r, _)| *r == id).map_or(0, |i| i + 1);
        format!("T{n}")
    }

    /// Sizes the framing of `roof` and finds where its beams bear (the
    /// supports are matched by the structure report).
    pub fn analyse_roof(&self, roof: &Roof) -> RoofResult {
        let cos = roof.cos();
        let slope = roof.slope();
        let run = roof.run_m();
        let eaves = roof.eaves_m();
        let oh = roof.overhang_m.clamp(0.0, 1.5);
        let total_run = run + 2.0 * oh;
        let total_eaves = eaves + 2.0 * oh;
        let tile = roof.tile;
        let ceiling = if roof.exposed { 0.0 } else { CEILING_KN_M2 };
        // Sheet roofs (fibre-cement, metal) bear directly on purlins without battens/rafters.
        let framing = if tile.gauge_m().is_some() {
            FRAMING_KN_M2
        } else {
            0.0
        };
        let surface = (tile.weight_kn_m2() + framing) / cos;
        let dead = surface + ceiling;
        let mut elements = Vec::new();
        let mut sheet_span_exceeded = false;
        let slope_length = total_run / cos;
        let sized = |piece: Piece| {
            let (section, ok, bending, shear, deflection) = size_piece(&piece, roof.timber, cos);
            let total = piece.count as f64 * piece.each_m;
            let deflection_limit_m = piece.span / if piece.inclined { cos } else { 1.0 } / 200.0;
            RoofElementResult {
                element: piece.element,
                section_cm: section,
                ok,
                span_m: piece.span,
                spacing_m: piece.spacing,
                load_kn_m: piece.g + piece.q,
                bending_ratio: bending,
                shear_ratio: shear,
                deflection_m: deflection * deflection_limit_m,
                deflection_limit_m,
                deflection_ratio: deflection,
                count: piece.count,
                total_length_m: total,
                volume_m3: total * section.0 * section.1 / 10_000.0,
            }
        };
        let purlin_spacing = match tile.sheet_span_m() {
            Some(max) => {
                sheet_span_exceeded = roof.purlin_spacing_m / cos > max + 1e-9;
                roof.purlin_spacing_m
            }
            None => roof.purlin_spacing_m,
        };
        if let Some(gauge) = tile.gauge_m() {
            // Battens: tiles over the gauge, span = rafter spacing.
            let rows = (slope_length / gauge).ceil() as usize + 1;
            elements.push(sized(Piece {
                element: RoofElement::Battens,
                g: tile.weight_kn_m2() * gauge,
                q: LIVE_KN_M2 * gauge * cos,
                span: roof.rafter_spacing_m,
                spacing: gauge,
                inclined: false,
                normal: cos,
                count: rows,
                each_m: total_eaves,
                section_cm: None,
            }));
            // Rafters: down the slope, span = purlin spacing (ceil ensures spacing <= configured).
            let count = (total_eaves / roof.rafter_spacing_m).ceil() as usize + 1;
            elements.push(sized(Piece {
                element: RoofElement::Rafters,
                g: (surface + ceiling) * roof.rafter_spacing_m,
                q: LIVE_KN_M2 * roof.rafter_spacing_m,
                span: purlin_spacing,
                spacing: roof.rafter_spacing_m,
                inclined: true,
                normal: 1.0,
                count,
                each_m: slope_length,
                section_cm: None,
            }));
        }
        // Concrete columns and timber posts (#146) on the edges.
        let mut anchors: Vec<f64> = self
            .fixtures
            .iter()
            .filter(|f| matches!(f.kind, FixtureKind::Column | FixtureKind::TimberPost))
            .filter(|c| roof.on_edge(c.center, 0.35))
            .map(|c| roof.along(c.center))
            .collect();
        anchors.extend(roof.extra_beam_positions_m.iter().copied());
        let positions = beam_positions(eaves, roof.beam_spacing_m, &anchors);
        let gaps: Vec<f64> = positions.windows(2).map(|p| p[1] - p[0]).collect();
        let widest = gaps.iter().copied().fold(0.0, f64::max);
        // Purlins: along the eaves, span = the widest gap between beams/trusses.
        let purlin_rows = (total_run / purlin_spacing).ceil() as usize + 1;
        let purlins = sized(Piece {
            element: RoofElement::Purlins,
            g: dead * purlin_spacing,
            q: LIVE_KN_M2 * purlin_spacing,
            span: widest,
            spacing: purlin_spacing,
            inclined: false,
            normal: 1.0,
            count: purlin_rows,
            each_m: total_eaves,
            section_cm: None,
        });
        let purlin = purlins.section_cm;
        elements.push(purlins);
        let (fc0k, _, _, density) = roof.timber.properties();
        let purlins_kn_m2 =
            density * 9.81 / 1000.0 * purlin.0 * purlin.1 / 10_000.0 / purlin_spacing;
        // Main beams or pitched trusses: down the span, carrying the widest tributary.
        let tributary = |i: usize| {
            let left = if i > 0 { gaps[i - 1] / 2.0 } else { 0.0 };
            let right = gaps.get(i).map_or(0.0, |g| g / 2.0);
            let end_oh = if i == 0 || i + 1 == positions.len() {
                oh
            } else {
                0.0
            };
            left + right + end_oh
        };
        let widest_trib = (0..positions.len()).map(tributary).fold(0.0, f64::max);
        let beams = sized(Piece {
            element: RoofElement::Beams,
            g: (dead + purlins_kn_m2) * widest_trib,
            q: LIVE_KN_M2 * widest_trib,
            span: if roof.style == RoofStyle::MonoPitch {
                run
            } else {
                roof.pitch_run_m()
            },
            spacing: widest_trib,
            inclined: true,
            normal: 1.0,
            count: positions.len(),
            each_m: slope_length,
            section_cm: roof.beam_section_cm,
        });
        let beam = beams.section_cm;
        elements.push(beams);

        // Wall plates (*frechais*) along the eave lines (full perimeter on Hip roofs).
        let wall_plate_count = if roof.style == RoofStyle::Hip { 4 } else { 2 };
        let wall_plate_each = if roof.style == RoofStyle::Hip {
            0.5 * (eaves + run)
        } else {
            eaves
        };
        let plate_span = widest.clamp(1.0, 2.0);
        let wall_plate_res = sized(Piece {
            element: RoofElement::WallPlates,
            g: (dead + purlins_kn_m2) * (total_run * 0.5),
            q: LIVE_KN_M2 * (total_run * 0.5),
            span: plate_span,
            spacing: total_run * 0.5,
            inclined: false,
            normal: 1.0,
            count: wall_plate_count,
            each_m: wall_plate_each,
            section_cm: None,
        });

        // Pitched Howe timber trusses (*tesouras*) for gable/hip roofs or when requested.
        let use_trusses = roof.style != RoofStyle::MonoPitch || roof.use_trusses;
        let truss = if use_trusses {
            let total_design_load_kn =
                1.4 * (dead + purlins_kn_m2 + LIVE_KN_M2) * widest_trib * total_run;
            let truss_count = if roof.style == RoofStyle::Hip {
                positions.len().saturating_sub(2).max(1)
            } else {
                positions.len()
            };
            Some(size_howe_truss(
                run,
                (roof.high_m - roof.low_m).max(0.30),
                widest_trib,
                total_design_load_kn,
                truss_count,
                roof.timber,
            ))
        } else {
            None
        };

        // Diagonal hip rafters (*espigões*) for 4-water hip roofs.
        let hip_rafters = if roof.style == RoofStyle::Hip {
            let hip_horiz = (roof.pitch_run_m() + oh) * std::f64::consts::SQRT_2;
            let hip_cos = hip_horiz / hip_horiz.hypot(roof.high_m - roof.low_m).max(1e-6);
            let hip_trib = roof.pitch_run_m() * 0.5;
            Some(sized(Piece {
                element: RoofElement::HipRafters,
                g: (dead + purlins_kn_m2) * hip_trib,
                q: LIVE_KN_M2 * hip_trib,
                span: hip_horiz,
                spacing: hip_trib,
                inclined: true,
                normal: 1.0,
                count: 4,
                each_m: hip_horiz / hip_cos.max(0.2),
                section_cm: None,
            }))
        } else {
            None
        };

        // Overhang cantilever bending check (M_cant = 1.4 * (g + q) * oh^2 / 2).
        let overhang_bending_ratio = if oh > 0.01 {
            let (b_cm, h_cm) = elements
                .iter()
                .find(|e| e.element == RoofElement::Rafters)
                .map(|e| e.section_cm)
                .unwrap_or(beam);
            let (b, h) = (b_cm / 100.0, h_cm / 100.0);
            let w_sec = (b * h * h / 6.0).max(1e-6);
            let fc0d = super::materials::KMOD * fc0k / 1.4 * 1000.0;
            let trib_oh = if tile.gauge_m().is_some() {
                roof.rafter_spacing_m
            } else {
                widest_trib
            };
            let wd = 1.4 * (dead + LIVE_KN_M2) * trib_oh;
            let m_cant = wd * oh * oh * 0.5;
            m_cant / w_sec / fc0d
        } else {
            0.0
        };

        // Integrated NBR 6123 wind uplift check (including overhang edge suction).
        let q_wind_kpa = self
            .structure
            .wind
            .pressure_kpa(roof.high_m, total_eaves.max(total_run));
        let cpe_main = if roof.style == RoofStyle::MonoPitch && roof.exposed {
            1.20
        } else {
            0.80
        };
        let suction_kpa = q_wind_kpa * cpe_main;
        let overhang_suction_kpa = if oh > 0.01 { q_wind_kpa * 1.50 } else { 0.0 };
        let dead_total_kn_m2 = dead + purlins_kn_m2;
        let net_design_uplift_kpa =
            (1.4 * suction_kpa.max(overhang_suction_kpa * 0.85) - 0.9 * dead_total_kn_m2).max(0.0);
        let anchor_design_kn = net_design_uplift_kpa * widest_trib * (total_run * 0.5);
        let uplift = RoofUpliftSummary {
            wind_v0_ms: self.structure.wind.v0_ms,
            suction_kpa,
            overhang_suction_kpa,
            net_design_uplift_kpa,
            anchor_design_kn,
            lifts: net_design_uplift_kpa > 1e-4,
        };

        let beam_own = density * 9.81 / 1000.0 * beam.0 * beam.1 / 10_000.0 / cos;
        let mut reactions = Vec::new();
        for (i, along) in positions.iter().enumerate() {
            let t = tributary(i);
            let g = dead_total_kn_m2 * t + beam_own;
            let q = LIVE_KN_M2 * t;
            for high in [true, false] {
                reactions.push(RoofReaction {
                    point: roof.edge_point(*along, high),
                    high,
                    load_kn: (g + q) * total_run / 2.0,
                    dead_kn: g * total_run / 2.0,
                    support: None,
                });
            }
        }
        let surface_m2 = slope_length * total_eaves;
        let mut timber_m3: f64 = elements.iter().map(|e| e.volume_m3).sum();
        if roof.style != RoofStyle::MonoPitch || roof.use_trusses || oh > 0.01 {
            timber_m3 += wall_plate_res.volume_m3;
        }
        if let Some(t) = &truss {
            timber_m3 += t.volume_m3;
        }
        if let Some(h) = &hip_rafters {
            timber_m3 += h.volume_m3;
        }
        RoofResult {
            id: roof.id,
            label: self.roof_label(roof.id),
            style: roof.style,
            overhang_m: oh,
            slope_percent: slope * 100.0,
            min_slope_percent: tile.min_slope_percent(),
            surface_m2,
            dead_kn_m2: dead_total_kn_m2,
            live_kn_m2: LIVE_KN_M2,
            elements,
            wall_plate: Some(wall_plate_res),
            truss,
            hip_rafters,
            overhang_bending_ratio,
            uplift,
            reactions,
            tiles: tile.per_m2().map(|n| (surface_m2 * n * 1.05).ceil()),
            timber_m3,
            sheet_span_exceeded,
        }
    }
}
