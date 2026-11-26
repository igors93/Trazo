//! Keyboard shortcuts: reading the keys pressed in a frame and running
//! the matching commands, plus the snap, ortho and grid toggles.

use eframe::egui::{self, Key};

use super::{DocDialog, FloorPlanApp, Tool, tools};
use crate::domain::{geometry::Point, model::OpeningKind};

/// How far an arrow key moves the selection: one grid step.
const NUDGE_STEP: f64 = 0.25;

impl FloorPlanApp {
    pub(super) fn toggle_snap(&mut self) {
        self.snap_enabled = !self.snap_enabled;
        self.message = if self.snap_enabled {
            "Encaixe ativado: grade de 0,25 m e extremidades de paredes."
        } else {
            "Encaixe desativado."
        }
        .to_owned();
    }

    pub(super) fn toggle_ortho(&mut self) {
        self.ortho_enabled = !self.ortho_enabled;
        self.message = if self.ortho_enabled {
            "Modo orto ativado: paredes apenas horizontais ou verticais."
        } else {
            "Modo orto desativado."
        }
        .to_owned();
    }

    pub(super) fn toggle_grid(&mut self) {
        self.show_grid = !self.show_grid;
    }

    pub(super) fn handle_shortcuts(&mut self, context: &egui::Context) {
        let blocking_dialog = self.pending_room.is_some()
            || self.pending_action.is_some()
            || self.repeat_form.is_some();
        let toggle_palette = !blocking_dialog
            && context.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: Key::K,
                            pressed: true,
                            repeat: false,
                            modifiers,
                            ..
                        } if (modifiers.command || modifiers.ctrl) && !modifiers.shift
                    )
                })
            });
        if toggle_palette {
            if self.show_command_palette {
                self.show_command_palette = false;
            } else {
                self.open_command_palette();
            }
            return;
        }

        // Canvas and toolbar widgets can hold focus without editing text.
        let editing_text = context.text_edit_focused();
        // While a dialog is open, editing commands must not act on the
        // project behind it; the dialog handles its own keys.
        let modal_open = self.pending_room.is_some()
            || self.pending_action.is_some()
            || self.show_help
            || self.show_mcp_dialog
            || self.show_command_palette
            || self.active_doc_dialog != DocDialog::None
            || self.repeat_form.is_some();

        // Capture direct CAD numeric input typed while a drawing tool is active.
        if !editing_text && !modal_open && self.supports_numeric_input() {
            let typed: Vec<char> = context.input(|input| {
                if input.modifiers.ctrl || input.modifiers.command || input.modifiers.alt {
                    return Vec::new();
                }
                input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::Text(text) => Some(text.chars().collect::<Vec<_>>()),
                        _ => None,
                    })
                    .flatten()
                    .collect()
            });
            for ch in typed {
                if self.draft_input.chars().count() < 32
                    && tools::numeric_input::accepts_char(ch, !self.draft_input.is_empty())
                {
                    self.draft_input.push(ch);
                }
            }
        }
        let typing_numeric = !self.draft_input.is_empty();

        let actions = context.input(|input| {
            let pressed = |key: Key| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, .. } if *event_key == key)
                })
            };
            let plain = |key: Key| {
                !editing_text
                    && !modal_open
                    && !typing_numeric
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                            if *event_key == key && modifiers.is_none())
                    })
            };
            let key_unmodified = |key: Key, allow_repeat: bool| {
                !editing_text
                    && !modal_open
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat, modifiers, .. }
                            if *event_key == key && modifiers.is_none() && (allow_repeat || !repeat))
                    })
            };
            let shortcut = |key: Key, shift: bool| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                        if *event_key == key && (modifiers.command || modifiers.ctrl) && modifiers.shift == shift)
                })
            };
            // Arrows nudge the selection: 0,25 m, or 1 m with Shift. Held
            // keys repeat, as in other editors.
            let nudge = [
                (Key::ArrowLeft, -1.0, 0.0),
                (Key::ArrowRight, 1.0, 0.0),
                (Key::ArrowUp, 0.0, -1.0),
                (Key::ArrowDown, 0.0, 1.0),
            ]
            .into_iter()
            .find_map(|(key, dx, dy)| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key: event_key,
                        pressed: true,
                        modifiers,
                        ..
                    } if *event_key == key
                        && !editing_text
                        && !modal_open
                        && !modifiers.ctrl
                        && !modifiers.command
                        && !modifiers.alt =>
                    {
                        let step = if modifiers.shift { 1.0 } else { NUDGE_STEP };
                        Some(Point::new(dx * step, dy * step))
                    }
                    _ => None,
                })
            });
            let tool = [
                (Key::V, Tool::Move),
                (Key::T, Tool::Terrain),
                (Key::P, Tool::Wall),
                (Key::C, Tool::Room),
                (Key::L, Tool::RoomPolygon),
                (Key::R, Tool::Opening(OpeningKind::Door)),
                (Key::J, Tool::Opening(OpeningKind::Window)),
                (Key::M, Tool::Measure),
            ]
            .into_iter()
            .chain(tools::ModifyKind::ALL.into_iter().filter_map(|kind| {
                kind.key().map(|(key, _)| (key, Tool::Modify(kind)))
            }))
            .find_map(|(key, tool)| plain(key).then_some(tool))
            .or_else(|| {
                (!editing_text && !modal_open && !typing_numeric)
                    .then(|| Self::drafting_shortcut(input))
                    .flatten()
            });
            let free = !modal_open;
            // Ctrl+C, Ctrl+X and Ctrl+V reach the app as clipboard events.
            let clipboard_event = |accept: fn(&egui::Event) -> bool| {
                !editing_text && free && input.events.iter().any(accept)
            };
            let shifted = |key: Key| {
                !editing_text
                    && !modal_open
                    && !typing_numeric
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                            if *event_key == key && modifiers.shift && !modifiers.ctrl && !modifiers.command && !modifiers.alt)
                    })
            };
            ShortcutActions {
                copy: clipboard_event(|event| matches!(event, egui::Event::Copy))
                    || (shortcut(Key::C, false) && !editing_text && free),
                cut: clipboard_event(|event| matches!(event, egui::Event::Cut))
                    || (shortcut(Key::X, false) && !editing_text && free),
                paste: clipboard_event(|event| {
                    matches!(event, egui::Event::Paste(_) | egui::Event::PasteImage(_))
                }) || (shortcut(Key::V, false) && !editing_text && free),
                duplicate: shortcut(Key::D, false) && !editing_text && free,
                group: shortcut(Key::G, false) && !editing_text && free,
                ungroup: shortcut(Key::G, true) && !editing_text && free,
                join: plain(Key::U),
                rotate_back: shifted(Key::G),
                mirror_left_right: plain(Key::H),
                mirror_up_down: shifted(Key::H),
                save: shortcut(Key::S, false) && free,
                print: shortcut(Key::P, false) && free,
                open: shortcut(Key::O, false) && free,
                new: (shortcut(Key::N, false) || shortcut(Key::T, false)) && free,
                new_window: shortcut(Key::N, true) && free,
                close_tab: shortcut(Key::W, false) && free,
                tab_step: if free && shortcut(Key::Tab, false) {
                    1
                } else if free && shortcut(Key::Tab, true) {
                    -1
                } else {
                    0
                },
                undo: shortcut(Key::Z, false) && !editing_text && free,
                redo: (shortcut(Key::Y, false) || shortcut(Key::Z, true)) && !editing_text && free,
                select_all: shortcut(Key::A, false) && !editing_text && free,
                escape: pressed(Key::Escape)
                    && self.pending_action.is_none()
                    && self.repeat_form.is_none(),
                delete: pressed(Key::Delete) && !editing_text && !typing_numeric && free,
                tool,
                nudge,
                rotate: plain(Key::G),
                flip_side: plain(Key::I),
                flip_hinge: !editing_text
                    && !modal_open
                    && !typing_numeric
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: Key::I, pressed: true, repeat: false, modifiers, .. }
                            if modifiers.shift && !modifiers.ctrl && !modifiers.command && !modifiers.alt)
                    }),
                fit: plain(Key::F),
                finish_polygon: plain(Key::Enter),
                undo_point: plain(Key::Backspace),
                numeric_commit: typing_numeric && key_unmodified(Key::Enter, false),
                numeric_backspace: typing_numeric && key_unmodified(Key::Backspace, true),
                cycle_wall_reference: matches!(
                    self.tool,
                    Tool::Wall | Tool::Room | Tool::RoomPolygon
                ) && key_unmodified(Key::Tab, false),
                snap: pressed(Key::F3) && free,
                grid: pressed(Key::F7) && free,
                ortho: pressed(Key::F8) && free,
                help: pressed(Key::F1) && !blocking_dialog,
            }
        });
        if actions.save {
            self.save();
        }
        if actions.print {
            self.open_print_dialog();
        }
        if actions.open {
            self.request_open();
        }
        if actions.new_window {
            self.open_new_window();
        }
        if actions.new {
            self.request_new_project();
        }
        if actions.close_tab {
            self.request_close_tab(self.active_tab);
        }
        if actions.tab_step != 0 {
            self.next_tab(actions.tab_step);
        }
        if actions.undo {
            self.undo();
        }
        if actions.redo {
            self.redo();
        }
        if actions.select_all {
            self.select_all();
        }
        if actions.copy {
            self.copy_selection();
        }
        if actions.cut {
            self.cut_selection();
        }
        if actions.paste {
            self.paste();
        }
        if actions.group {
            self.group_selection();
        }
        if actions.ungroup {
            self.ungroup_selection();
        }
        if actions.join {
            self.join_selection();
        }
        if actions.duplicate {
            self.duplicate_selection();
        }
        // Ctrl+V only reaches the app while the system clipboard holds
        // something, so a copy leaves a note there.
        if let Some(note) = self.system_clipboard_note.take() {
            context.copy_text(note);
        }
        if actions.escape {
            if self.underlay_state.calibration.take().is_some() {
                self.message = "Calibração de escala cancelada.".to_owned();
            } else if self.show_help
                || self.show_mcp_dialog
                || self.active_doc_dialog != DocDialog::None
            {
                self.show_help = false;
                self.show_mcp_dialog = false;
                self.active_doc_dialog = DocDialog::None;
                self.mcp_feedback = None;
            } else if self.box_select.take().is_some() {
                self.message = "Seleção por janela cancelada.".to_owned();
            } else if !self.draft_input.is_empty() {
                self.draft_input.clear();
                self.message = "Entrada numérica limpa.".to_owned();
            } else if self.tool == Tool::Measure
                && self.draft_start().is_none()
                && !self.measurements.is_empty()
            {
                self.measurements.clear();
                self.message = "Medidas apagadas.".to_owned();
            } else if self.draft_start().is_none()
                && self.pending_room.is_none()
                && self.selection.is_some()
            {
                self.clear_selection();
                self.message = "Seleção limpa.".to_owned();
            } else {
                self.cancel_creation();
                self.pending_room = None;
                self.message = "Ação cancelada.".to_owned();
            }
        }
        if actions.numeric_backspace {
            self.draft_input.pop();
        }
        if actions.numeric_commit {
            self.commit_numeric_input();
        }
        if actions.cycle_wall_reference {
            self.wall_reference = self.wall_reference.next();
            self.message = format!(
                "Referência da parede: {} (Tab alterna).",
                self.wall_reference.label()
            );
        }
        if actions.delete {
            self.delete_selection();
        }
        if let Some(delta) = actions.nudge {
            self.nudge_selection(delta);
        }
        if actions.rotate {
            self.rotate_fixture_or_next();
        }
        if actions.rotate_back {
            self.rotate_selection_quarter(false);
        }
        if actions.mirror_left_right || actions.mirror_up_down {
            self.mirror_selection(actions.mirror_left_right);
        }
        if actions.flip_side || actions.flip_hinge {
            self.flip_selected_door(actions.flip_side, actions.flip_hinge);
        }
        if let Some(tool) = actions.tool {
            self.set_tool(tool);
            if tool == Tool::Terrain {
                self.sync_terrain_fields();
            }
        }
        if actions.fit {
            self.fit_view_requested();
        }
        if let Tool::Draw(kind) = self.tool {
            if actions.finish_polygon {
                self.finish_drawing(kind, false);
            }
            if actions.undo_point {
                self.undo_draw_point();
            }
        }
        if self.tool == Tool::RoomPolygon {
            if actions.finish_polygon {
                self.close_polygon_draft();
            }
            if actions.undo_point {
                self.undo_polygon_point();
            }
        }
        if actions.snap {
            self.toggle_snap();
        }
        if actions.grid {
            self.toggle_grid();
        }
        if actions.ortho {
            self.toggle_ortho();
        }
        if actions.help {
            self.show_help = !self.show_help;
        }
    }
}

struct ShortcutActions {
    save: bool,
    print: bool,
    open: bool,
    new: bool,
    new_window: bool,
    close_tab: bool,
    tab_step: isize,
    undo: bool,
    redo: bool,
    select_all: bool,
    copy: bool,
    cut: bool,
    paste: bool,
    duplicate: bool,
    group: bool,
    ungroup: bool,
    join: bool,
    rotate_back: bool,
    mirror_left_right: bool,
    mirror_up_down: bool,
    escape: bool,
    delete: bool,
    tool: Option<Tool>,
    nudge: Option<Point>,
    rotate: bool,
    flip_side: bool,
    flip_hinge: bool,
    fit: bool,
    finish_polygon: bool,
    undo_point: bool,
    numeric_commit: bool,
    numeric_backspace: bool,
    cycle_wall_reference: bool,
    snap: bool,
    grid: bool,
    ortho: bool,
    help: bool,
}
