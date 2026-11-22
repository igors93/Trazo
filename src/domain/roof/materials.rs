//! Roof tile types, NBR 7190 timber strength classes and Brazilian timber species.

use serde::{Deserialize, Serialize};

/// What covers the roof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileKind {
    #[default]
    Ceramic,
    Roman,
    American,
    Concrete,
    FiberCement,
    Metal,
}

impl TileKind {
    pub const ALL: [Self; 6] = [
        Self::Ceramic,
        Self::Roman,
        Self::American,
        Self::Concrete,
        Self::FiberCement,
        Self::Metal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ceramic => "Telha cerâmica",
            Self::Roman => "Telha cer\u{00e2}mica romana",
            Self::American => "Telha cer\u{00e2}mica americana",
            Self::Concrete => "Telha de concreto",
            Self::FiberCement => "Fibrocimento 6 mm",
            Self::Metal => "Telha metálica",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Ceramic => "ceramica",
            Self::Roman => "romana",
            Self::American => "americana",
            Self::Concrete => "concreto",
            Self::FiberCement => "fibrocimento",
            Self::Metal => "metalica",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase();
        let raw = raw.replace('â', "a").replace('é', "e").replace('á', "a");
        match raw.as_str() {
            "ceramica_romana" | "telha_romana" => Some(Self::Roman),
            "ceramica_americana" | "telha_americana" => Some(Self::American),
            _ => Self::ALL.into_iter().find(|t| t.key() == raw),
        }
    }

    /// Saturated/conservative weight on the roof surface, kN/m².
    /// Roman: 16 tiles/m² at about 2.7 kg each; American: manufacturer
    /// catalogues range from about 26 to 36 kg/m² dry, so 0.40 kN/m² keeps
    /// moisture and product variation in the preliminary model.
    pub fn weight_kn_m2(self) -> f64 {
        match self {
            Self::Ceramic | Self::Roman | Self::Concrete => 0.50,
            Self::American => 0.40,
            Self::FiberCement => 0.18,
            Self::Metal => 0.10,
        }
    }

    /// Usual minimum slope, percent (confirm with the manufacturer).
    pub fn min_slope_percent(self) -> f64 {
        match self {
            Self::Ceramic | Self::American | Self::Concrete => 30.0,
            Self::Roman => 35.0,
            Self::FiberCement => 15.0,
            Self::Metal => 10.0,
        }
    }

    /// Batten spacing (the tile gauge, *galga*), or `None` for sheets that
    /// rest on the purlins.
    pub fn gauge_m(self) -> Option<f64> {
        match self {
            Self::Ceramic => Some(0.35),
            Self::Roman => Some(0.33),
            Self::American => Some(0.36),
            Self::Concrete => Some(0.32),
            Self::FiberCement | Self::Metal => None,
        }
    }

    /// Largest purlin spacing a sheet spans, metres.
    pub fn sheet_span_m(self) -> Option<f64> {
        match self {
            Self::FiberCement => Some(1.69),
            Self::Metal => Some(2.0),
            Self::Ceramic | Self::Roman | Self::American | Self::Concrete => None,
        }
    }

    /// Tiles per m² of roof surface; `None` for sheets (counted in m²).
    pub fn per_m2(self) -> Option<f64> {
        match self {
            Self::Ceramic => Some(16.0),
            Self::Roman => Some(16.0),
            Self::American => Some(12.5),
            Self::Concrete => Some(10.5),
            Self::FiberCement | Self::Metal => None,
        }
    }
}

/// Timber strength class used by the preliminary NBR 7190 checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimberClass {
    C20,
    #[default]
    C30,
    C40,
    C60,
}

/// Species/catalogue entry specified for posts and their eave beams.  A
/// supplier may instead certify a lot directly to an NBR 7190 strength class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimberSpecies {
    #[serde(rename = "c20")]
    ClassC20,
    #[default]
    #[serde(rename = "c30")]
    ClassC30,
    #[serde(rename = "c40")]
    ClassC40,
    #[serde(rename = "c60")]
    ClassC60,
    #[serde(rename = "pinus_elliottii")]
    PinusElliottii,
    #[serde(rename = "eucalipto_citriodora")]
    EucalyptusCitriodora,
    #[serde(rename = "itauba")]
    Itauba,
    #[serde(rename = "cumaru")]
    Cumaru,
    #[serde(rename = "jatoba")]
    Jatoba,
}

impl TimberSpecies {
    pub const ALL: [Self; 9] = [
        Self::ClassC20,
        Self::ClassC30,
        Self::ClassC40,
        Self::ClassC60,
        Self::PinusElliottii,
        Self::EucalyptusCitriodora,
        Self::Itauba,
        Self::Cumaru,
        Self::Jatoba,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ClassC20 => "Lote classificado C20",
            Self::ClassC30 => "Lote classificado C30",
            Self::ClassC40 => "Lote classificado C40",
            Self::ClassC60 => "Lote classificado C60",
            Self::PinusElliottii => "Pinus elliottii",
            Self::EucalyptusCitriodora => "Eucalipto-citriodora",
            Self::Itauba => "Itaúba",
            Self::Cumaru => "Cumaru",
            Self::Jatoba => "Jatobá",
        }
    }

    pub fn scientific_name(self) -> &'static str {
        match self {
            Self::PinusElliottii => "Pinus elliottii",
            Self::EucalyptusCitriodora => "Corymbia citriodora",
            Self::Itauba => "Mezilaurus itauba",
            Self::Cumaru => "Dipteryx odorata",
            Self::Jatoba => "Hymenaea spp.",
            Self::ClassC20 | Self::ClassC30 | Self::ClassC40 | Self::ClassC60 => {
                "lote classificado"
            }
        }
    }

    /// Conservative strength class used for design. Named species still need
    /// the delivered lot classified; the mapping is a pre-sizing floor.
    pub fn class(self) -> TimberClass {
        match self {
            Self::ClassC20 | Self::PinusElliottii => TimberClass::C20,
            Self::ClassC30
            | Self::EucalyptusCitriodora
            | Self::Itauba
            | Self::Cumaru
            | Self::Jatoba => TimberClass::C30,
            Self::ClassC40 => TimberClass::C40,
            Self::ClassC60 => TimberClass::C60,
        }
    }

    /// Mechanical properties `(fc0,k, fv0,k, Ec0,m [MPa], ρ_ap [kg/m³])`.
    /// Conifers (Pinus elliottii) use NBR 7190:1997 Table 8 (`Ec0,m = 3,500 MPa`,
    /// `ρ = 500 kg/m³`) instead of the much stiffer Dicotyledon Table 9 C20.
    pub fn properties(self) -> (f64, f64, f64, f64) {
        match self {
            Self::PinusElliottii => (20.0, 4.0, 3_500.0, 500.0),
            other => other.class().properties(),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::ClassC20 => "c20",
            Self::ClassC30 => "c30",
            Self::ClassC40 => "c40",
            Self::ClassC60 => "c60",
            Self::PinusElliottii => "pinus_elliottii",
            Self::EucalyptusCitriodora => "eucalipto_citriodora",
            Self::Itauba => "itauba",
            Self::Cumaru => "cumaru",
            Self::Jatoba => "jatoba",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let key = raw
            .trim()
            .to_lowercase()
            .replace([' ', '-'], "_")
            .replace(['á', 'ã'], "a")
            .replace('ú', "u");
        Self::ALL.into_iter().find(|species| species.key() == key)
    }
}

impl TimberClass {
    pub const ALL: [Self; 4] = [Self::C20, Self::C30, Self::C40, Self::C60];

    pub fn label(self) -> &'static str {
        match self {
            Self::C20 => "Classe C20",
            Self::C30 => "Classe C30",
            Self::C40 => "Classe C40",
            Self::C60 => "Classe C60",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::C20 => "c20",
            Self::C30 => "c30",
            Self::C40 => "c40",
            Self::C60 => "c60",
        }
    }

    /// fc0,k, fv0,k and Ec0,m in MPa, apparent density in kg/m³
    /// (NBR 7190:1997, table 9).
    pub fn properties(self) -> (f64, f64, f64, f64) {
        match self {
            Self::C20 => (20.0, 4.0, 9_500.0, 650.0),
            Self::C30 => (30.0, 5.0, 14_500.0, 800.0),
            Self::C40 => (40.0, 6.0, 19_500.0, 950.0),
            Self::C60 => (60.0, 8.0, 24_500.0, 1_000.0),
        }
    }
}

/// kmod = kmod1 · kmod2 · kmod3 (long-term loads, service class 1–2,
/// second-grade sawn timber).
pub const KMOD: f64 = 0.70 * 1.0 * 0.80;

/// Design values of a timber class: fc0,d and fv0,d and Ec0,ef in kPa, and
/// the unit weight in kN/m³.
pub fn timber_design(timber: TimberClass) -> (f64, f64, f64, f64) {
    let (fc0k, fv0k, e0m, density) = timber.properties();
    (
        KMOD * fc0k / 1.4 * 1000.0,
        KMOD * fv0k / 1.8 * 1000.0,
        KMOD * e0m * 1000.0,
        density * 9.81 / 1000.0,
    )
}

/// Design values of a catalogued species or class (respecting Conifer Table 8
/// for Pinus elliottii).
pub fn timber_design_species(species: TimberSpecies) -> (f64, f64, f64, f64) {
    let (fc0k, fv0k, e0m, density) = species.properties();
    (
        KMOD * fc0k / 1.4 * 1000.0,
        KMOD * fv0k / 1.8 * 1000.0,
        KMOD * e0m * 1000.0,
        density * 9.81 / 1000.0,
    )
}
