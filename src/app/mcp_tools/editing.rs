//! Assistant tools for the editing commands: selecting what the person
//! sees, moving, turning, mirroring and copying several elements, aligning,
//! distributing, grouping, repeating, trimming, extending, offsetting,
//! dividing and joining, and the geometric constraints.

use serde_json::{Value, json};

use super::{Change, ToolOutput, element_id, flag, number, required_number, text, to_model};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::element::ElementKind;
use crate::domain::geometry::Point;
use crate::domain::model::{Alignment, Axis, ConstraintKind, Entity, Vertex};
use crate::domain::transform::Transform;

/// `ids` as a list of numbers (or a single one, or "1, 2, 3").
fn ids_of(args: &Value) -> Result<Vec<u64>, String> {
    let wrong = || "Informe 'ids': a lista de ids dos elementos (veja obter_planta).".to_owned();
    let one = |value: &Value| match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    };
    let ids: Vec<u64> = match args.get("ids") {
        Some(Value::Array(items)) => items
            .iter()
            .map(one)
            .collect::<Option<_>>()
            .ok_or_else(wrong)?,
        Some(Value::String(list)) => list
            .split(',')
            .map(|part| part.trim().parse::<u64>().ok())
            .collect::<Option<_>>()
            .ok_or_else(wrong)?,
        Some(value) => vec![one(value).ok_or_else(wrong)?],
        None => return Err(wrong()),
    };
    if ids.is_empty() {
        return Err(wrong());
    }
    Ok(ids)
}

/// The ids of `items`, for answers.
fn ids_json(items: &[Selection]) -> Value {
    json!(items.iter().map(|item| item.id()).collect::<Vec<_>>())
}

fn entity_of(selection: Selection, id: u64) -> Result<Entity, String> {
    Entity::of(selection).ok_or(format!(
        "O elemento {id} não é uma parede nem um desenho: restrições ligam paredes e linhas."
    ))
}

impl FloorPlanApp {
    /// The elements named by `ids`.
    pub(super) fn elements_of(&self, args: &Value) -> Result<Vec<Selection>, String> {
        ids_of(args)?
            .into_iter()
            .map(|id| self.find_element(id))
            .collect()
    }

    /// A point given as `x`/`y` (or with a prefix, `centro_x`/`centro_y`).
    fn point_arg(args: &Value, prefix: &str) -> Result<Option<Point>, String> {
        let (x, y) = (
            number(args, &format!("{prefix}x"))?,
            number(args, &format!("{prefix}y"))?,
        );
        match (x, y) {
            (Some(x), Some(y)) => Ok(Some(to_model(x, y))),
            (None, None) => Ok(None),
            _ => Err(format!("Informe '{prefix}x' e '{prefix}y' juntos.")),
        }
    }

    /// `selecionar`: shows the person which elements the assistant means.
    pub(super) fn tool_select(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let items = if flag(args, "tudo") {
            self.project.filtered_elements(Default::default())
        } else if let Some(guid_raw) = text(args, "guid") {
            let id = self
                .project
                .resolve_element_ref(&guid_raw)
                .ok_or_else(|| format!("Nenhum elemento encontrado para o GUID '{guid_raw}'."))?;
            vec![self.find_element(id)?]
        } else if let Some(kind) = text(args, "tipo") {
            let kind = match kind.as_str() {
                "comodos" | "comodo" => ElementKind::Room,
                "paredes" | "parede" => ElementKind::Wall,
                "aberturas" | "abertura" => ElementKind::Opening,
                "objetos" | "objeto" => ElementKind::Fixture,
                "desenhos" | "desenho" => ElementKind::Drawing,
                "terreno" => ElementKind::Terrain,
                other => {
                    return Err(format!(
                        "Tipo '{other}' desconhecido: comodos, paredes, aberturas, objetos, desenhos ou terreno."
                    ));
                }
            };
            self.project
                .elements()
                .into_iter()
                .filter(|item| item.kind() == kind)
                .collect()
        } else if args.get("ids").is_some() {
            self.elements_of(args)?
        } else {
            Vec::new()
        };
        self.cancel_creation();
        self.set_tool(crate::app::Tool::Move);
        self.set_selection_set(items.clone());
        if flag(args, "aproximar")
            && let Some(primary) = self.selection
        {
            self.zoom_to_selection(primary);
        }
        let message = if items.is_empty() {
            "Seleção limpa.".to_owned()
        } else {
            format!(
                "{} elemento(s) selecionado(s) na tela do usuário.",
                items.len()
            )
        };
        let guids: Vec<Option<String>> = items
            .iter()
            .map(|item| self.project.guid_of(item.id()).map(|g| g.to_uuid_string()))
            .collect();
        Ok(ToolOutput::ok(message)
            .with_structured(json!({ "selecao": ids_json(&items), "guids": guids })))
    }

    /// Runs an edit on the listed elements and selects what it reports.
    fn edit_elements(
        &mut self,
        unchanged: &str,
        change: impl FnOnce(
            &mut crate::domain::model::Project,
        ) -> Result<(String, Vec<Selection>), String>,
    ) -> Result<ToolOutput, String> {
        let mut selected = Vec::new();
        let output = self.apply_change(Some(unchanged), |project| {
            let (message, items) = change(project)?;
            selected = items.clone();
            Ok(Change::new(message).data(json!({ "ids": ids_json(&items) })))
        })?;
        if !selected.is_empty() {
            self.set_selection_set(selected);
        }
        Ok(output)
    }

    /// `transformar`: moves, turns or mirrors elements, or copies them so.
    pub(super) fn tool_transform(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let items = self.elements_of(args)?;
        let copy = flag(args, "copiar");
        let operation = text(args, "operacao").unwrap_or_else(|| "mover".to_owned());
        let center = match Self::point_arg(args, "centro_")? {
            Some(center) => center,
            None => {
                let (low, high) = self
                    .project
                    .elements_bounds(&items)
                    .ok_or("Elementos sem posição.")?;
                low.midpoint(high).rounded()
            }
        };
        let transform = match operation.as_str() {
            "mover" => {
                let dx = number(args, "dx")?.unwrap_or(0.0);
                let dy = number(args, "dy")?.unwrap_or(0.0);
                Transform::Translate(Point::new(dx, -dy))
            }
            "girar" => Transform::rotate_degrees(center, required_number(args, "angulo")?),
            "espelhar" => match text(args, "eixo").as_deref() {
                Some("vertical") | None if args.get("x1").is_none() => {
                    Transform::mirror(center, Point::new(center.x, center.y + 1.0))?
                }
                Some("horizontal") => {
                    Transform::mirror(center, Point::new(center.x + 1.0, center.y))?
                }
                _ => {
                    let a = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                    let b = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                    Transform::mirror(a, b)?
                }
            },
            other => {
                return Err(format!(
                    "Operação '{other}' desconhecida: mover, girar ou espelhar."
                ));
            }
        };
        self.edit_elements(
            "Nada mudou: verifique o terreno e os elementos.",
            |project| {
                if copy {
                    let copies = project.copy_elements(&items, &transform)?;
                    Ok((
                        format!("{} cópia(s) criada(s) ({operation}).", copies.len()),
                        copies,
                    ))
                } else {
                    project.transform_elements(&items, &transform)?;
                    Ok((
                        format!("{} elemento(s): {operation} feito.", items.len()),
                        items.clone(),
                    ))
                }
            },
        )
    }

    /// `alinhar` and `distribuir`.
    pub(super) fn tool_arrange(
        &mut self,
        args: &Value,
        distribute: bool,
    ) -> Result<ToolOutput, String> {
        let items = self.elements_of(args)?;
        if distribute {
            let axis = match text(args, "direcao").as_deref() {
                Some("horizontal") | None => Axis::Horizontal,
                Some("vertical") => Axis::Vertical,
                Some(other) => {
                    return Err(format!("Direção '{other}': use horizontal ou vertical."));
                }
            };
            return self.edit_elements("Já estavam distribuídos.", |project| {
                let moved = project.distribute_elements(&items, axis)?;
                Ok((
                    format!("{} ({moved} movido(s)).", axis.label()),
                    items.clone(),
                ))
            });
        }
        let alignment = match text(args, "alinhamento").as_deref() {
            Some("esquerda") => Alignment::Left,
            Some("direita") => Alignment::Right,
            Some("centro_horizontal") => Alignment::CenterX,
            Some("topo") => Alignment::Top,
            Some("base") => Alignment::Bottom,
            Some("centro_vertical") => Alignment::CenterY,
            _ => {
                return Err(
                    "Informe 'alinhamento': esquerda, direita, centro_horizontal, topo, base ou centro_vertical."
                        .to_owned(),
                );
            }
        };
        self.edit_elements("Já estavam alinhados.", |project| {
            let moved = project.align_elements(&items, alignment)?;
            Ok((
                format!("{} ({moved} movido(s)).", alignment.label()),
                items.clone(),
            ))
        })
    }

    /// `agrupar` and `desagrupar`.
    pub(super) fn tool_group(&mut self, args: &Value, ungroup: bool) -> Result<ToolOutput, String> {
        let items = self.elements_of(args)?;
        if ungroup {
            return self.edit_elements("Nenhum desses elementos está em grupo.", |project| {
                let count = project.ungroup_elements(&items);
                Ok((format!("{count} grupo(s) desfeito(s)."), items.clone()))
            });
        }
        let name = text(args, "nome").unwrap_or_default();
        self.edit_elements("Nada foi agrupado.", |project| {
            let id = project.group_elements(&items, &name)?;
            let group = project.groups.iter().find(|group| group.id == id);
            Ok((
                format!(
                    "Grupo \"{}\" criado (id {id}).",
                    group.map(|group| group.name.as_str()).unwrap_or_default()
                ),
                group.map(|group| group.members.clone()).unwrap_or_default(),
            ))
        })
    }

    /// `repetir`: copies in a grid or around a centre.
    pub(super) fn tool_repeat(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let items = self.elements_of(args)?;
        let count = |key: &str, default: f64| -> Result<usize, String> {
            let value = number(args, key)?.unwrap_or(default);
            (value >= 1.0 && value.fract() == 0.0)
                .then_some(value as usize)
                .ok_or(format!(
                    "'{key}' deve ser um número inteiro de 1 em diante."
                ))
        };
        let polar = text(args, "modo").as_deref() == Some("polar");
        let center = Self::point_arg(args, "centro_")?;
        self.edit_elements("Nenhuma cópia foi feita.", |project| {
            let copies = if polar {
                let center = match center {
                    Some(center) => center,
                    None => {
                        let (low, high) = project
                            .elements_bounds(&items)
                            .ok_or("Elementos sem posição.")?;
                        low.midpoint(high)
                    }
                };
                project.repeat_around(
                    &items,
                    center,
                    count("quantidade", 4.0)?,
                    number(args, "angulo_total")?.unwrap_or(360.0),
                )?
            } else {
                let dx = required_number(args, "espacamento_x")?;
                let dy = number(args, "espacamento_y")?.unwrap_or(0.0);
                project.repeat_grid(
                    &items,
                    count("colunas", 1.0)?,
                    count("linhas", 1.0)?,
                    Point::new(dx, -dy),
                )?
            };
            Ok((format!("{} cópia(s) criada(s).", copies.len()), copies))
        })
    }

    /// `editar_geometria`: trim, extend, offset or divide a wall or drawing.
    pub(super) fn tool_geometry(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        let target = self.find_element(id)?;
        let operation = text(args, "operacao")
            .ok_or("Informe 'operacao': aparar, estender, deslocar ou dividir.")?;
        let point = Self::point_arg(args, "")?;
        let need_point = || {
            point.ok_or(format!(
                "'{operation}' precisa de x e y: o ponto sobre o trecho ou perto da ponta."
            ))
        };
        let parts = number(args, "partes")?;
        let distance = number(args, "distancia")?;
        let point_value = need_point();
        self.edit_elements("Nada mudou.", |project| match operation.as_str() {
            "aparar" => {
                project.trim_element(target, point_value?)?;
                Ok(("Trecho aparado.".to_owned(), vec![target]))
            }
            "estender" => {
                project.extend_element(target, point_value?)?;
                Ok(("Ponta estendida.".to_owned(), vec![target]))
            }
            "deslocar" => {
                let distance = distance.ok_or("Informe 'distancia' em metros.")?;
                let copy = project.offset_element(target, distance, point_value?)?;
                Ok((
                    format!("Cópia paralela criada (id {}).", copy.id()),
                    vec![copy],
                ))
            }
            "dividir" => {
                let pieces = match parts {
                    Some(parts) if parts >= 2.0 => {
                        project.divide_element(target, parts as usize)?
                    }
                    _ => project.split_element(target, point_value?)?,
                };
                Ok((format!("Dividido em {} partes.", pieces.len()), pieces))
            }
            other => Err(format!(
                "Operação '{other}' desconhecida: aparar, estender, deslocar ou dividir."
            )),
        })
    }

    /// `unir`: walls in line, or drawings that meet end to end.
    pub(super) fn tool_join(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let items = self.elements_of(args)?;
        self.edit_elements("Nada foi unido.", |project| {
            let joined = project.join_elements(&items)?;
            Ok((
                format!("Unidos em um só (id {}).", joined.id()),
                vec![joined],
            ))
        })
    }

    /// `restringir`: a constraint between walls or drawn lines.
    pub(super) fn tool_constrain(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let kind_name = text(args, "tipo").ok_or(
            "Informe 'tipo': paralelas, perpendiculares, coincidentes, distancia, afastamento ou comprimento.",
        )?;
        let a_id = element_id(args, "a")?;
        let a = entity_of(self.find_element(a_id)?, a_id)?;
        let other = || -> Result<Entity, String> {
            let b_id = element_id(args, "b")?;
            entity_of(self.find_element(b_id)?, b_id)
        };
        let vertex = |entity: Entity, key: &str| -> Result<Vertex, String> {
            let index = number(args, key)?.unwrap_or(0.0);
            if index < 0.0 || index.fract() != 0.0 {
                return Err(format!(
                    "'{key}' deve ser 0 (início), 1 (fim) ou o índice do ponto."
                ));
            }
            Ok(Vertex {
                entity,
                index: index as usize,
            })
        };
        let measure = |a: Point, b: Point| a.distance(b);
        let kind = match kind_name.as_str() {
            "paralelas" => ConstraintKind::Parallel { a, b: other()? },
            "perpendiculares" => ConstraintKind::Perpendicular { a, b: other()? },
            "coincidentes" => ConstraintKind::Coincident {
                a: vertex(a, "ponto_a")?,
                b: vertex(other()?, "ponto_b")?,
            },
            "distancia" => {
                let (pa, pb) = (vertex(a, "ponto_a")?, vertex(other()?, "ponto_b")?);
                let current = match (self.project.vertex_point(pa), self.project.vertex_point(pb)) {
                    (Some(p), Some(q)) => measure(p, q),
                    _ => return Err("Ponto não encontrado nesses elementos.".to_owned()),
                };
                ConstraintKind::PointDistance {
                    a: pa,
                    b: pb,
                    distance: number(args, "distancia")?.unwrap_or(current),
                }
            }
            "comprimento" => {
                let (start, end) = self
                    .project
                    .entity_line(a)
                    .ok_or("O comprimento fixo vale para paredes e linhas.")?;
                ConstraintKind::PointDistance {
                    a: Vertex {
                        entity: a,
                        index: 0,
                    },
                    b: Vertex {
                        entity: a,
                        index: 1,
                    },
                    distance: number(args, "distancia")?.unwrap_or(measure(start, end)),
                }
            }
            "afastamento" => {
                let b = other()?;
                let current = match (self.project.entity_line(a), self.project.entity_line(b)) {
                    (Some((a0, a1)), Some((b0, b1))) => {
                        let along = a1.minus(a0).normalized().unwrap_or(Point::new(1.0, 0.0));
                        b0.midpoint(b1)
                            .minus(a0)
                            .dot(Point::new(-along.y, along.x))
                            .abs()
                    }
                    _ => return Err("O afastamento vale entre paredes ou linhas.".to_owned()),
                };
                ConstraintKind::LineDistance {
                    a,
                    b,
                    distance: number(args, "distancia")?.unwrap_or(current),
                }
            }
            other => return Err(format!("Tipo '{other}' desconhecido.")),
        };
        self.apply_change(None, |project| {
            let id = project.add_constraint(kind)?;
            Ok(Change::new(format!(
                "Restrição criada (id {id}): {}. Ela é mantida nas próximas edições, suas e do usuário.",
                kind.label().to_lowercase()
            ))
            .data(json!({ "id": id })))
        })
    }

    /// `remover_restricao`.
    pub(super) fn tool_remove_constraint(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        self.apply_change(Some("Não existe restrição com esse id."), |project| {
            project.remove_constraint(id);
            Ok(Change::new(format!("Restrição {id} removida.")))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use crate::mcp::ToolOutput;
    use eframe::egui;
    use serde_json::{Value, json};

    fn run(app: &mut FloorPlanApp, name: &str, args: Value) -> ToolOutput {
        app.run_mcp_tool(name, &args)
    }

    fn ok(app: &mut FloorPlanApp, name: &str, args: Value) -> ToolOutput {
        let output = run(app, name, args);
        assert!(!output.is_error, "{name}: {}", output.text);
        output
    }

    #[test]
    fn the_assistant_edits_with_the_new_commands() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let room = ok(
            &mut app,
            "criar_comodo",
            json!({ "nome": "Quarto", "x": 0, "y": 0, "largura": 3, "comprimento": 3 }),
        )
        .structured
        .unwrap()["id"]
            .as_u64()
            .unwrap();

        // Copied 4 m to the right, then turned in place.
        ok(
            &mut app,
            "transformar",
            json!({ "ids": [room], "operacao": "mover", "dx": 4, "copiar": true }),
        );
        assert_eq!(app.project.rooms.len(), 2);
        ok(
            &mut app,
            "transformar",
            json!({ "ids": [room], "operacao": "girar", "angulo": 90 }),
        );
        // The person sees what the assistant points at.
        let selected = ok(&mut app, "selecionar", json!({ "tipo": "comodos" }));
        assert_eq!(
            selected.structured.unwrap()["selecao"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(app.selection_count(), 2);

        // Grouped, repeated around, aligned.
        let rooms: Vec<u64> = app.project.rooms.iter().map(|room| room.id).collect();
        ok(
            &mut app,
            "agrupar",
            json!({ "ids": rooms, "nome": "Quartos" }),
        );
        assert_eq!(app.project.groups[0].name, "Quartos");
        ok(&mut app, "desagrupar", json!({ "ids": [rooms[0]] }));
        assert!(app.project.groups.is_empty());

        // Lines: trim, constrain, join.
        let a = ok(
            &mut app,
            "desenhar",
            json!({ "tipo": "linha", "pontos": [[20, 0], [26, 0]] }),
        )
        .structured
        .unwrap()["id"]
            .as_u64()
            .unwrap();
        let b = ok(
            &mut app,
            "desenhar",
            json!({ "tipo": "linha", "pontos": [[23, -2], [23, 2]] }),
        )
        .structured
        .unwrap()["id"]
            .as_u64()
            .unwrap();
        ok(
            &mut app,
            "editar_geometria",
            json!({ "id": a, "operacao": "aparar", "x": 25, "y": 0 }),
        );
        assert!((app.project.drawing(a).unwrap().shape.length() - 3.0).abs() < 1e-6);
        ok(
            &mut app,
            "restringir",
            json!({ "tipo": "perpendiculares", "a": a, "b": b }),
        );
        assert_eq!(app.project.constraints.len(), 1);
        let id = app.project.constraints[0].id;
        ok(&mut app, "remover_restricao", json!({ "id": id }));
        assert!(app.project.constraints.is_empty());
        assert!(
            run(
                &mut app,
                "restringir",
                json!({ "tipo": "paralelas", "a": room, "b": a })
            )
            .is_error
        );
        assert!(
            run(
                &mut app,
                "transformar",
                json!({ "ids": [], "operacao": "mover" })
            )
            .is_error
        );
    }
}
