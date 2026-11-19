//! Building blocks shared by the panels: ribbon buttons, menu rows,
//! collapsible property sections, status-bar toggles and buttons.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Galley, Margin, Painter, Pos2, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, text::LayoutJob, vec2,
};

use super::icons::{self, Icon};
use crate::app::theme::{self, *};

pub const MENU_WIDTH: f32 = 252.0;
const RIBBON_ROW: f32 = 54.0;

/// The tooltip shows whether or not the widget is enabled.
pub fn tip(response: Response, text: &str) -> Response {
    response
        .on_hover_text(text)
        .on_disabled_hover_text(text)
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

/// A square icon-only button.
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, active: bool) -> Response {
    icon_button_sized(ui, icon, tooltip, active, 28.0, 18.0)
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
    let width = (galley.size().x + 16.0).max(52.0);
    let sense = if draggable {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, RIBBON_ROW), sense);
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
            Pos2::new(rect.center().x, rect.top() + 19.0),
            vec2(25.0, 25.0),
        ),
        icon,
        ink,
        accent,
    );
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - 11.0 - galley.size().y / 2.0,
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

/// A compact ribbon row: small icon and label, stacked three high.
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
    let (rect, response) = ui.allocate_exact_size(
        vec2(galley.size().x + 38.0, (RIBBON_ROW - 4.0) / 3.0),
        Sense::click(),
    );
    let painter = ui.painter();
    paint_control_background(painter, rect, 5, &response, enabled, selected);
    let (ink, accent) = tones(enabled, selected);
    icons::paint(
        painter,
        Rect::from_center_size(Pos2::new(rect.left() + 13.0, rect.center().y), vec2(16.0, 16.0)),
        icon,
        ink,
        accent,
    );
    painter.galley(
        Pos2::new(rect.left() + 27.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A ribbon group: its buttons, with its title underneath.
pub fn ribbon_panel<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let title_galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), regular(11.0), MUTED);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
        ui.set_min_width(title_galley.size().x + 16.0);
        let row = ui.horizontal(|ui| {
            ui.set_min_height(RIBBON_ROW);
            add(ui)
        });
        let width = row
            .response
            .rect
            .width()
            .max(title_galley.size().x + 16.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 17.0), Sense::hover());
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

/// A column of `ribbon_small` rows inside a ribbon panel.
pub fn ribbon_stack<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        add(ui)
    })
    .inner
}

pub fn vertical_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(13.0, height), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top() + 4.0),
            Pos2::new(rect.center().x, rect.bottom() - 4.0),
        ],
        Stroke::new(1.0, BORDER),
    );
}

fn menu_row(
    ui: &mut Ui,
    label: &str,
    shortcut: &str,
    paint_slot: impl FnOnce(&Painter, Rect, bool),
) -> Response {
    let enabled = ui.is_enabled();
    let (rect, response) = ui.allocate_exact_size(vec2(MENU_WIDTH, 27.0), Sense::click());
    let painter = ui.painter();
    if enabled && response.hovered() {
        painter.rect_filled(rect, 5, ACCENT_SOFT);
    }
    paint_slot(
        painter,
        Rect::from_center_size(Pos2::new(rect.left() + 15.0, rect.center().y), vec2(16.0, 16.0)),
        enabled,
    );
    painter.text(
        Pos2::new(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        regular(13.0),
        if enabled { TEXT } else { FAINT },
    );
    if !shortcut.is_empty() {
        painter.text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            regular(11.5),
            if enabled { MUTED } else { FAINT },
        );
    }
    response
}

/// A menu entry with an icon and its shortcut on the right.
pub fn menu_item(ui: &mut Ui, icon: Option<Icon>, label: &str, shortcut: &str) -> Response {
    menu_row(ui, label, shortcut, |painter, slot, enabled| {
        if let Some(icon) = icon {
            let (ink, accent) = tones(enabled, false);
            icons::paint(painter, slot, icon, ink, accent);
        }
    })
}

/// A menu entry that opens a submenu, marked with an arrow.
pub fn menu_submenu(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let response = menu_row(ui, label, "", |painter, slot, enabled| {
        let (ink, accent) = tones(enabled, false);
        icons::paint(painter, slot, icon, ink, accent);
    });
    let arrow = Rect::from_center_size(
        Pos2::new(response.rect.right() - 14.0, response.rect.center().y),
        vec2(13.0, 13.0),
    );
    icons::paint(ui.painter(), arrow, Icon::ChevronRight, MUTED, MUTED);
    response
}

/// A menu entry that shows a check mark while `checked`.
pub fn menu_check(ui: &mut Ui, label: &str, shortcut: &str, checked: bool) -> Response {
    menu_row(ui, label, shortcut, |painter, slot, enabled| {
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
    let (rect, _) = ui.allocate_exact_size(vec2(MENU_WIDTH, 9.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.left() + 6.0, rect.center().y),
            Pos2::new(rect.right() - 6.0, rect.center().y),
        ],
        Stroke::new(1.0, BORDER),
    );
}

pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let response = ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(strong(13.0))
                .color(if enabled { Color32::WHITE } else { FAINT }),
        )
        .fill(if enabled { ACCENT } else { BAND })
        .stroke(Stroke::new(
            1.0,
            if enabled { ACCENT_STRONG } else { BORDER },
        ))
        .corner_radius(6)
        .min_size(vec2(0.0, 28.0)),
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

pub fn secondary_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(TEXT_2))
            .corner_radius(6)
            .min_size(vec2(0.0, 28.0)),
    )
}

pub fn danger_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(DANGER))
            .fill(Color32::from_rgb(0xFF, 0xF6, 0xF5))
            .stroke(Stroke::new(1.0, Color32::from_rgb(0xF0, 0xC4, 0xBF)))
            .corner_radius(6)
            .min_size(vec2(0.0, 28.0)),
    )
}

/// The title row of a side panel: caps title and an optional count.
pub fn panel_header(ui: &mut Ui, title: &str, trailing: &str) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    let painter = ui.painter();
    let galley = caps_galley(painter, title, 11.0, MUTED);
    painter.galley(
        Pos2::new(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
        galley,
        MUTED,
    );
    if !trailing.is_empty() {
        painter.text(
            Pos2::new(rect.right() - 44.0, rect.center().y),
            Align2::RIGHT_CENTER,
            trailing,
            regular(11.5),
            FAINT,
        );
    }
    rect
}

/// A full-width collapsible section of the properties panel.
pub fn section(ui: &mut Ui, id_salt: &str, title: &str, body: impl FnOnce(&mut Ui)) {
    let id = ui.make_persistent_id(("property_section", id_salt));
    let mut open = ui.data_mut(|data| *data.get_temp_mut_or(id, true));
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
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
    for y in [rect.top(), rect.bottom()] {
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0, BORDER),
        );
    }
    icons::paint(
        painter,
        Rect::from_center_size(Pos2::new(rect.left() + 14.0, rect.center().y), vec2(13.0, 13.0)),
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
        strong(12.5),
        TEXT,
    );
    if open {
        egui::Frame::new()
            .inner_margin(Margin {
                left: 14,
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
                ui.add(
                    egui::Label::new(RichText::new(text).size(12.0).color(TEXT_2)).wrap(),
                );
            });
        });
}
