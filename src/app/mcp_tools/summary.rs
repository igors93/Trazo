//! The plan summary the assistant reads with obter_planta: every element
//! with its id, position and measurements.

use serde_json::{Value, json};

use super::{from_model, round, xy};
use crate::app::FloorPlanApp;
use crate::domain::{fixtures::FixtureKind, geometry::Point, model::OpeningKind};

impl FloorPlanApp {
    /// The plan as the assistant sees it, in its own coordinates.
    pub(super) fn plan_summary(&self) -> Value {
        let project = &self.project;
        let terrain = project.terrain.as_ref().map(|terrain| {
            let (x, y) = from_model(Point::new(terrain.min.x, terrain.max.y));
            json!({
                "id": terrain.id,
                "guid": project.guid_of(terrain.id).map(|g| g.to_uuid_string()),
                "forma": if terrain.vertices.len() >= 3 { "poligono" } else { "retangulo" },
                "divisas": terrain.outline().into_iter().map(xy).collect::<Vec<_>>(),
                "x": round(x),
                "y": round(y),
                "largura": round(terrain.width()),
                "comprimento": round(terrain.height()),
                "area_m2": round(terrain.area()),
                "recuos": project.setbacks.is_active().then(|| json!({
                    "frente": round(project.setbacks.front),
                    "fundos": round(project.setbacks.rear),
                    "lateral": round(project.setbacks.side),
                })),
            })
        });
        let rooms: Vec<Value> = project
            .rooms
            .iter()
            .map(|room| {
                let bounds = project.room_bounds(room);
                let rectangle = project.room_is_rectangle(room);
                let finishes = room.finishes.as_ref().map(|f| {
                    json!({
                        "piso": f.floor,
                        "parede": f.wall,
                        "teto": f.ceiling,
                        "rodape": f.baseboard,
                        "rodape_cm": round(f.baseboard_height_cm),
                        "impermeabilizacao": f.waterproofing,
                        "notas": f.notes,
                    })
                });
                json!({
                    "id": room.id,
                    "guid": project.guid_of(room.id).map(|g| g.to_uuid_string()),
                    "nome": room.display_name(),
                    "forma": if rectangle { "retangulo" } else { "poligono" },
                    "centro": project.room_centroid(room).map(xy),
                    "cantos": (!rectangle).then(|| {
                        project
                            .room_outline(room)
                            .unwrap_or_default()
                            .into_iter()
                            .map(xy)
                            .collect::<Vec<_>>()
                    }),
                    "x": bounds.map(|(min, _)| round(min.x + 0.0)),
                    "y": bounds.map(|(_, max)| round(-max.y + 0.0)),
                    "largura": bounds.map(|(min, max)| round(max.x - min.x)),
                    "comprimento": bounds.map(|(min, max)| round(max.y - min.y)),
                    "area_m2": project.room_area(room).map(round),
                    "paredes": room.wall_ids,
                    "acabamentos": finishes,
                })
            })
            .collect();
        let walls: Vec<Value> = project
            .walls
            .iter()
            .map(|wall| {
                let rooms: Vec<&str> = project
                    .rooms
                    .iter()
                    .filter(|room| room.wall_ids.contains(&wall.id))
                    .map(|room| room.display_name())
                    .collect();
                let openings: Vec<u64> = project
                    .openings
                    .iter()
                    .filter(|opening| opening.wall_id == wall.id)
                    .map(|opening| opening.id)
                    .collect();
                json!({
                    "id": wall.id,
                    "guid": project.guid_of(wall.id).map(|g| g.to_uuid_string()),
                    "espessura": round(wall.thickness()),
                    "material": wall.masonry.as_ref().map(|material| material.name.clone()),
                    "inicio": xy(wall.start),
                    "fim": xy(wall.end),
                    "comprimento": round(wall.length()),
                    "altura": wall.has_own_height().then(|| {
                        let (start, end) = wall.heights(project.wall_height);
                        json!({ "inicio_m": round(start), "fim_m": round(end) })
                    }),
                    "comodos": rooms,
                    "aberturas": openings,
                })
            })
            .collect();
        let frame_types = crate::domain::annotations::frame_types(project);
        let openings: Vec<Value> = project
            .openings
            .iter()
            .map(|opening| {
                json!({
                    "id": opening.id,
                    "guid": project.guid_of(opening.id).map(|g| g.to_uuid_string()),
                    "tipo": match opening.kind {
                        OpeningKind::Door => "porta",
                        OpeningKind::Window => "janela",
                        OpeningKind::SaloonDoor => "porta_vai_e_vem",
                        OpeningKind::Gap => "vao_livre",
                        OpeningKind::Gate => "portao",
                    },
                    "parede_id": opening.wall_id,
                    "codigo": crate::domain::annotations::frame_of(&frame_types, opening)
                        .map(|frame| frame.code.clone()),
                    "tipologia": opening
                        .door_type
                        .map(|dt| dt.short_label())
                        .or_else(|| opening.window_type.map(|wt| wt.short_label())),
                    "altura": opening.height().map(round),
                    "peitoril": round(opening.sill()),
                    "lado_invertido": opening.flip_side,
                    "dobradica_no_fim": opening.flip_hinge,
                    "posicao": round(opening.t),
                    "largura": round(opening.width()),
                    "centro": self.opening_center(opening.id).map(xy),
                })
            })
            .collect();
        let fixtures: Vec<Value> = project
            .fixtures
            .iter()
            .map(|fixture| {
                let (width, depth) = fixture.extent();
                json!({
                    "id": fixture.id,
                    "guid": project.guid_of(fixture.id).map(|g| g.to_uuid_string()),
                    "tipo": fixture.kind.key(),
                    "nome": fixture.kind.label(),
                    "centro": xy(fixture.center),
                    "largura": round(fixture.width),
                    "profundidade": round(fixture.depth),
                    "rotacao": fixture.quarter_turns as u32 * 90,
                    "angulo_graus": (fixture.angle_rad.abs() >= 1e-6)
                        .then(|| round(fixture.rotation_rad().to_degrees().rem_euclid(360.0))),
                    "ocupa_x": round(width),
                    "ocupa_y": round(depth),
                })
            })
            .collect();
        let areas = project.area_summary();
        let pid = self.active_project_id();
        let linked_tabs: Vec<usize> = (0..self.tab_count())
            .filter(|&i| self.tab_project_id(i) == Some(pid))
            .map(|i| i + 1)
            .collect();
        let active_floor = project.active_floor().map(|f| {
            json!({
                "id": f.id,
                "nome": f.name,
                "nivel_m": round(f.elevation_m),
                "pe_direito_m": round(project.wall_height),
            })
        });
        let floors: Vec<Value> = project
            .floors
            .levels
            .iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "nome": f.name,
                    "nivel_m": round(f.elevation_m),
                    "ativo": Some(f.id) == project.floors.active_floor_id,
                })
            })
            .collect();
        let audit = project.cached_dependency_audit();
        let pending_reviews = audit
            .review_items
            .iter()
            .filter(|r| r.severity != crate::domain::dependencies::ReviewSeverity::Info)
            .count();
        json!({
            "projeto_guid": project.project_guid().to_uuid_string(),
            "projeto_ifc_global_id": project.project_guid().to_ifc_global_id(),
            "espessura_parede_m": round(self.project.masonry.thickness()),
            "alvenaria": {
                "material": self.project.masonry.name,
                "bloco_largura_cm": round(self.project.masonry.block_width * 100.0),
                "bloco_comprimento_cm": round(self.project.masonry.block_length * 100.0),
                "junta_cm": round(self.project.masonry.joint * 100.0),
                "reboco_cm": round(self.project.masonry.plaster * 100.0),
                "modulo_m": round(self.project.masonry.module()),
                "pe_direito_m": round(self.project.wall_height),
            },
            "revisao": self.plan_log.revision,
            "aba_ativa": self.active_tab() + 1,
            "disciplina": self.discipline.key(),
            "abas_vinculadas": linked_tabs,
            "pavimento_ativo": active_floor,
            "pavimentos": floors,
            "revisoes_pendentes": pending_reviews,
            "ultima_sincronizacao": {
                "atualizados_automaticamente": self.sync_report.auto_updated.len(),
                "bloqueados": self.sync_report.blocked_updates.len(),
            },
            "objetos": fixtures,
            "marcacoes": self.markup_summary(),
            "marcacao_selecionada": self.current_markup_selection(),
            "recursos": {"biblioteca":project.library.len(),"blocos":project.blocks.len(),"instancias_blocos":project.block_instances.len(),"referencias":self.references_summary(),"lotes":project.lots.len()},
            "desenhos": super::drawings::drawings_summary(project),
            "estrutura_resumo": {
                "pilares": project.fixtures.iter().filter(|f| f.kind == FixtureKind::Column).count(),
                "sapatas": project.fixtures.iter().filter(|f| f.kind == FixtureKind::Footing).count(),
                "blocos_broca": project.fixtures.iter().filter(|f| f.kind == FixtureKind::PileCap).count(),
                "mouroes": project.fixtures.iter().filter(|f| f.kind == FixtureKind::TimberPost).count(),
                "vigas": project.structure.beams.len(),
                "lajes": project.structure.slabs.len(),
                "telhados": project.structure.roofs.len(),
                "vigas_alavanca": project.structure.balance_beams.len(),
                "fundacao_paredes": project.structure.wall_foundations.len(),
                "sondagem": project.structure.soil_survey.is_some(),
            },
            "documentacao": {
                "camadas": project.layers.len(),
                "anotacoes": project.notes.len(),
                "cotas": project.dimensions.len(),
                "circulacoes": project.circulations.len(),
                "cortes": project.views.sections.len(),
                "fachadas": project.views.elevations.len(),
                "detalhes": project.views.details.len(),
                "prancha": {
                    "formato": project.sheet.size.short_name(),
                    "orientacao": project.sheet.orientation.label(),
                    "escala": project.sheet.scale.label(),
                },
            },
            // What the person has selected on screen, so "this one" makes sense.
            "selecao": self.selected().iter().map(|item| item.id()).collect::<Vec<_>>(),
            "grupos": project
                .groups
                .iter()
                .map(|group| json!({
                    "id": group.id,
                    "nome": group.name,
                    "membros": group.members.iter().map(|member| member.id()).collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
            "restricoes": project
                .constraints
                .iter()
                .map(|constraint| {
                    let [a, b] = constraint.kind.entities();
                    json!({
                        "id": constraint.id,
                        "tipo": constraint.kind.label(),
                        "a": a.element().id(),
                        "b": b.element().id(),
                        "distancia": constraint.kind.distance().map(round),
                    })
                })
                .collect::<Vec<_>>(),
            "projeto": {
                "id": pid,
                "nome": project.name,
                "arquivo": self.file_path.as_ref().map(|path| path.display().to_string()),
                "alteracoes_nao_salvas": self.dirty,
                "unidade": "metro",
                "coordenadas": "X para a direita, Y para cima",
            },
            "terreno": terrain,
            "comodos": rooms,
            "paredes": walls,
            "aberturas": openings,
            "quadro_esquadrias": frame_types
                .iter()
                .map(|frame| json!({
                    "codigo": frame.code,
                    "tipo": frame.kind.label(),
                    "largura": round(frame.width),
                    "altura": frame.height.map(round),
                    "peitoril": round(frame.sill),
                    "quantidade": frame.ids.len(),
                }))
                .collect::<Vec<_>>(),
            "areas": {
                "util_m2": round(areas.usable),
                "paredes_m2": round(areas.walls),
                "construida_m2": round(areas.constructed),
                "projecao_m2": round(areas.projection),
                "terreno_m2": round(areas.terrain),
            },
            "area_util_m2": round(areas.usable),
            "area_construida_m2": round(areas.constructed),
            "area_projecao_m2": round(areas.projection),
            "area_terreno_m2": round(areas.terrain),
            "taxa_ocupacao_percentual": (areas.terrain > 0.0)
                .then(|| round(areas.projection / areas.terrain * 100.0)),
        })
    }
}
