//! Roof CRUD on `Project`, wall-height fitting and piece-by-piece roof analysis.

use crate::domain::{fixtures::FixtureKind, geometry::Point, model::Project};

use super::{
    Roof, RoofElement, RoofElementResult, RoofReaction, RoofResult,
    sizing::{CEILING_KN_M2, FRAMING_KN_M2, LIVE_KN_M2, Piece, beam_positions, size_piece},
};

impl Project {
    /// Adds a roof, returning its id.
    pub fn add_roof(&mut self, mut roof: Roof) -> Result<u64, String> {
        roof.check()?;
        roof.id = self.id();
        self.structure.roofs.push(roof);
        Ok(roof.id)
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
        let roof = *self
            .structure
            .roofs
            .iter()
            .find(|r| r.id == id)
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
        let slope_length = run / cos;
        let sized = |piece: Piece| {
            let (section, ok, bending, shear, deflection) = size_piece(&piece, roof.timber, cos);
            let total = piece.count as f64 * piece.each_m;
            RoofElementResult {
                element: piece.element,
                section_cm: section,
                ok,
                span_m: piece.span,
                spacing_m: piece.spacing,
                load_kn_m: piece.g + piece.q,
                bending_ratio: bending,
                shear_ratio: shear,
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
                each_m: eaves,
            }));
            // Rafters: down the slope, span = purlin spacing (ceil ensures spacing <= configured).
            let count = (eaves / roof.rafter_spacing_m).ceil() as usize + 1;
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
            }));
        }
        // Concrete columns and timber posts (#146) on the edges.
        let anchors: Vec<f64> = self
            .fixtures
            .iter()
            .filter(|f| matches!(f.kind, FixtureKind::Column | FixtureKind::TimberPost))
            .filter(|c| roof.on_edge(c.center, 0.35))
            .map(|c| roof.along(c.center))
            .collect();
        let positions = beam_positions(eaves, roof.beam_spacing_m, &anchors);
        let gaps: Vec<f64> = positions.windows(2).map(|p| p[1] - p[0]).collect();
        let widest = gaps.iter().copied().fold(0.0, f64::max);
        // Purlins: along the eaves, span = the widest gap between beams.
        let purlin_rows = (run / purlin_spacing).ceil() as usize + 1;
        let purlins = sized(Piece {
            element: RoofElement::Purlins,
            g: dead * purlin_spacing,
            q: LIVE_KN_M2 * purlin_spacing,
            span: widest,
            spacing: purlin_spacing,
            inclined: false,
            normal: 1.0,
            count: purlin_rows,
            each_m: eaves,
        });
        let purlin = purlins.section_cm;
        elements.push(purlins);
        let (_, _, _, density) = roof.timber.properties();
        let purlins_kn_m2 =
            density * 9.81 / 1000.0 * purlin.0 * purlin.1 / 10_000.0 / purlin_spacing;
        // Main beams: down the slope, carrying the widest tributary.
        let tributary = |i: usize| {
            let left = if i > 0 { gaps[i - 1] / 2.0 } else { 0.0 };
            let right = gaps.get(i).map_or(0.0, |g| g / 2.0);
            left + right
        };
        let widest_trib = (0..positions.len()).map(tributary).fold(0.0, f64::max);
        let beams = sized(Piece {
            element: RoofElement::Beams,
            g: (dead + purlins_kn_m2) * widest_trib,
            q: LIVE_KN_M2 * widest_trib,
            span: run,
            spacing: widest_trib,
            inclined: true,
            normal: 1.0,
            count: positions.len(),
            each_m: slope_length,
        });
        let beam = beams.section_cm;
        elements.push(beams);
        let beam_own = density * 9.81 / 1000.0 * beam.0 * beam.1 / 10_000.0 / cos;
        let mut reactions = Vec::new();
        for (i, along) in positions.iter().enumerate() {
            let t = tributary(i);
            let g = (dead + purlins_kn_m2) * t + beam_own;
            let q = LIVE_KN_M2 * t;
            for high in [true, false] {
                reactions.push(RoofReaction {
                    point: roof.edge_point(*along, high),
                    high,
                    load_kn: (g + q) * run / 2.0,
                    dead_kn: g * run / 2.0,
                    support: None,
                });
            }
        }
        let surface_m2 = slope_length * eaves;
        let timber_m3 = elements.iter().map(|e| e.volume_m3).sum();
        RoofResult {
            id: roof.id,
            label: self.roof_label(roof.id),
            slope_percent: slope * 100.0,
            min_slope_percent: tile.min_slope_percent(),
            surface_m2,
            dead_kn_m2: dead + purlins_kn_m2,
            live_kn_m2: LIVE_KN_M2,
            elements,
            reactions,
            tiles: tile.per_m2().map(|n| (surface_m2 * n * 1.05).ceil()),
            timber_m3,
            sheet_span_exceeded,
        }
    }
}
