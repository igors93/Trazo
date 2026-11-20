//! "Conectar uma IA": everything needed to use Trazo from ChatGPT, Claude or
//! another MCP client, ready to copy or set up with one click.

use super::super::{FloorPlanApp, McpStatus, theme::*};
use super::icons::{self, Icon};
use super::widgets::{code_block, hint, icon_button_sized, primary_button, secondary_button};
use crate::mcp::{
    self, clients,
    tunnel::{self, Tunnel, TunnelState},
};
use eframe::egui::{self, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

fn step_title(ui: &mut Ui, number: usize, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (badge, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
        ui.painter()
            .circle_filled(badge.center(), 12.0, ACCENT_SOFT);
        ui.painter().text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            number.to_string(),
            strong(12.5),
            ACCENT_TEXT,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(title).font(strong(14.0)).color(TEXT));
            ui.label(RichText::new(subtitle).size(12.0).color(MUTED));
        });
    });
    ui.add_space(6.0);
}

impl FloorPlanApp {
    /// Opens, shows and closes the public tunnel for cloud assistants.
    fn tunnel_controls(&mut self, ui: &mut Ui, port: Option<u16>) {
        let state = self.tunnel.as_mut().map(Tunnel::state);
        match state {
            None => {
                ui.horizontal(|ui| {
                    let installed = tunnel::find_cloudflared().is_some();
                    let enabled = port.is_some() && installed;
                    if ui
                        .add_enabled_ui(enabled, |ui| primary_button(ui, "Abrir túnel"))
                        .inner
                        .clicked()
                        && let Some(port) = port
                    {
                        match Tunnel::start(port, self.tunnel_token.clone()) {
                            Ok(started) => {
                                self.tunnel = Some(started);
                                self.mcp_feedback = None;
                            }
                            Err(error) => self.mcp_feedback = Some((false, error)),
                        }
                    }
                    if !installed {
                        if secondary_button(ui, "Instalar cloudflared").clicked() {
                            self.mcp_feedback = Some(match tunnel::install_cloudflared() {
                                Ok(()) => (
                                    true,
                                    "Instalador aberto. Quando terminar, clique em Abrir túnel."
                                        .to_owned(),
                                ),
                                Err(error) => (false, error),
                            });
                        }
                        ui.label(
                            RichText::new("O túnel usa o cloudflared, gratuito.")
                                .size(12.0)
                                .color(MUTED),
                        );
                    }
                });
            }
            Some(TunnelState::Starting) => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0).color(ACCENT));
                    ui.label(RichText::new("Abrindo o túnel...").color(TEXT_2));
                });
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(400));
            }
            Some(TunnelState::Ready(address)) => {
                ui.label(
                    RichText::new("Endereço protegido por token automático: cole no ChatGPT (modo desenvolvedor) ou no Claude na web, sem configurar autenticação extra.")
                        .size(12.0)
                        .color(TEXT_2),
                );
                ui.add_space(4.0);
                let connector = self
                    .tunnel
                    .as_ref()
                    .map(|tunnel| tunnel.connector_url(&address))
                    .unwrap_or_else(|| format!("{address}/mcp"));
                if code_block(ui, &connector) {
                    self.copy(ui, &connector, "Endereço protegido do conector");
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, "Fechar túnel").clicked() {
                        self.tunnel = None;
                    }
                    ui.label(
                        RichText::new("Um novo token secreto é gerado cada vez que o túnel abre e revogado ao fechar.")
                            .size(11.5)
                            .color(MUTED),
                    );
                });
                // The tunnel may still stop; keep checking while shown.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs(2));
            }
            Some(TunnelState::Failed(error)) => {
                hint(ui, Icon::Warning, &error, DANGER);
                ui.add_space(4.0);
                if primary_button(ui, "Tentar de novo").clicked() {
                    self.tunnel = None;
                    if let Some(port) = port {
                        match Tunnel::start(port, self.tunnel_token.clone()) {
                            Ok(started) => self.tunnel = Some(started),
                            Err(error) => self.mcp_feedback = Some((false, error)),
                        }
                    }
                }
            }
        }
    }

    fn copy(&mut self, ui: &Ui, text: &str, what: &str) {
        ui.ctx().copy_text(text.to_owned());
        self.message = format!("{what} copiado para a área de transferência.");
        self.mcp_feedback = Some((true, format!("{what} copiado.")));
    }

    /// The window that explains how to connect an AI assistant.
    pub(super) fn connect_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_mcp_dialog {
            return;
        }
        let mut close = false;
        let max_height = (ctx.content_rect().height() - 120.0).max(320.0);
        let response = egui::Modal::new(egui::Id::unique("connect_ai_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(600.0);
                let close_rect = Rect::from_min_size(
                    Pos2::new(ui.max_rect().right() - 26.0, ui.max_rect().top()),
                    vec2(26.0, 26.0),
                );
                if ui
                    .put(close_rect, |ui: &mut Ui| {
                        icon_button_sized(ui, Icon::Close, "Fechar (Esc)", false, 26.0, 14.0)
                    })
                    .clicked()
                {
                    close = true;
                }
                super::dialogs::dialog_title(
                    ui,
                    Icon::Ai,
                    "Conectar uma IA",
                    "Use o Trazo pelo ChatGPT, Claude ou outro assistente compatível com MCP.",
                );
                egui::ScrollArea::vertical()
                    .max_height(max_height - 140.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| self.connect_dialog_body(ui));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if let Some((ok, text)) = &self.mcp_feedback {
                        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        let tone = if *ok { SUCCESS } else { DANGER };
                        icons::paint(
                            ui.painter(),
                            rect,
                            if *ok { Icon::Check } else { Icon::Warning },
                            tone,
                            tone,
                        );
                        ui.label(RichText::new(text).size(12.0).color(TEXT_2));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Fechar").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close || response.should_close() {
            self.show_mcp_dialog = false;
            self.mcp_feedback = None;
        }
    }

    fn connect_dialog_body(&mut self, ui: &mut Ui) {
        let port = match &self.mcp_status {
            McpStatus::Listening(port) => Some(*port),
            _ => None,
        };
        let url = mcp::local_url(port.unwrap_or_else(mcp::port));

        // Server state.
        egui::Frame::new()
            .fill(if port.is_some() { ACCENT_SOFTER } else { PANEL_ALT })
            .stroke(Stroke::new(1.0, if port.is_some() { ACCENT_RING } else { BORDER }))
            .corner_radius(8)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(
                        dot.center(),
                        5.0,
                        if port.is_some() { SUCCESS } else { DANGER },
                    );
                    match &self.mcp_status {
                        McpStatus::Listening(_) => {
                            ui.label(RichText::new("Servidor ativo").font(strong(13.0)).color(TEXT));
                            ui.label(
                                RichText::new(format!(
                                    "· {} ação(ões) da IA nesta sessão",
                                    self.mcp_actions
                                ))
                                .size(12.0)
                                .color(MUTED),
                            );
                        }
                        McpStatus::Failed(error) => {
                            ui.label(
                                RichText::new(format!("Servidor desligado: {error}."))
                                    .color(DANGER),
                            );
                        }
                        McpStatus::Off => {
                            ui.label(RichText::new("Servidor desligado.").color(DANGER));
                        }
                    }
                });
                ui.add_space(6.0);
                if code_block(ui, &url) {
                    self.copy(ui, &url, "Endereço");
                }
                if port.is_none() {
                    ui.add_space(4.0);
                    hint(
                        ui,
                        Icon::Warning,
                        "Feche outras janelas do Trazo (só uma pode atender a IA) e abra o Trazo de novo. Para usar outra porta, defina a variável TRAZO_MCP_PORT.",
                        WARNING,
                    );
                }
            });
        ui.add_space(12.0);
        self.ai_log_section(ui);
        ui.add_space(16.0);

        // Claude Desktop.
        step_title(
            ui,
            1,
            "Claude Desktop",
            "Um clique adiciona o Trazo às configurações; depois feche e abra o Claude Desktop.",
        );
        let executable = std::env::current_exe().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.add_space(34.0);
            if primary_button(ui, "Configurar Claude Desktop").clicked() {
                self.mcp_feedback = Some(match clients::install_claude_desktop(&executable) {
                    Ok(path) => (
                        true,
                        format!(
                            "Claude Desktop configurado ({}). Reinicie o Claude Desktop.",
                            path.display()
                        ),
                    ),
                    Err(error) => (false, error),
                });
            }
        });
        ui.add_space(6.0);
        egui::Frame::new()
            .inner_margin(Margin {
                left: 34,
                right: 0,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "Ou cole em Configurações › Desenvolvedor › Editar configuração (claude_desktop_config.json):",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(4.0);
                let snippet = clients::claude_desktop_snippet(&executable);
                if code_block(ui, &snippet) {
                    self.copy(ui, &snippet, "Configuração");
                }
            });
        ui.add_space(16.0);

        // Claude Code.
        step_title(
            ui,
            2,
            "Claude Code",
            "Rode este comando no terminal uma vez.",
        );
        egui::Frame::new()
            .inner_margin(Margin {
                left: 34,
                right: 0,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                let command = clients::claude_code_command(&url);
                if code_block(ui, &command) {
                    self.copy(ui, &command, "Comando");
                }
            });
        ui.add_space(16.0);

        // ChatGPT and other web assistants.
        step_title(
            ui,
            3,
            "ChatGPT e Claude na web",
            "Eles rodam na nuvem e precisam de um endereço público https até o seu computador.",
        );
        egui::Frame::new()
            .inner_margin(Margin {
                left: 34,
                right: 0,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                self.tunnel_controls(ui, port);
                ui.add_space(8.0);
                let active_token = self.tunnel_token.current();
                ui.label(
                    RichText::new("Ou abra o túnel manualmente num terminal e acrescente o caminho com token ao endereço que aparecer:")
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(4.0);
                let command = clients::tunnel_command(port.unwrap_or_else(mcp::port));
                if code_block(ui, &command) {
                    self.copy(ui, &command, "Comando do túnel");
                }
                if self.tunnel.is_none() {
                    ui.add_space(4.0);
                    match active_token {
                        Some(token) => {
                            let suffix = format!("/mcp/{token}");
                            if code_block(ui, &suffix) {
                                self.copy(ui, &suffix, "Caminho com token");
                            }
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if secondary_button(ui, "Novo token").clicked() {
                                    self.tunnel_token.set(Some(tunnel::generate_token()));
                                    self.mcp_feedback = Some((true, "Novo token gerado.".to_owned()));
                                }
                                if secondary_button(ui, "Revogar token").clicked() {
                                    self.tunnel_token.clear();
                                    self.mcp_feedback = Some((true, "Token revogado.".to_owned()));
                                }
                            });
                        }
                        None => {
                            if secondary_button(ui, "Gerar token para túnel manual").clicked() {
                                self.tunnel_token.set(Some(tunnel::generate_token()));
                                self.mcp_feedback =
                                    Some((true, "Token para túnel manual ativado.".to_owned()));
                            }
                        }
                    }
                }
            });
        ui.add_space(14.0);
        hint(
            ui,
            Icon::Info,
            "Deixe o Trazo aberto enquanto usa a IA. Tudo o que ela faz aparece ao vivo na planta e pode ser desfeito com Ctrl+Z.",
            ACCENT,
        );
    }
}

impl FloorPlanApp {
    /// What the assistant did in this session, newest first, and whether
    /// the view follows it.
    fn ai_log_section(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Ações da IA").font(strong(13.0)).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if super::widgets::toggle_pill(
                    ui,
                    "Acompanhar a IA",
                    self.follow_ai,
                    "Aproxima a vista do que a IA cria ou altera quando está fora da tela",
                )
                .clicked()
                {
                    self.follow_ai = !self.follow_ai;
                }
            });
        });
        ui.add_space(4.0);
        if self.ai_activity.log.is_empty() {
            ui.label(
                RichText::new(
                    "Nenhuma ação ainda. Cada alteração da IA aparece na planta com um contorno roxo.",
                )
                .size(12.0)
                .color(MUTED),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(132.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for action in self.ai_activity.log.iter().rev().take(40) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                        ui.painter().circle_filled(
                            dot.center(),
                            3.5,
                            if action.ok { SUCCESS } else { DANGER },
                        );
                        ui.label(
                            RichText::new(&action.tool)
                                .monospace()
                                .size(11.5)
                                .color(TEXT_2),
                        );
                        let detail = if action.changed > 0 {
                            format!("{} · {} alteração(ões)", action.summary, action.changed)
                        } else {
                            action.summary.clone()
                        };
                        ui.add(
                            egui::Label::new(RichText::new(detail).size(11.5).color(MUTED))
                                .truncate(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(super::super::ai_activity::time_ago(action.at))
                                    .size(11.0)
                                    .color(FAINT),
                            );
                        });
                    });
                }
            });
    }
}
