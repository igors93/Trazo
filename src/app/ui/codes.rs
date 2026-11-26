//! "Normas, Local e Códigos de Obra" dialog (`T-08`, `T-09`, `T-10`):
//! - Tab 0: Layered building codes (`Base -> Amendments -> Project`), edition pinning,
//!   rule inspection/override and Format-2 XML / JSON export & import.
//! - Tab 1: Shared Site & Climate parameters (`SiteClimate`) with editable Brazilian
//!   city presets synced directly to NBR 6123 structural wind `V0`.
//! - Tab 2: Real-time architectural code compliance report (stairs, ramps, doors, rooms).

use eframe::egui::{self, RichText, Ui};
use serde_json::Value;

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::codes::{
    CodeDiscipline, KeraunicLevel, RuleKind, SiteClimate, builtin_presets, builtin_site_presets,
    check_project_codes, is_whole_section, xml,
};

const TAB_NAMES: [&str; 3] = ["Normas em camadas", "Local e clima", "Códigos de obra"];

impl FloorPlanApp {
    pub(super) fn codes_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Codes {
            return;
        }
        let mut open = true;
        let tab_id = egui::Id::unique("trazo_codes_dialog_tab");
        let disc_id = egui::Id::unique("trazo_codes_dialog_disc");
        let sec_id = egui::Id::unique("trazo_codes_dialog_sec");
        let mut active_tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
        let mut active_disc_idx: usize = ctx.data(|d| d.get_temp(disc_id)).unwrap_or(0);
        let mut active_sec: String = ctx
            .data(|d| d.get_temp(sec_id))
            .unwrap_or_else(|| "stairs".to_owned());

        let mut open_dialog: Option<DocDialog> = None;

        egui::Window::new("Normas, Local e Códigos de Obra")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .default_height(600.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (idx, name) in TAB_NAMES.iter().enumerate() {
                        if toggle_pill(ui, name, active_tab == idx, name).clicked() {
                            active_tab = idx;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Abrir Segurança (Semáforo)").clicked() {
                            open_dialog = Some(DocDialog::Safety);
                        }
                        if secondary_button(ui, "Abrir Escadas / Solver").clicked() {
                            open_dialog = Some(DocDialog::Circulation);
                        }
                    });
                });
                ui.separator();

                match active_tab {
                    0 => self.codes_layers_tab(ui, &mut active_disc_idx, &mut active_sec),
                    1 => self.codes_site_climate_tab(ui),
                    _ => self.codes_check_report_tab(ui, &mut open_dialog),
                }
            });

        ctx.data_mut(|d| {
            d.insert_temp(tab_id, active_tab);
            d.insert_temp(disc_id, active_disc_idx);
            d.insert_temp(sec_id, active_sec);
        });
        if let Some(target) = open_dialog {
            self.active_doc_dialog = target;
        } else if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }

    fn codes_layers_tab(
        &mut self,
        ui: &mut Ui,
        active_disc_idx: &mut usize,
        active_sec: &mut String,
    ) {
        ui.label(
            RichText::new(
                "Motor orientado a dados: Base → até 16 Emendas ordenadas → Ajustes do projeto. A edição ativa fica pinada no arquivo do projeto.",
            )
            .size(12.0)
            .color(TEXT_2),
        );
        ui.add_space(6.0);

        // Regional presets + XML/JSON export/import bar
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Preset regional:").font(strong(12.0)));
            for preset in builtin_presets() {
                if secondary_button(ui, &preset.name)
                    .on_hover_text(&preset.note)
                    .clicked()
                {
                    let previous = self.project.clone();
                    match self.project.codes.apply_preset(&preset.id) {
                        Ok(p) => {
                            self.record_change(previous);
                            self.message = format!("Preset normativo aplicado: {}.", p.name);
                        }
                        Err(err) => {
                            self.message = err;
                        }
                    }
                }
            }
            ui.separator();
            if secondary_button(ui, "Exportar XML").clicked() {
                self.export_codes_file(true);
            }
            if secondary_button(ui, "Exportar JSON").clicked() {
                self.export_codes_file(false);
            }
            if secondary_button(ui, "Importar norma…").clicked() {
                self.import_codes_file();
            }
        });

        // Edition drift warning if pinned edition differs from bundled
        let drifts = self.project.codes.edition_drifts();
        if !drifts.is_empty() {
            ui.add_space(4.0);
            for drift in drifts {
                ui.label(
                    RichText::new(format!(
                        "Edição pinada: {} ({}) está fixada na edição '{}', versão atual '{}'.",
                        drift.code,
                        drift.discipline.label(),
                        drift.pinned,
                        drift.available
                    ))
                    .size(11.5)
                    .color(WARNING),
                );
            }
        }

        ui.add_space(6.0);
        // Discipline selector
        ui.horizontal_wrapped(|ui| {
            for (idx, disc) in CodeDiscipline::ALL.iter().enumerate() {
                if toggle_pill(ui, disc.label(), *active_disc_idx == idx, disc.label()).clicked() {
                    *active_disc_idx = idx;
                }
            }
        });
        let disc = CodeDiscipline::ALL
            .get(*active_disc_idx)
            .copied()
            .unwrap_or(CodeDiscipline::Architectural);

        ui.add_space(6.0);
        let base_key = self.project.codes.active_base_key(disc);
        let available: Vec<(String, String, String, String)> = self
            .project
            .codes
            .available_codes(disc)
            .into_iter()
            .map(|c| {
                (
                    c.code.clone(),
                    c.name.clone(),
                    c.edition.clone(),
                    c.amends.clone(),
                )
            })
            .collect();

        // Base selector
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Norma base:").font(strong(12.0)));
            let mut chosen_base: Option<String> = None;
            let base_label = available
                .iter()
                .find(|(k, _, _, _)| k == &base_key)
                .map(|(k, n, ed, _)| {
                    if ed.is_empty() {
                        format!("{n} ({k})")
                    } else {
                        format!("{n} · ed. {ed}")
                    }
                })
                .unwrap_or_else(|| base_key.clone());
            egui::ComboBox::from_id_salt(("codes_base", disc.key()))
                .selected_text(base_label)
                .width(340.0)
                .show_ui(ui, |ui| {
                    for (k, name, ed, amends) in &available {
                        if !amends.is_empty() {
                            continue;
                        }
                        let text = if ed.is_empty() {
                            format!("{name} ({k})")
                        } else {
                            format!("{name} · ed. {ed}")
                        };
                        if ui.selectable_label(*k == base_key, text).clicked() {
                            chosen_base = Some(k.clone());
                        }
                    }
                });
            if let Some(new_base) = chosen_base
                && new_base != base_key
            {
                let previous = self.project.clone();
                match self.project.codes.set_base(disc, &new_base) {
                    Ok(()) => {
                        self.record_change(previous);
                        self.message =
                            format!("Norma base de {} definida para '{new_base}'.", disc.label());
                    }
                    Err(err) => self.message = err,
                }
            }
            if let Some(pinned) = self.project.codes.choice(disc).editions.get(&base_key) {
                ui.label(
                    RichText::new(format!("Edição pinada: {pinned}"))
                        .size(11.5)
                        .color(ACCENT_TEXT),
                );
            }
        });

        // Amendments list + add selector
        ui.add_space(4.0);
        let active_layers = self.project.codes.active_layer_keys(disc);
        let current_amendments = self.project.codes.choice(disc).amendments.clone();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Emendas ativas:").font(strong(12.0)));
            if current_amendments.is_empty() {
                ui.label(RichText::new("Nenhuma emenda ativa.").color(MUTED));
            } else {
                let mut remove_key: Option<String> = None;
                for amend in &current_amendments {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(amend).font(strong(11.5)));
                            if ui.small_button("×").clicked() {
                                remove_key = Some(amend.clone());
                            }
                        });
                    });
                }
                if let Some(key) = remove_key {
                    let previous = self.project.clone();
                    self.project.codes.remove_amendment(disc, &key);
                    self.record_change(previous);
                    self.message = format!("Emenda '{key}' removida.");
                }
            }

            let compatible: Vec<(String, String)> = available
                .iter()
                .filter(|(k, _, _, amends)| {
                    !amends.is_empty()
                        && active_layers.iter().any(|a| a == amends)
                        && !current_amendments.iter().any(|c| c == k)
                })
                .map(|(k, name, _, _)| (k.clone(), name.clone()))
                .collect();
            if !compatible.is_empty() {
                let mut to_add: Option<String> = None;
                egui::ComboBox::from_id_salt(("codes_add_amend", disc.key()))
                    .selected_text("+ Adicionar emenda…")
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (k, name) in &compatible {
                            if ui
                                .selectable_label(false, format!("{name} ({k})"))
                                .clicked()
                            {
                                to_add = Some(k.clone());
                            }
                        }
                    });
                if let Some(key) = to_add {
                    let previous = self.project.clone();
                    match self.project.codes.add_amendment(disc, &key) {
                        Ok(()) => {
                            self.record_change(previous);
                            self.message = format!("Emenda '{key}' adicionada.");
                        }
                        Err(err) => self.message = err,
                    }
                }
            }
        });

        ui.separator();

        // Section selector for inspection & project overrides
        let mut sections: Vec<String> = Vec::new();
        for key in &active_layers {
            if let Some(pack) = self.project.codes.find_code(disc, key) {
                for sec in pack.sections.keys() {
                    if !sections.contains(sec) {
                        sections.push(sec.clone());
                    }
                }
            }
        }
        sections.sort();
        if !sections.is_empty()
            && !sections.iter().any(|s| s == active_sec)
            && let Some(first) = sections.first()
        {
            *active_sec = first.clone();
        }

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Seção:").font(strong(12.0)));
            for sec in &sections {
                if toggle_pill(ui, sec, active_sec == sec, sec).clicked() {
                    *active_sec = sec.clone();
                }
            }
        });
        ui.add_space(4.0);

        if sections.is_empty() {
            ui.label(
                RichText::new(
                    "A norma ativa desta disciplina ainda não possui seções cadastradas.",
                )
                .color(MUTED),
            );
            return;
        }

        let resolved = self.project.codes.resolve_section(disc, active_sec);
        let whole = is_whole_section(disc, active_sec);
        if whole {
            ui.label(
                RichText::new(
                    "Seção WHOLE (substituída por inteiro pela concessionária ativa; não aceita sobrescrita isolada de regra).",
                )
                .size(11.5)
                .color(MUTED),
            );
        }

        let mut edit_num: Option<(String, f64)> = None;
        let mut edit_bool: Option<(String, bool)> = None;
        let mut reset_key: Option<String> = None;

        egui::ScrollArea::vertical()
            .id_salt(("codes_rules_scroll", disc.key(), active_sec.as_str()))
            .show(ui, |ui| {
                egui::Grid::new(("codes_rules_grid", disc.key(), active_sec.as_str()))
                    .num_columns(5)
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Regra").font(strong(11.5)));
                        ui.label(RichText::new("Valor").font(strong(11.5)));
                        ui.label(RichText::new("Camada").font(strong(11.5)));
                        ui.label(RichText::new("Tipo").font(strong(11.5)));
                        ui.label(RichText::new("Fonte normativa").font(strong(11.5)));
                        ui.end_row();

                        for (k, rule) in &resolved.rules {
                            ui.label(RichText::new(k).font(strong(11.5)));
                            // Value cell (editable for scalar rules on non-WHOLE sections)
                            ui.horizontal(|ui| {
                                if !whole && let Some(mut v) = rule.value.as_f64() {
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut v).speed(1.0).max_decimals(3),
                                        )
                                        .changed()
                                    {
                                        edit_num = Some((k.clone(), v));
                                    }
                                } else if !whole && let Some(mut b) = rule.value.as_bool() {
                                    if ui.checkbox(&mut b, "").changed() {
                                        edit_bool = Some((k.clone(), b));
                                    }
                                } else {
                                    let preview = match &rule.value {
                                        Value::String(s) => s.clone(),
                                        Value::Array(a) => format!("[{} itens]", a.len()),
                                        Value::Object(o) => format!("{{{} chaves}}", o.len()),
                                        other => other.to_string(),
                                    };
                                    ui.label(preview);
                                }
                                if rule.layer == crate::domain::codes::Layer::Project
                                    && ui
                                        .small_button("↺")
                                        .on_hover_text("Restaurar valor da norma")
                                        .clicked()
                                {
                                    reset_key = Some(k.clone());
                                }
                            });
                            ui.label(RichText::new(rule.layer.label()).size(11.0).color(TEXT_2));
                            ui.label(RichText::new(rule.kind.label()).size(11.0).color(MUTED));
                            ui.label(
                                RichText::new(rule.source.as_deref().unwrap_or("—"))
                                    .size(11.0)
                                    .color(TEXT_2),
                            );
                            ui.end_row();
                        }
                    });
            });

        if let Some((k, v)) = edit_num
            && let Some(num) = serde_json::Number::from_f64(v)
        {
            let previous = self.project.clone();
            if self
                .project
                .codes
                .set_project_rule(disc, active_sec, &k, Value::Number(num))
                .is_ok()
            {
                self.record_change(previous);
            }
        }
        if let Some((k, b)) = edit_bool {
            let previous = self.project.clone();
            if self
                .project
                .codes
                .set_project_rule(disc, active_sec, &k, Value::Bool(b))
                .is_ok()
            {
                self.record_change(previous);
            }
        }
        if let Some(k) = reset_key {
            let previous = self.project.clone();
            self.project
                .codes
                .reset_project_rule(disc, active_sec, Some(&k));
            self.record_change(previous);
        }
    }

    fn codes_site_climate_tab(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(
                "Parâmetros climáticos e geográficos compartilhados entre Estrutura (vento V0 NBR 6123), Drenagem (chuva I NBR 10844), Elétrica (índice ceráunico AQ1–AQ3) e Conforto (NBR 15220).",
            )
            .size(12.0)
            .color(TEXT_2),
        );
        ui.add_space(8.0);

        // City preset ComboBox
        let mut chosen_preset: Option<String> = None;
        let current_loc = self
            .project
            .codes
            .site
            .location_label()
            .unwrap_or_else(|| "Escolher cidade brasileira…".to_owned());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Preset de cidade brasileira:").font(strong(12.0)));
            egui::ComboBox::from_id_salt("trazo_site_city_preset")
                .selected_text(current_loc)
                .width(260.0)
                .show_ui(ui, |ui| {
                    for preset in builtin_site_presets() {
                        let label = format!(
                            "{} / {} · ZB{} · V0={:.0} m/s · I={:.0} mm/h · {}",
                            preset.city,
                            preset.state_uf,
                            preset.bioclimatic_zone,
                            preset.wind_v0_ms,
                            preset.rain_intensity_mm_h,
                            preset.thunderstorms.code()
                        );
                        let selected = self.project.codes.site.preset_id.as_deref()
                            == Some(preset.id.as_str());
                        if ui.selectable_label(selected, label).clicked() {
                            chosen_preset = Some(preset.id.clone());
                        }
                    }
                });
        });

        if let Some(id) = chosen_preset {
            let previous = self.project.clone();
            match self.project.apply_site_preset(&id) {
                Ok(p) => {
                    self.record_change(previous);
                    self.message = format!(
                        "Local atualizado para {} / {} (V0 = {:.0} m/s sincronizado com vento estrutural).",
                        p.city, p.state_uf, p.wind_v0_ms
                    );
                }
                Err(err) => self.message = err,
            }
        }

        ui.add_space(8.0);
        let mut site: SiteClimate = self.project.codes.site.clone();
        let mut changed = false;

        egui::Grid::new("site_climate_grid")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label("Cidade / UF:");
                ui.horizontal(|ui| {
                    let mut city = site.city.clone().unwrap_or_default();
                    if ui
                        .add(egui::TextEdit::singleline(&mut city).desired_width(180.0))
                        .changed()
                    {
                        site.city = if city.trim().is_empty() {
                            None
                        } else {
                            Some(city)
                        };
                        changed = true;
                    }
                    let mut uf = site.state_uf.clone().unwrap_or_default();
                    if ui
                        .add(egui::TextEdit::singleline(&mut uf).desired_width(50.0))
                        .changed()
                    {
                        site.state_uf = if uf.trim().is_empty() { None } else { Some(uf) };
                        changed = true;
                    }
                });
                ui.end_row();

                ui.label("Latitude / Longitude (°):");
                ui.horizontal(|ui| {
                    let mut lat = site.latitude.unwrap_or(-23.55);
                    if ui
                        .add(
                            egui::DragValue::new(&mut lat)
                                .range(-90.0..=90.0)
                                .speed(0.05),
                        )
                        .changed()
                    {
                        site.latitude = Some(lat);
                        changed = true;
                    }
                    let mut lon = site.longitude.unwrap_or(-46.63);
                    if ui
                        .add(
                            egui::DragValue::new(&mut lon)
                                .range(-180.0..=180.0)
                                .speed(0.05),
                        )
                        .changed()
                    {
                        site.longitude = Some(lon);
                        changed = true;
                    }
                });
                ui.end_row();

                ui.label("Zona bioclimática (NBR 15220):");
                let mut zb = site.bioclimatic_zone.unwrap_or(3);
                if ui
                    .add(egui::DragValue::new(&mut zb).range(1..=8).prefix("ZB"))
                    .changed()
                {
                    site.bioclimatic_zone = Some(zb);
                    changed = true;
                }
                ui.end_row();

                ui.label("Vento básico V0 (NBR 6123):");
                let mut v0 = site.wind_v0_ms.unwrap_or(self.project.structure.wind.v0_ms);
                if ui
                    .add(
                        egui::DragValue::new(&mut v0)
                            .range(25.0..=65.0)
                            .speed(1.0)
                            .suffix(" m/s"),
                    )
                    .changed()
                {
                    site.wind_v0_ms = Some(v0);
                    changed = true;
                }
                ui.end_row();

                ui.label("Intensidade pluviométrica I (NBR 10844):");
                let mut rain = site.rain_intensity_mm_h.unwrap_or(150.0);
                if ui
                    .add(
                        egui::DragValue::new(&mut rain)
                            .range(10.0..=400.0)
                            .speed(1.0)
                            .suffix(" mm/h"),
                    )
                    .changed()
                {
                    site.rain_intensity_mm_h = Some(rain);
                    changed = true;
                }
                ui.end_row();

                ui.label("Nível ceráunico (NBR 5410 · DPS/SPDA):");
                let cur_aq = site.thunderstorms.unwrap_or(KeraunicLevel::Aq3);
                egui::ComboBox::from_id_salt("site_keraunic_combo")
                    .selected_text(cur_aq.label())
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for aq in KeraunicLevel::ALL {
                            if ui.selectable_label(cur_aq == aq, aq.label()).clicked() {
                                site.thunderstorms = Some(aq);
                                changed = true;
                            }
                        }
                    });
                ui.end_row();

                ui.label("Temperaturas de projeto (verão / inverno):");
                ui.horizontal(|ui| {
                    let mut cool = site.cooling_design_c.unwrap_or(32.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut cool)
                                .range(-20.0..=55.0)
                                .speed(0.5)
                                .suffix(" °C"),
                        )
                        .changed()
                    {
                        site.cooling_design_c = Some(cool);
                        changed = true;
                    }
                    let mut heat = site.heating_design_c.unwrap_or(12.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut heat)
                                .range(-20.0..=40.0)
                                .speed(0.5)
                                .suffix(" °C"),
                        )
                        .changed()
                    {
                        site.heating_design_c = Some(heat);
                        changed = true;
                    }
                });
                ui.end_row();
            });

        if changed {
            let previous = self.project.clone();
            if self.project.set_site_climate(site).is_ok() {
                self.record_change(previous);
            }
        }

        if !self.project.codes.site.sources.is_empty() {
            ui.add_space(10.0);
            ui.label(RichText::new("Fontes normativas do sítio:").font(strong(11.5)));
            for (k, v) in &self.project.codes.site.sources {
                ui.label(RichText::new(format!("• {k}: {v}")).size(11.0).color(MUTED));
            }
        }
    }

    fn codes_check_report_tab(&mut self, ui: &mut Ui, open_dialog: &mut Option<DocDialog>) {
        let report = check_project_codes(&self.project);
        let reqs = report.requirement_count();
        let advs = report.advisory_count();

        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!(
                    "Norma arquitetural ativa: {}{}",
                    report.active_code,
                    if report.active_amendments.is_empty() {
                        String::new()
                    } else {
                        format!(" + {}", report.active_amendments.join(" + "))
                    }
                ))
                .font(strong(12.5)),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("{reqs} requisito(s) · {advs} recomendação(ões)"))
                    .color(if reqs > 0 {
                        DANGER
                    } else if advs > 0 {
                        WARNING
                    } else {
                        SUCCESS
                    })
                    .font(strong(12.0)),
            );
            if report.active_code == "generic"
                && primary_button(ui, "Ativar Brasil (NBR 9050 + Código de Obras)").clicked()
            {
                let previous = self.project.clone();
                if self.project.codes.apply_preset("brasil").is_ok() {
                    self.record_change(previous);
                    self.message =
                        "Preset Brasil ativado para verificação de códigos de obra.".to_owned();
                }
            }
        });
        ui.separator();

        if report.findings.is_empty() {
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "Nenhum desvio encontrado para as regras arquiteturais ativas (escadas, rampas, portas e cômodos).",
                )
                .color(SUCCESS),
            );
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("codes_check_findings_scroll")
            .show(ui, |ui| {
                for f in &report.findings {
                    let color = if f.kind == RuleKind::Requirement {
                        DANGER
                    } else {
                        WARNING
                    };
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}.{}",
                                        f.element_name, f.section, f.rule
                                    ))
                                    .font(strong(12.0))
                                    .color(color),
                                );
                                if let Some(src) = &f.source {
                                    ui.label(RichText::new(src).size(11.0).color(MUTED));
                                }
                                if f.element_kind == "escada"
                                    && ui.small_button("Resolver no Solver →").clicked()
                                {
                                    *open_dialog = Some(DocDialog::Circulation);
                                }
                            });
                            ui.label(&f.message);
                            ui.label(
                                RichText::new(format!("Sugestão: {}", f.suggestion))
                                    .size(11.5)
                                    .color(ACCENT_TEXT),
                            );
                        });
                    ui.add_space(4.0);
                }
            });
    }

    fn export_codes_file(&mut self, as_xml: bool) {
        let (ext, desc, default_name) = if as_xml {
            ("xml", "Pacote de Normas XML", "normas-trazo.xml")
        } else {
            ("json", "Pacote de Normas JSON", "normas-trazo.json")
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(desc, &[ext])
            .set_file_name(default_name)
            .save_file()
        else {
            return;
        };
        let payload = if as_xml {
            Ok(self.project.codes.export_all_xml())
        } else {
            let packs: Vec<_> = CodeDiscipline::ALL
                .iter()
                .map(|&d| self.project.codes.export_effective_pack(d))
                .collect();
            xml::export_bundle_json(&packs)
        };
        match payload.and_then(|text| {
            std::fs::write(&path, text).map_err(|e| format!("Falha ao gravar arquivo: {e}"))
        }) {
            Ok(()) => self.message = format!("Pacote de normas exportado em {}.", path.display()),
            Err(err) => self.message = err,
        }
    }

    fn import_codes_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Pacote de Normas (XML / JSON)", &["xml", "json"])
            .pick_file()
        else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            self.message = "Não foi possível ler o arquivo de norma selecionado.".to_owned();
            return;
        };
        let previous = self.project.clone();
        match self.project.codes.import_packs(&raw, true) {
            Ok(imported) => {
                self.record_change(previous);
                self.message = format!(
                    "{} pacote(s) de norma importado(s) e ativado(s).",
                    imported.codes.len()
                );
            }
            Err(err) => self.message = err,
        }
    }
}
