//! Masonry & materials schedule (*Quadro de Materiais e Alvenaria*), using
//! each wall's own masonry height and `Masonry::blocks_per_square_metre` (B20).

use super::{ScheduleConfig, ScheduleTable, fmt_area, fmt_len};
use crate::domain::{masonry::Masonry, model::Project};

type MasonryKey = (String, i32, i32, i32, i32);

struct MasonryGroup {
    name: String,
    thickness_cm: f64,
    blocks_per_m2: f64,
    net_area_m2: f64,
    blocks: f64,
}

fn group_key(masonry: &Masonry) -> MasonryKey {
    (
        masonry.name.clone(),
        (masonry.block_width * 1000.0).round() as i32,
        (masonry.block_length * 1000.0).round() as i32,
        (masonry.block_height * 1000.0).round() as i32,
        (masonry.joint * 1000.0).round() as i32,
    )
}

pub fn build_materials_schedule(project: &Project, config: &ScheduleConfig) -> ScheduleTable {
    let waste_factor = 1.0 + config.waste_percent.clamp(0.0, 50.0) / 100.0;
    let wall_height = project.wall_height.max(0.5);
    let mut total_wall_len = 0.0;
    let mut total_net_wall_area = 0.0;
    let mut total_blocks = 0.0;
    let mut groups: Vec<(MasonryKey, MasonryGroup)> = Vec::new();

    for wall in &project.walls {
        total_wall_len += wall.length();
        let net = project.wall_net_area(wall);
        total_net_wall_area += net;
        let masonry = wall.masonry.as_ref().unwrap_or(&project.masonry);
        let rate = masonry.blocks_per_square_metre();
        let blocks = net * rate;
        total_blocks += blocks;

        let key = group_key(masonry);
        if let Some((_, entry)) = groups.iter_mut().find(|(k, _)| *k == key) {
            entry.net_area_m2 += net;
            entry.blocks += blocks;
        } else {
            groups.push((
                key,
                MasonryGroup {
                    name: masonry.name.clone(),
                    thickness_cm: masonry.thickness() * 100.0,
                    blocks_per_m2: rate,
                    net_area_m2: net,
                    blocks,
                },
            ));
        }
    }

    let blocks_with_waste = (total_blocks * waste_factor).ceil() as u64;
    let waste_label = format!("{:.0}%", config.waste_percent.clamp(0.0, 50.0));
    let mut rows = Vec::new();

    if groups.len() == 1 {
        let g = &groups[0].1;
        rows.push(vec![
            "Alvenaria".to_owned(),
            g.name.clone(),
            format!("{:.1} cm · {:.1} un./m²", g.thickness_cm, g.blocks_per_m2).replace('.', ","),
        ]);
    } else if groups.is_empty() {
        rows.push(vec![
            "Alvenaria padrão".to_owned(),
            project.masonry.name.clone(),
            format!(
                "{:.1} cm · {:.1} un./m²",
                project.masonry.thickness() * 100.0,
                project.masonry.blocks_per_square_metre()
            )
            .replace('.', ","),
        ]);
    } else {
        for (_, g) in &groups {
            let with_waste = (g.blocks * waste_factor).ceil() as u64;
            rows.push(vec![
                format!("Alvenaria · {}", g.name),
                format!("{with_waste} un."),
                format!(
                    "{} · {:.1} cm · {:.1} un./m²",
                    fmt_area(g.net_area_m2),
                    g.thickness_cm,
                    g.blocks_per_m2
                )
                .replace('.', ","),
            ]);
        }
    }

    rows.push(vec![
        "Extensão total de paredes".to_owned(),
        fmt_len(total_wall_len),
        format!("Pé-direito {}", fmt_len(wall_height)),
    ]);
    rows.push(vec![
        "Área líquida de alvenaria".to_owned(),
        fmt_area(total_net_wall_area),
        "Descontando vãos".to_owned(),
    ]);
    rows.push(vec![
        format!("Blocos estimados (+{waste_label} perda)"),
        format!("{blocks_with_waste} un."),
        format!("Base: {} un.", total_blocks.ceil() as u64),
    ]);

    ScheduleTable {
        title: "Quadro de Materiais e Alvenaria".to_owned(),
        headers: vec![
            "Especificação".to_owned(),
            "Quantidade".to_owned(),
            "Observação".to_owned(),
        ],
        rows,
        footer: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn materials_schedule_uses_each_walls_block_height_and_masonry_name() {
        // Default masonry: Tijolo cerâmico 6 furos (em pé), 9 × 19 × 14 cm, joint 1 cm
        // → 1 / (0.20 × 0.15) = 33.333... blocks/m² (NOT 25 blocks/m² from fixed 19 cm height!).
        let mut project = Project::default();
        project.wall_height = 3.0;
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(10.0, 10.0));
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        // Net area = 4.0 × 3.0 = 12.0 m² → base blocks = 12.0 / (0.20 × 0.15) = 400 un.
        // With 10% waste → 440 un.
        let cfg = ScheduleConfig::default();
        let table = build_materials_schedule(&project, &cfg);
        let blocks_row = table
            .rows
            .iter()
            .find(|r| r[0].starts_with("Blocos estimados"))
            .unwrap();
        assert_eq!(blocks_row[1], "440 un.");
        assert_eq!(blocks_row[2], "Base: 400 un.");

        // Add a second wall with an explicit per-wall masonry override (39 × 19 × 14 concrete block,
        // 12.5 blocks/m²): the schedule must list both masonries and sum their exact block counts.
        let w2 = project
            .add_wall(Point::new(0.0, 2.0), Point::new(4.0, 2.0))
            .unwrap();
        let concrete_block = Masonry::presets()
            .into_iter()
            .find(|m| (m.block_length - 0.39).abs() < 1e-3 && (m.block_height - 0.19).abs() < 1e-3)
            .expect("39×19 concrete block preset");
        let expected_w2_blocks = 12.0 * concrete_block.blocks_per_square_metre();
        let concrete_name = concrete_block.name.clone();
        project
            .walls
            .iter_mut()
            .find(|w| w.id == w2)
            .unwrap()
            .masonry = Some(concrete_block);

        let mixed = build_materials_schedule(&project, &cfg);
        assert!(
            mixed
                .rows
                .iter()
                .any(|r| r[0].contains(&project.masonry.name))
        );
        assert!(mixed.rows.iter().any(|r| r[0].contains(&concrete_name)));
        let expected_base = (400.0 + expected_w2_blocks).ceil() as u64;
        let expected_waste = ((400.0 + expected_w2_blocks) * 1.10).ceil() as u64;
        let mixed_blocks = mixed
            .rows
            .iter()
            .find(|r| r[0].starts_with("Blocos estimados"))
            .unwrap();
        assert_eq!(mixed_blocks[1], format!("{expected_waste} un."));
        assert_eq!(mixed_blocks[2], format!("Base: {expected_base} un."));
    }
}
