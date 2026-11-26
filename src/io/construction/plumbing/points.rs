//! The plumbing points plan (#72): every water, drain and gas point with its
//! height, the legend, the counts and the fixture schedule.

use super::{Plan, draw_building, label, point_symbol, wrap};
use crate::domain::model::Project;
use crate::domain::plumbing::points::{PointKind, plumbing_points};
use crate::io::construction::{Drawn, INK, MUTED, Sheet, decimal};

/// Width of the column of legend and tables on the right, mm.
pub(crate) const TABLE_W: f64 = 120.0;

pub(crate) fn points_drawing(project: &Project) -> Option<Drawn> {
    let points = plumbing_points(project);
    if points.is_empty() {
        return None;
    }
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let extent = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(points.points.iter().map(|p| p.at));
    let plan = Plan::new(extent, (ax, ay, aw - TABLE_W - 6.0, ah));
    draw_building(&mut sheet, project, &plan);

    // The points, each with its tag and height, the labels of one fixture
    // stacked so they do not overlap.
    let mut fixture = None;
    let mut row = 0.0;
    for point in &points.points {
        if fixture != Some(point.fixture_id) {
            fixture = Some(point.fixture_id);
            row = 0.0;
        }
        let (x, y) = plan.pt(point.at);
        point_symbol(&mut sheet, point.kind, (x, y), 0.9);
        let text = match point.kind {
            PointKind::Drain => format!(
                "{} DN{} h{}",
                point.kind.tag(),
                point.diameter_mm,
                decimal(point.height_m, 2)
            ),
            _ => format!("{} h{}", point.kind.tag(), decimal(point.height_m, 2)),
        };
        label(
            &mut sheet,
            (x + 1.4, y - 1.0 + row),
            1.7,
            &point.kind.hex(),
            &text,
        );
        row += 2.1;
    }

    let tx = ax + aw - TABLE_W;
    let mut ty = ay + 8.0;
    sheet.text(tx, ty, 3.0, true, INK, "Legenda");
    ty += 5.0;
    for kind in PointKind::ALL {
        point_symbol(&mut sheet, kind, (tx + 1.5, ty - 1.0), 1.0);
        let text = format!("{} — {}", kind.tag(), kind.label());
        sheet.text(tx + 4.5, ty, 2.3, false, INK, &text);
        ty += 4.0;
    }
    sheet.text(
        tx,
        ty,
        2.0,
        false,
        MUTED,
        "h = altura do ponto a partir do piso acabado (m)",
    );
    ty += 7.0;

    sheet.text(tx, ty, 3.0, true, INK, "Pontos");
    ty += 5.0;
    let counts = PointKind::ALL
        .iter()
        .map(|k| format!("{} {}", k.tag(), points.count(*k)))
        .collect::<Vec<_>>()
        .join(" · ");
    sheet.text(tx, ty, 2.3, false, INK, &counts);
    ty += 8.0;

    sheet.text(tx, ty, 3.0, true, INK, "Quadro de aparelhos");
    ty += 5.0;
    let columns = [0.0, 32.0, 41.0, 53.0, 65.0, 86.0, 99.0, 110.0];
    for (i, head) in [
        "Aparelho", "Qtd", "AF", "AQ", "Esgoto", "Gás", "Peso", "UHC",
    ]
    .iter()
    .enumerate()
    {
        sheet.text(tx + columns[i], ty, 2.0, true, MUTED, head);
    }
    ty += 1.4;
    sheet.line((tx, ty), (tx + TABLE_W - 2.0, ty), MUTED, 0.2, None);
    ty += 3.6;
    let height = |h: Option<f64>| h.map_or("—".to_owned(), |h| decimal(h, 2));
    for line in &points.schedule {
        let cells = [
            line.kind.label().to_owned(),
            line.count.to_string(),
            height(line.cold_m),
            height(line.hot_m),
            line.drain.map_or("—".to_owned(), |(dn, h)| {
                format!("DN{dn} h{}", decimal(h, 2))
            }),
            height(line.gas_m),
            if line.weight > 0.0 {
                decimal(line.weight, 1)
            } else {
                "—".to_owned()
            },
            if line.uhc > 0.0 {
                decimal(line.uhc, 0)
            } else {
                "—".to_owned()
            },
        ];
        for (i, cell) in cells.iter().enumerate() {
            sheet.text(tx + columns[i], ty, 2.1, false, INK, cell);
        }
        ty += 3.0;
        let rooms = line.rooms.join(", ");
        if !rooms.is_empty() {
            for text in wrap(&rooms, 60).into_iter().take(2) {
                sheet.text(tx + 1.5, ty, 1.7, false, MUTED, &text);
                ty += 2.4;
            }
        }
        ty += 1.4;
    }
    sheet.line(
        (tx, ty - 2.0),
        (tx + TABLE_W - 2.0, ty - 2.0),
        MUTED,
        0.2,
        None,
    );
    ty += 2.0;
    let totals = format!(
        "Soma dos pesos {} · Soma das UHC {}",
        decimal(points.total_weight, 1),
        decimal(points.total_uhc, 0)
    );
    sheet.text(tx, ty, 2.3, true, INK, &totals);
    ty += 7.0;
    for note in &points.notes {
        for text in wrap(note, 78) {
            sheet.text(tx, ty, 1.9, false, MUTED, &text);
            ty += 2.6;
        }
        ty += 1.0;
    }

    Some(Drawn {
        sheet,
        title: "Pontos hidrossanitários".to_owned(),
        scale: plan.scale.label(),
    })
}
