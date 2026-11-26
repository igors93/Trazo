use super::*;
use std::f64::consts::FRAC_PI_2;

use crate::domain::{
    dimensions::{DimensionAnchor, DimensionKind},
    drawing::Shape,
    fixtures::SymbolShape,
    layers::{LayerCategory, LineType},
    model::OpeningKind,
    notes::NoteKind,
};
use crate::domain::{
    fixtures::FixtureKind,
    hatch::{HatchPattern, HatchStyle},
    samples::SampleProject,
};

#[test]
fn fixture_export_keeps_every_symbol_part_and_free_rotation() {
    let mut project = Project::default();
    let id = project
        .add_fixture(FixtureKind::KitchenSink, Point::new(3.0, 4.0), 1.8, 0.6, 1)
        .unwrap();
    project
        .fixtures
        .iter_mut()
        .find(|f| f.id == id)
        .unwrap()
        .angle_rad = 0.37;
    let fixture = &project.fixtures[0];
    let shapes = fixture.symbol();
    let expected_circles = shapes
        .iter()
        .filter(|s| matches!(s, SymbolShape::Circle { .. }))
        .count();
    let expected_polylines = shapes.len() - expected_circles;
    let dxf = to_dxf_string(&project);
    let entities = dxf.split("ENTITIES\n").nth(1).unwrap();
    assert_eq!(entities.matches("  0\nCIRCLE\n").count(), expected_circles);
    assert_eq!(
        entities.matches("  0\nPOLYLINE\n").count(),
        expected_polylines
    );
    for shape in shapes {
        if let SymbolShape::Circle { center, radius, .. } = shape {
            let center = fixture.to_world(center);
            let mut expected = String::new();
            write_circle(&mut expected, "Mobiliario", center, radius, None);
            assert!(dxf.contains(&expected));
        }
    }
}

#[test]
fn round_trip_exports_and_reimports_walls_drawings_texts_dimensions_and_layers() {
    let mut project = Project::default();
    project.add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0));
    project.add_wall(Point::new(5.0, 0.0), Point::new(5.0, 4.0));
    let _ = project.add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala");
    if let Some(room) = project.rooms.first_mut() {
        room.hatch = Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid));
    }
    let wall_id = project.walls[0].id;
    let _ = project.add_opening(wall_id, OpeningKind::Door, 0.4);
    let _ = project.add_fixture(FixtureKind::Sofa, Point::new(2.0, 2.0), 2.0, 0.9, 0);
    let _ = project.add_drawing(Shape::Circle {
        center: Point::new(1.5, 1.5),
        radius: 0.45,
    });
    let _ = project.add_drawing(Shape::Arc {
        center: Point::new(3.0, 2.0),
        radius: 0.60,
        start: 0.0,
        sweep: FRAC_PI_2,
    });
    let _ = project.add_note(Point::new(2.5, 1.0), "NOTA TECNICA", NoteKind::Text);
    let _ = project.add_dimension(DimensionKind::Aligned {
        a: DimensionAnchor::from_point(Point::new(0.0, 0.0)),
        b: DimensionAnchor::from_point(Point::new(5.0, 0.0)),
        offset: -0.5,
    });

    let dxf = to_dxf_string(&project);
    assert!(dxf.contains("$ACADVER\n  1\nAC1009"));
    assert!(dxf.contains("$INSUNITS\n 70\n6"));
    assert!(dxf.contains("SECTION\n  2\nTABLES"));
    assert!(dxf.contains("SECTION\n  2\nBLOCKS"));
    assert!(dxf.contains("SECTION\n  2\nENTITIES"));
    assert!(dxf.contains("DIMENSION"));
    assert!(dxf.contains("Hachuras"));
    assert!(dxf.trim_end().ends_with("EOF"));

    let mut reimported = Project::default();
    let report = import_str(&mut reimported, &dxf).unwrap();
    assert!(
        report.walls >= 4,
        "expected walls reconstructed from exported polylines, got {}",
        report.walls
    );
    assert!(report.drawings >= 2);
    assert!(report.notes >= 1);
    assert_eq!(report.dimensions, 1);
    assert_eq!(report.units_label, "metros");
    assert!(
        reimported
            .notes
            .iter()
            .any(|n| n.text.contains("NOTA TECNICA"))
    );
}

#[test]
fn imports_autocad_millimetre_dxf_with_lwpolyline_mtext_and_custom_layers() {
    // Minimal AutoCAD-style DXF in millimetres ($INSUNITS = 4).
    let dxf = "\
  0
SECTION
  2
HEADER
  9
$INSUNITS
 70
4
  0
ENDSEC
  0
SECTION
  2
TABLES
  0
TABLE
  2
LAYER
 70
2
  0
LAYER
  2
A-WALL
 70
0
 62
1
  6
CONTINUOUS
  0
LAYER
  2
A-ANNO
 70
0
 62
5
  6
DASHED
  0
ENDTAB
  0
ENDSEC
  0
SECTION
  2
ENTITIES
  0
LINE
  8
A-WALL
 10
0.0
 20
0.0
 11
4000.0
 21
0.0
  0
LWPOLYLINE
  8
A-WALL
 70
1
 10
0.0
 20
0.0
 10
4000.0
 20
0.0
 10
4000.0
 20
150.0
 10
0.0
 20
150.0
  0
CIRCLE
  8
A-ANNO
 10
2000.0
 20
1500.0
 40
500.0
  0
MTEXT
  8
A-ANNO
 10
1000.0
 20
2000.0
 40
180.0
  1
{\\fArial|b1;SALA DE ESTAR\\P16,00 m2}
  0
DIMENSION
  8
A-ANNO
 10
4000.0
 20
500.0
 13
0.0
 23
0.0
 14
4000.0
 24
0.0
  1
400
  0
ENDSEC
  0
EOF
";
    let mut project = Project::default();
    let report = import_str(&mut project, dxf).unwrap();
    assert_eq!(report.units_label, "milímetros");
    assert_eq!(report.walls, 2);
    assert_eq!(report.drawings, 1);
    assert_eq!(report.notes, 1);
    assert_eq!(report.dimensions, 1);

    // Verify 4000 mm scaled to 4.0 m.
    assert!((project.walls[0].length() - 4.0).abs() < 1e-4);
    // Verify custom layer color and linetype imported.
    let anno = project
        .layers
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case("A-ANNO"))
        .expect("A-ANNO layer imported");
    assert_eq!(anno.color, [0, 0, 255]);
    assert_eq!(anno.line_type, LineType::Dashed);
    // Verify MTEXT formatting codes stripped.
    assert_eq!(project.notes[0].text, "SALA DE ESTAR 16,00 m2");
}

#[test]
fn all_sample_projects_export_to_valid_dxf() {
    for sample in SampleProject::ALL {
        let project = sample.load_embedded();
        let dxf = to_dxf_string(&project);
        assert!(dxf.contains("SECTION\n  2\nHEADER"));
        assert!(dxf.contains("SECTION\n  2\nTABLES"));
        assert!(dxf.contains("SECTION\n  2\nENTITIES"));
        assert!(dxf.trim_end().ends_with("EOF"));
        let sections = dxf.matches("\nSECTION\n").count();
        let endsecs = dxf.matches("\nENDSEC\n").count();
        assert_eq!(
            sections,
            endsecs,
            "unbalanced sections for {}",
            sample.file_name()
        );
    }
}

#[test]
fn aci_color_mapping_round_trips_primary_colors() {
    assert_eq!(rgb_to_aci([255, 0, 0]), 1);
    assert_eq!(aci_to_rgb(1), [255, 0, 0]);
    assert_eq!(rgb_to_aci([0, 0, 255]), 5);
    assert_eq!(aci_to_rgb(-5), [0, 0, 255]);
}

#[test]
fn floor_conduits_use_dashed_linetype_and_structure_labels_scale_with_sheet() {
    use crate::domain::{conduits::ConduitRun, fixtures::FixtureKind, sheet::DrawingScale};

    let mut project = Project::default();
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
        .unwrap();
    let (pw, pd) = FixtureKind::Panel.default_size();
    let panel = project
        .add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0)
        .unwrap();
    let (ow, od) = FixtureKind::OutletLow.default_size();
    let outlet = project
        .add_fixture(FixtureKind::OutletLow, Point::new(3.6, 2.6), ow, od, 0)
        .unwrap();
    let _ = project
        .add_conduit(panel, outlet, ConduitRun::Floor)
        .unwrap();
    project.generate_structure();

    project.sheet.scale = DrawingScale::S1_50;
    let dxf_50 = to_dxf_string(&project);
    let electrical_layer = sanitize_layer_name(
        &project
            .layer_for_category(LayerCategory::Electrical)
            .unwrap()
            .name,
    );
    let dashed_electrical = format!("  8\n{electrical_layer}\n  6\nDASHED\n");
    // Floor conduit POLYLINE carries entity-level DASHED linetype (group 6).
    let entities_50 = dxf_50.split("ENTITIES").nth(1).unwrap();
    assert!(
        entities_50.contains(&dashed_electrical),
        "expected floor conduit to export with DASHED linetype"
    );
    // Structure text height at 1:50 is 2.2 / 20.0 = 0.11 m.
    assert!(entities_50.contains(" 40\n0.110000\n"));

    // Switching to a ceiling conduit removes the DASHED linetype override.
    project.conduits[0].run = ConduitRun::Ceiling;
    let dxf_ceiling = to_dxf_string(&project);
    let entities_ceiling = dxf_ceiling.split("ENTITIES").nth(1).unwrap();
    assert!(!entities_ceiling.contains(&dashed_electrical));
    // Footing outlines retain their own dashed projection, independently
    // of the conduit run and its electrical layer.
    assert!(entities_ceiling.contains("  6\nDASHED\n"));

    // At 1:100 structure text height doubles to 0.22 m.
    project.sheet.scale = DrawingScale::S1_100;
    let dxf_100 = to_dxf_string(&project);
    let entities_100 = dxf_100.split("ENTITIES").nth(1).unwrap();
    assert!(entities_100.contains(" 40\n0.220000\n"));
}
