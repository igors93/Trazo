//! Building blocks shared by the panels: ribbon buttons, menu rows,
//! collapsible property sections, status-bar toggles and buttons.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Galley, Margin, Painter, Pos2, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, text::LayoutJob, vec2,
};

use super::icons::{self, Icon};
use crate::app::theme::{self, *};

pub const MENU_WIDTH: f32 = 286.0;
pub const RIBBON_ROW: f32 = 66.0;
pub const RIBBON_PANEL_HEIGHT: f32 = RIBBON_ROW + 16.0;

/// Lays out `text` on a single line, truncating cleanly with `…` when wider than `max_width`.
pub fn truncated_galley(
    painter: &Painter,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let max_w = max_width.max(12.0);
    let full = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
    if full.size().x <= max_w {
        return full;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let mut cand: String = chars[..mid].iter().collect();
        cand.push('…');
        if painter.layout_no_wrap(cand, font.clone(), color).size().x <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut out: String = chars[..lo].iter().collect();
    out.push('…');
    painter.layout_no_wrap(out, font, color)
}

/// The tooltip shows whether or not the widget is enabled.
pub fn tip(response: Response, text: &str) -> Response {
    response.on_hover_text(text).on_disabled_hover_text(text)
}

/// Ink and accent tones of an icon for a control's state.
fn tones(enabled: bool, active: bool) -> (Color32, Color32) {
    if !enabled {
        (FAINT, FAINT)
    } else if active {
        (ACCENT_TEXT, ACCENT)
    } else {
        (TEXT_2, ACCENT)
    }
}

fn paint_control_background(
    painter: &Painter,
    rect: Rect,
    radius: u8,
    response: &Response,
    enabled: bool,
    active: bool,
) {
    let pressed = enabled && response.is_pointer_button_down_on();
    if active || pressed {
        painter.rect(
            rect,
            radius,
            ACCENT_SOFT,
            Stroke::new(1.0, ACCENT_RING),
            StrokeKind::Inside,
        );
    } else if enabled && response.hovered() {
        painter.rect(
            rect,
            radius,
            ACCENT_SOFTER,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
    }
}

/// Text with letter spacing, in capitals: panel and group titles.
pub fn caps_galley(painter: &Painter, text: &str, size: f32, color: Color32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: strong(size),
            color,
            extra_letter_spacing: 0.7,
            ..Default::default()
        },
    );
    painter.layout_job(job)
}

pub fn icon_button_sized(
    ui: &mut Ui,
    icon: Icon,
    tooltip: &str,
    active: bool,
    size: f32,
    icon_size: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let enabled = ui.is_enabled();
    let painter = ui.painter();
    paint_control_background(painter, rect, 6, &response, enabled, active);
    let (ink, accent) = tones(enabled, active);
    icons::paint(
        painter,
        Rect::from_center_size(rect.center(), vec2(icon_size, icon_size)),
        icon,
        ink,
        accent,
    );
    let response = tip(response, tooltip);
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A large ribbon button: icon above, label below.
pub fn ribbon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    draggable: bool,
) -> Response {
    let enabled = ui.is_enabled();
    let color = if !enabled {
        FAINT
    } else if selected {
        ACCENT_TEXT
    } else {
        TEXT_2
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(12.0), color);
    let width = (galley.size().x + 18.0).max(54.0);
    let sense = if draggable {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, RIBBON_ROW), sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let painter = ui.painter();
    paint_control_background(painter, rect, 7, &response, enabled, selected);
    if selected {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 12.0, rect.bottom() - 3.0),
                Pos2::new(rect.right() - 12.0, rect.bottom() - 1.0),
            ),
            1.0,
            ACCENT,
        );
    }
    let (ink, accent) = tones(enabled, selected);
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 23.0),
            vec2(24.0, 24.0),
        ),
        icon,
        ink,
        accent,
    );
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - 13.0 - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    if enabled {
        response.on_hover_cursor(if draggable {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::PointingHand
        })
    } else {
        response
    }
}

/// A compact ribbon row: crisp 14 px icon and label, stacked up to three high
/// with comfortable vertical room (`20.67 px` per row) and uniform stack width.
pub fn ribbon_small(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let enabled = ui.is_enabled();
    let color = if !enabled {
        FAINT
    } else if selected {
        ACCENT_TEXT
    } else {
        TEXT_2
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(12.0), color);
    let row_height = (RIBBON_ROW - 4.0) / 3.0;
    let natural_w = galley.size().x + 34.0;
    let width = ui.min_rect().width().max(natural_w);
    let (rect, response) = ui.allocate_exact_size(vec2(width, row_height), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let painter = ui.painter();
    paint_control_background(painter, rect, 5, &response, enabled, selected);
    let (ink, accent) = tones(enabled, selected);
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 13.0, rect.center().y),
            vec2(14.0, 14.0),
        ),
        icon,
        ink,
        accent,
    );
    painter.galley(
        Pos2::new(rect.left() + 25.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A ribbon group: its buttons, with its subtle title underneath.
pub fn ribbon_panel<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let title_galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), regular(10.5), MUTED);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
        ui.set_min_width(title_galley.size().x + 14.0);
        let row = ui.horizontal_top(|ui| {
            ui.set_min_height(RIBBON_ROW);
            add(ui)
        });
        let width = row.response.rect.width().max(title_galley.size().x + 14.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 16.0), Sense::hover());
        ui.painter().galley(
            Pos2::new(
                rect.center().x - title_galley.size().x / 2.0,
                rect.center().y - title_galley.size().y / 2.0 + 1.0,
            ),
            title_galley,
            MUTED,
        );
        row.inner
    })
    .inner
}

/// A column of `ribbon_small` rows inside a ribbon panel, top-aligned to match
/// full-height ribbon buttons even when containing 1 or 2 entries.
pub fn ribbon_stack<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.set_min_height(RIBBON_ROW);
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        add(ui)
    })
    .inner
}

pub fn vertical_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(13.0, height), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top() + 5.0),
            Pos2::new(rect.center().x, rect.bottom() - 5.0),
        ],
        Stroke::new(1.0, BORDER),
    );
}

fn menu_row(
    ui: &mut Ui,
    label: &str,
    shortcut: &str,
    has_submenu_arrow: bool,
    paint_slot: impl FnOnce(&Painter, Rect, bool),
) -> Response {
    let enabled = ui.is_enabled();
    let (rect, response) = ui.allocate_exact_size(vec2(MENU_WIDTH, 26.0), Sense::click());
    let painter = ui.painter();
    if enabled && response.hovered() {
        painter.rect_filled(rect, 5, ACCENT_SOFT);
    }
    paint_slot(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 15.0, rect.center().y),
            vec2(15.0, 15.0),
        ),
        enabled,
    );
    let label_color = if enabled { TEXT } else { FAINT };
    let shortcut_galley = (!shortcut.is_empty()).then(|| {
        painter.layout_no_wrap(
            shortcut.to_owned(),
            regular(11.5),
            if enabled { MUTED } else { FAINT },
        )
    });
    let right_reserve = match &shortcut_galley {
        Some(galley) => galley.size().x + 20.0,
        None if has_submenu_arrow => 24.0,
        None => 10.0,
    };
    let max_label_w = (rect.width() - 34.0 - right_reserve).max(24.0);
    let label_galley = truncated_galley(painter, label, regular(12.5), label_color, max_label_w);
    painter.galley(
        Pos2::new(
            rect.left() + 34.0,
            rect.center().y - label_galley.size().y / 2.0,
        ),
        label_galley,
        label_color,
    );
    if let Some(galley) = shortcut_galley {
        let color = if enabled { MUTED } else { FAINT };
        painter.galley(
            Pos2::new(
                rect.right() - 10.0 - galley.size().x,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A menu entry with an icon and its shortcut on the right.
pub fn menu_item(ui: &mut Ui, icon: Option<Icon>, label: &str, shortcut: &str) -> Response {
    menu_row(ui, label, shortcut, false, |painter, slot, enabled| {
        if let Some(icon) = icon {
            let (ink, accent) = tones(enabled, false);
            icons::paint(painter, slot, icon, ink, accent);
        }
    })
}

/// A menu entry that opens a submenu, marked with an arrow.
pub fn menu_submenu(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let response = menu_row(ui, label, "", true, |painter, slot, enabled| {
        let (ink, accent) = tones(enabled, false);
        icons::paint(painter, slot, icon, ink, accent);
    });
    let arrow = Rect::from_center_size(
        Pos2::new(response.rect.right() - 14.0, response.rect.center().y),
        vec2(13.0, 13.0),
    );
    let arrow_tone = if enabled { MUTED } else { FAINT };
    icons::paint(
        ui.painter(),
        arrow,
        Icon::ChevronRight,
        arrow_tone,
        arrow_tone,
    );
    response
}

/// Renders an icon-aligned submenu entry and opens its popup menu on hover/click.
pub fn submenu(ui: &mut Ui, icon: Icon, label: &str, add_contents: impl FnOnce(&mut Ui)) {
    let trigger = menu_submenu(ui, icon, label);
    if ui.is_enabled() {
        egui::containers::menu::SubMenu::new().show(ui, &trigger, |ui| {
            ui.set_width(MENU_WIDTH);
            add_contents(ui);
        });
    }
}

/// A menu entry that shows a check mark while `checked`.
pub fn menu_check(ui: &mut Ui, label: &str, shortcut: &str, checked: bool) -> Response {
    menu_row(ui, label, shortcut, false, |painter, slot, enabled| {
        if checked {
            painter.rect_filled(slot.expand(1.0), 4, ACCENT_SOFT);
            let (_, accent) = tones(enabled, true);
            icons::paint(painter, slot, Icon::Check, accent, accent);
        } else {
            painter.rect_stroke(
                slot.shrink(1.5),
                3,
                Stroke::new(1.0, BORDER_STRONG),
                StrokeKind::Inside,
            );
        }
    })
}

pub fn menu_separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(MENU_WIDTH, 8.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.left() + 6.0, rect.center().y),
            Pos2::new(rect.right() - 6.0, rect.center().y),
        ],
        Stroke::new(1.0, BORDER),
    );
}

/// Subtle uppercase section label inside menus for clear scannability.
pub fn menu_section_label(ui: &mut Ui, label: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(MENU_WIDTH, 20.0), Sense::hover());
    let painter = ui.painter();
    let galley = caps_galley(painter, label, 9.5, MUTED);
    painter.galley(
        Pos2::new(
            rect.left() + 10.0,
            rect.center().y - galley.size().y / 2.0 + 1.0,
        ),
        galley,
        MUTED,
    );
}

struct ActionButtonStyle {
    bold: bool,
    text: Color32,
    fill: Color32,
    hover_fill: Color32,
    active_fill: Color32,
    border: Color32,
    hover_border: Color32,
}

fn styled_action_button(ui: &mut Ui, label: &str, style: ActionButtonStyle) -> Response {
    let enabled = ui.is_enabled();
    let fg = if enabled { style.text } else { FAINT };
    let font = if style.bold {
        strong(12.5)
    } else {
        regular(12.5)
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let width = (galley.size().x + 22.0).max(48.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    let painter = ui.painter();
    let (bg, stroke_col) = if !enabled {
        (BAND, BORDER)
    } else if response.is_pointer_button_down_on() {
        (style.active_fill, style.hover_border)
    } else if response.hovered() {
        (style.hover_fill, style.hover_border)
    } else {
        (style.fill, style.border)
    };
    painter.rect(
        rect,
        6,
        bg,
        Stroke::new(1.0, stroke_col),
        StrokeKind::Inside,
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            5,
            Stroke::new(1.5, ACCENT_RING),
            StrokeKind::Inside,
        );
    }
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    styled_action_button(
        ui,
        label,
        ActionButtonStyle {
            bold: true,
            text: Color32::WHITE,
            fill: ACCENT,
            hover_fill: ACCENT_HOVER,
            active_fill: ACCENT_STRONG,
            border: ACCENT_STRONG,
            hover_border: ACCENT_STRONG,
        },
    )
}

pub fn secondary_button(ui: &mut Ui, label: &str) -> Response {
    styled_action_button(
        ui,
        label,
        ActionButtonStyle {
            bold: false,
            text: TEXT_2,
            fill: CONTROL,
            hover_fill: ACCENT_SOFTER,
            active_fill: ACCENT_SOFT,
            border: BORDER_STRONG,
            hover_border: ACCENT_RING,
        },
    )
}

pub fn danger_button(ui: &mut Ui, label: &str) -> Response {
    styled_action_button(
        ui,
        label,
        ActionButtonStyle {
            bold: false,
            text: DANGER,
            fill: DANGER_SOFT,
            hover_fill: DANGER_HOVER,
            active_fill: DANGER_HOVER,
            border: DANGER_BORDER,
            hover_border: DANGER,
        },
    )
}

/// The title row of a side panel: caps title truncated safely clear of the
/// right-side close button, with an optional muted count.
pub fn panel_header(ui: &mut Ui, title: &str, trailing: &str) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    let painter = ui.painter();
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.bottom()),
            Pos2::new(rect.right(), rect.bottom()),
        ],
        Stroke::new(1.0, BORDER),
    );
    let trailing_w = if trailing.is_empty() {
        0.0
    } else {
        painter
            .layout_no_wrap(trailing.to_owned(), regular(11.0), MUTED)
            .size()
            .x
            + 8.0
    };
    let max_title_w = (rect.width() - 14.0 - 38.0 - trailing_w).max(32.0);
    let title_text = title.to_uppercase();
    let galley = truncated_galley(painter, &title_text, strong(10.5), MUTED, max_title_w);
    painter.galley(
        Pos2::new(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
        galley,
        MUTED,
    );
    if !trailing.is_empty() {
        painter.text(
            Pos2::new(rect.right() - 38.0, rect.center().y),
            Align2::RIGHT_CENTER,
            trailing,
            regular(11.0),
            MUTED,
        );
    }
    rect
}

/// A full-width collapsible section of the properties panel with a single clean
/// top hairline (no double borders or white gaps between collapsed sections).
pub fn section(ui: &mut Ui, id_salt: &str, title: &str, body: impl FnOnce(&mut Ui)) {
    let id = ui.make_persistent_id(("property_section", id_salt));
    let mut open = ui.data_mut(|data| *data.get_temp_mut_or(id, true));
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        open = !open;
        ui.data_mut(|data| data.insert_temp(id, open));
    }
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        0,
        if response.hovered() {
            ACCENT_SOFTER
        } else {
            PANEL_ALT
        },
    );
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.top()),
            Pos2::new(rect.right(), rect.top()),
        ],
        Stroke::new(1.0, BORDER),
    );
    if open {
        painter.line_segment(
            [
                Pos2::new(rect.left(), rect.bottom()),
                Pos2::new(rect.right(), rect.bottom()),
            ],
            Stroke::new(1.0, BORDER),
        );
    }
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            vec2(12.0, 12.0),
        ),
        if open {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        },
        MUTED,
        MUTED,
    );
    painter.text(
        Pos2::new(rect.left() + 26.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        strong(12.0),
        TEXT,
    );
    if open {
        egui::Frame::new()
            .fill(PANEL)
            .inner_margin(Margin {
                left: 12,
                right: 12,
                top: 8,
                bottom: 10,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                body(ui);
            });
    }
}

/// A status-bar switch in capitals, lit while on (like CAD drafting aids).
pub fn toggle_pill(ui: &mut Ui, label: &str, on: bool, tooltip: &str) -> Response {
    let color = if on { ACCENT_TEXT } else { MUTED };
    let galley = caps_galley(ui.painter(), label, 10.0, color);
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 16.0, 20.0), Sense::click());
    let painter = ui.painter();
    if on {
        painter.rect(
            rect,
            CornerRadius::same(4),
            ACCENT_SOFT,
            Stroke::new(1.0, ACCENT_RING),
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect(
            rect,
            CornerRadius::same(4),
            PANEL,
            Stroke::new(1.0, BORDER_STRONG),
            StrokeKind::Inside,
        );
    }
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    tip(response, tooltip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A keyboard key, as shown in the shortcuts list.
pub fn key_cap(ui: &mut Ui, key: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(key.to_owned(), mono(11.5), TEXT_2);
    let (rect, _) = ui.allocate_exact_size(
        vec2((galley.size().x + 12.0).max(22.0), 20.0),
        Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect(
        rect,
        4,
        PANEL_ALT,
        Stroke::new(1.0, BORDER_STRONG),
        StrokeKind::Inside,
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + 2.0, rect.bottom() - 0.5),
            Pos2::new(rect.right() - 2.0, rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, BORDER_STRONG),
    );
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        TEXT_2,
    );
}

/// A muted label for the left column of property grids.
pub fn property_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(MUTED));
}

/// A read-only value in a property grid.
pub fn property_value(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(TEXT));
}

/// A block of code or configuration with a "Copiar" button; returns
/// whether the button was clicked.
pub fn code_block(ui: &mut Ui, code: &str) -> bool {
    let mut copy = false;
    egui::Frame::new()
        .fill(PANEL_ALT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let text_width = (ui.available_width() - 76.0).max(80.0);
                ui.allocate_ui_with_layout(
                    vec2(text_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(code).font(mono(12.0)).color(TEXT))
                                .wrap()
                                .selectable(true),
                        );
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    copy = ui
                        .add(
                            egui::Button::new(
                                RichText::new("Copiar").size(12.0).color(ACCENT_TEXT),
                            )
                            .corner_radius(5)
                            .min_size(vec2(64.0, 24.0)),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked();
                });
            });
        });
    copy
}

/// A boxed hint line with an icon, for guidance under a form.
pub fn hint(ui: &mut Ui, icon: Icon, text: &str, tone: Color32) {
    egui::Frame::new()
        .fill(PANEL_ALT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                icons::paint(ui.painter(), rect, icon, tone, tone);
                ui.add(egui::Label::new(RichText::new(text).size(12.0).color(TEXT_2)).wrap());
            });
        });
}
