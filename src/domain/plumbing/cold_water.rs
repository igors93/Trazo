//! Cold water (#118, NBR 5626): how much water the family uses in a day,
//! the water tank that keeps it, the pipes from the tank to every fixture
//! and the pressure left at each one, so a weak shower shows up before the
//! tank's base is built.

use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

use super::network::{GRAVITY, Network, Node, Pressure, SHOWER_COMFORT_M, build_network};
use super::points::{PlumbingPoint, PointKind, plumbing_points};
use super::rules::{ColdWaterRules, FixtureTable, ReservoirRules};

/// Water a person uses in a day when nobody says otherwise, L: the usual
/// figure for a house (NBR 5626 leaves it to the designer).
pub const DEFAULT_CONSUMPTION_L: f64 = 150.0;
/// People per bedroom when the family's size is not given.
pub const PEOPLE_PER_BEDROOM: u32 = 2;
/// The tank's base over the top of the walls when nobody says: the slab
/// (10 cm) and a low base (10 cm), m.
pub const TANK_OVER_WALLS_M: f64 = 0.20;
/// Weight of the empty tank, its lid and fittings, kN (as the structure's
/// default water-tank load assumes).
const TANK_SELF_KN: f64 = 1.0;
/// Commercial water tanks, L.
const TANKS_L: [f64; 12] = [
    250.0, 310.0, 500.0, 750.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 10_000.0, 15_000.0,
    20_000.0,
];

/// Where the tank's position comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TankPlace {
    /// Given in the plumbing settings.
    Set,
    /// The water tank of the structural design.
    Structure,
    /// Chosen by Trazo over the wet rooms.
    Trazo,
}

/// The cold-water design.
#[derive(Clone, Debug, PartialEq)]
pub struct ColdWater {
    pub residents: u32,
    /// Whether the number of residents was given (or counted by bedroom).
    pub residents_given: bool,
    pub bedrooms: u32,
    /// Water per person per day, L.
    pub per_person_l: f64,
    pub daily_l: f64,
    pub reserve_days: f64,
    /// Water to keep, L.
    pub reserve_l: f64,
    /// The commercial tank that keeps it, L.
    pub tank_l: f64,
    /// The full tank's weight on the structure, kN.
    pub tank_full_kn: f64,
    pub tank_at: Point,
    pub tank_place: TankPlace,
    /// Height of the tank's base (its outlet) over the floor, m.
    pub tank_base_m: f64,
    pub tank_base_given: bool,
    pub network: Network,
    /// The base height that gives every fixture its least pressure, m.
    pub needed_base_m: f64,
    /// … and every shower a good bath, m.
    pub comfort_base_m: f64,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

impl ColdWater {
    /// The outlets whose pressure is not good.
    #[allow(dead_code)]
    pub fn problems(&self) -> impl Iterator<Item = &super::network::Outlet> {
        self.network
            .outlets
            .iter()
            .filter(|o| o.state != Pressure::Good)
    }
}

/// Bedrooms, by their names.
pub fn bedroom_count(project: &Project) -> u32 {
    project
        .rooms
        .iter()
        .filter(|room| {
            let name = room.display_name().to_lowercase();
            ["quarto", "suíte", "suite", "dormit"]
                .iter()
                .any(|w| name.contains(w))
                && !["banheiro", "wc", "lavabo", "closet", "banho"]
                    .iter()
                    .any(|w| name.contains(w))
        })
        .count() as u32
}

/// The smallest commercial tank that keeps `litres`.
pub fn commercial_tank(litres: f64) -> f64 {
    TANKS_L
        .iter()
        .copied()
        .find(|t| *t >= litres - 1e-9)
        .unwrap_or_else(|| (litres / 1000.0).ceil() * 1000.0)
}

/// The full weight of a tank of `litres`, kN.
pub fn tank_full_kn(litres: f64) -> f64 {
    litres * GRAVITY / 1000.0 + TANK_SELF_KN
}

/// The cold-water design, or `None` when no fixture takes water.
pub fn cold_water(project: &Project) -> Option<ColdWater> {
    let points = plumbing_points(project);
    let cold: Vec<&PlumbingPoint> = points
        .points
        .iter()
        .filter(|p| p.kind == PointKind::Cold)
        .collect();
    if cold.is_empty() {
        return None;
    }
    let rules = ColdWaterRules::for_project(project);
    let reservoir = ReservoirRules::for_project(project);
    let table = FixtureTable::for_project(project);
    let settings = &project.plumbing;

    let bedrooms = bedroom_count(project);
    let residents = settings
        .residents
        .filter(|n| *n > 0)
        .unwrap_or((PEOPLE_PER_BEDROOM * bedrooms).max(PEOPLE_PER_BEDROOM));
    let per_person_l = settings
        .consumption_l_day
        .filter(|c| *c > 0.0)
        .unwrap_or(DEFAULT_CONSUMPTION_L);
    let reserve_days = settings
        .reserve_days
        .unwrap_or(reservoir.min_days)
        .clamp(reservoir.min_days, reservoir.max_days);
    let daily_l = f64::from(residents) * per_person_l;
    let reserve_l = daily_l * reserve_days;
    let tank_l = commercial_tank(reserve_l);
    let full_kn = tank_full_kn(tank_l);

    let tank_fixture = project
        .fixtures
        .iter()
        .find(|f| f.kind == FixtureKind::WaterTank);
    let (tank_at, tank_place) = match (settings.tank_at, tank_fixture) {
        (Some(at), _) => (at, TankPlace::Set),
        (None, Some(tank)) => (tank.center, TankPlace::Structure),
        (None, None) => {
            let n = cold.len() as f64;
            let sum = cold.iter().fold(Point::new(0.0, 0.0), |acc, p| {
                Point::new(acc.x + p.at.x, acc.y + p.at.y)
            });
            (Point::new(sum.x / n, sum.y / n), TankPlace::Trazo)
        }
    };
    let tank_base_m = settings
        .tank_base_m
        .unwrap_or(project.wall_height + TANK_OVER_WALLS_M);
    let network = build_network(&cold, Node::new(tank_at, tank_base_m), "AF", &table, &rules);
    let needed_base_m = network.needed_source_z(false);
    let comfort_base_m = network.needed_source_z(true);

    let mut warnings = Vec::new();
    let place = |o: &super::network::Outlet| match &o.room {
        Some(room) => format!("{} ({room})", o.fixture.label()),
        None => o.fixture.label().to_owned(),
    };
    for outlet in &network.outlets {
        match outlet.state {
            Pressure::Low => warnings.push(format!(
                "{}: {} m de pressão, abaixo do mínimo da NBR 5626 ({} m): a água chega fraca demais.",
                place(outlet),
                decimal(outlet.pressure_m.max(0.0), 2),
                decimal(network.min_m, 2)
            )),
            Pressure::Weak => warnings.push(format!(
                "{}: {} m de pressão; o chuveiro funciona, mas fraco (bom a partir de {} m).",
                place(outlet),
                decimal(outlet.pressure_m, 2),
                decimal(SHOWER_COMFORT_M, 1)
            )),
            Pressure::Good => {}
        }
    }
    if needed_base_m > tank_base_m + 1e-6 {
        warnings.push(format!(
            "Suba a base da caixa-d'água para {} m do piso ({} m acima do previsto), numa torre ou base de alvenaria calculada para o peso dela, ou ponha um pressurizador nos pontos fracos.",
            decimal(needed_base_m, 2),
            decimal(needed_base_m - tank_base_m, 2)
        ));
    } else if comfort_base_m > tank_base_m + 1e-6 {
        warnings.push(format!(
            "Para um banho forte, a base da caixa-d'água precisa ficar a {} m do piso ({} m acima do previsto).",
            decimal(comfort_base_m, 2),
            decimal(comfort_base_m - tank_base_m, 2)
        ));
    }
    match tank_fixture.and_then(|t| t.structural_load_kn.or(t.kind.default_structural_load_kn())) {
        Some(load) if load + 0.05 < full_kn => warnings.push(format!(
            "A caixa-d'água de {} L cheia pesa {} kN, mas a estrutura foi conferida para {} kN: corrija a carga da caixa no projeto estrutural.",
            decimal(tank_l, 0),
            decimal(full_kn, 1),
            decimal(load, 1)
        )),
        Some(_) => {}
        None => warnings.push(format!(
            "A caixa-d'água ({} L, {} kN cheia) não está no projeto estrutural: coloque-a sobre a laje ou a base para que o peso dela seja conferido.",
            decimal(tank_l, 0),
            decimal(full_kn, 1)
        )),
    }

    let mut notes = vec![format!(
        "{residents} morador(es) × {} L por dia = {} L/dia; reserva de {} dia(s) = {} L: caixa de {} L.",
        decimal(per_person_l, 0),
        decimal(daily_l, 0),
        decimal(reserve_days, 1),
        decimal(reserve_l, 0),
        decimal(tank_l, 0)
    )];
    if settings.residents.is_none() {
        notes.push(format!(
            "Moradores estimados em {PEOPLE_PER_BEDROOM} por quarto ({bedrooms} quarto(s)); informe o número da família para refazer a conta."
        ));
    }
    if reserve_days < 2.0 {
        notes.push(format!(
            "Onde a água da rua falta com frequência, guarde 2 dias: {} L.",
            decimal(daily_l * 2.0, 0)
        ));
    }
    match tank_place {
        TankPlace::Trazo => notes.push(
            "Posição da caixa escolhida pelo Trazo, sobre as áreas molhadas; ajuste-a se ela ficar em outro lugar."
                .to_owned(),
        ),
        TankPlace::Structure => {
            notes.push("Caixa na posição da caixa-d'água do projeto estrutural.".to_owned())
        }
        TankPlace::Set => {}
    }
    if settings.tank_base_m.is_none() {
        notes.push(format!(
            "Base da caixa suposta a {} m do piso (laje sobre as paredes de {} m e uma base baixa).",
            decimal(tank_base_m, 2),
            decimal(project.wall_height, 2)
        ));
    }
    notes.push(
        "Vazão provável Q = 0,3·√ΣP; tubos de PVC soldável com velocidade até 3 m/s; perdas por Darcy-Weisbach com os comprimentos equivalentes das conexões e registros."
            .to_owned(),
    );

    Some(ColdWater {
        residents,
        residents_given: settings.residents.is_some(),
        bedrooms,
        per_person_l,
        daily_l,
        reserve_days,
        reserve_l,
        tank_l,
        tank_full_kn: full_kn,
        tank_at,
        tank_place,
        tank_base_m,
        tank_base_given: settings.tank_base_m.is_some(),
        network,
        needed_base_m,
        comfort_base_m,
        warnings,
        notes,
    })
}

pub(crate) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}
