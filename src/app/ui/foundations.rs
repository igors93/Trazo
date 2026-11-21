//! Foundation commands in the interface: balance beams for boundary
//! footings (#144).

use super::super::FloorPlanApp;

impl FloorPlanApp {
    /// Gives every ground-floor wall a ground beam or a strip footing.
    pub(crate) fn generate_wall_foundations(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let (beams, strips) = self.project.generate_wall_foundations();
        if beams + strips == 0 {
            self.message =
                "Todas as paredes do térreo já têm fundação (ou você está num pavimento superior)."
                    .to_owned();
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "Fundação das paredes: {beams} viga(s) baldrame e {strips} sapata(s) corrida(s). Use Ajustar sapatas para incluir o peso das paredes."
            );
        }
    }

    /// Lays a balance beam from every off-centre boundary footing to the
    /// nearest interior column.
    pub(crate) fn generate_balance_beams(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let (added, alone) = self.project.generate_balance_beams();
        if added == 0 {
            self.message = if alone > 0 {
                format!(
                    "{alone} pilar(es) de divisa sem pilar interno a até 10 m para ancorar uma viga alavanca."
                )
            } else {
                "Nenhuma sapata de divisa fora do centro do pilar (ou todas já têm viga alavanca)."
                    .to_owned()
            };
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "{added} viga(s) alavanca lançada(s): as sapatas de divisa ficam centradas.{}",
                if alone > 0 {
                    format!(" {alone} pilar(es) de divisa ficaram sem pilar interno para ancorar.")
                } else {
                    String::new()
                }
            );
        }
    }
}
