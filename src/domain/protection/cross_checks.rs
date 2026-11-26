//! Independent cross-checks (#172): a second, simpler opinion on the
//! structure. The detailed simulation follows the load path beam by beam;
//! these checks only weigh the house (walls and slabs, floor by floor) and
//! compare the total with what the simulation carried to the ground and with
//! what the foundations can hold. A large disagreement means something is
//! missing from the model (walls without foundations, slabs, loads), so the
//! detailed result is contested instead of trusted.

use crate::domain::model::Project;
use crate::domain::structure::StructureReport;
use crate::domain::wall_foundations::WallFoundationKind;

/// The simulation must carry at least this share of the simple weight.
pub const MIN_CARRIED_SHARE: f64 = 0.6;

/// One independent check: utilisation as in the traffic light (1.0 is the
/// limit), with what to do and the numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct CrossCheck {
    pub name: &'static str,
    pub utilisation: f64,
    pub simple: String,
    pub detail: String,
}

/// The house's weight by the simple method: walls (net of openings) and
/// slabs (self weight, finish and live load), times the floors the columns
/// carry, kN. `None` when it cannot be compared fairly: no structure, or
/// real upper floors that the active floor does not represent.
pub fn simple_weight_kn(project: &Project) -> Option<f64> {
    let settings = &project.structure.settings;
    let real_upper_floors = project
        .floors
        .levels
        .iter()
        .any(|level| !level.data.beams.is_empty() || !level.data.slabs.is_empty());
    if real_upper_floors {
        return None;
    }
    let storey = project.wall_height;
    let walls: f64 = project
        .walls
        .iter()
        .map(|wall| {
            let openings: f64 = project
                .openings
                .iter()
                .filter(|o| o.wall_id == wall.id)
                .map(|o| o.width() * o.height().unwrap_or(storey).min(storey))
                .sum();
            let area = (wall.length() * wall.mean_height(storey) - openings).max(0.0);
            area * wall.thickness() * settings.masonry_kn_m3
        })
        .sum();
    let slabs: f64 = project
        .structure
        .slabs
        .iter()
        .filter_map(|slab| {
            let room = project.rooms.iter().find(|r| r.id == slab.room_id)?;
            let area = project.room_area(room)?;
            Some(
                area * (slab.kind.self_weight(slab.thickness_m)
                    + settings.finish_kn_m2
                    + settings.live_kn_m2),
            )
        })
        .sum();
    let total = (walls + slabs) * f64::from(settings.floors.max(1));
    (total > 0.0).then_some(total)
}

/// What the detailed simulation carries to the ground, kN: the column
/// loads at the foundation plus the walls on ground beams and strip
/// footings.
pub fn carried_weight_kn(report: &StructureReport) -> f64 {
    let columns: f64 = report.columns.iter().map(|c| c.load_kn.max(0.0)).sum();
    let walls: f64 = report
        .wall_foundations
        .iter()
        .map(|w| w.load_kn_m.max(0.0) * w.length_m)
        .sum();
    columns + walls
}

/// What the foundations can hold together, kN: footing and strip-footing
/// areas times the allowable soil pressure, plus the pile caps' capacity.
pub fn foundation_capacity_kn(report: &StructureReport) -> f64 {
    let footings: f64 = report
        .footings
        .iter()
        .map(|f| f.width_m * f.depth_m * f.soil_kpa)
        .sum();
    let strips: f64 = report
        .wall_foundations
        .iter()
        .filter(|w| w.kind == WallFoundationKind::StripFooting)
        .map(|w| w.width_m * w.length_m * w.soil_kpa)
        .sum();
    let piles: f64 = report.pile_caps.iter().map(|p| p.allowable_kn).sum();
    footings + strips + piles
}

/// The independent checks of the structure, empty when there is nothing to
/// compare.
pub fn cross_checks(project: &Project, report: &StructureReport) -> Vec<CrossCheck> {
    if report.columns.is_empty() && report.wall_foundations.is_empty() {
        return Vec::new();
    }
    let Some(simple) = simple_weight_kn(project) else {
        return Vec::new();
    };
    let carried = carried_weight_kn(report);
    let weight_use = MIN_CARRIED_SHARE * simple / carried.max(1e-9);
    let mut checks = vec![CrossCheck {
        name: "Peso total da casa",
        utilisation: weight_use,
        simple: if weight_use > 1.0 {
            format!(
                "A simulação levou ao chão só {carried:.0} kN, mas as paredes e lajes pesam por volta de {simple:.0} kN. Falta alguma coisa no modelo: confira se todas as paredes têm baldrame ou sapata corrida (botão Baldrames) e se as lajes e cargas estão desenhadas."
            )
        } else {
            format!(
                "A simulação levou ao chão {carried:.0} kN, coerente com o peso das paredes e lajes ({simple:.0} kN)."
            )
        },
        detail: format!(
            "Conferência independente (#172): peso simples = paredes (comprimento × altura × espessura × peso da alvenaria, descontadas as aberturas) + lajes (área × (peso próprio + revestimento + sobrecarga)), × pavimentos = {simple:.0} kN; carga levada às fundações pela simulação detalhada (pilares + paredes sobre baldrames e sapatas corridas) = {carried:.0} kN; exige-se pelo menos {:.0} % do peso simples.",
            MIN_CARRIED_SHARE * 100.0
        ),
    }];
    let capacity = foundation_capacity_kn(report);
    if weight_use <= 1.0 && capacity > 0.0 {
        let load = simple.max(carried);
        let use_ = load / capacity;
        checks.push(CrossCheck {
            name: "Fundações somadas",
            utilisation: use_,
            simple: if use_ > 1.0 {
                format!(
                    "Somadas, as fundações aguentam {capacity:.0} kN, menos que o peso da casa ({load:.0} kN). Aumente as sapatas (botão Ajustar sapatas) ou confira o solo."
                )
            } else {
                format!(
                    "Somadas, as fundações aguentam {capacity:.0} kN para uma casa de {load:.0} kN."
                )
            },
            detail: format!(
                "Conferência independente (#172): soma de área × tensão do solo das sapatas e sapatas corridas, mais a capacidade dos blocos sobre broca = {capacity:.0} kN, contra o maior entre o peso simples e a carga da simulação = {load:.0} kN."
            ),
        });
    }
    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        project
    }

    #[test]
    fn the_simple_weight_counts_walls_and_slabs() {
        let project = house();
        let simple = simple_weight_kn(&project).unwrap();
        assert!(simple > 100.0, "a 4 × 4 m room weighs {simple} kN");
    }

    #[test]
    fn a_model_that_carries_far_less_than_the_house_weighs_is_contested() {
        let project = house();
        let mut report = project.structure_report();
        for column in &mut report.columns {
            column.load_kn = 1.0;
        }
        report.wall_foundations.clear();
        let checks = cross_checks(&project, &report);
        assert!(checks[0].utilisation > 1.0);
        assert!(checks[0].simple.contains("Falta alguma coisa"));
    }
}
