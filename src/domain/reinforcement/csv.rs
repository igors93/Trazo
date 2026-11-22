//! Brazilian-style semicolon-delimited steel bar schedule CSV exporter.

use super::{ReinforcementReport, decimal, diameter_text};

pub fn steel_schedule_csv(report: &ReinforcementReport) -> String {
    let mut csv = String::from(
        "Marca;Elemento;Posição;Forma;Diâmetro (mm);Quantidade;Comprimento unitário (m);Comprimento total (m);Massa (kg)\n",
    );
    for row in &report.schedule {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{};{}\n",
            row.mark,
            row.element,
            row.position,
            row.shape,
            diameter_text(row.diameter_mm),
            row.quantity,
            decimal(row.unit_length_m, 2),
            decimal(row.total_length_m, 2),
            decimal(row.mass_kg, 1)
        ));
    }
    csv.push_str(&format!(
        ";;;;;;;Massa total teórica (kg);{}\n",
        decimal(report.total_mass_kg(), 1)
    ));
    csv.push_str(&format!(
        ";;;;;;;Massa total +10% perda (kg);{}\n",
        decimal(report.total_mass_with_waste_kg(), 1)
    ));
    csv.push_str(&format!(
        ";;;;;;;Total barras comerciais 12 m;{}\n",
        report.total_bars_12m()
    ));
    if !report.by_diameter.is_empty() {
        csv.push_str("\nResumo por bitola (CA-50)\n");
        csv.push_str(
            "Bitola (mm);Comprimento total (m);Barras de 12 m (+10%);Massa teórica (kg);Massa +10% perda (kg)\n",
        );
        for row in &report.by_diameter {
            csv.push_str(&format!(
                "{};{};{};{};{}\n",
                diameter_text(row.diameter_mm),
                decimal(row.total_length_m, 2),
                row.bars_12m,
                decimal(row.mass_kg, 1),
                decimal(row.mass_with_waste_kg, 1)
            ));
        }
    }
    csv.push_str(
        "\nCaráter;Estimativa preliminar NBR 6118:2023 (B12 Branson, ancoragem §9.4 e traspasse §9.5) - auditoria da edição 2026 e revisão por engenheiro habilitado obrigatórias\n",
    );
    csv
}
