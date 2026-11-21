//! MCP tools covering Trazo's quantities takeoff, ergonomics verification,
//! CAD layers, annotations, associative dimensions, vertical circulation,
//! technical views (sections/elevations/details) and ABNT sheet layout.

use serde_json::{Value, json};

use super::{
    Change, documents_dir, element_id, ensure_parent, file_stem, flag, number, optional_count,
    required_number, resolve_path, round, text, to_model, unique_path, xy,
};
use crate::{
    app::FloorPlanApp,
    domain::{
        circulation::{CirculationKind, RailingSide, StairShape},
        dimensions::{DimensionAnchor, DimensionKind, ResolvedDimension},
        ergonomics::{ErgonomicsIssueKind, IssueSeverity, analyze_ergonomics},
        geometry::Point,
        layers::LineType,
        notes::{NoteKind, TechnicalSymbol},
        sheet::{DrawingScale, Orientation, PaperSize, SheetMargins},
        views::{
            ElevationSide, detail_svg, elevation_svg, export_view_pdf, export_view_svg, section_svg,
        },
    },
    mcp::ToolOutput,
};

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0 + 0.0
}

fn parse_hex_color(raw: &str) -> Result<[u8; 3], String> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Cor inválida: use hexadecimal #RRGGBB (ex.: #c62828).".to_owned());
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| "Cor hexadecimal inválida (use #RRGGBB).".to_owned())?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| "Cor hexadecimal inválida (use #RRGGBB).".to_owned())?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| "Cor hexadecimal inválida (use #RRGGBB).".to_owned())?;
    Ok([r, g, b])
}

fn parse_line_type(raw: &str) -> Option<LineType> {
    match raw.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "continua" | "contínua" | "solid" => Some(LineType::Solid),
        "tracejada" | "dashed" => Some(LineType::Dashed),
        "traco_ponto" | "traço_ponto" | "dash_dot" | "dashdot" => Some(LineType::DashDot),
        "pontilhada" | "dotted" => Some(LineType::Dotted),
        _ => None,
    }
}

fn line_type_key(lt: LineType) -> &'static str {
    match lt {
        LineType::Solid => "continua",
        LineType::Dashed => "tracejada",
        LineType::DashDot => "traco_ponto",
        LineType::Dotted => "pontilhada",
    }
}

fn parse_scale(raw: &str) -> Option<DrawingScale> {
    let clean = raw.trim().to_lowercase().replace(' ', "");
    match clean.as_str() {
        "1:25" | "25" => Some(DrawingScale::S1_25),
        "1:50" | "50" => Some(DrawingScale::S1_50),
        "1:75" | "75" => Some(DrawingScale::S1_75),
        "1:100" | "100" => Some(DrawingScale::S1_100),
        "1:200" | "200" => Some(DrawingScale::S1_200),
        other => {
            let denom = other.strip_prefix("1:").unwrap_or(other);
            denom
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|d| d.is_finite() && (1.0..=10_000.0).contains(d))
                .map(|denominator| DrawingScale::Custom { denominator })
        }
    }
}

fn parse_elevation_side(raw: &str) -> Option<ElevationSide> {
    match raw.trim().to_lowercase().as_str() {
        "norte" | "north" => Some(ElevationSide::North),
        "sul" | "south" => Some(ElevationSide::South),
        "leste" | "east" => Some(ElevationSide::East),
        "oeste" | "west" => Some(ElevationSide::West),
        _ => None,
    }
}

fn parse_railing_side(raw: &str) -> Option<RailingSide> {
    match raw.trim().to_lowercase().as_str() {
        "nenhum" | "none" | "sem" => Some(RailingSide::None),
        "esquerda" | "left" => Some(RailingSide::Left),
        "direita" | "right" => Some(RailingSide::Right),
        "ambos" | "both" => Some(RailingSide::Both),
        _ => None,
    }
}

fn parse_stair_shape(raw: &str) -> Option<StairShape> {
    match raw.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "reta" | "lance_reto" | "straight" => Some(StairShape::Straight),
        "l" | "em_l" | "l_shaped" => Some(StairShape::LShaped),
        "u" | "em_u" | "u_shaped" => Some(StairShape::UShaped),
        _ => None,
    }
}

impl FloorPlanApp {
    /// Read-only consolidated construction quantities takeoff (#56).
    pub(super) fn tool_quantities(&mut self) -> Result<ToolOutput, String> {
        let takeoff = self.project.construction_takeoff();
        let structure = self.project.cached_structure_report();
        let reinforcement = self.project.cached_reinforcement_report();
        let roof_tiles: u64 = structure
            .roofs
            .iter()
            .filter_map(|r| r.tiles)
            .map(|t| t.ceil() as u64)
            .sum();
        let roof_timber_m3: f64 = structure.roofs.iter().map(|r| r.timber_m3).sum();
        let roof_area_m2: f64 = structure.roofs.iter().map(|r| r.surface_m2).sum();
        let steel_kg = reinforcement.total_mass_kg();

        let data = json!({
            "projeto_id": self.active_project_id(),
            "perda_percentual": round2(takeoff.waste_percent),
            "alvenaria": takeoff.masonry.iter().map(|m| json!({
                "material": m.masonry,
                "blocos": m.blocks.ceil() as u64,
                "blocos_com_perda": m.blocks_with_waste.ceil() as u64,
                "meios_blocos": m.half_blocks.ceil() as u64,
                "meios_blocos_com_perda": m.half_blocks_with_waste.ceil() as u64,
                "canaletas": m.u_blocks.ceil() as u64,
            })).collect::<Vec<_>>(),
            "paredes": takeoff.walls.iter().map(|w| json!({
                "parede_id": w.wall_id,
                "material": w.masonry,
                "comprimento_m": round2(w.length_m),
                "altura_m": round2(w.height_m),
                "area_bruta_m2": round2(w.gross_m2),
                "aberturas_m2": round2(w.openings_m2),
                "area_liquida_m2": round2(w.net_m2),
                "blocos": w.blocks.ceil() as u64,
                "meios_blocos": w.half_blocks.ceil() as u64,
                "canaletas": w.u_blocks.ceil() as u64,
                "argamassa_m3": round(w.mortar_m3),
                "reboco_m2": round2(w.plaster_m2),
                "reboco_m3": round(w.plaster_m3),
            })).collect::<Vec<_>>(),
            "vergas_contravergas": takeoff.lintels.iter().map(|l| json!({
                "abertura_id": l.opening_id,
                "tipo": l.kind,
                "abertura": l.opening,
                "comprimento_m": round2(l.length_m),
                "canaletas": l.u_blocks.ceil() as u64,
            })).collect::<Vec<_>>(),
            "acabamentos": takeoff.rooms.iter().map(|r| json!({
                "comodo": r.room,
                "piso_m2": round2(r.floor_m2),
                "piso_com_perda_m2": round2(r.floor_with_waste_m2),
                "rodape_m": round2(r.skirting_m),
                "paredes_m2": round2(r.walls_m2),
                "teto_m2": round2(r.ceiling_m2),
            })).collect::<Vec<_>>(),
            "telhados": {
                "quantidade": structure.roofs.len(),
                "area_m2": round2(roof_area_m2),
                "telhas": roof_tiles,
                "madeira_m3": round2(roof_timber_m3),
            },
            "totais": {
                "alvenaria_liquida_m2": round2(takeoff.net_wall_m2),
                "argamassa_m3": round(takeoff.mortar_m3),
                "reboco_m2": round2(takeoff.plaster_m2),
                "reboco_m3": round(takeoff.plaster_m3),
                "vergas_m": round2(takeoff.lintel_m),
                "piso_m2": round2(takeoff.floor_m2),
                "piso_com_perda_m2": round2(takeoff.floor_with_waste_m2),
                "rodape_m": round2(takeoff.skirting_m),
                "paredes_internas_m2": round2(takeoff.room_walls_m2),
                "teto_m2": round2(takeoff.ceiling_m2),
                "aco_preliminar_kg": round2(steel_kg),
                "madeira_telhado_m3": round2(roof_timber_m3),
            },
        });
        Ok(ToolOutput::ok(format!(
            "Quantitativos: {:.2} m² de alvenaria líquida, {:.2} m² de piso ({:.2} m² com perda), {:.1} kg de aço preliminar.",
            takeoff.net_wall_m2,
            takeoff.floor_m2,
            takeoff.floor_with_waste_m2,
            steel_kg
        )
        .replace('.', ","))
        .with_structured(data))
    }

    /// Read-only residential ergonomics and ABNT NBR 9050 clearance analysis.
    pub(super) fn tool_ergonomics(&self, args: &Value) -> Result<ToolOutput, String> {
        let min_passage = number(args, "passagem_minima_m")?.unwrap_or(0.80);
        if !(0.40..=2.50).contains(&min_passage) {
            return Err("'passagem_minima_m' deve ficar entre 0,40 e 2,50 m.".to_owned());
        }
        let report = analyze_ergonomics(&self.project, min_passage);
        let issues: Vec<Value> = report
            .issues
            .iter()
            .map(|issue| {
                let kind = match issue.kind {
                    ErgonomicsIssueKind::FixtureCollision => "colisao_objetos",
                    ErgonomicsIssueKind::WallCollision => "colisao_parede",
                    ErgonomicsIssueKind::DoorSwingObstruction => "abertura_porta_obstruida",
                    ErgonomicsIssueKind::ClearanceBlocked => "area_uso_obstruida",
                    ErgonomicsIssueKind::NarrowPassage => "passagem_estreita",
                };
                let severity = match issue.severity {
                    IssueSeverity::Error => "erro",
                    IssueSeverity::Warning => "alerta",
                };
                json!({
                    "severidade": severity,
                    "tipo": kind,
                    "posicao": xy(issue.location),
                    "mensagem": issue.message,
                })
            })
            .collect();
        let zones: Vec<Value> = report
            .clearance_zones
            .iter()
            .map(|z| {
                json!({
                    "objeto_id": z.fixture_id,
                    "rotulo": z.label,
                    "profundidade_m": round2(z.depth_m),
                    "min": xy(Point::new(z.min.x, z.max.y)),
                    "max": xy(Point::new(z.max.x, z.min.y)),
                })
            })
            .collect();
        Ok(ToolOutput::ok(
            format!(
                "Ergonomia (passagem mínima {:.2} m): {} erro(s), {} alerta(s).",
                report.min_passage_m,
                report.error_count(),
                report.warning_count()
            )
            .replace('.', ","),
        )
        .with_structured(json!({
            "projeto_id": self.active_project_id(),
            "passagem_minima_m": round2(report.min_passage_m),
            "erros": report.error_count(),
            "alertas": report.warning_count(),
            "ocorrencias": issues,
            "zonas_de_uso": zones,
        })))
    }

    /// CAD layer management (`camadas`).
    pub(super) fn tool_layers(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let list: Vec<Value> = self
                    .project
                    .layers
                    .iter()
                    .map(|l| {
                        json!({
                            "id": l.id,
                            "nome": l.name,
                            "cor": l.hex_color(),
                            "espessura_mm": round2(l.line_weight_mm),
                            "tipo_linha": line_type_key(l.line_type),
                            "visivel": l.visible,
                            "bloqueada": l.locked,
                            "padrao": l.category.is_some(),
                            "ativa": self.project.active_layer_id == Some(l.id),
                        })
                    })
                    .collect();
                Ok(
                    ToolOutput::ok(format!("{} camada(s) no projeto.", list.len()))
                        .with_structured(json!({
                            "projeto_id": self.active_project_id(),
                            "camada_ativa_id": self.project.active_layer_id,
                            "camadas": list,
                        })),
                )
            }
            "criar" => {
                let name = text(args, "nome").ok_or("Informe 'nome' da camada.")?;
                let color = match text(args, "cor") {
                    Some(hex) => parse_hex_color(&hex)?,
                    None => [40, 90, 150],
                };
                let weight = number(args, "espessura_mm")?.unwrap_or(0.25);
                let line_type = match text(args, "tipo_linha") {
                    Some(raw) => parse_line_type(&raw).ok_or(
                        "'tipo_linha' deve ser continua, tracejada, traco_ponto ou pontilhada.",
                    )?,
                    None => LineType::Solid,
                };
                let activate = flag(args, "ativar");
                self.apply_change(None, |project| {
                    let id = project.add_layer(&name, color, weight, line_type)?;
                    if activate {
                        project.active_layer_id = Some(id);
                    }
                    Ok(Change::new(format!("Camada \"{name}\" criada (id {id})."))
                        .data(json!({ "id": id, "nome": name })))
                })
            }
            "editar" => {
                let id = element_id(args, "id")?;
                let current = self
                    .project
                    .layer(id)
                    .cloned()
                    .ok_or_else(|| format!("Camada {id} não encontrada."))?;
                let new_name = text(args, "nome").unwrap_or_else(|| current.name.clone());
                let new_color = match text(args, "cor") {
                    Some(hex) => parse_hex_color(&hex)?,
                    None => current.color,
                };
                let new_weight = number(args, "espessura_mm")?.unwrap_or(current.line_weight_mm);
                let new_line_type = match text(args, "tipo_linha") {
                    Some(raw) => parse_line_type(&raw).ok_or(
                        "'tipo_linha' deve ser continua, tracejada, traco_ponto ou pontilhada.",
                    )?,
                    None => current.line_type,
                };
                let visible = args.get("visivel").and_then(Value::as_bool);
                let locked = args.get("bloqueada").and_then(Value::as_bool);
                let activate = args.get("ativar").and_then(Value::as_bool);
                self.apply_change(
                    Some("A camada já estava com essas propriedades."),
                    |project| {
                        project.update_layer(
                            id,
                            &new_name,
                            new_color,
                            new_weight,
                            new_line_type,
                        )?;
                        if let Some(layer) = project.layers.iter_mut().find(|l| l.id == id) {
                            if let Some(v) = visible {
                                layer.visible = v;
                            }
                            if let Some(b) = locked {
                                layer.locked = b;
                            }
                        }
                        if let Some(act) = activate {
                            project.active_layer_id = if act {
                                Some(id)
                            } else if project.active_layer_id == Some(id) {
                                None
                            } else {
                                project.active_layer_id
                            };
                        }
                        Ok(Change::new(format!(
                            "Camada {id} (\"{new_name}\") atualizada."
                        )))
                    },
                )
            }
            "excluir" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    project.delete_layer(id)?;
                    Ok(Change::new(format!("Camada {id} excluída.")))
                })
            }
            "atribuir" => {
                let layer_id = optional_count(args, "id")?;
                let wall_id = optional_count(args, "parede_id")?;
                let drawing_id = optional_count(args, "desenho_id")?;
                if wall_id.is_none() && drawing_id.is_none() {
                    return Err(
                        "Informe 'parede_id' e/ou 'desenho_id' para atribuir à camada.".to_owned(),
                    );
                }
                self.apply_change(Some("O elemento já estava nessa camada."), |project| {
                    if let Some(wid) = wall_id {
                        project.set_wall_layer(wid, layer_id)?;
                    }
                    if let Some(did) = drawing_id {
                        project.set_drawing_layer(did, layer_id)?;
                    }
                    Ok(Change::new("Camada atribuída ao elemento.".to_owned()))
                })
            }
            _ => Err("Informe 'acao': listar, criar, editar, excluir ou atribuir.".to_owned()),
        }
    }

    /// Texts, technical notes, leader callouts and symbols (`anotacoes`).
    pub(super) fn tool_notes(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let notes: Vec<Value> = self
                    .project
                    .notes
                    .iter()
                    .map(|n| {
                        let (kind, extra) = match n.kind {
                            NoteKind::Text => ("texto", json!({})),
                            NoteKind::Note => ("nota", json!({})),
                            NoteKind::Callout { target } => {
                                ("chamada", json!({ "alvo": xy(target) }))
                            }
                            NoteKind::Symbol(TechnicalSymbol::NorthArrow) => ("norte", json!({})),
                            NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m }) => {
                                ("cota_nivel", json!({ "nivel_m": round2(elevation_m) }))
                            }
                            NoteKind::Symbol(TechnicalSymbol::SlopeArrow { percent }) => {
                                ("caimento", json!({ "percentual": round2(percent) }))
                            }
                            NoteKind::Symbol(TechnicalSymbol::EntryArrow) => ("acesso", json!({})),
                        };
                        json!({
                            "id": n.id,
                            "tipo": kind,
                            "texto": n.text,
                            "posicao": xy(n.position),
                            "tamanho_fonte": round2(n.font_size),
                            "rotacao_graus": round2(n.rotation_deg),
                            "detalhes": extra,
                        })
                    })
                    .collect();
                Ok(
                    ToolOutput::ok(format!("{} anotação(ões) na planta.", notes.len()))
                        .with_structured(json!({
                            "projeto_id": self.active_project_id(),
                            "anotacoes": notes,
                        })),
                )
            }
            "adicionar" => {
                let kind_key = text(args, "tipo")
                    .unwrap_or_else(|| "texto".to_owned())
                    .to_lowercase()
                    .replace(['-', ' '], "_");
                let pos = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let content = text(args, "texto").unwrap_or_default();
                let kind = match kind_key.as_str() {
                    "texto" | "text" => NoteKind::Text,
                    "nota" | "note" => NoteKind::Note,
                    "chamada" | "callout" => {
                        let target = to_model(
                            required_number(args, "alvo_x")?,
                            required_number(args, "alvo_y")?,
                        );
                        NoteKind::Callout { target }
                    }
                    "norte" | "north" => NoteKind::Symbol(TechnicalSymbol::NorthArrow),
                    "cota_nivel" | "nivel" | "level" => {
                        let elevation_m = number(args, "nivel_m")?.unwrap_or(0.0);
                        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m })
                    }
                    "caimento" | "slope" => {
                        let percent = number(args, "percentual")?.unwrap_or(1.0);
                        NoteKind::Symbol(TechnicalSymbol::SlopeArrow { percent })
                    }
                    "acesso" | "entry" => NoteKind::Symbol(TechnicalSymbol::EntryArrow),
                    _ => {
                        return Err(
                            "'tipo' deve ser texto, nota, chamada, norte, cota_nivel, caimento ou acesso."
                                .to_owned(),
                        );
                    }
                };
                let font_size = number(args, "tamanho_fonte")?;
                let rotation_deg = number(args, "rotacao_graus")?;
                self.apply_change(None, |project| {
                    let id = project.add_note(pos, &content, kind)?;
                    if let Some(note) = project.notes.iter_mut().find(|n| n.id == id) {
                        if let Some(fs) = font_size {
                            note.font_size = fs.clamp(6.0, 72.0);
                        }
                        if let Some(rot) = rotation_deg {
                            note.rotation_deg = rot;
                        }
                    }
                    let saved = project.notes.iter().find(|n| n.id == id).unwrap();
                    Ok(Change::new(format!(
                        "{} adicionado(a) (id {id}): \"{}\".",
                        kind.label(),
                        saved.text
                    ))
                    .data(json!({ "id": id, "texto": saved.text })))
                })
            }
            "editar" => {
                let id = element_id(args, "id")?;
                let new_text = text(args, "texto");
                let dx = number(args, "dx")?;
                let dy = number(args, "dy")?;
                let font_size = number(args, "tamanho_fonte")?;
                let rotation_deg = number(args, "rotacao_graus")?;
                if new_text.is_none()
                    && dx.is_none()
                    && dy.is_none()
                    && font_size.is_none()
                    && rotation_deg.is_none()
                {
                    return Err(
                        "Informe 'texto', 'dx'/'dy', 'tamanho_fonte' ou 'rotacao_graus'."
                            .to_owned(),
                    );
                }
                self.apply_change(Some("A anotação não sofreu alterações."), |project| {
                    if !project.notes.iter().any(|n| n.id == id) {
                        return Err("Anotação não encontrada.".to_owned());
                    }
                    if let Some(t) = &new_text {
                        project.update_note_text(id, t)?;
                    }
                    if dx.is_some() || dy.is_some() {
                        project.move_note(id, Point::new(dx.unwrap_or(0.0), -dy.unwrap_or(0.0)));
                    }
                    if let Some(note) = project.notes.iter_mut().find(|n| n.id == id) {
                        if let Some(fs) = font_size {
                            note.font_size = fs.clamp(6.0, 72.0);
                        }
                        if let Some(rot) = rotation_deg {
                            note.rotation_deg = rot;
                        }
                    }
                    Ok(Change::new(format!("Anotação {id} atualizada.")))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_note(id) {
                        return Err("Anotação não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Anotação {id} removida.")))
                })
            }
            _ => Err("Informe 'acao': listar, adicionar, editar ou remover.".to_owned()),
        }
    }

    /// Persistent associative dimensions (`cotas`).
    pub(super) fn tool_dimensions(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let dims: Vec<Value> = self
                    .project
                    .dimensions
                    .iter()
                    .map(|d| {
                        let resolved = self.project.resolve_dimension(d);
                        let (valor_m, rotulo) = match resolved {
                            Some(ResolvedDimension::Linear { value_m, label, .. }) => {
                                (Some(round(value_m)), Some(label))
                            }
                            Some(ResolvedDimension::Angular { angle_deg, label, .. }) => {
                                (Some(round2(angle_deg)), Some(label))
                            }
                            None => (None, None),
                        };
                        json!({
                            "id": d.id,
                            "tipo": d.kind.label(),
                            "valor": valor_m,
                            "rotulo": rotulo,
                            "texto_personalizado": d.override_text,
                            "visivel": d.visible,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!("{} cota(s) associativa(s).", dims.len()))
                    .with_structured(json!({
                        "projeto_id": self.active_project_id(),
                        "cotas": dims,
                    })))
            }
            "gerar_internas" => self.apply_change(
                Some("Todos os cômodos já possuem cotas internas."),
                |project| {
                    let added = project.generate_room_internal_dimensions();
                    Ok(Change::new(format!(
                        "{added} cota(s) interna(s) gerada(s) nos cômodos."
                    ))
                    .data(json!({ "adicionadas": added })))
                },
            ),
            "adicionar_parede" => {
                let wall_id = element_id(args, "parede_id")?;
                let offset = number(args, "deslocamento")?.unwrap_or(0.45);
                self.apply_change(None, |project| {
                    let id = project.add_wall_aligned_dimension(wall_id, offset)?;
                    Ok(Change::new(format!(
                        "Cota associativa {id} adicionada à parede {wall_id}."
                    ))
                    .data(json!({ "id": id })))
                })
            }
            "adicionar_alinhada" => {
                let p1 = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let p2 = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let offset = number(args, "deslocamento")?.unwrap_or(0.45);
                let external = flag(args, "externa");
                self.apply_change(None, |project| {
                    let a = DimensionAnchor::from_point(p1);
                    let b = DimensionAnchor::from_point(p2);
                    let kind = if external {
                        DimensionKind::External { a, b, offset }
                    } else {
                        DimensionKind::Aligned { a, b, offset }
                    };
                    let id = project.add_dimension(kind)?;
                    Ok(Change::new(format!("Cota {id} adicionada.")).data(json!({ "id": id })))
                })
            }
            "editar" => {
                let id = element_id(args, "id")?;
                let has_text = args.get("texto").is_some();
                let override_text = text(args, "texto");
                let offset = number(args, "deslocamento")?;
                if !has_text && offset.is_none() {
                    return Err("Informe 'texto' e/ou 'deslocamento'.".to_owned());
                }
                self.apply_change(Some("A cota já estava assim."), |project| {
                    if has_text {
                        project.update_dimension_override(id, override_text.clone())?;
                    }
                    if let Some(off) = offset {
                        project.update_dimension_offset(id, off)?;
                    }
                    Ok(Change::new(format!("Cota {id} atualizada.")))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_dimension(id) {
                        return Err("Cota não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Cota {id} removida.")))
                })
            }
            _ => Err(
                "Informe 'acao': listar, gerar_internas, adicionar_parede, adicionar_alinhada, editar ou remover."
                    .to_owned(),
            ),
        }
    }

    /// Vertical circulation & level transitions (`circulacao`).
    pub(super) fn tool_circulation(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let items: Vec<Value> = self
                    .project
                    .circulations
                    .iter()
                    .map(|c| {
                        let kind = match &c.kind {
                            CirculationKind::Stair { .. } => "escada",
                            CirculationKind::Ramp { .. } => "rampa",
                            CirculationKind::Landing { .. } => "patamar",
                            CirculationKind::Railing { .. } => "guarda_corpo",
                            CirculationKind::LevelChange { .. } => "desnivel",
                        };
                        let stair = c.stair_metrics().map(|m| {
                            json!({
                                "degraus": m.risers,
                                "espelho_cm": round2(m.riser_height_m * 100.0),
                                "piso_cm": round2(m.tread_m * 100.0),
                                "blondel_cm": round2(m.blondel_m * 100.0),
                                "blondel_ok": m.blondel_ok,
                            })
                        });
                        let ramp = c.ramp_metrics().map(|m| {
                            json!({
                                "inclinacao_pct": round2(m.slope_percent),
                                "nbr_9050_ok": m.nbr_9050_ok,
                            })
                        });
                        json!({
                            "id": c.id,
                            "nome": c.name,
                            "tipo": kind,
                            "origem": xy(c.origin),
                            "rotacao_graus": round2(c.rotation_deg),
                            "resumo": c.summary(),
                            "escada": stair,
                            "rampa": ramp,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!("{} elemento(s) de circulação.", items.len()))
                    .with_structured(json!({
                        "projeto_id": self.active_project_id(),
                        "circulacoes": items,
                    })))
            }
            "adicionar_escada" => {
                let origin = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let shape = match text(args, "formato") {
                    Some(raw) => parse_stair_shape(&raw)
                        .ok_or("'formato' deve ser reta, em_l ou em_u.")?,
                    None => StairShape::Straight,
                };
                let width = number(args, "largura_m")?.unwrap_or(1.0);
                let rise = number(args, "desnivel_m")?.unwrap_or(self.project.wall_height);
                let tread = number(args, "piso_m")?.unwrap_or(0.28);
                let rot = number(args, "rotacao_graus")?;
                let railing = match text(args, "guarda_corpo") {
                    Some(raw) => Some(parse_railing_side(&raw).ok_or(
                        "'guarda_corpo' deve ser nenhum, esquerda, direita ou ambos.",
                    )?),
                    None => None,
                };
                let custom_name = text(args, "nome");
                self.apply_change(None, |project| {
                    let id = project.add_stair(origin, shape, width, rise, tread)?;
                    if let Some(el) = project.circulations.iter_mut().find(|c| c.id == id) {
                        if let Some(deg) = rot {
                            el.rotation_deg = deg;
                        }
                        if let Some(r) = railing {
                            el.railing = r;
                        }
                        if let Some(n) = &custom_name {
                            el.name = n.clone();
                        }
                    }
                    let el = project.circulations.iter().find(|c| c.id == id).unwrap();
                    let m = el.stair_metrics().unwrap();
                    Ok(Change::new(format!("{} adicionada (id {id}): {}.", el.name, el.summary()))
                        .data(json!({
                            "id": id,
                            "degraus": m.risers,
                            "blondel_cm": round2(m.blondel_m * 100.0),
                            "blondel_ok": m.blondel_ok,
                        })))
                })
            }
            "adicionar_rampa" => {
                let origin = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let width = number(args, "largura_m")?.unwrap_or(1.20);
                let length = required_number(args, "comprimento_m")?;
                let rise = required_number(args, "desnivel_m")?;
                let rot = number(args, "rotacao_graus")?;
                let railing = match text(args, "guarda_corpo") {
                    Some(raw) => Some(parse_railing_side(&raw).ok_or(
                        "'guarda_corpo' deve ser nenhum, esquerda, direita ou ambos.",
                    )?),
                    None => None,
                };
                let custom_name = text(args, "nome");
                self.apply_change(None, |project| {
                    let id = project.add_ramp(origin, width, length, rise)?;
                    if let Some(el) = project.circulations.iter_mut().find(|c| c.id == id) {
                        if let Some(deg) = rot {
                            el.rotation_deg = deg;
                        }
                        if let Some(r) = railing {
                            el.railing = r;
                        }
                        if let Some(n) = &custom_name {
                            el.name = n.clone();
                        }
                    }
                    let el = project.circulations.iter().find(|c| c.id == id).unwrap();
                    let m = el.ramp_metrics().unwrap();
                    Ok(Change::new(format!("{} adicionada (id {id}): {}.", el.name, el.summary()))
                        .data(json!({
                            "id": id,
                            "inclinacao_pct": round2(m.slope_percent),
                            "nbr_9050_ok": m.nbr_9050_ok,
                        })))
                })
            }
            "adicionar_patamar" => {
                let origin = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let width = required_number(args, "largura_m")?;
                let depth = required_number(args, "profundidade_m")?;
                let elev = number(args, "nivel_m")?.unwrap_or(0.0);
                let rot = number(args, "rotacao_graus")?;
                self.apply_change(None, |project| {
                    let id = project.add_landing(origin, width, depth, elev)?;
                    if let Some(deg) = rot
                        && let Some(el) = project.circulations.iter_mut().find(|c| c.id == id)
                    {
                        el.rotation_deg = deg;
                    }
                    Ok(Change::new(format!("Patamar {id} adicionado.")).data(json!({ "id": id })))
                })
            }
            "adicionar_guarda_corpo" => {
                let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let height = number(args, "altura_m")?.unwrap_or(1.10);
                self.apply_change(None, |project| {
                    let id = project.add_railing(start, end, height)?;
                    Ok(Change::new(format!("Guarda-corpo {id} adicionado."))
                        .data(json!({ "id": id })))
                })
            }
            "adicionar_desnivel" => {
                let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let from_elev = number(args, "nivel_de_m")?.unwrap_or(0.0);
                let to_elev = required_number(args, "nivel_para_m")?;
                self.apply_change(None, |project| {
                    let id = project.add_level_change(start, end, from_elev, to_elev)?;
                    Ok(Change::new(format!("Desnível {id} adicionado.")).data(json!({ "id": id })))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.remove_circulation(id) {
                        return Err("Elemento de circulação não encontrado.".to_owned());
                    }
                    Ok(Change::new(format!("Circulação {id} removida.")))
                })
            }
            _ => Err(
                "Informe 'acao': listar, adicionar_escada, adicionar_rampa, adicionar_patamar, adicionar_guarda_corpo, adicionar_desnivel ou remover."
                    .to_owned(),
            ),
        }
    }

    /// Architectural sections, facade elevations and detail callouts (`vistas_tecnicas`).
    pub(super) fn tool_views(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let sections: Vec<Value> = self
                    .project
                    .views
                    .sections
                    .iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "rotulo": s.label,
                            "inicio": xy(s.start),
                            "fim": xy(s.end),
                            "olhar_esquerda": s.look_left,
                            "prancha_ref": s.target_sheet,
                        })
                    })
                    .collect();
                let elevations: Vec<Value> = self
                    .project
                    .views
                    .elevations
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "rotulo": e.label,
                            "lado": e.side.short().to_lowercase(),
                            "prancha_ref": e.target_sheet,
                        })
                    })
                    .collect();
                let details: Vec<Value> = self
                    .project
                    .views
                    .details
                    .iter()
                    .map(|d| {
                        json!({
                            "id": d.id,
                            "rotulo": d.label,
                            "centro": xy(d.center),
                            "raio_m": round2(d.radius),
                            "escala": d.scale.label(),
                            "prancha_ref": d.target_sheet,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "{} corte(s), {} fachada(s), {} detalhe(s).",
                    sections.len(),
                    elevations.len(),
                    details.len()
                ))
                .with_structured(json!({
                    "projeto_id": self.active_project_id(),
                    "cortes": sections,
                    "fachadas": elevations,
                    "detalhes": details,
                })))
            }
            "adicionar_corte" => {
                let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let label = text(args, "rotulo").unwrap_or_default();
                let look_left = args
                    .get("olhar_esquerda")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let target_sheet = text(args, "prancha_ref");
                self.apply_change(None, |project| {
                    let id = project.add_section_line(
                        &label,
                        start,
                        end,
                        look_left,
                        target_sheet.clone(),
                    )?;
                    let saved = project
                        .views
                        .sections
                        .iter()
                        .find(|s| s.id == id)
                        .unwrap();
                    Ok(Change::new(format!("Corte {} adicionado (id {id}).", saved.label))
                        .data(json!({ "id": id, "rotulo": saved.label })))
                })
            }
            "adicionar_corte_padrao" => {
                let horizontal = args
                    .get("horizontal")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.apply_change(None, |project| {
                    let id = project.add_default_section(horizontal)?;
                    let saved = project
                        .views
                        .sections
                        .iter()
                        .find(|s| s.id == id)
                        .unwrap();
                    Ok(Change::new(format!("Corte padrão {} criado (id {id}).", saved.label))
                        .data(json!({ "id": id, "rotulo": saved.label })))
                })
            }
            "adicionar_fachada" => {
                let side_raw = text(args, "lado").ok_or("Informe 'lado': norte, sul, leste ou oeste.")?;
                let side = parse_elevation_side(&side_raw)
                    .ok_or("'lado' deve ser norte, sul, leste ou oeste.")?;
                let target_sheet = text(args, "prancha_ref");
                self.apply_change(None, |project| {
                    let id = project.add_elevation_view(side, target_sheet.clone());
                    Ok(Change::new(format!("{} adicionada (id {id}).", side.label()))
                        .data(json!({ "id": id, "rotulo": side.label() })))
                })
            }
            "adicionar_detalhe" => {
                let center = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let radius = number(args, "raio_m")?.unwrap_or(1.0);
                let label = text(args, "rotulo").unwrap_or_default();
                let scale = match text(args, "escala") {
                    Some(raw) => parse_scale(&raw)
                        .ok_or("'escala' inválida (ex.: 1:25, 1:50, 1:20).")?,
                    None => DrawingScale::S1_25,
                };
                let target_sheet = text(args, "prancha_ref");
                self.apply_change(None, |project| {
                    let id = project.add_detail_callout(
                        &label,
                        center,
                        radius,
                        scale,
                        target_sheet.clone(),
                    )?;
                    let saved = project
                        .views
                        .details
                        .iter()
                        .find(|d| d.id == id)
                        .unwrap();
                    Ok(Change::new(format!("Detalhe {} adicionado (id {id}).", saved.label))
                        .data(json!({ "id": id, "rotulo": saved.label })))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    let removed = project.remove_section(id)
                        || project.remove_elevation(id)
                        || project.remove_detail_callout(id);
                    if !removed {
                        return Err("Vista técnica (corte, fachada ou detalhe) não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Vista técnica {id} removida.")))
                })
            }
            "exportar" => {
                let id = element_id(args, "id")?;
                let format = text(args, "formato")
                    .unwrap_or_else(|| "svg".to_owned())
                    .to_lowercase();
                if format != "svg" && format != "pdf" {
                    return Err("'formato' deve ser svg ou pdf.".to_owned());
                }
                let (label, markup) = if let Some(s) =
                    self.project.views.sections.iter().find(|s| s.id == id)
                {
                    (format!("corte-{}", s.label), section_svg(&self.project, s))
                } else if let Some(e) =
                    self.project.views.elevations.iter().find(|e| e.id == id)
                {
                    (
                        format!("fachada-{}", e.side.short().to_lowercase()),
                        elevation_svg(&self.project, e),
                    )
                } else if let Some(d) = self.project.views.details.iter().find(|d| d.id == id) {
                    (format!("detalhe-{}", d.label), detail_svg(&self.project, d))
                } else {
                    return Err("Vista técnica não encontrada.".to_owned());
                };
                let path = match text(args, "caminho") {
                    Some(input) => resolve_path(&input, &format),
                    None => unique_path(documents_dir().join(format!(
                        "{}-{}.{format}",
                        file_stem(&self.project.name),
                        file_stem(&label)
                    ))),
                };
                ensure_parent(&path)?;
                if format == "pdf" {
                    export_view_pdf(&markup, &path)?;
                } else {
                    export_view_svg(&markup, &path)?;
                }
                Ok(ToolOutput::ok(format!(
                    "Vista técnica exportada em {}",
                    path.display()
                ))
                .with_structured(json!({
                    "id": id,
                    "formato": format,
                    "caminho": path.display().to_string(),
                })))
            }
            _ => Err(
                "Informe 'acao': listar, adicionar_corte, adicionar_corte_padrao, adicionar_fachada, adicionar_detalhe, remover ou exportar."
                    .to_owned(),
            ),
        }
    }

    /// ABNT technical sheet format, scale and title block (`prancha`).
    pub(super) fn tool_sheet(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "obter".to_owned())
            .to_lowercase();
        let sheet_json = |sheet: &crate::domain::sheet::SheetLayout| {
            let (w, h) = sheet.sheet_mm();
            let (_, _, pw, ph) = sheet.printable_rect_mm();
            json!({
                "formato": sheet.size.short_name(),
                "largura_mm": round2(w),
                "altura_mm": round2(h),
                "area_util_mm": [round2(pw), round2(ph)],
                "orientacao": match sheet.orientation {
                    Orientation::Landscape => "paisagem",
                    Orientation::Portrait => "retrato",
                },
                "escala": sheet.scale.label(),
                "carimbo": {
                    "titulo": sheet.title_block.drawing_title,
                    "cliente": sheet.title_block.client,
                    "endereco": sheet.title_block.address,
                    "autor": sheet.title_block.author,
                    "registro": sheet.title_block.registration,
                    "data": sheet.title_block.date,
                    "folha": sheet.title_block.sheet_number,
                    "revisao": sheet.title_block.revision,
                    "notas": sheet.title_block.notes,
                    "exibir_carimbo": sheet.title_block.show_title_block,
                    "exibir_legenda": sheet.title_block.show_legend,
                    "exibir_norte": sheet.title_block.show_north,
                    "exibir_escala": sheet.title_block.show_scale_bar,
                },
            })
        };
        match action.as_str() {
            "obter" => {
                let data = sheet_json(&self.project.sheet);
                Ok(ToolOutput::ok(format!(
                    "Prancha {} {} em escala {}.",
                    self.project.sheet.size.short_name(),
                    self.project.sheet.orientation.label().to_lowercase(),
                    self.project.sheet.scale.label()
                ))
                .with_structured(data))
            }
            "configurar" => {
                let mut next = self.project.sheet.clone();
                if let Some(raw) = text(args, "formato") {
                    let size = match raw.to_uppercase().as_str() {
                        "A4" => PaperSize::A4,
                        "A3" => PaperSize::A3,
                        "A2" => PaperSize::A2,
                        "A1" => PaperSize::A1,
                        "A0" => PaperSize::A0,
                        "PERSONALIZADO" | "CUSTOM" => {
                            let width_mm = required_number(args, "largura_mm")?;
                            let height_mm = required_number(args, "altura_mm")?;
                            PaperSize::Custom {
                                width_mm,
                                height_mm,
                            }
                        }
                        _ => {
                            return Err("'formato' deve ser A4, A3, A2, A1, A0 ou personalizado."
                                .to_owned());
                        }
                    };
                    next.size = size;
                    next.margins = SheetMargins::abnt_for(size);
                }
                if let Some(raw) = text(args, "orientacao") {
                    next.orientation = match raw.to_lowercase().as_str() {
                        "paisagem" | "landscape" => Orientation::Landscape,
                        "retrato" | "portrait" => Orientation::Portrait,
                        _ => return Err("'orientacao' deve ser paisagem ou retrato.".to_owned()),
                    };
                }
                if let Some(raw) = text(args, "escala") {
                    if matches!(
                        raw.to_lowercase().as_str(),
                        "melhor" | "auto" | "melhor_ajuste"
                    ) {
                        let (_, _, pw, ph) = next.printable_rect_mm();
                        let (plan_w, plan_h) = self
                            .project
                            .bounds()
                            .map(|(min, max)| ((max.x - min.x).max(1.0), (max.y - min.y).max(1.0)))
                            .unwrap_or((10.0, 10.0));
                        next.scale = DrawingScale::best_fit(plan_w, plan_h, pw, ph);
                    } else {
                        next.scale = parse_scale(&raw).ok_or(
                            "'escala' deve ser 1:25, 1:50, 1:75, 1:100, 1:200, 1:N ou 'melhor'."
                                .to_owned(),
                        )?;
                    }
                }
                if let Some(v) = text(args, "titulo") {
                    next.title_block.drawing_title = v;
                }
                if let Some(v) = text(args, "cliente") {
                    next.title_block.client = v;
                }
                if let Some(v) = text(args, "endereco") {
                    next.title_block.address = v;
                }
                if let Some(v) = text(args, "autor") {
                    next.title_block.author = v;
                }
                if let Some(v) = text(args, "registro") {
                    next.title_block.registration = v;
                }
                if let Some(v) = text(args, "data") {
                    next.title_block.date = v;
                }
                if let Some(v) = text(args, "folha") {
                    next.title_block.sheet_number = v;
                }
                if let Some(v) = text(args, "revisao") {
                    next.title_block.revision = v;
                }
                if let Some(v) = text(args, "notas") {
                    next.title_block.notes = v;
                }
                if let Some(v) = args.get("exibir_carimbo").and_then(Value::as_bool) {
                    next.title_block.show_title_block = v;
                }
                if let Some(v) = args.get("exibir_legenda").and_then(Value::as_bool) {
                    next.title_block.show_legend = v;
                }
                if let Some(v) = args.get("exibir_norte").and_then(Value::as_bool) {
                    next.title_block.show_north = v;
                }
                if let Some(v) = args.get("exibir_escala").and_then(Value::as_bool) {
                    next.title_block.show_scale_bar = v;
                }
                next.validate()?;
                let summary = sheet_json(&next);
                self.apply_change(
                    Some("A prancha já estava com essa configuração."),
                    |project| {
                        project.sheet = next.clone();
                        Ok(Change::new(format!(
                            "Prancha configurada: {} {} ({}).",
                            next.size.short_name(),
                            next.orientation.label().to_lowercase(),
                            next.scale.label()
                        ))
                        .data(summary))
                    },
                )
            }
            _ => Err("Informe 'acao': obter ou configurar.".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    fn app() -> FloorPlanApp {
        FloorPlanApp::new(&egui::Context::default())
    }

    fn ok(app: &mut FloorPlanApp, name: &str, args: Value) -> ToolOutput {
        let out = app.run_mcp_tool(name, &args);
        assert!(!out.is_error, "{name}: {}", out.text);
        out
    }

    #[test]
    fn quantities_ergonomics_finishes_setbacks_and_openings_work_via_mcp() {
        let mut app = app();
        ok(
            &mut app,
            "definir_terreno",
            json!({ "largura": 12, "comprimento": 20, "recuo_frente": 2.0, "recuo_fundos": 1.5, "recuo_lateral": 1.0 }),
        );
        // Update only setbacks without respecifying width/length.
        ok(&mut app, "definir_terreno", json!({ "recuo_frente": 1.5 }));
        assert!((app.project.setbacks.front - 1.5).abs() < 1e-9);

        let room = ok(
            &mut app,
            "criar_comodo",
            json!({ "nome": "Sala", "x": 2, "y": 3, "largura": 5, "comprimento": 4 }),
        );
        let room_id = room.structured.as_ref().unwrap()["id"].as_u64().unwrap();
        let wall_id = room.structured.as_ref().unwrap()["paredes"][0]
            .as_u64()
            .unwrap();

        // Finishes preset + custom override.
        ok(
            &mut app,
            "editar_comodo",
            json!({
                "id": room_id,
                "preset_acabamento": "social_porcelanato",
                "piso": "Porcelanato 80x80",
                "rodape_cm": 12,
                "impermeabilizacao": true
            }),
        );

        // Door & window typologies.
        let door = ok(
            &mut app,
            "inserir_abertura",
            json!({ "parede_id": wall_id, "tipo": "porta", "posicao": 0.3 }),
        );
        let door_id = door.structured.as_ref().unwrap()["id"].as_u64().unwrap();
        ok(
            &mut app,
            "editar_abertura",
            json!({ "id": door_id, "tipo_porta": "pivotante" }),
        );
        let win = ok(
            &mut app,
            "inserir_abertura",
            json!({ "parede_id": wall_id, "tipo": "janela", "posicao": 0.75 }),
        );
        let win_id = win.structured.as_ref().unwrap()["id"].as_u64().unwrap();
        ok(
            &mut app,
            "editar_abertura",
            json!({ "id": win_id, "tipo_janela": "maxim_ar" }),
        );

        // Free angle on fixture.
        let table = ok(
            &mut app,
            "inserir_objeto",
            json!({ "tipo": "mesa", "x": 4.5, "y": 5.0 }),
        );
        let table_id = table.structured.as_ref().unwrap()["id"].as_u64().unwrap();
        ok(
            &mut app,
            "editar_objeto",
            json!({ "id": table_id, "angulo_graus": 30 }),
        );

        let plan = ok(&mut app, "obter_planta", json!({})).structured.unwrap();
        assert_eq!(plan["terreno"]["recuos"]["frente"], 1.5);
        assert_eq!(
            plan["comodos"][0]["acabamentos"]["piso"],
            "Porcelanato 80x80"
        );
        assert_eq!(plan["comodos"][0]["acabamentos"]["impermeabilizacao"], true);
        assert_eq!(plan["aberturas"][0]["tipologia"], "Pivotante");
        assert_eq!(plan["aberturas"][1]["tipologia"], "Maxim-ar");
        assert_eq!(plan["objetos"][0]["angulo_graus"], 30.0);

        // Structure + roof + soil survey + quantities.
        ok(
            &mut app,
            "projeto_estrutural",
            json!({ "acao": "gerar_estrutura" }),
        );
        ok(
            &mut app,
            "projeto_estrutural",
            json!({ "acao": "configurar", "peso_alvenaria_kn_m3": 15.0, "sobrecarga_por_comodo": false }),
        );
        let roof = ok(
            &mut app,
            "projeto_estrutural",
            json!({
                "acao": "definir_telhado",
                "x": 2, "y": 3, "largura_m": 5, "comprimento_m": 4,
                "lado_alto": "oeste", "altura_alta_m": 3.8, "altura_baixa_m": 2.6,
                "ajustar_paredes": false
            }),
        );
        let roof_id = roof.structured.unwrap()["id"].as_u64().unwrap();
        let fitted = ok(
            &mut app,
            "projeto_estrutural",
            json!({ "acao": "ajustar_paredes_telhado", "id": roof_id }),
        );
        assert!(
            fitted.structured.unwrap()["paredes_ajustadas"]
                .as_u64()
                .unwrap()
                > 0
        );

        ok(
            &mut app,
            "projeto_estrutural",
            json!({
                "acao": "definir_sondagem",
                "nome": "SP-01",
                "leituras": [
                    { "profundidade_m": 1, "n": 8, "solo": "argila" },
                    { "profundidade_m": 2, "n": 10, "solo": "argila" },
                    { "profundidade_m": 3, "n": 12, "solo": "areia" }
                ]
            }),
        );
        let presizing = ok(&mut app, "pre_dimensionamento", json!({}))
            .structured
            .unwrap();
        assert_eq!(presizing["materiais"]["peso_alvenaria_kn_m3"], 15.0);
        assert_eq!(presizing["materiais"]["sobrecarga_por_comodo"], false);
        assert_eq!(presizing["sondagem"]["nome"], "SP-01");
        assert!(presizing["estudo_fundacoes"]["opcoes"].is_array());

        let qty = ok(&mut app, "quantitativos", json!({})).structured.unwrap();
        assert!(qty["totais"]["alvenaria_liquida_m2"].as_f64().unwrap() > 0.0);
        assert!(qty["totais"]["aco_preliminar_kg"].as_f64().unwrap() > 0.0);
        assert!(qty["telhados"]["madeira_m3"].as_f64().unwrap() > 0.0);
        assert!(!qty["vergas_contravergas"].as_array().unwrap().is_empty());

        let ergo = ok(&mut app, "ergonomia", json!({ "passagem_minima_m": 0.90 }))
            .structured
            .unwrap();
        assert_eq!(ergo["passagem_minima_m"], 0.9);
        assert!(ergo["ocorrencias"].is_array());
    }

    #[test]
    fn layers_notes_dimensions_circulation_views_sheet_and_exports_work_via_mcp() {
        let mut app = app();
        ok(
            &mut app,
            "definir_terreno",
            json!({ "largura": 14, "comprimento": 18 }),
        );
        let room = ok(
            &mut app,
            "criar_comodo",
            json!({ "nome": "Sala", "x": 1, "y": 1, "largura": 6, "comprimento": 5 }),
        );
        let wall_id = room.structured.unwrap()["paredes"][0].as_u64().unwrap();

        // Layers.
        let layer = ok(
            &mut app,
            "camadas",
            json!({ "acao": "criar", "nome": "Demolir", "cor": "#c62828", "espessura_mm": 0.30, "tipo_linha": "tracejada", "ativar": true }),
        );
        let layer_id = layer.structured.unwrap()["id"].as_u64().unwrap();
        ok(
            &mut app,
            "camadas",
            json!({ "acao": "atribuir", "id": layer_id, "parede_id": wall_id }),
        );
        ok(
            &mut app,
            "camadas",
            json!({ "acao": "editar", "id": layer_id, "visivel": false, "bloqueada": true }),
        );
        let layers = ok(&mut app, "camadas", json!({ "acao": "listar" }))
            .structured
            .unwrap();
        assert!(
            layers["camadas"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["id"] == layer_id && l["visivel"] == false && l["bloqueada"] == true)
        );
        ok(
            &mut app,
            "camadas",
            json!({ "acao": "excluir", "id": layer_id }),
        );
        // Deleting a built-in layer must fail.
        assert!(
            app.run_mcp_tool("camadas", &json!({ "acao": "excluir", "id": 1 }))
                .is_error
        );

        // Notes & symbols.
        let note = ok(
            &mut app,
            "anotacoes",
            json!({ "acao": "adicionar", "tipo": "chamada", "x": 3, "y": 3, "alvo_x": 2, "alvo_y": 2, "texto": "Ralo linear" }),
        );
        let note_id = note.structured.unwrap()["id"].as_u64().unwrap();
        ok(
            &mut app,
            "anotacoes",
            json!({ "acao": "adicionar", "tipo": "cota_nivel", "x": 4, "y": 4, "nivel_m": 0.15 }),
        );
        ok(
            &mut app,
            "anotacoes",
            json!({ "acao": "editar", "id": note_id, "texto": "Ralo oculto", "dx": 0.5, "dy": 0.2 }),
        );
        let listed_notes = ok(&mut app, "anotacoes", json!({ "acao": "listar" }))
            .structured
            .unwrap();
        assert_eq!(listed_notes["anotacoes"].as_array().unwrap().len(), 2);
        ok(
            &mut app,
            "anotacoes",
            json!({ "acao": "remover", "id": note_id }),
        );
        assert_eq!(app.project.notes.len(), 1);

        // Dimensions.
        let generated = ok(&mut app, "cotas", json!({ "acao": "gerar_internas" }))
            .structured
            .unwrap();
        assert_eq!(generated["adicionadas"], 2);
        let wall_dim = ok(
            &mut app,
            "cotas",
            json!({ "acao": "adicionar_parede", "parede_id": wall_id, "deslocamento": 0.6 }),
        )
        .structured
        .unwrap()["id"]
            .as_u64()
            .unwrap();
        ok(
            &mut app,
            "cotas",
            json!({ "acao": "editar", "id": wall_dim, "texto": "6,00 m (eixo)" }),
        );
        let dims = ok(&mut app, "cotas", json!({ "acao": "listar" }))
            .structured
            .unwrap();
        assert_eq!(dims["cotas"].as_array().unwrap().len(), 3);
        ok(
            &mut app,
            "cotas",
            json!({ "acao": "remover", "id": wall_dim }),
        );
        assert_eq!(app.project.dimensions.len(), 2);

        // Circulation.
        let stair = ok(
            &mut app,
            "circulacao",
            json!({ "acao": "adicionar_escada", "x": 2, "y": 2, "formato": "em_l", "largura_m": 1.1, "desnivel_m": 2.8, "piso_m": 0.28 }),
        )
        .structured
        .unwrap();
        assert_eq!(stair["blondel_ok"], true);
        let ramp = ok(
            &mut app,
            "circulacao",
            json!({ "acao": "adicionar_rampa", "x": 8, "y": 2, "largura_m": 1.2, "comprimento_m": 6.0, "desnivel_m": 0.45 }),
        )
        .structured
        .unwrap();
        assert_eq!(ramp["nbr_9050_ok"], true);
        ok(
            &mut app,
            "circulacao",
            json!({ "acao": "adicionar_patamar", "x": 8, "y": 8, "largura_m": 1.5, "profundidade_m": 1.5, "nivel_m": 0.45 }),
        );
        ok(
            &mut app,
            "circulacao",
            json!({ "acao": "adicionar_guarda_corpo", "x1": 8, "y1": 8, "x2": 9.5, "y2": 8, "altura_m": 1.1 }),
        );
        let dc_id = ok(
            &mut app,
            "circulacao",
            json!({ "acao": "adicionar_desnivel", "x1": 7, "y1": 1, "x2": 7, "y2": 6, "nivel_de_m": 0.15, "nivel_para_m": 0.0 }),
        )
        .structured
        .unwrap()["id"]
            .as_u64()
            .unwrap();
        assert_eq!(
            ok(&mut app, "circulacao", json!({ "acao": "listar" }))
                .structured
                .unwrap()["circulacoes"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        ok(
            &mut app,
            "circulacao",
            json!({ "acao": "remover", "id": dc_id }),
        );
        assert_eq!(app.project.circulations.len(), 4);

        // Technical views & sheet configuration.
        let sec_id = ok(
            &mut app,
            "vistas_tecnicas",
            json!({ "acao": "adicionar_corte_padrao", "horizontal": true }),
        )
        .structured
        .unwrap()["id"]
            .as_u64()
            .unwrap();
        ok(
            &mut app,
            "vistas_tecnicas",
            json!({ "acao": "adicionar_fachada", "lado": "sul", "prancha_ref": "PR-02" }),
        );
        ok(
            &mut app,
            "vistas_tecnicas",
            json!({ "acao": "adicionar_detalhe", "x": 3, "y": 3, "raio_m": 1.2, "escala": "1:20" }),
        );
        let views = ok(&mut app, "vistas_tecnicas", json!({ "acao": "listar" }))
            .structured
            .unwrap();
        assert_eq!(views["cortes"].as_array().unwrap().len(), 1);
        assert_eq!(views["fachadas"].as_array().unwrap().len(), 1);
        assert_eq!(views["detalhes"].as_array().unwrap().len(), 1);

        let sheet = ok(
            &mut app,
            "prancha",
            json!({
                "acao": "configurar",
                "formato": "A2",
                "orientacao": "paisagem",
                "escala": "melhor",
                "titulo": "Planta Arquitetônica",
                "cliente": "Cliente Teste",
                "autor": "Arq. Trazo",
                "registro": "CAU A12345-6",
                "folha": "01/02",
                "revisao": "R01"
            }),
        )
        .structured
        .unwrap();
        assert_eq!(sheet["formato"], "A2");
        assert_eq!(sheet["carimbo"]["cliente"], "Cliente Teste");

        // Exports (view SVG + plan SVG + scaled sheet PDF + report PDF + CSVs).
        ok(
            &mut app,
            "projeto_estrutural",
            json!({ "acao": "gerar_estrutura" }),
        );
        let tmp = std::env::temp_dir().join(format!("trazo-mcp-exports-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let sec_svg = tmp.join("corte.svg");
        ok(
            &mut app,
            "vistas_tecnicas",
            json!({ "acao": "exportar", "id": sec_id, "formato": "svg", "caminho": sec_svg.display().to_string() }),
        );
        assert!(sec_svg.is_file());

        for (fmt, fname) in [
            ("svg", "planta.svg"),
            ("prancha_svg", "prancha.svg"),
            ("prancha_pdf", "prancha.pdf"),
            ("memorial_pdf", "memorial.pdf"),
            ("unifilar_svg", "unifilar.svg"),
            ("esforcos_svg", "esforcos.svg"),
            ("quantitativos_csv", "quantitativos.csv"),
            ("tabela_aco_csv", "tabela_aco.csv"),
            ("csv", "quadros.csv"),
        ] {
            let p = tmp.join(fname);
            let res = ok(
                &mut app,
                "exportar_planta",
                json!({ "formato": fmt, "caminho": p.display().to_string() }),
            );
            assert!(p.is_file(), "missing export for {fmt}");
            assert!(
                std::fs::metadata(&p).unwrap().len() > 0,
                "empty export for {fmt}"
            );
            if fmt == "memorial_pdf" {
                assert!(res.structured.unwrap()["paginas"].as_u64().unwrap() >= 1);
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);

        // Undo works cleanly across new tools.
        let before_undo = app.project.structure.beams.len();
        assert!(before_undo > 0);
        ok(&mut app, "desfazer", json!({ "passos": 1 }));
        assert_eq!(app.project.structure.beams.len(), 0);
    }
}
