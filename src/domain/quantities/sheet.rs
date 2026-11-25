//! Printable takeoff tables: wrap within each column and paginate without
//! dropping rows. SVG exports keep all sheets stacked; PDFs use separate A3 pages.

use std::fmt::Write;

use super::{ConstructionTakeoff, Project, ScheduleTable, esc, takeoff_tables_for};

const WIDTH: f64 = 1189.0;
const HEIGHT: f64 = 841.0;
const LEFT: f64 = 42.0;
const TABLE_WIDTH: f64 = 1105.0;
const BOTTOM: f64 = 782.0;
const FONT: f64 = 10.0;
const LINE: f64 = 14.0;
const PAD: f64 = 6.0;

/// Conservative Arial advances, including bold headers. Long words are also
/// split, so a specification or identifier cannot spill into its neighbour.
fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|ch| match ch {
            ' ' | '\t' => 0.36,
            'M' | 'W' | 'm' | '@' | '%' | '&' | 'Æ' | 'Œ' | 'æ' | 'œ' => 1.05,
            'w' => 0.9,
            ch if ch.is_ascii_digit() => 0.65,
            '?' => 0.7,
            ch if ch.is_ascii_punctuation() => 0.6,
            ch if ch.is_uppercase() && (ch as u32) < 0x250 => 0.85,
            ch if ch.is_alphabetic() && (ch as u32) < 0x250 => 0.7,
            _ => 1.4,
        } * size)
        .sum()
}

fn wrap(text: &str, width: f64, size: f64) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut advance = 0.0;
        for word in paragraph.split_whitespace() {
            let word_width = text_width(word, size);
            if !line.is_empty() && advance + text_width(" ", size) + word_width > width {
                lines.push(std::mem::take(&mut line));
                advance = 0.0;
            }
            if !line.is_empty() {
                line.push(' ');
                advance += text_width(" ", size);
            }
            for ch in word.chars() {
                let ch_width = text_width(&ch.to_string(), size);
                if !line.is_empty() && advance + ch_width > width {
                    lines.push(std::mem::take(&mut line));
                    advance = 0.0;
                }
                line.push(ch);
                advance += ch_width;
            }
        }
        // Preserve explicit line breaks, including empty cells.
        lines.push(line);
    }
    lines
}

fn column_widths(table: &ScheduleTable) -> Vec<f64> {
    let desired: Vec<f64> = table
        .headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            let widest = table
                .rows
                .iter()
                .chain(table.footer.iter())
                .filter_map(|row| row.get(i))
                .map(|cell| text_width(cell, FONT))
                .fold(text_width(header, FONT), f64::max);
            (widest + 2.0 * PAD).clamp(64.0, 280.0)
        })
        .collect();
    let total: f64 = desired.iter().sum();
    desired
        .into_iter()
        .map(|w| w * TABLE_WIDTH / total)
        .collect()
}

fn cells(row: &[String], widths: &[f64]) -> Vec<Vec<String>> {
    widths
        .iter()
        .enumerate()
        .map(|(i, width)| {
            wrap(
                row.get(i).map_or("", String::as_str),
                width - 2.0 * PAD,
                FONT,
            )
        })
        .collect()
}

fn line_count(row: &[Vec<String>]) -> usize {
    row.iter().map(Vec::len).max().unwrap_or(1)
}

struct Sheets {
    pages: Vec<String>,
    body: String,
    header: String,
    top: f64,
    y: f64,
}

impl Sheets {
    fn new(project: &Project, takeoff: &ConstructionTakeoff) -> Self {
        let mut sheet = Self {
            pages: Vec::new(),
            body: String::new(),
            header: String::new(),
            top: 0.0,
            y: 54.0,
        };
        sheet.text(LEFT, 54.0, 18.0, true, "Quantitativos de Obra");
        let scope = if takeoff.all_floors && takeoff.floors.len() > 1 {
            format!("Todos os pavimentos ({})", takeoff.floors.len())
        } else {
            takeoff.floors.first().map_or_else(
                || "Pavimento ativo".to_owned(),
                |f| format!("Pavimento: {}", f.floor_name),
            )
        };
        let label = format!("{} · {scope}", project.name);
        let mut y = 74.0;
        let label_lines = wrap(&label, TABLE_WIDTH, FONT);
        // The project name is metadata repeated on every sheet. Bound its
        // height so even an imported oversized name leaves space for rows.
        let _ = writeln!(sheet.body, "<title>{}</title>", esc(&label));
        for (index, line) in label_lines.iter().take(4).enumerate() {
            let line = if index == 3 && label_lines.len() > 4 {
                format!("{}…", line.trim_end())
            } else {
                line.clone()
            };
            sheet.text(LEFT, y, FONT, false, &line);
            y += LINE;
        }
        sheet.text(
            LEFT,
            y,
            FONT,
            false,
            &format!(
                "Perda de blocos {:.0}% · Argamassa/reboco +10% · Transpasse de vergas 30 cm",
                takeoff.waste_percent
            ),
        );
        sheet.header = std::mem::take(&mut sheet.body);
        sheet.top = y + 28.0;
        sheet.y = sheet.top;
        sheet
    }

    fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, value: &str) {
        let _ = writeln!(
            self.body,
            r##"<text x="{x:.2}" y="{y:.2}" font-family="Arial, sans-serif" font-size="{size}" font-weight="{}" fill="#1e293b">{}</text>"##,
            if bold { "700" } else { "400" },
            esc(value)
        );
    }

    fn page_break(&mut self) {
        self.pages.push(std::mem::take(&mut self.body));
        self.y = self.top;
    }

    fn row(
        &mut self,
        cells: &[Vec<String>],
        widths: &[f64],
        range: std::ops::Range<usize>,
        bold: bool,
        fill: &str,
    ) {
        let height = range.len() as f64 * LINE + 2.0 * PAD;
        let _ = writeln!(
            self.body,
            r##"<rect x="{LEFT}" y="{:.2}" width="{TABLE_WIDTH}" height="{height:.2}" fill="{fill}"/>"##,
            self.y
        );
        let mut x = LEFT;
        for (cell, width) in cells.iter().zip(widths) {
            for (i, line) in cell.iter().skip(range.start).take(range.len()).enumerate() {
                self.text(
                    x + PAD,
                    self.y + PAD + FONT + i as f64 * LINE,
                    FONT,
                    bold,
                    line,
                );
            }
            x += width;
        }
        self.y += height;
        let _ = writeln!(
            self.body,
            r##"<line x1="{LEFT}" y1="{:.2}" x2="{}" y2="{:.2}" stroke="#dbe3ec" stroke-width="0.6"/>"##,
            self.y,
            LEFT + TABLE_WIDTH,
            self.y
        );
    }

    fn table_header(&mut self, table: &ScheduleTable, widths: &[f64], continued: bool) {
        let title = if continued {
            format!("{} (continuação)", table.title)
        } else {
            table.title.clone()
        };
        self.text(LEFT, self.y + 14.0, 14.0, true, &title);
        self.y += 24.0;
        let header = cells(&table.headers, widths);
        self.row(&header, widths, 0..line_count(&header), true, "#e2e8f0");
    }

    fn table(&mut self, table: &ScheduleTable) {
        let widths = column_widths(table);
        let header_height =
            24.0 + line_count(&cells(&table.headers, &widths)) as f64 * LINE + 2.0 * PAD;
        let first_height = table
            .rows
            .first()
            .or(table.footer.as_ref())
            .map_or(0.0, |r| {
                (line_count(&cells(r, &widths)) as f64 * LINE + 2.0 * PAD)
                    .min(BOTTOM - self.top - header_height)
            });
        if self.y + header_height + first_height > BOTTOM {
            self.page_break();
        }
        self.table_header(table, &widths, false);
        for (index, row) in table.rows.iter().chain(table.footer.iter()).enumerate() {
            let footer = index == table.rows.len();
            let wrapped = cells(row, &widths);
            let lines = line_count(&wrapped);
            let full_height = lines as f64 * LINE + 2.0 * PAD;
            let page_capacity = BOTTOM - self.top - header_height;
            // Keep an ordinary row together; very long specifications continue
            // on another page with the table heading and columns repeated.
            if self.y + full_height > BOTTOM && full_height <= page_capacity {
                self.page_break();
                self.table_header(table, &widths, true);
            }
            let mut start = 0;
            while start < lines {
                let available = ((BOTTOM - self.y - 2.0 * PAD) / LINE).floor().max(0.0) as usize;
                if available == 0 {
                    self.page_break();
                    self.table_header(table, &widths, true);
                    continue;
                }
                let end = (start + available).min(lines);
                let fill = if footer {
                    "#edf2f7"
                } else if index % 2 == 0 {
                    "#ffffff"
                } else {
                    "#f8fafc"
                };
                self.row(&wrapped, &widths, start..end, footer, fill);
                start = end;
            }
        }
        self.y += 18.0;
    }

    fn finish(mut self) -> Vec<String> {
        self.page_break();
        let count = self.pages.len();
        self.pages.into_iter().enumerate().map(|(i, body)| {
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="420mm" height="297mm" viewBox="0 0 {WIDTH} {HEIGHT}">
<rect width="{WIDTH}" height="{HEIGHT}" fill="#ffffff"/>
<rect x="24" y="24" width="1141" height="793" fill="none" stroke="#1e293b" stroke-width="1.4"/>
{}
{body}
<line x1="24" y1="800" x2="1165" y2="800" stroke="#cbd5e1"/>
<text x="42" y="812" font-family="Arial, sans-serif" font-size="9" fill="#64748b">Quantitativos preliminares pelo eixo das paredes. Confira medidas e perdas antes do pedido de obra.</text>
<text x="1147" y="812" text-anchor="end" font-family="Arial, sans-serif" font-size="9" fill="#64748b">{} / {count}</text>
</svg>"##,
                self.header, i + 1
            )
        }).collect()
    }
}

/// Complete takeoff on A3 landscape pages, with repeated table headers.
pub fn takeoff_sheet_pages(project: &Project, all_floors: bool) -> Vec<String> {
    let takeoff = if all_floors {
        project.construction_takeoff_all_floors()
    } else {
        project.construction_takeoff()
    };
    let mut sheets = Sheets::new(project, &takeoff);
    for table in takeoff_tables_for(&takeoff) {
        sheets.table(&table);
    }
    sheets.finish()
}

/// All printable sheets in one SVG, stacked vertically with their page borders.
pub fn takeoff_sheet_svg(project: &Project, all_floors: bool) -> String {
    let pages = takeoff_sheet_pages(project, all_floors);
    if pages.len() == 1 {
        return pages.into_iter().next().unwrap_or_default();
    }
    let height = HEIGHT * pages.len() as f64;
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="420mm" height="{}mm" viewBox="0 0 {WIDTH} {height}">"#,
        297 * pages.len()
    );
    for (i, page) in pages.iter().enumerate() {
        if let Some((_, inner)) = page.split_once('>') {
            let _ = writeln!(
                svg,
                r#"<g transform="translate(0 {})">{}</g>"#,
                i as f64 * HEIGHT,
                inner.trim_end_matches("</svg>")
            );
        }
    }
    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{finishes::RoomFinishPreset, geometry::Point};

    fn assert_readable_pages(pages: &[String]) {
        use svg2pdf::usvg::{Group, Node, Rect};
        fn text_boxes(group: &Group, boxes: &mut Vec<Rect>) {
            for node in group.children() {
                match node {
                    Node::Group(group) => text_boxes(group, boxes),
                    Node::Text(text) => boxes.push(text.abs_bounding_box()),
                    _ => {}
                }
            }
        }
        let options = crate::io::export::svg2pdf_options_with_bundled_font();
        for (page, markup) in pages.iter().enumerate() {
            let tree = svg2pdf::usvg::Tree::from_str(markup, &options).unwrap();
            let scale = tree.size().width() / WIDTH as f32;
            let mut boxes = Vec::new();
            text_boxes(tree.root(), &mut boxes);
            assert!(!boxes.is_empty(), "The export must contain actual text");
            for (i, a) in boxes.iter().enumerate() {
                assert!(
                    a.left() >= 24.0 * scale && a.right() <= 1165.0 * scale,
                    "Text outside page {page}: {a:?}"
                );
                assert!(
                    a.top() >= 24.0 * scale && a.bottom() <= 817.0 * scale,
                    "Text outside page {page}: {a:?}"
                );
                for b in &boxes[i + 1..] {
                    let dx = a.right().min(b.right()) - a.left().max(b.left());
                    let dy = a.bottom().min(b.bottom()) - a.top().max(b.top());
                    assert!(
                        dx <= 0.05 || dy <= 0.05,
                        "Overlapping text on page {page}: {a:?} / {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn long_descriptions_and_nine_column_headers_do_not_overlap() {
        let mut project = Project::default();
        project
            .add_room(
                Point::new(0.0, 0.0),
                Point::new(12.0, 8.0),
                "Ambiente de convivência e refeições",
            )
            .unwrap();
        project
            .apply_room_finish_preset(project.rooms[0].id, RoomFinishPreset::SocialPorcelanato)
            .unwrap();
        let mut takeoff = project.construction_takeoff();
        for wall in &mut takeoff.walls {
            wall.masonry =
                "Tijolo cerâmico 8 furos 9 × 19 × 19 cm - especificação completa & acabamento"
                    .to_owned();
        }
        let mut sheets = Sheets::new(&project, &takeoff);
        for table in takeoff_tables_for(&takeoff) {
            sheets.table(&table);
        }
        let pages = sheets.finish();
        assert_readable_pages(&pages);
        let markup = pages.join("");
        assert!(markup.contains("especificação completa"));
        assert!(markup.contains("&amp; acabamento"));
        assert!(markup.contains("Paredes"));
        assert!(markup.contains("+10%"));
    }

    #[test]
    fn pagination_keeps_every_row_footer_and_repeats_headers() {
        let project = Project::default();
        let takeoff = project.construction_takeoff();
        let mut sheets = Sheets::new(&project, &takeoff);
        let table = ScheduleTable {
            title: "Alvenaria por parede".into(),
            headers: [
                "Parede",
                "Alvenaria",
                "L × H",
                "Líq. (m²)",
                "Blocos",
                "Meios",
                "Canaletas",
                "Argam. (m³)",
                "Reboco (m²)",
            ]
            .map(String::from)
            .to_vec(),
            rows: (0..95)
                .map(|i| {
                    vec![
                        format!("ROW_{i:03}"),
                        "Tijolo cerâmico 8 furos com acabamento especial".into(),
                        "50,00 × 2,50 m".into(),
                        "124,64".into(),
                        "2859".into(),
                        "14".into(),
                        "250".into(),
                        "1,537".into(),
                        "249,28".into(),
                    ]
                })
                .collect(),
            footer: Some(vec!["TOTAL_FINAL".into(), "95 paredes".into()]),
        };
        sheets.table(&table);
        let pages = sheets.finish();
        assert!(pages.len() >= 4);
        for page in &pages {
            assert!(page.contains("Alvenaria por parede"));
            assert!(page.contains("Argam. (m³)"));
        }
        let markup = pages.join("");
        for i in 0..95 {
            assert_eq!(markup.matches(&format!("ROW_{i:03}")).count(), 1);
        }
        assert_eq!(markup.matches("TOTAL_FINAL").count(), 1);
        assert!(pages.last().unwrap().contains("TOTAL_FINAL"));
        assert_readable_pages(&pages);
    }

    #[test]
    fn oversized_unicode_cells_continue_without_losing_characters() {
        let value = "Áçã界MW123".repeat(180);
        let wrapped = wrap(&value, 80.0, FONT);
        assert_eq!(wrapped.concat(), value);
        assert!(wrapped.iter().all(|line| text_width(line, FONT) <= 80.0));
        let project = Project::default();
        let mut sheets = Sheets::new(&project, &project.construction_takeoff());
        sheets.table(&ScheduleTable {
            title: "Especificações".into(),
            headers: vec!["Código".into(), "Descrição".into()],
            rows: vec![vec!["P1".into(), "ÁçãMW123&ÆŒ".repeat(900)]],
            footer: Some(vec!["Última linha".into()]),
        });
        let pages = sheets.finish();
        assert!(pages.len() > 1);
        assert!(pages.last().unwrap().contains("Última linha"));
        assert_readable_pages(&pages);
    }
}
