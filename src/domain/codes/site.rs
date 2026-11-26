//! Shared site & climate parameters (T-09): city/UF, coordinates, NBR 15220
//! bioclimatic zone, NBR 6123 basic wind speed `V0`, NBR 10844 rainfall
//! intensity `I`, NBR 5410 keraunic level (`AQ1`/`AQ2`/`AQ3`) and design
//! temperatures, with per-field normative provenance and editable Brazilian
//! city presets.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::data::builtin_site_presets;

/// Atmospheric discharge / thunderstorm exposure class (ABNT NBR 5410 Tabela 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeraunicLevel {
    /// Desprezível (≤ 25 dias/ano, sem influência externa relevante).
    #[serde(rename = "AQ1", alias = "aq1")]
    Aq1,
    /// Indireta (≤ 25 dias/ano com exposição indireta).
    #[serde(rename = "AQ2", alias = "aq2")]
    Aq2,
    /// Direta (> 25 dias de trovoada/ano — exige DPS na entrada).
    #[serde(rename = "AQ3", alias = "aq3")]
    Aq3,
}

impl KeraunicLevel {
    pub const ALL: [KeraunicLevel; 3] = [Self::Aq1, Self::Aq2, Self::Aq3];

    pub fn code(self) -> &'static str {
        match self {
            Self::Aq1 => "AQ1",
            Self::Aq2 => "AQ2",
            Self::Aq3 => "AQ3",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Aq1 => "AQ1 — Desprezível (≤ 25 dias/ano)",
            Self::Aq2 => "AQ2 — Indireta (≤ 25 dias/ano)",
            Self::Aq3 => "AQ3 — Direta (> 25 dias/ano · DPS obrigatório)",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "AQ1" => Some(Self::Aq1),
            "AQ2" => Some(Self::Aq2),
            "AQ3" => Some(Self::Aq3),
            _ => None,
        }
    }
}

/// Shared project site & climate configuration across all disciplines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteClimate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_uf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude_m: Option<f64>,
    /// Bioclimatic zone 1..=8 (ABNT NBR 15220-3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bioclimatic_zone: Option<u8>,
    /// Basic wind velocity `V0` in m/s (ABNT NBR 6123).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wind_v0_ms: Option<f64>,
    /// Design rainfall intensity `I` in mm/h (ABNT NBR 10844).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rain_intensity_mm_h: Option<f64>,
    /// Rainfall return period `T` in years (default 5 years for roofs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rain_return_period_years: Option<f64>,
    /// Keraunic exposure level AQ1/AQ2/AQ3 (ABNT NBR 5410 / DPS / SPDA).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thunderstorms: Option<KeraunicLevel>,
    /// Thunderstorm days per year (`Td`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thunderstorm_days_year: Option<f64>,
    /// Summer cooling design temperature (°C).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooling_design_c: Option<f64>,
    /// Winter heating design temperature (°C).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heating_design_c: Option<f64>,
    /// Per-field normative provenance (field name -> standard / table / note).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, String>,
}

/// Built-in regional city preset loaded from `data/codes/site_presets_br.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitePreset {
    pub id: String,
    pub city: String,
    pub state_uf: String,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub bioclimatic_zone: u8,
    pub wind_v0_ms: f64,
    pub rain_intensity_mm_h: f64,
    pub rain_return_period_years: f64,
    pub thunderstorms: KeraunicLevel,
    pub thunderstorm_days_year: f64,
    pub cooling_design_c: f64,
    pub heating_design_c: f64,
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
}

impl SitePreset {
    pub fn display_name(&self) -> String {
        format!("{} / {}", self.city, self.state_uf)
    }

    pub fn to_site_climate(&self) -> SiteClimate {
        SiteClimate {
            preset_id: Some(self.id.clone()),
            city: Some(self.city.clone()),
            state_uf: Some(self.state_uf.clone()),
            latitude: Some(self.latitude),
            longitude: Some(self.longitude),
            altitude_m: Some(self.altitude_m),
            bioclimatic_zone: Some(self.bioclimatic_zone),
            wind_v0_ms: Some(self.wind_v0_ms),
            rain_intensity_mm_h: Some(self.rain_intensity_mm_h),
            rain_return_period_years: Some(self.rain_return_period_years),
            thunderstorms: Some(self.thunderstorms),
            thunderstorm_days_year: Some(self.thunderstorm_days_year),
            cooling_design_c: Some(self.cooling_design_c),
            heating_design_c: Some(self.heating_design_c),
            sources: self.sources.clone(),
        }
    }
}

/// Bounds for sanity-checking user input (physical limits, not building codes).
const LAT_MIN: f64 = -90.0;
const LAT_MAX: f64 = 90.0;
const LON_MIN: f64 = -180.0;
const LON_MAX: f64 = 180.0;
const ALT_MIN: f64 = -500.0;
const ALT_MAX: f64 = 9_000.0;
const WIND_MIN: f64 = 0.0;
const WIND_MAX: f64 = 150.0;
const RAIN_MIN: f64 = 0.0;
const RAIN_MAX: f64 = 2_000.0;
const RETURN_PERIOD_MAX: f64 = 10_000.0;
const TEMP_MIN: f64 = -80.0;
const TEMP_MAX: f64 = 70.0;
const DAYS_YEAR_MAX: f64 = 366.0;
const MAX_TEXT_LEN: usize = 64;
const MAX_SOURCE_LEN: usize = 500;

impl SiteClimate {
    pub fn is_empty(&self) -> bool {
        self.preset_id.is_none()
            && self.city.is_none()
            && self.state_uf.is_none()
            && self.latitude.is_none()
            && self.longitude.is_none()
            && self.altitude_m.is_none()
            && self.bioclimatic_zone.is_none()
            && self.wind_v0_ms.is_none()
            && self.rain_intensity_mm_h.is_none()
            && self.rain_return_period_years.is_none()
            && self.thunderstorms.is_none()
            && self.thunderstorm_days_year.is_none()
            && self.cooling_design_c.is_none()
            && self.heating_design_c.is_none()
            && self.sources.is_empty()
    }

    pub fn location_label(&self) -> Option<String> {
        match (&self.city, &self.state_uf) {
            (Some(c), Some(uf)) if !c.trim().is_empty() && !uf.trim().is_empty() => {
                Some(format!("{} / {}", c.trim(), uf.trim()))
            }
            (Some(c), _) if !c.trim().is_empty() => Some(c.trim().to_owned()),
            _ => None,
        }
    }

    /// Look up a bundled Brazilian city preset by `id` or city name (case-insensitive).
    pub fn find_preset(query: &str) -> Option<&'static SitePreset> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return None;
        }
        builtin_site_presets().iter().find(|p| {
            p.id.to_ascii_lowercase() == q
                || p.city.to_ascii_lowercase() == q
                || p.display_name().to_ascii_lowercase() == q
        })
    }

    /// Validate all physical ranges and text lengths.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(c) = &self.city
            && c.chars().count() > MAX_TEXT_LEN
        {
            return Err("Nome da cidade excede 64 caracteres.".to_owned());
        }
        if let Some(uf) = &self.state_uf
            && uf.chars().count() > MAX_TEXT_LEN
        {
            return Err("UF excede 64 caracteres.".to_owned());
        }
        check_range("Latitude", self.latitude, LAT_MIN, LAT_MAX)?;
        check_range("Longitude", self.longitude, LON_MIN, LON_MAX)?;
        check_range("Altitude", self.altitude_m, ALT_MIN, ALT_MAX)?;
        if let Some(zb) = self.bioclimatic_zone
            && !(1..=8).contains(&zb)
        {
            return Err("Zona bioclimática (NBR 15220) deve estar entre 1 e 8.".to_owned());
        }
        check_range(
            "Velocidade básica do vento V0",
            self.wind_v0_ms,
            WIND_MIN,
            WIND_MAX,
        )?;
        check_range(
            "Intensidade pluviométrica I",
            self.rain_intensity_mm_h,
            RAIN_MIN,
            RAIN_MAX,
        )?;
        check_range(
            "Período de retorno da chuva",
            self.rain_return_period_years,
            0.0,
            RETURN_PERIOD_MAX,
        )?;
        check_range(
            "Dias de trovoada por ano",
            self.thunderstorm_days_year,
            0.0,
            DAYS_YEAR_MAX,
        )?;
        check_range(
            "Temperatura de verão",
            self.cooling_design_c,
            TEMP_MIN,
            TEMP_MAX,
        )?;
        check_range(
            "Temperatura de inverno",
            self.heating_design_c,
            TEMP_MIN,
            TEMP_MAX,
        )?;
        for (k, v) in &self.sources {
            if k.chars().count() > MAX_TEXT_LEN || v.chars().count() > MAX_SOURCE_LEN {
                return Err("Fonte normativa de sítio excede o limite de tamanho.".to_owned());
            }
        }
        Ok(())
    }

    /// Plain-language summary line for the Safety assumptions list and reports.
    pub fn assumption_line(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(loc) = self.location_label() {
            parts.push(loc);
        }
        if let Some(zb) = self.bioclimatic_zone {
            parts.push(format!("ZB{zb} (NBR 15220)"));
        }
        if let Some(v0) = self.wind_v0_ms {
            parts.push(format!("V0 = {v0:.0} m/s (NBR 6123)"));
        }
        if let Some(rain) = self.rain_intensity_mm_h {
            parts.push(format!("I = {rain:.0} mm/h (NBR 10844)"));
        }
        if let Some(aq) = self.thunderstorms {
            parts.push(format!("Ceráunico {} (NBR 5410)", aq.code()));
        }
        if parts.is_empty() {
            None
        } else {
            Some(format!("Local e clima: {}.", parts.join(" · ")))
        }
    }
}

fn check_range(label: &str, value: Option<f64>, min: f64, max: f64) -> Result<(), String> {
    if let Some(v) = value
        && (!v.is_finite() || v < min || v > max)
    {
        return Err(format!("{label} deve estar entre {min} e {max}."));
    }
    Ok(())
}
