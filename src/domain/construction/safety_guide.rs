//! Site safety and permits guide (#138): per-stage NR-18 / NR-10 / NR-35 /
//! NR-06 safety checklists tailored to the project's real geometry and risks
//! (excavations, shoring, scaffolding, roof work at height, electrical
//! lock-out, PPE), Brazilian permits & professional responsibility guide
//! (ART/RRT, *alvará*, CNO/INSS, *habite-se*, ATHIS Lei 11.888/2008, Defesa
//! Civil 199), printable A3 SVG and the one-click M9 construction bundle PDF.

use std::fmt::Write as _;
use std::path::Path;

use crate::domain::{
    construction::{
        cost::cost_estimate_svg,
        materials::materials_catalogue_svg,
        schedule::construction_schedule_gantt_svg,
        shopping::{ConstructionStage, stage_shopping_list_svg},
    },
    model::Project,
    protection::help::HELP_SENTENCE,
};

/// Severity/priority of a safety checklist item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafetyPriority {
    /// Life-safety / mandatory NR stop condition (trench shoring, NR-35, NR-10, 21d shoring).
    Critical,
    /// Required site practice (NR-18 / NR-06 PPE / workmanship control).
    Required,
}

impl SafetyPriority {
    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "Crítico (risco de vida)",
            Self::Required => "Obrigatório",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Critical => "critico",
            Self::Required => "obrigatorio",
        }
    }
}

/// One checkable site-safety item belonging to a construction stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafetyCheckItem {
    pub id: String,
    pub stage: ConstructionStage,
    pub norm: String,
    pub priority: SafetyPriority,
    pub text: String,
    pub checked: bool,
}

/// All safety requirements and PPE for one construction stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageSafetyChecklist {
    pub stage: ConstructionStage,
    pub ppe: Vec<String>,
    pub items: Vec<SafetyCheckItem>,
}

/// One permit, professional registration or legal step required for the work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermitRequirement {
    pub id: String,
    pub title: String,
    pub authority: String,
    pub when_needed: String,
    pub why_it_matters: String,
    pub mandatory: bool,
}

/// Complete site safety & permits guide for a project (#138).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SiteSafetyAndPermitsGuide {
    pub stages: Vec<StageSafetyChecklist>,
    pub permits: Vec<PermitRequirement>,
    pub site_alerts: Vec<String>,
    pub help_guidance: String,
    pub total_checks: usize,
    pub completed_checks: usize,
}

/// Toggles or sets one safety checklist item in `project.construction.checked_safety_items`.
pub fn set_safety_check(
    project: &mut Project,
    item_id: &str,
    checked: bool,
) -> Result<String, String> {
    let guide = build_safety_and_permits_guide(project);
    let exists = guide
        .stages
        .iter()
        .flat_map(|s| s.items.iter())
        .any(|i| i.id == item_id);
    if !exists {
        return Err(format!(
            "Item de segurança '{item_id}' não encontrado no guia de obra."
        ));
    }
    if checked {
        project
            .construction
            .checked_safety_items
            .insert(item_id.to_owned());
        Ok(format!(
            "Item de segurança '{item_id}' marcado como conferido."
        ))
    } else {
        project.construction.checked_safety_items.remove(item_id);
        Ok(format!("Item de segurança '{item_id}' desmarcado."))
    }
}

/// Builds the complete per-stage NR-18 safety checklists and Brazilian permits guide (#138).
pub fn build_safety_and_permits_guide(project: &Project) -> SiteSafetyAndPermitsGuide {
    let checked = &project.construction.checked_safety_items;
    let floors = project.floors.levels.len().max(1);
    let max_wall_h = project
        .walls
        .iter()
        .map(|w| {
            w.height_m
                .unwrap_or(project.wall_height)
                .max(w.height_end_m.unwrap_or(project.wall_height))
        })
        .fold(project.wall_height, f64::max);
    let base_depth = project.footing_base_depth_m();
    let has_roof = !project.structure.roofs.is_empty();
    let has_slabs = !project.structure.slabs.is_empty();
    let total_height_m = max_wall_h * (floors as f64);

    let item =
        |id: &str, stage: ConstructionStage, norm: &str, priority: SafetyPriority, text: String| {
            SafetyCheckItem {
                id: id.to_owned(),
                stage,
                norm: norm.to_owned(),
                priority,
                text,
                checked: checked.contains(id),
            }
        };

    let stages = vec![
        // Stage 1: Site & setting out
        StageSafetyChecklist {
            stage: ConstructionStage::SiteAndSettingOut,
            ppe: vec![
                "Capacete classe B com jugular".to_owned(),
                "Botina de segurança com biqueira".to_owned(),
                "Luvas de vaqueta / raspa".to_owned(),
                "Óculos de proteção".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s1_isolamento",
                    ConstructionStage::SiteAndSettingOut,
                    "NR-18.4 / NR-18.14",
                    SafetyPriority::Required,
                    "Isolar o terreno com tapume/tela, impedir acesso de crianças e afixar placa da obra com ART/RRT.".to_owned(),
                ),
                item(
                    "nr18_s1_rede_eletrica",
                    ConstructionStage::SiteAndSettingOut,
                    "NR-10 / NR-18.6",
                    SafetyPriority::Critical,
                    "Verificar distância segura da rede elétrica aérea da rua antes de manusear barras de aço, réguas ou madeiras compridas.".to_owned(),
                ),
                item(
                    "nr18_s1_quadro_provisorio",
                    ConstructionStage::SiteAndSettingOut,
                    "NR-18.6 / NBR 5410",
                    SafetyPriority::Critical,
                    "Montar quadro elétrico provisório protegido da chuva, com aterramento e interruptor DR de 30 mA.".to_owned(),
                ),
                item(
                    "nr18_s1_estoque_cimento",
                    ConstructionStage::SiteAndSettingOut,
                    "NR-18.15",
                    SafetyPriority::Required,
                    "Guardar cimento e cal sobre estrados elevados (≥ 15 cm do chão), cobertos, pilhas de no máximo 10 sacos.".to_owned(),
                ),
            ],
        },
        // Stage 2: Foundations
        StageSafetyChecklist {
            stage: ConstructionStage::Foundations,
            ppe: vec![
                "Capacete com jugular".to_owned(),
                "Bota impermeável de PVC (concreto/vala)".to_owned(),
                "Luvas nitrílicas / raspa".to_owned(),
                "Óculos de proteção".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s2_escoramento_vala",
                    ConstructionStage::Foundations,
                    "NR-18.7.2",
                    SafetyPriority::Critical,
                    format!(
                        "Escavações acima de 1,25 m de profundidade exigem escoramento das paredes e escada de saída a cada 15 m (cota prevista de fundação: {:.2} m).",
                        base_depth
                    ),
                ),
                item(
                    "nr18_s2_borda_vala",
                    ConstructionStage::Foundations,
                    "NR-18.7.4",
                    SafetyPriority::Critical,
                    "Manter terra escavada, blocos e ferramentas a pelo menos 1,0 m da borda da vala para evitar desmoronamento.".to_owned(),
                ),
                item(
                    "nr18_s2_vizinhos_redes",
                    ConstructionStage::Foundations,
                    "NR-18.7 / NBR 6122",
                    SafetyPriority::Critical,
                    "Confirmar ausência de tubulações enterradas (água/esgoto/gás/energia) e nunca escavar abaixo da fundação do vizinho sem projeto de contenção.".to_owned(),
                ),
                item(
                    "nr18_s2_pontas_aco",
                    ConstructionStage::Foundations,
                    "NR-18.8",
                    SafetyPriority::Critical,
                    "Proteger todas as pontas expostas dos arranques de aço dos pilares com protetores plásticos (risco de empalamento).".to_owned(),
                ),
            ],
        },
        // Stage 3: Structure
        StageSafetyChecklist {
            stage: ConstructionStage::Structure,
            ppe: vec![
                "Capacete com jugular".to_owned(),
                "Botina de segurança / bota PVC".to_owned(),
                "Luvas nitrílicas (concreto)".to_owned(),
                "Óculos e protetor auricular (vibrador/serra)".to_owned(),
                "Cinto paraquedista (altura > 2,0 m)".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s3_escoramento_formas",
                    ConstructionStage::Structure,
                    "NR-18.8 / NBR 14931",
                    SafetyPriority::Critical,
                    format!(
                        "Conferir prumo, travamento em X e apoios rígidos das escoras antes de concretar{}; nunca apoiar escora diretamente sobre terra fofa.",
                        if has_slabs { " (lajes presentes no modelo)" } else { "" }
                    ),
                ),
                item(
                    "nr18_s3_cura_desforma",
                    ConstructionStage::Structure,
                    "NBR 14931",
                    SafetyPriority::Critical,
                    "Manter cura úmida por no mínimo 7 dias e conservar o escoramento de lajes/vigas por pelo menos 21 dias (ou conforme engenheiro/fabricante).".to_owned(),
                ),
                item(
                    "nr18_s3_guarda_corpo",
                    ConstructionStage::Structure,
                    "NR-18.9 / NR-35",
                    SafetyPriority::Critical,
                    format!(
                        "Instalar guarda-corpo rígido (1,20 m + travessa intermediária 0,70 m + rodapé 0,20 m) na borda da laje e aberturas de escada (altura estimada {:.1} m).",
                        total_height_m
                    ),
                ),
                item(
                    "nr18_s3_serra_circular",
                    ConstructionStage::Structure,
                    "NR-18.10 / NR-12",
                    SafetyPriority::Required,
                    "Serra circular de bancada somente com coifa protetora do disco, cutelo divisor, aterramento e empurrador de madeira.".to_owned(),
                ),
            ],
        },
        // Stage 4: Masonry
        StageSafetyChecklist {
            stage: ConstructionStage::Masonry,
            ppe: vec![
                "Capacete com jugular".to_owned(),
                "Botina com biqueira".to_owned(),
                "Luvas de látex/nitrílica".to_owned(),
                "Óculos de proteção".to_owned(),
                "Cinto paraquedista (> 2,0 m)".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s4_andaime",
                    ConstructionStage::Masonry,
                    "NR-18.12 / NR-35",
                    SafetyPriority::Critical,
                    format!(
                        "Acima de 2,0 m usar andaime com piso completo travado, guarda-corpo e rodapé (pé-direito máximo do projeto: {:.2} m). Nunca subir sobre pilhas de blocos ou tambores.",
                        max_wall_h
                    ),
                ),
                item(
                    "nr18_s4_escoramento_parede_alta",
                    ConstructionStage::Masonry,
                    "NBR 16868 / NR-18",
                    SafetyPriority::Required,
                    "Não levantar mais de 1,5 m de alvenaria por dia sem travamento e escorar provisoriamente paredes soltas em dias de vento forte.".to_owned(),
                ),
                item(
                    "nr18_s4_vergas_cintas",
                    ConstructionStage::Masonry,
                    "NBR 16868",
                    SafetyPriority::Required,
                    "Executar vergas, contravergas (30 cm de transpasse de cada lado) e cinta de amarração no topo antes de apoiar laje ou telhado.".to_owned(),
                ),
            ],
        },
        // Stage 5: Roof
        StageSafetyChecklist {
            stage: ConstructionStage::Roof,
            ppe: vec![
                "Cinto tipo paraquedista com talabarte duplo (NR-35)".to_owned(),
                "Capacete com jugular".to_owned(),
                "Calçado antiderrapante".to_owned(),
                "Luvas de vaqueta e óculos".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s5_nr35_linha_vida",
                    ConstructionStage::Roof,
                    "NR-35 / NR-18.9",
                    SafetyPriority::Critical,
                    format!(
                        "Trabalho em telhado exige cinto paraquedista ancorado em linha de vida independente e estrutural — nunca amarrar em ripas ou telhas{}.",
                        if has_roof { " (telhado modelado no projeto)" } else { "" }
                    ),
                ),
                item(
                    "nr18_s5_tabuas_telhas",
                    ConstructionStage::Roof,
                    "NR-18.9.10",
                    SafetyPriority::Critical,
                    "Usar tábuas de caminhamento distribuindo o peso sobre os apoios ao pisar em telhas cerâmicas ou de fibrocimento; suspender serviço em chuva ou vento forte.".to_owned(),
                ),
                item(
                    "nr18_s5_isolamento_inferior",
                    ConstructionStage::Roof,
                    "NR-18.9",
                    SafetyPriority::Required,
                    "Isolar e sinalizar toda a área abaixo do beiral contra queda de telhas, madeiras e ferramentas.".to_owned(),
                ),
            ],
        },
        // Stage 6: Installations
        StageSafetyChecklist {
            stage: ConstructionStage::Installations,
            ppe: vec![
                "Capacete classe B (isolante)".to_owned(),
                "Calçado sem partes metálicas (isolante)".to_owned(),
                "Luvas isolantes (NR-10) / nitrílicas".to_owned(),
                "Óculos e respirador PFF2 (corte de parede)".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s6_nr10_desenergizar",
                    ConstructionStage::Installations,
                    "NR-10.5",
                    SafetyPriority::Critical,
                    "Nunca trabalhar em circuito energizado: desligar disjuntor geral, bloquear/sinalizar e confirmar ausência de tensão com multímetro ou caneta de teste.".to_owned(),
                ),
                item(
                    "nr18_s6_dr_aterramento",
                    ConstructionStage::Installations,
                    "NBR 5410 §5.1.3",
                    SafetyPriority::Critical,
                    "Conferir cores dos cabos (azul-claro neutro, verde/amarelo terra PE), continuidade do aterramento e funcionamento do botão de teste do DR 30 mA.".to_owned(),
                ),
                item(
                    "nr18_s6_teste_hidraulico_gas",
                    ConstructionStage::Installations,
                    "NBR 5626 / NBR 13103",
                    SafetyPriority::Required,
                    "Testar estanqueidade das tubulações de água fria/quente e esgoto antes de fechar os rasgos; instalar botijão/aquecedor a gás em local ventilado fora de banheiros e quartos.".to_owned(),
                ),
            ],
        },
        // Stage 7: Plaster & waterproofing
        StageSafetyChecklist {
            stage: ConstructionStage::PlasterAndWaterproofing,
            ppe: vec![
                "Capacete com jugular".to_owned(),
                "Luvas nitrílicas (proteção alcalina contra cimento/cal)".to_owned(),
                "Óculos ampla visão".to_owned(),
                "Respirador PFF2 / máscara para vapores orgânicos".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s7_andaime_fachada",
                    ConstructionStage::PlasterAndWaterproofing,
                    "NR-18.12",
                    SafetyPriority::Critical,
                    "No reboco externo e fachadas acima de 2,0 m, usar andaime estável com guarda-corpo, rodapé e cinto paraquedista.".to_owned(),
                ),
                item(
                    "nr18_s7_ventilacao_impermeabilizante",
                    ConstructionStage::PlasterAndWaterproofing,
                    "NR-15 / FISPQ",
                    SafetyPriority::Required,
                    "Manter ventilação cruzada e proibir chamas/faíscas ao aplicar impermeabilizantes asfálticos ou com solventes; seguir a FISPQ do fabricante.".to_owned(),
                ),
            ],
        },
        // Stage 8: Finishes
        StageSafetyChecklist {
            stage: ConstructionStage::Finishes,
            ppe: vec![
                "Óculos de proteção e protetor auricular".to_owned(),
                "Respirador PFF2 (poeira de sílica no corte de piso)".to_owned(),
                "Luvas adequadas e botina".to_owned(),
            ],
            items: vec![
                item(
                    "nr18_s8_corte_umido_silica",
                    ConstructionStage::Finishes,
                    "NR-15 / NR-18",
                    SafetyPriority::Required,
                    "Preferir corte úmido ou em área aberta com respirador PFF2 e óculos ao cortar cerâmicas e porcelanatos (evita inalação de poeira de sílica).".to_owned(),
                ),
                item(
                    "nr18_s8_escada_pintura",
                    ConstructionStage::Finishes,
                    "NR-18.12 / NR-35",
                    SafetyPriority::Required,
                    "Usar escada de abrir firme com sapatas antiderrapantes e limitador de abertura; nunca ficar de pé nos dois últimos degraus nem apoiar escada em piso molhado.".to_owned(),
                ),
                item(
                    "nr18_s8_teste_final_dr",
                    ConstructionStage::Finishes,
                    "NBR 5410",
                    SafetyPriority::Critical,
                    "Após instalar tomadas, luminárias e chuveiros, identificar todos os circuitos no quadro e apertar o botão 'T' do interruptor DR confirmando o desligamento instantâneo.".to_owned(),
                ),
            ],
        },
    ];

    let permits = vec![
        PermitRequirement {
            id: "art_rrt".to_owned(),
            title: "ART (CREA — Engenheiro Civil) ou RRT (CAU — Arquiteto)".to_owned(),
            authority: "CREA ou CAU do estado (profissional habilitado ou ATHIS)".to_owned(),
            when_needed: "Antes de iniciar a obra (projeto e execução)".to_owned(),
            why_it_matters: "O Trazo faz pré-dimensionamento de apoio, mas a responsabilidade técnica legal e o projeto executivo estrutural, elétrico e hidráulico exigem ART ou RRT (Lei 5.194/1966 e Lei 12.378/2010).".to_owned(),
            mandatory: true,
        },
        PermitRequirement {
            id: "alvara".to_owned(),
            title: "Alvará de Construção / Aprovação de Projeto Legal".to_owned(),
            authority: "Prefeitura Municipal (Secretaria de Obras / Urbanismo)".to_owned(),
            when_needed: "Antes da escavação e locação".to_owned(),
            why_it_matters: "Verifica recuos, taxa de ocupação, permeabilidade, ventilação e código de obras local; construir sem alvará sujeita a obra a embargo e multa.".to_owned(),
            mandatory: true,
        },
        PermitRequirement {
            id: "cno_inss".to_owned(),
            title: "CNO (Cadastro Nacional de Obras) e regularização INSS / SERO".to_owned(),
            authority: "Receita Federal do Brasil (portal e-CAC)".to_owned(),
            when_needed: "No início da obra (até 30 dias) e encerramento via SERO".to_owned(),
            why_it_matters: "Obrigatório para emitir a Certidão Negativa de Débitos (CND) da obra e averbar a casa na matrícula do terreno.".to_owned(),
            mandatory: true,
        },
        PermitRequirement {
            id: "concessionarias".to_owned(),
            title: "Ligação de energia, água e esgoto (padrão de entrada)".to_owned(),
            authority: "Concessionária local de energia e companhia de saneamento".to_owned(),
            when_needed: "Ligação provisória no início; definitiva após quadro/hidrômetro".to_owned(),
            why_it_matters: "Garante padrão de entrada homologado (poste, disjuntor geral, aterramento) e esgotamento sanitário adequado (rede pública ou fossa+sumidouro NBR 7229/13969).".to_owned(),
            mandatory: true,
        },
        PermitRequirement {
            id: "habite_se".to_owned(),
            title: "Habite-se (CVCO) e Averbação no Registro de Imóveis".to_owned(),
            authority: "Prefeitura Municipal + Cartório de Registro de Imóveis".to_owned(),
            when_needed: "Ao concluir a obra, antes de financiar ou vender o imóvel".to_owned(),
            why_it_matters: "Atesta que a edificação foi executada conforme o projeto aprovado e em condições seguras de moradia.".to_owned(),
            mandatory: true,
        },
        PermitRequirement {
            id: "athis".to_owned(),
            title: "Assistência Técnica Gratuita para Habitação de Interesse Social (ATHIS)".to_owned(),
            authority: "Prefeitura, CAU, CREA ou escritórios-modelo universitários (Lei 11.888/2008)".to_owned(),
            when_needed: "Famílias com renda de até 3 salários mínimos".to_owned(),
            why_it_matters: "Garante por lei projeto e acompanhamento gratuitos por arquiteto ou engenheiro habilitado para moradia própria.".to_owned(),
            mandatory: false,
        },
    ];

    let mut site_alerts = Vec::new();
    let risks = &project.protection.site_risks;
    if !risks.risky().is_empty() {
        site_alerts.push(
            "ATENÇÃO — RISCO DE TERRENO DECLARADO: pare as escavações e acione um engenheiro geotécnico ou a Defesa Civil (199) antes de abrir qualquer vala.".to_owned(),
        );
    }
    if risks.unknown_existing_base() {
        site_alerts.push(
            "ATENÇÃO — OBRA SOBRE CASA EXISTENTE COM BASE DESCONHECIDA: não construa andar superior sem vistoria e laudo de profissional habilitado.".to_owned(),
        );
    }
    if floors >= 2 || total_height_m > 3.5 {
        site_alerts.push(format!(
            "Obra com altura estimada de {:.1} m ({} pavimento(s)): NR-35 (trabalho em altura > 2,0 m) e guarda-corpo periférico são obrigatórios em estrutura, alvenaria e cobertura.",
            total_height_m, floors
        ));
    }
    if base_depth > 1.25 {
        site_alerts.push(format!(
            "Profundidade de fundação ({:.2} m) excede 1,25 m: escoramento de valas e escada de fuga são obrigatórios pela NR-18.7.",
            base_depth
        ));
    }

    let total_checks: usize = stages.iter().map(|s| s.items.len()).sum();
    let completed_checks: usize = stages
        .iter()
        .flat_map(|s| s.items.iter())
        .filter(|i| i.checked)
        .count();

    SiteSafetyAndPermitsGuide {
        stages,
        permits,
        site_alerts,
        help_guidance: HELP_SENTENCE.to_owned(),
        total_checks,
        completed_checks,
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Printable A3 landscape SVG sheet of the NR-18 site safety checklists & permits guide (#138).
pub fn safety_permits_svg(project: &Project) -> String {
    let guide = build_safety_and_permits_guide(project);
    let rows = guide.total_checks + guide.stages.len() * 2 + guide.permits.len() + 6;
    let width = 420.0;
    let height = (85.0 + rows as f64 * 6.4 + 25.0).max(297.0);
    let ink = "#1d2836";
    let muted = "#4a5d73";
    let font = "font-family=\"Arial, sans-serif\"";

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                color: &str,
                value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };

    text(
        &mut svg,
        18.0,
        22.0,
        5.0,
        true,
        ink,
        "GUIA DE SEGURANÇA NO CANTEIRO (NR-18 / NR-10 / NR-35) E LICENÇAS",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        2.9,
        false,
        muted,
        &format!(
            "{} · {}/{} verificações marcadas · {}",
            project.name, guide.completed_checks, guide.total_checks, guide.help_guidance
        ),
    );

    let mut y = 37.0;
    for alert in &guide.site_alerts {
        let _ = writeln!(
            svg,
            "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.2\" fill=\"#ffebee\" stroke=\"#c62828\" stroke-width=\"0.3\"/>",
            y - 4.2
        );
        text(&mut svg, 18.0, y, 2.4, true, "#b71c1c", alert);
        y += 7.5;
    }

    for st in &guide.stages {
        let _ = writeln!(
            svg,
            "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.0\" fill=\"#e8eef5\" stroke=\"{ink}\" stroke-width=\"0.2\"/>",
            y - 4.1
        );
        text(&mut svg, 18.0, y, 2.7, true, ink, st.stage.label());
        text(
            &mut svg,
            165.0,
            y,
            2.2,
            false,
            muted,
            &format!("EPIs: {}", st.ppe.join(" · ")),
        );
        y += 6.5;

        for item in &st.items {
            let mark = if item.checked { "[X]" } else { "[ ]" };
            let col = if item.priority == SafetyPriority::Critical {
                "#b71c1c"
            } else {
                ink
            };
            text(&mut svg, 18.0, y, 2.4, true, col, mark);
            text(&mut svg, 28.0, y, 2.2, true, muted, &item.norm);
            text(&mut svg, 66.0, y, 2.3, false, ink, &item.text);
            y += 5.8;
        }
        y += 2.5;
    }

    // Permits section
    y += 3.0;
    let _ = writeln!(
        svg,
        "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.4\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.25\"/>",
        y - 4.3
    );
    text(
        &mut svg,
        18.0,
        y,
        2.9,
        true,
        ink,
        "RESPONSABILIDADE TÉCNICA, LICENÇAS E REGULARIZAÇÃO DA OBRA",
    );
    y += 7.0;

    for p in &guide.permits {
        text(
            &mut svg,
            18.0,
            y,
            2.5,
            true,
            if p.mandatory { ink } else { "#1b5e20" },
            &format!("{} ({})", p.title, p.when_needed),
        );
        text(
            &mut svg,
            18.0,
            y + 3.5,
            2.1,
            false,
            muted,
            &p.why_it_matters,
        );
        y += 8.2;
    }

    svg.push_str("</svg>\n");
    svg
}

/// One-click M9 construction management bundle PDF: joins the materials
/// catalogue (#135), stage shopping list (#136), cost estimate (#87),
/// Gantt physical-financial schedule (#137) and NR-18 safety & permits guide
/// (#138) into a single multi-page A3 PDF.
pub fn export_construction_bundle_pdf(project: &Project, path: &Path) -> Result<usize, String> {
    let pages = vec![
        materials_catalogue_svg(project),
        stage_shopping_list_svg(project),
        cost_estimate_svg(project),
        construction_schedule_gantt_svg(project),
        safety_permits_svg(project),
    ];
    let pdf = crate::io::report::pages_to_pdf(&pages)?;
    std::fs::write(path, pdf).map_err(|e| format!("Não foi possível gravar: {e}"))?;
    Ok(pages.len())
}

impl Project {
    /// Per-stage NR-18 safety checklists and Brazilian permits guide (#138).
    pub fn site_safety_and_permits(&self) -> SiteSafetyAndPermitsGuide {
        build_safety_and_permits_guide(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    #[test]
    fn safety_and_permits_guide_covers_every_stage_and_persists_checks() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();

        let guide = project.site_safety_and_permits();
        assert_eq!(guide.stages.len(), ConstructionStage::ALL.len());
        for st in ConstructionStage::ALL {
            let group = guide
                .stages
                .iter()
                .find(|g| g.stage == st)
                .expect("every stage covered");
            assert!(!group.ppe.is_empty(), "missing PPE for {st:?}");
            assert!(!group.items.is_empty(), "missing checklist for {st:?}");
        }
        assert!(guide.permits.iter().any(|p| p.id == "art_rrt"));
        assert!(guide.permits.iter().any(|p| p.id == "alvara"));
        assert!(guide.permits.iter().any(|p| p.id == "habite_se"));
        assert!(guide.permits.iter().any(|p| p.id == "athis"));

        // Checking an item persists in ConstructionPlan.
        set_safety_check(&mut project, "nr18_s2_escoramento_vala", true).unwrap();
        let after = project.site_safety_and_permits();
        assert_eq!(after.completed_checks, 1);

        let svg = safety_permits_svg(&project);
        assert!(svg.contains("GUIA DE SEGURANÇA NO CANTEIRO"));
        assert!(svg.contains("ART"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn one_click_m9_bundle_produces_valid_five_page_pdf_for_sample_house() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();
        project
            .import_sinapi_table(
                "codigo;descricao;unidade;preco\n00001379;CIMENTO 50KG;UN;42,00\n",
                Some("SINAPI Teste"),
            )
            .unwrap();

        let tmp = std::env::temp_dir().join("trazo-m9-bundle-test.pdf");
        let pages = export_construction_bundle_pdf(&project, &tmp).unwrap();
        assert_eq!(pages, 5);
        let bytes = std::fs::read(&tmp).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let _ = std::fs::remove_file(&tmp);
    }
}
