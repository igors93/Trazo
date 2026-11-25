//! MCP adapters for catalogues, reusable blocks, references and document resources.
use super::{
    Change, element_id, flag, number, optional_count, quarter_turns, required_number, resolve_path,
    text, to_model, xy,
};
use crate::{
    app::FloorPlanApp,
    domain::{
        blocks::BlockAttribute,
        fixtures::FixtureKind,
        library::{LibraryCategory, LibraryItem},
        templates::ProjectTemplate,
    },
    mcp::ToolOutput,
};
use serde_json::{Value, json};

fn origin(args: &Value) -> Result<String, String> {
    let origin = text(args, "origem").unwrap_or_else(|| "projeto".into());
    if !["catalogo", "projeto", "pessoal"].contains(&origin.as_str()) {
        return Err("origem: catalogo, projeto ou pessoal.".into());
    }
    Ok(origin)
}
fn library_json(item: &LibraryItem, origin: &str) -> Value {
    json!({"id":item.id,"origem":origin,"nome":item.name,"categoria":item.category.label(),"largura":item.width_m,"profundidade":item.depth_m,"tipo":item.base_kind.map(|k|k.key()),"simbolo":item.symbol_tag,"lugares":item.base_kind.and_then(|k|k.seat_count(item.width_m))})
}
fn category(raw: &str) -> Result<LibraryCategory, String> {
    LibraryCategory::ALL
        .into_iter()
        .find(|c| c.label().eq_ignore_ascii_case(raw))
        .or(match raw {
            "sala" => Some(LibraryCategory::Living),
            "quarto" => Some(LibraryCategory::Bedroom),
            "cozinha" => Some(LibraryCategory::Kitchen),
            "banheiro" => Some(LibraryCategory::Bathroom),
            "escritorio" => Some(LibraryCategory::Office),
            "lavanderia" => Some(LibraryCategory::Laundry),
            "externo" => Some(LibraryCategory::Outdoor),
            "simbolos" => Some(LibraryCategory::Symbols),
            "personalizados" => Some(LibraryCategory::Custom),
            _ => None,
        })
        .ok_or("Categoria de biblioteca inválida.".into())
}
fn positive(args: &Value, key: &str, default: f64) -> Result<f64, String> {
    let n = number(args, key)?.unwrap_or(default);
    if n <= 0.0 {
        return Err(format!("{key} deve ser positivo."));
    }
    Ok(n)
}

impl FloorPlanApp {
    pub(super) fn tool_library(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".into());
        if action == "listar" {
            let query = text(args, "busca").unwrap_or_default();
            let source = text(args, "origem");
            if source.is_some() {
                origin(args)?;
            }
            let builtin = LibraryItem::builtin_catalog();
            let source = source.as_deref();
            let query = &query;
            let items: Vec<_> = [
                ("catalogo", builtin.as_slice()),
                ("projeto", self.project.library.as_slice()),
                ("pessoal", self.personal_library.items.as_slice()),
            ]
            .into_iter()
            .flat_map(|(o, items)| {
                items
                    .iter()
                    .filter(move |i| source.is_none_or(|s| s == o) && i.matches_query(query))
                    .map(move |i| library_json(i, o))
            })
            .collect();
            return Ok(
                ToolOutput::ok(format!("{} itens da biblioteca.", items.len()))
                    .with_structured(json!({"itens":items})),
            );
        }
        let source = origin(args)?;
        let id = optional_count(args, "id")?;
        let item = match source.as_str() {
            "catalogo" => LibraryItem::builtin_catalog()
                .into_iter()
                .find(|i| Some(i.id) == id),
            "pessoal" => self
                .personal_library
                .items
                .iter()
                .find(|i| Some(i.id) == id)
                .cloned(),
            _ => self
                .project
                .library
                .iter()
                .find(|i| Some(i.id) == id)
                .cloned(),
        };
        if action == "inserir" {
            let item = item.ok_or(
                "Item não encontrado. Informe origem e id retornados por biblioteca(listar).",
            )?;
            let center = to_model(required_number(args, "x")?, required_number(args, "y")?);
            let turns = quarter_turns(args)?.unwrap_or(0);
            return self.apply_change(None,|p|{let id=p.place_library_item(&item,center,turns)?;Ok(Change::new(format!("{} inserido (id {id}).",item.name)).data(json!({"id":id,"largura":item.width_m,"profundidade":item.depth_m,"centro":xy(center)})))});
        }
        if action == "compartilhar" {
            let item = item.ok_or("Item não encontrado.")?;
            let mut personal = self.personal_library.clone();
            let id = personal.add_item(item);
            crate::io::personal_library::save(&personal)?;
            self.personal_library = personal;
            return Ok(ToolOutput::ok("Item salvo na biblioteca pessoal.")
                .with_structured(json!({"id":id,"origem":"pessoal"})));
        }
        if source == "catalogo" {
            return Err("O catálogo padrão é somente leitura; crie um item personalizado.".into());
        }
        let mut candidate = item.clone();
        if action == "criar" || action == "editar" {
            if action == "editar" && item.is_none() {
                return Err("Item não encontrado.".into());
            }
            let name = text(args, "nome")
                .or_else(|| item.as_ref().map(|i| i.name.clone()))
                .ok_or("Informe nome.")?;
            let base = if let Some(raw) = text(args, "tipo") {
                Some(
                    FixtureKind::from_key(&raw)
                        .ok_or("Tipo de objeto inválido (use os tipos de inserir_objeto).")?,
                )
            } else {
                item.as_ref().and_then(|i| i.base_kind)
            };
            let cat = text(args, "categoria")
                .map(|r| category(&r))
                .transpose()?
                .unwrap_or_else(|| {
                    item.as_ref()
                        .map(|i| i.category)
                        .unwrap_or(LibraryCategory::Custom)
                });
            let width = positive(
                args,
                "largura",
                item.as_ref().map(|i| i.width_m).unwrap_or(1.0),
            )?;
            let depth = positive(
                args,
                "profundidade",
                item.as_ref().map(|i| i.depth_m).unwrap_or(0.6),
            )?;
            let mut check = crate::domain::model::Project::default();
            let cid = check.add_custom_library_item(
                &name,
                cat,
                width,
                depth,
                base,
                text(args, "simbolo").or_else(|| item.as_ref().and_then(|i| i.symbol_tag.clone())),
            )?;
            candidate = check.library.into_iter().find(|i| i.id == cid);
        }
        if source == "pessoal" {
            let mut personal = self.personal_library.clone();
            let id = match action.as_str() {
                "criar" => personal.add_item(candidate.ok_or("Informe o item.")?),
                "editar" => {
                    let id = id.ok_or("Informe id.")?;
                    let mut candidate = candidate.ok_or("Item não encontrado.")?;
                    candidate.id = id;
                    *personal
                        .items
                        .iter_mut()
                        .find(|i| i.id == id)
                        .ok_or("Item não encontrado.")? = candidate;
                    id
                }
                "excluir" => {
                    let id = id.ok_or("Informe id.")?;
                    if !personal.remove_item(id) {
                        return Err("Item não encontrado.".into());
                    }
                    id
                }
                _ => {
                    return Err(
                        "Ação: listar, criar, editar, excluir, inserir ou compartilhar.".into(),
                    );
                }
            };
            crate::io::personal_library::save(&personal)?;
            self.personal_library = personal;
            return Ok(ToolOutput::ok("Biblioteca pessoal atualizada.")
                .with_structured(json!({"id":id,"origem":"pessoal"})));
        }
        self.apply_change(None, |p| {
            let id = match action.as_str() {
                "criar" => {
                    let i = candidate.ok_or("Informe o item.")?;
                    p.add_custom_library_item(
                        &i.name,
                        i.category,
                        i.width_m,
                        i.depth_m,
                        i.base_kind,
                        i.symbol_tag,
                    )?
                }
                "editar" => {
                    let id = id.ok_or("Informe id.")?;
                    let mut i = candidate.ok_or("Item não encontrado.")?;
                    i.id = id;
                    *p.library
                        .iter_mut()
                        .find(|i| i.id == id)
                        .ok_or("Item não encontrado.")? = i;
                    id
                }
                "excluir" => {
                    let id = id.ok_or("Informe id.")?;
                    if !p.remove_library_item(id) {
                        return Err("Item não encontrado.".into());
                    }
                    id
                }
                _ => return Err("Ação de biblioteca inválida.".into()),
            };
            Ok(Change::new("Biblioteca do projeto atualizada.")
                .data(json!({"id":id,"origem":"projeto"})))
        })
    }

    pub(super) fn tool_blocks(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".into());
        if action == "listar" {
            let definitions:Vec<_>=[("projeto",self.project.blocks.as_slice()),("pessoal",self.personal_library.blocks.as_slice())].into_iter().flat_map(|(o,items)|items.iter().map(move|b|json!({"id":b.id,"origem":o,"nome":b.name,"categoria":b.category,"descricao":b.description,"tamanho":b.size_m(),"elementos":b.element_count(),"atributos":b.attributes.iter().map(|a|json!({"chave":a.key,"rotulo":a.label,"padrao":a.default_value})).collect::<Vec<_>>()}))).collect();
            let instances:Vec<_>=self.project.block_instances.iter().map(|i|json!({"id":i.id,"bloco_id":i.block_id,"nome":i.block_name,"origem":xy(i.origin),"ids":i.members.iter().map(|e|e.id()).collect::<Vec<_>>(),"atributos":i.attributes})).collect();
            return Ok(ToolOutput::ok("Blocos e instâncias.")
                .with_structured(json!({"blocos":definitions,"instancias":instances})));
        }
        if action == "criar" {
            let elements = self.elements_of(args)?;
            let name = text(args, "nome").ok_or("Informe nome.")?;
            let attrs = match args.get("atributos") {
                None => vec![],
                Some(Value::Array(items)) if items.len() <= 100 => items
                    .iter()
                    .map(|a| {
                        Ok(BlockAttribute::new(
                            text(a, "chave").ok_or("Informe chave de cada atributo.")?,
                            text(a, "rotulo").unwrap_or_default(),
                            text(a, "padrao").unwrap_or_default(),
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                _ => return Err("atributos deve ser uma lista com até 100 itens.".into()),
            };
            return self.apply_change(None, |p| {
                let b = p.define_block_from_elements(
                    &name,
                    &text(args, "categoria").unwrap_or_default(),
                    &text(args, "descricao").unwrap_or_default(),
                    &elements,
                    attrs,
                )?;
                Ok(Change::new("Bloco criado.").data(json!({"id":b.id,"origem":"projeto"})))
            });
        }
        let id = element_id(args, "id")?;
        let source = origin(args)?;
        if action == "atributo" {
            let key = text(args, "chave").ok_or("Informe chave.")?;
            let value = args
                .get("valor")
                .and_then(Value::as_str)
                .ok_or("Informe valor (texto).")?;
            return self.apply_change(None, |p| {
                p.set_block_instance_attribute(id, &key, value)?;
                Ok(Change::new("Atributo da instância atualizado."))
            });
        }
        let block = if source == "pessoal" {
            self.personal_library.blocks.iter().find(|b| b.id == id)
        } else if source == "projeto" {
            self.project.blocks.iter().find(|b| b.id == id)
        } else {
            None
        }
        .cloned()
        .ok_or("Bloco não encontrado nesta origem.")?;
        match action.as_str() {
            "inserir" => {
                let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let overrides = match args.get("valores") {
                    None => vec![],
                    Some(Value::Object(map)) if map.len() <= 100 => map
                        .iter()
                        .map(|(k, v)| {
                            Ok((
                                k.clone(),
                                v.as_str()
                                    .ok_or("Valores de atributos devem ser textos.")?
                                    .to_owned(),
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    _ => return Err("valores deve ser um objeto com até 100 atributos.".into()),
                };
                self.apply_change(None, |p| {
                    let id = p.insert_block(&block, point, &overrides)?;
                    Ok(Change::new("Bloco inserido.").data(json!({"instancia_id":id})))
                })
            }
            "compartilhar" => {
                let mut personal = self.personal_library.clone();
                let id = personal.add_block(block);
                crate::io::personal_library::save(&personal)?;
                self.personal_library = personal;
                Ok(ToolOutput::ok("Bloco salvo na biblioteca pessoal.")
                    .with_structured(json!({"id":id,"origem":"pessoal"})))
            }
            "excluir" if source == "pessoal" => {
                let mut personal = self.personal_library.clone();
                personal.remove_block(id);
                crate::io::personal_library::save(&personal)?;
                self.personal_library = personal;
                Ok(ToolOutput::ok("Bloco pessoal excluído."))
            }
            "excluir" => self.apply_change(None, |p| {
                p.remove_block_definition(id);
                Ok(Change::new(
                    "Definição excluída; instâncias existentes preservadas.",
                ))
            }),
            _ => Err("Ação: listar, criar, inserir, atributo, compartilhar ou excluir.".into()),
        }
    }

    pub(super) fn tool_templates(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".into());
        if action == "listar" {
            return Ok(ToolOutput::ok("Modelos e exemplos disponíveis.").with_structured(json!({"modelos":ProjectTemplate::ALL.into_iter().enumerate().map(|(i,t)|json!({"id":i+1,"titulo":t.title(),"descricao":t.summary()})).collect::<Vec<_>>(),"exemplos":crate::domain::samples::SampleProject::ALL.into_iter().enumerate().map(|(i,t)|json!({"id":i+1,"titulo":t.title(),"arquivo":t.file_name()})).collect::<Vec<_>>(),"pessoais":self.user_templates.shown().iter().map(|p|p.display().to_string()).collect::<Vec<_>>()})));
        }
        if action == "salvar" {
            let path = resolve_path(&text(args, "caminho").ok_or("Informe caminho.")?, "trazot");
            if !crate::io::templates::is_template_path(&path) {
                return Err("Use extensão .trazot.".into());
            }
            if path.exists() && !flag(args, "sobrescrever") {
                return Err("Arquivo já existe; use sobrescrever: true.".into());
            }
            super::ensure_parent(&path)?;
            crate::io::project_file::save(&self.project, &path)?;
            self.user_templates.pick(&path);
            self.user_templates.save()?;
            return Ok(ToolOutput::ok("Modelo salvo.")
                .with_structured(json!({"caminho":path.display().to_string()})));
        }
        if action != "aplicar" {
            return Err("Ação: listar, aplicar ou salvar.".into());
        }
        self.check_discard(args)?;
        let project = if let Some(raw) = text(args, "caminho") {
            crate::io::project_file::load(&resolve_path(&raw, "trazot"))?.0
        } else {
            let index = element_id(args, "id")?
                .checked_sub(1)
                .ok_or("id inválido.")? as usize;
            if flag(args, "exemplo") {
                crate::domain::samples::SampleProject::ALL
                    .get(index)
                    .ok_or("Exemplo não encontrado.")?
                    .load_embedded()
            } else {
                ProjectTemplate::ALL
                    .get(index)
                    .ok_or("Modelo não encontrado.")?
                    .build()
            }
        };
        let before = std::mem::replace(&mut self.project, project);
        self.reset_editing_state();
        self.file_path = None;
        self.active_tab_floor_id = self.project.floors.active_floor_id;
        self.clear_selection();
        self.record_change(before);
        self.fit_view_requested();
        self.show_start_page = false;
        Ok(ToolOutput::ok(
            "Modelo aplicado em um projeto sem arquivo associado.",
        ))
    }
}

mod documentation;
mod element_edits;
mod references;
#[cfg(test)]
mod tests;
