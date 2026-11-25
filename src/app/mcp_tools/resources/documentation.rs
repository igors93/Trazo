//! Graphic styles, schedules, wall assemblies, hatches and site operations.
use super::*;
use crate::domain::{
    doc_style::{DimensionTick, DimensionUnit, DocStyle, DocStylePreset},
    hatch::{HatchPattern, HatchStyle},
    wall_layers::{WallAssembly, WallLayer, WallLayerRole},
};

impl FloorPlanApp {
    pub(in crate::app::mcp_tools) fn tool_documentation(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "obter".into());
        if action == "obter" {
            let p = &self.project;
            let d = &p.doc_style;
            let c = &p.schedules;
            let tables = crate::domain::schedules::active_schedules(p, c)
                .into_iter()
                .map(|t| json!({"titulo":t.title,"cabecalho":t.headers,"linhas":t.rows}))
                .collect::<Vec<_>>();
            return Ok(ToolOutput::ok("Estilo gráfico e tabelas do projeto.").with_structured(json!({"estilo":{"preset":match d.preset{DocStylePreset::AbntNbr6492=>"abnt",DocStylePreset::TrazoVisual=>"visual",DocStylePreset::ObraMonocromatico=>"monocromatico"},"monocromatico":d.monochrome,"unidade":if d.unit==DimensionUnit::Centimetres{"cm"}else{"m"},"terminacao":match d.tick{DimensionTick::Oblique45=>"obliqua",DimensionTick::Arrow=>"seta",DimensionTick::Dot=>"ponto"},"pena_parede_mm":d.wall_pen_mm,"pena_abertura_mm":d.opening_pen_mm,"pena_cota_mm":d.dimension_pen_mm,"pena_hachura_mm":d.hatch_pen_mm,"fonte":d.font_family},"configuracao_tabelas":{"areas":c.show_area_schedule,"esquadrias":c.show_openings_schedule,"comodos":c.show_rooms_schedule,"materiais":c.show_materials_schedule,"quantitativos":c.show_quantities_schedule,"perimetro":c.show_perimeter,"peitoril":c.show_sill,"ocupacao":c.show_occupancy,"perda_pct":c.waste_percent,"incluir_prancha":c.include_on_sheet},"tabelas":tables})));
        }
        if action == "exportar_protocolo" {
            let path = resolve_path(&text(args, "caminho").ok_or("Informe caminho.")?, "csv");
            if path.exists() && !flag(args, "sobrescrever") {
                return Err("Arquivo já existe; use sobrescrever: true.".into());
            }
            super::super::ensure_parent(&path)?;
            std::fs::write(&path, self.project.sheet_set.transmittal_csv(&self.project))
                .map_err(|e| e.to_string())?;
            return Ok(ToolOutput::ok("Protocolo de emissão exportado.")
                .with_structured(json!({"caminho":path.display().to_string()})));
        }
        self.apply_change(None,|p|{
            match action.as_str(){
                "configurar_estilo"=>{
                    if let Some(preset)=text(args,"estilo"){p.doc_style=DocStyle::from_preset(match preset.as_str(){"abnt"=>DocStylePreset::AbntNbr6492,"visual"=>DocStylePreset::TrazoVisual,"monocromatico"=>DocStylePreset::ObraMonocromatico,_=>return Err("estilo: abnt, visual ou monocromatico.".into())});}
                    let d=&mut p.doc_style;
                    if args.get("monocromatico").is_some(){d.monochrome=flag(args,"monocromatico");}
                    if let Some(unit)=text(args,"unidade"){d.unit=match unit.as_str(){"cm"=>DimensionUnit::Centimetres,"m"=>DimensionUnit::Metres,_=>return Err("unidade: cm ou m.".into())};}
                    if let Some(t)=text(args,"terminacao"){d.tick=match t.as_str(){"obliqua"=>DimensionTick::Oblique45,"seta"=>DimensionTick::Arrow,"ponto"=>DimensionTick::Dot,_=>return Err("terminacao: obliqua, seta ou ponto.".into())};}
                    for (key,field) in [("pena_parede_mm",&mut d.wall_pen_mm),("pena_abertura_mm",&mut d.opening_pen_mm),("pena_cota_mm",&mut d.dimension_pen_mm),("pena_hachura_mm",&mut d.hatch_pen_mm)] {if let Some(n)=number(args,key)?{if !(0.01..=2.0).contains(&n){return Err("Penas devem estar entre 0,01 e 2 mm.".into());}*field=n;}}
                    if let Some(f)=text(args,"fonte"){if f.len()>200{return Err("Nome de fonte muito longo.".into());}d.font_family=f;}
                },
                "configurar_tabelas"=>{
                    let c=&mut p.schedules;
                    for (key,field) in [("areas",&mut c.show_area_schedule),("esquadrias",&mut c.show_openings_schedule),("comodos",&mut c.show_rooms_schedule),("materiais",&mut c.show_materials_schedule),("quantitativos",&mut c.show_quantities_schedule),("perimetro",&mut c.show_perimeter),("peitoril",&mut c.show_sill),("ocupacao",&mut c.show_occupancy),("incluir_prancha",&mut c.include_on_sheet)]{if args.get(key).is_some(){*field=flag(args,key);}}
                    if let Some(n)=number(args,"perda_pct")?{if !(0.0..=100.0).contains(&n){return Err("perda_pct deve estar entre 0 e 100.".into());}c.waste_percent=n;}
                },
                "adicionar_revisao"=>{let id=element_id(args,"id")?;let code=p.sheet_set.add_revision(id,&text(args,"data").unwrap_or_default(),&text(args,"autor").unwrap_or_default(),&text(args,"descricao").ok_or("Informe descricao.")?)?;return Ok(Change::new("Revisão adicionada.").data(json!({"codigo":code})));},
                "remover_prancha"=>{if !p.sheet_set.remove_sheet(element_id(args,"id")?){return Err("Prancha não encontrada.".into());}},
                "reordenar_prancha"=>{let n=required_number(args,"deslocamento")?;if n.fract()!=0.0||n.abs()>1000.0{return Err("deslocamento deve ser inteiro entre -1000 e 1000.".into());}
                    if !p.sheet_set.move_sheet(element_id(args,"id")?,n as isize){return Err("Não foi possível reordenar a prancha.".into());}},
                "atualizar_prancha"=>{let id=element_id(args,"id")?;let layout=p.sheet.clone();let sheet=p.sheet_set.sheets.iter_mut().find(|s|s.id==id).ok_or("Prancha não encontrada.")?;sheet.layout=layout;sheet.release_review=None;if sheet.status==crate::domain::sheet_set::SheetStatus::LiberadoObra{sheet.status=crate::domain::sheet_set::SheetStatus::ParaAprovacao;}
                    if let Some(title)=text(args,"titulo"){sheet.title=title;}},
                _=>return Err("Ação: obter, configurar_estilo, configurar_tabelas, adicionar_revisao, remover_prancha, reordenar_prancha, atualizar_prancha ou exportar_protocolo.".into())
            }
            Ok(Change::new("Documentação atualizada."))
        })
    }

    pub(in crate::app::mcp_tools) fn tool_finishes(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".into());
        if action == "listar" {
            let assemblies = WallAssembly::presets();
            return Ok(ToolOutput::ok("Hachuras e composições multicamadas.").with_structured(json!({"hachuras":HatchPattern::ALL.into_iter().enumerate().map(|(i,h)|json!({"id":i,"nome":h.label()})).collect::<Vec<_>>(),"composicoes":assemblies.iter().enumerate().map(|(i,a)|json!({"id":i+1,"nome":a.name,"espessura_m":a.total_thickness(),"camadas":a.layers.iter().map(|l|json!({"nome":l.name,"funcao":l.role.label(),"espessura_m":l.thickness_m,"cor":l.hex_color()})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"paredes":self.project.walls.iter().map(|w|json!({"id":w.id,"composicao":w.assembly})).collect::<Vec<_>>(),"comodos":self.project.rooms.iter().map(|r|json!({"id":r.id,"hachura":r.hatch})).collect::<Vec<_>>()})));
        }
        self.apply_change(None, |p| {
            let id = element_id(args, "id")?;
            match action.as_str() {
                "hachura" => {
                    let index = element_id(args, "padrao_id")? as usize;
                    let pattern = *HatchPattern::ALL
                        .get(index)
                        .ok_or("Padrão de hachura inválido.")?;
                    let mut h = HatchStyle::for_pattern(pattern);
                    if let Some(v) = number(args, "espacamento_m")? {
                        if !(0.01..=10.0).contains(&v) {
                            return Err("Espaçamento deve estar entre 0,01 e 10 m.".into());
                        }
                        h.spacing_m = v;
                    }
                    if let Some(v) = number(args, "angulo_graus")? {
                        h.angle_deg = -v;
                    }
                    if let Some(v) = text(args, "cor") {
                        h.color = super::super::markups::color(&v)?;
                    }
                    p.set_room_hatch(id, Some(h))?;
                }
                "composicao" => {
                    let assembly = if flag(args, "limpar") {
                        None
                    } else if let Some(index) = optional_count(args, "preset_id")? {
                        Some(
                            WallAssembly::presets()
                                .get(index.checked_sub(1).ok_or("preset_id inválido.")? as usize)
                                .ok_or("Composição não encontrada.")?
                                .clone(),
                        )
                    } else {
                        let list = args
                            .get("camadas")
                            .and_then(Value::as_array)
                            .ok_or("Informe preset_id, camadas ou limpar: true.")?;
                        if list.is_empty() || list.len() > 30 {
                            return Err("Use de 1 a 30 camadas.".into());
                        }
                        let layers = list
                            .iter()
                            .map(|v| {
                                let role = match text(v, "funcao").as_deref() {
                                    Some("estrutura") => WallLayerRole::Structure,
                                    Some("reboco") => WallLayerRole::Plaster,
                                    Some("isolamento") => WallLayerRole::Insulation,
                                    Some("acabamento") => WallLayerRole::Finish,
                                    Some("ar") => WallLayerRole::AirGap,
                                    _ => return Err(
                                        "funcao: estrutura, reboco, isolamento, acabamento ou ar."
                                            .into(),
                                    ),
                                };
                                let thickness = required_number(v, "espessura_m")?;
                                if !(0.002..=1.0).contains(&thickness) {
                                    return Err(
                                        "Espessura de camada deve estar entre 0,002 e 1 m.".into(),
                                    );
                                }
                                Ok(WallLayer {
                                    name: text(v, "nome").ok_or("Informe nome da camada.")?,
                                    role,
                                    thickness_m: thickness,
                                    color: text(v, "cor")
                                        .map(|c| super::super::markups::color(&c))
                                        .transpose()?
                                        .unwrap_or_else(|| role.default_color()),
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        Some(WallAssembly {
                            name: text(args, "nome").unwrap_or_else(|| "Personalizada".into()),
                            layers,
                        })
                    };
                    p.set_wall_assembly(id, assembly)?;
                }
                _ => return Err("Ação: listar, hachura ou composicao.".into()),
            }
            Ok(Change::new("Acabamento gráfico atualizado."))
        })
    }

    pub(in crate::app::mcp_tools) fn tool_site(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "obter".into());
        if action == "obter" {
            return Ok(ToolOutput::ok("Terreno e lotes.").with_structured(json!({"terreno":self.project.terrain.as_ref().map(|t|json!({"id":t.id,"area_m2":t.area(),"pontos":t.outline().into_iter().map(xy).collect::<Vec<_>>()})),"lotes":self.project.lots.iter().map(|l|json!({"id":l.id,"nome":l.name,"area_m2":l.area(),"pontos":l.vertices.iter().copied().map(xy).collect::<Vec<_>>()})).collect::<Vec<_>>()})));
        }
        self.apply_change(None, |p| {
            match action.as_str() {
                "definir_poligono" => {
                    let raw = args
                        .get("pontos")
                        .and_then(Value::as_array)
                        .ok_or("Informe pontos.")?;
                    if !(3..=1000).contains(&raw.len()) {
                        return Err("Use de 3 a 1000 pontos.".into());
                    }
                    let points = super::super::shapes::points_of(&Value::Array(raw.clone()))?;
                    let id = p.set_polygon_terrain(&points)?;
                    return Ok(Change::new("Terreno poligonal definido.").data(json!({"id":id})));
                }
                "subdividir" => {
                    let a = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                    let b = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                    let (left, right) = p.subdivide_terrain(a, b)?;
                    return Ok(
                        Change::new("Terreno subdividido.").data(json!({"lotes":[left,right]}))
                    );
                }
                "renomear_lote" => {
                    let id = element_id(args, "id")?;
                    p.lots
                        .iter_mut()
                        .find(|l| l.id == id)
                        .ok_or("Lote não encontrado.")?
                        .name = text(args, "nome").ok_or("Informe nome.")?;
                }
                "remover_lote" => {
                    let id = element_id(args, "id")?;
                    if !p.lots.iter().any(|l| l.id == id) {
                        return Err("Lote não encontrado.".into());
                    }
                    p.lots.retain(|l| l.id != id);
                }
                _ => {
                    return Err(
                        "Ação: obter, definir_poligono, subdividir, renomear_lote ou remover_lote."
                            .into(),
                    );
                }
            }
            Ok(Change::new("Lote atualizado."))
        })
    }
}
