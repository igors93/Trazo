//! Structural engineering safety verdicts (slabs, beams, columns, footings,
//! balance beams, wall foundations, masonry, roofs, timber posts and wind).

use crate::domain::{model::Project, structure::StructureReport};

use super::{Check, Discipline, Light, Verdict, cm, decimal, limit_detail};

/// Column slenderness (#145): green up to the second-order limit, yellow
/// while the simplified methods apply, red beyond them.
fn slenderness_check(column: &crate::domain::structure::ColumnResult) -> Check {
    use crate::domain::wall_heights::{
        MAX_SIMPLE_SLENDERNESS, SECOND_ORDER_SLENDERNESS, side_for_slenderness,
    };
    let lambda = column.slenderness;
    let light = if lambda <= SECOND_ORDER_SLENDERNESS + 1e-9 {
        Light::Green
    } else if lambda <= MAX_SIMPLE_SLENDERNESS + 1e-9 {
        Light::Yellow
    } else {
        Light::Red
    };
    let simple = match light {
        Light::Green => "Pilar curto: os efeitos de 2ª ordem podem ser desprezados.".to_owned(),
        Light::Yellow => format!(
            "Pilar medianamente esbelto (λ = {lambda:.0}): o projeto final precisa considerar os efeitos de 2ª ordem."
        ),
        Light::Red => format!(
            "Pilar esbelto demais (λ = {lambda:.0}): aumente o menor lado para {} ou trave-o no meio da altura.",
            cm(side_for_slenderness(column.height_m))
        ),
    };
    Check {
        name: "Esbeltez".to_owned(),
        utilisation: lambda / MAX_SIMPLE_SLENDERNESS,
        light,
        simple,
        detail: format!(
            "λ = 3,46 · le / h = 3,46 × {} m / {} = {} (le = altura livre, pilar travado no topo pelas vigas; até 35 sem 2ª ordem, até 90 métodos simplificados, NBR 6118 15.8).",
            decimal(column.height_m, 2),
            cm(column.width_m.min(column.depth_m)),
            decimal(lambda, 0)
        ),
    }
}

/// The structural checks, from the pre-sizing report, with the
/// applicability limits (#102) applied: a project beyond the limits gets one
/// red verdict instead of any sizing, and an element beyond them gets a red
/// "Fora dos limites" with no numbers.
pub fn structure_verdicts(project: &Project, report: &StructureReport) -> Vec<Verdict> {
    let mut verdicts = limited_structure_verdicts(project, report);
    // A second, simpler opinion contests a simulation it disagrees with (#172).
    super::cross_checks::cross_check(project, report, &mut verdicts);
    // A result that rests on a hypothesis Trazo contests is never green (#169).
    super::hypotheses::contest(project, &mut verdicts);
    verdicts
}

/// A refusal's reason followed by where to find help (#159).
fn with_help(message: &str) -> String {
    format!(
        "{message} {}",
        crate::domain::protection::help::HELP_SENTENCE
    )
}

/// The structural verdicts, with the applicability limits applied.
fn limited_structure_verdicts(project: &Project, report: &StructureReport) -> Vec<Verdict> {
    let project_wide = crate::domain::limits::structure_violations(project);
    if !project_wide.is_empty() {
        return vec![Verdict {
            discipline: Discipline::Structure,
            element: "Estrutura".to_owned(),
            checks: project_wide
                .into_iter()
                .map(|v| {
                    let detail = limit_detail(&v);
                    Check::fail("Fora dos limites", with_help(&v.message), detail)
                })
                .collect(),
        }];
    }
    let elements = crate::domain::limits::element_violations(report);
    let reinforcement = crate::domain::reinforcement::build_reinforcement_report(project, report);
    let mut verdicts = sized_structure_verdicts(report, &reinforcement);
    for verdict in &mut verdicts {
        let refused: Vec<Check> = elements
            .iter()
            .filter(|v| v.element == verdict.element)
            .map(|v| Check::fail("Fora dos limites", with_help(&v.message), limit_detail(v)))
            .collect();
        if !refused.is_empty() {
            verdict.checks = refused;
        }
    }
    verdicts
}

/// The structural checks of every element, before the limits.
fn sized_structure_verdicts(
    report: &StructureReport,
    reinforcement: &crate::domain::reinforcement::ReinforcementReport,
) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    for failure in &report.frame_failures {
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: failure.label.clone(),
            checks: vec![Check::fail(
                "Análise 2D",
                "O pórtico não foi resolvido; não use resultados ausentes como aprovação.".to_owned(),
                format!(
                    "Solver de rigidez direta: {}. Revise estabilidade, vínculos, apoios e geometria.",
                    failure.reason
                ),
            )],
        });
    }
    for frame in &report.frames {
        let mut checks = vec![Check::new(
            "Deslocamento lateral (ELS-F)",
            frame.max_lateral_drift_mm / frame.drift_limit_mm.max(1e-9),
            if frame.drift_ok() {
                format!(
                    "Deslocamento lateral de {} mm fica dentro de H/850 = {} mm sob vento frequente.",
                    decimal(frame.max_lateral_drift_mm, 2),
                    decimal(frame.drift_limit_mm, 2)
                )
            } else {
                "O pórtico desloca demais com o vento: aumente os pilares ou adicione contraventamento."
                    .to_owned()
            },
            format!(
                "Combinação {} (NBR 8681 / NBR 6118 Tabela 13.3): Δh = {} mm ≤ H/850 = {} mm; vento característico Wk = {} kN (NBR 6123), ELU Md,viga = {} kN·m ({}), Md,pilar = {} kN·m ({}).",
                frame.envelope.els_f_drift_combo,
                decimal(frame.max_lateral_drift_mm, 2),
                decimal(frame.drift_limit_mm, 2),
                decimal(frame.characteristic_wind_kn, 1),
                decimal(
                    frame
                        .envelope
                        .elu_max_beam_pos_knm
                        .max(frame.envelope.elu_min_beam_neg_knm.abs()),
                    1
                ),
                if frame.envelope.elu_min_beam_neg_knm.abs() > frame.envelope.elu_max_beam_pos_knm {
                    &frame.envelope.elu_min_beam_neg_combo
                } else {
                    &frame.envelope.elu_max_beam_pos_combo
                },
                decimal(frame.envelope.elu_max_column_moment_knm, 1),
                frame.envelope.elu_max_column_moment_combo
            ),
        )];
        if frame.envelope.has_column_uplift {
            if let Some(min_cap) = report.frame_uplift_anchored_by_piles(frame) {
                let demand = frame.envelope.elu_min_column_axial_kn.max(0.0);
                checks.push(Check::new(
                    "Arrancamento ancorado (ELU)",
                    demand / min_cap.max(1e-6),
                    format!(
                        "Tração sob vento ({}, N = +{} kN) ancorada nos blocos sobre broca (capacidade mín. {} kN por atrito lateral).",
                        frame.envelope.elu_min_column_axial_combo,
                        decimal(demand, 1),
                        decimal(min_cap, 1)
                    ),
                    format!(
                        "Combinação {} (NBR 8681 + NBR 6122 §8.2.2): tração de {} kN resistida pelo atrito lateral das brocas armadas (0,70·Q_lat / FS ≥ {} kN, arranque ≥ 1,50 m).",
                        frame.envelope.elu_min_column_axial_combo,
                        decimal(demand, 1),
                        decimal(min_cap, 1)
                    ),
                ));
            } else {
                checks.push(Check::fail(
                    "Arrancamento (ELU)",
                    format!(
                        "Pilar em tração sob vento ({}, N = +{} kN): verifique ancoragem na fundação (ex.: bloco sobre broca armada) ou aumente a rigidez/contrapeso.",
                        frame.envelope.elu_min_column_axial_combo,
                        decimal(frame.envelope.elu_min_column_axial_kn, 1)
                    ),
                    format!(
                        "Combinação {} (NBR 8681, G favorável + 1,40·W): esforço axial de tração de {} kN em pilar do pórtico.",
                        frame.envelope.elu_min_column_axial_combo,
                        decimal(frame.envelope.elu_min_column_axial_kn, 1)
                    ),
                ));
            }
        } else {
            checks.push(Check::new(
                "Compressão sob vento (ELU)",
                0.0,
                "Todos os pilares permanecem comprimidos em todas as combinações de ELU com vento."
                    .to_owned(),
                format!(
                    "Envoltória NBR 8681 ({} combinações): compressão máx. {} kN ({}); mínimo axial {} kN ({}, sem tração).",
                    frame.combinations.len(),
                    decimal(frame.envelope.elu_max_column_compression_kn, 1),
                    frame.envelope.elu_max_column_compression_combo,
                    decimal(frame.envelope.elu_min_column_axial_kn, 1),
                    frame.envelope.elu_min_column_axial_combo
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: frame.label.clone(),
            checks,
        });
    }
    if (!report.columns.is_empty() || !report.beams.is_empty() || !report.slabs.is_empty())
        && report.loads.nbr6120.total_g_kn() > 1e-6
    {
        for axis in [
            crate::domain::loads::WindDirection::PosX,
            crate::domain::loads::WindDirection::PosY,
        ] {
            if let Some(dir) = report
                .loads
                .building_wind
                .directions
                .iter()
                .find(|d| d.direction == axis)
            {
                let axis_label = if axis.along_x() { "±X" } else { "±Y" };
                let design_overturning = crate::domain::loads::GAMMA_W * dir.overturning_moment_knm;
                let util = if dir.stabilizing_moment_knm > 1e-9 {
                    design_overturning / dir.stabilizing_moment_knm
                } else {
                    f64::INFINITY
                };
                let mut checks = vec![Check::new(
                    "Tombamento global (ELU-EQU)",
                    util,
                    if dir.overturning_ok() {
                        format!(
                            "O peso permanente vence o tombamento do vento no eixo {axis_label} (folga de {:.1}×).",
                            dir.overturning_safety_ratio.min(99.9)
                        )
                    } else {
                        format!(
                            "Vento no eixo {axis_label} supera o momento estabilizante em ELU-EQU: amplie a base, o contraventamento ou ancore as fundações."
                        )
                    },
                    format!(
                        "NBR 6123 (V0 = {:.0} m/s, cat. {}, classe {}, Ca = {}, Cpe = +{}/{}) e NBR 8681 ELU-EQU: Vw = {} kN, Mw,d = 1,40 × {} = {} kN·m ≤ Mestab = 0,90·G·L/2 = {} kN·m.",
                        report.loads.building_wind.settings.v0_ms,
                        report.loads.building_wind.settings.category,
                        report.loads.building_wind.building_class,
                        decimal(dir.coeffs.ca_drag, 2),
                        decimal(dir.coeffs.cpe_windward, 2),
                        decimal(dir.coeffs.cpe_leeward, 2),
                        decimal(dir.base_shear_kn, 1),
                        decimal(dir.overturning_moment_knm, 1),
                        decimal(design_overturning, 1),
                        decimal(dir.stabilizing_moment_knm, 1)
                    ),
                )];
                if !dir.coeffs.within_table4 {
                    checks.push(Check::new(
                        "Geometria Tabela 4",
                        0.95,
                        "Relação altura/largura ou comprimento/largura fora da Tabela 4 da NBR 6123: confirme coeficientes aerodinâmicos com engenheiro."
                            .to_owned(),
                        format!(
                            "h/b = {}, a/b = {} (Tabela 4 da NBR 6123 cobre h/b ≤ 6 e a/b ≤ 4).",
                            decimal(dir.coeffs.h_over_b, 2),
                            decimal(dir.coeffs.a_over_b, 2)
                        ),
                    ));
                }
                verdicts.push(Verdict {
                    discipline: Discipline::Structure,
                    element: format!("Vento na edificação ({axis_label})"),
                    checks,
                });
            }
        }
    }
    for slab in &report.slabs {
        let mut checks = Vec::new();
        if slab.supported {
            checks.push(Check::new(
                "Espessura",
                slab.thickness_min_m / slab.thickness_m.max(1e-6),
                if slab.thickness_m + 1e-6 >= slab.thickness_min_m {
                    format!("A espessura de {} serve para o vão de {} m.", cm(slab.thickness_m), decimal(slab.lx_m, 2))
                } else {
                    format!("Aumente a laje para pelo menos {}.", cm(slab.thickness_min_m))
                },
                format!(
                    "Laje {} de {} para o menor vão lx = {} m: mínima {} (maciça: lx/40 ≥ 8 cm, NBR 6118 13.2.4.1; treliçada: tabela de fabricante).",
                    slab.kind.label().to_lowercase(),
                    cm(slab.thickness_m),
                    decimal(slab.lx_m, 2),
                    cm(slab.thickness_min_m)
                ),
            ));
            if slab.kind == crate::domain::structure::SlabKind::Solid {
                checks.push(Check::new(
                    "Flecha bidirecional",
                    slab.deflection_mm / slab.deflection_limit_mm.max(1e-9),
                    if slab.deflection_mm <= slab.deflection_limit_mm + 1e-9 {
                        format!(
                            "A flecha final de {} mm fica dentro de L/250 = {} mm.",
                            decimal(slab.deflection_mm, 1),
                            decimal(slab.deflection_limit_mm, 1)
                        )
                    } else {
                        "A laje deforma demais: aumente a espessura ou reduza o vão.".to_owned()
                    },
                    format!(
                        "{} painel(is); Mx = {} kN·m/m ({}), My = {} kN·m/m ({}). Envelope das tabelas Czerny e Marcus; flecha de Czerny com g + 0,3q e fator diferido 2,32.",
                        slab.panels.len(),
                        decimal(slab.moment_x_knm_m, 2),
                        slab.method_x.label(),
                        decimal(slab.moment_y_knm_m, 2),
                        slab.method_y.label()
                    ),
                ));
            }
            if let Some(reinf) = reinforcement.slabs.iter().find(|r| r.slab_id == slab.id) {
                if reinf.domain_ok {
                    checks.push(Check::new(
                        "Armadura (domínio)",
                        0.5,
                        format!(
                            "Armadura preliminar OK: principal {}, distribuição {}.",
                            reinf.main.notation(),
                            reinf.distribution.notation()
                        ),
                        format!(
                            "Domínio de armadura simples (x/d ≤ 0,45); principal {} ({} cm²/m), distribuição {} ({} cm²/m); massa {} kg (NBR 6118 §14.6.4.3).",
                            reinf.main.notation(),
                            decimal(reinf.main.provided_cm2_m, 2),
                            reinf.distribution.notation(),
                            decimal(reinf.distribution.provided_cm2_m, 2),
                            decimal(reinf.mass_kg, 1)
                        ),
                    ));
                } else {
                    checks.push(Check::fail(
                        "Armadura (domínio)",
                        "Laje fora do domínio de armadura simples: aumente a espessura.".to_owned(),
                        "Momento de cálculo acima do limite μ_lim = 0,2952 (x/d > 0,45; NBR 6118 §14.6.4.3)."
                            .to_owned(),
                    ));
                }
            }
        } else {
            checks.push(Check::fail(
                "Apoio",
                "Nenhuma viga ao longo das bordas: ligue os pilares com vigas sob a laje."
                    .to_owned(),
                "A carga da laje não tem para onde ir: sem viga ao longo de nenhuma borda."
                    .to_owned(),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Laje {} ({})", slab.label, slab.room),
            checks,
        });
    }
    for line in &report.beams {
        for (i, span) in line.spans.iter().enumerate() {
            let name = if line.spans.len() > 1 {
                format!("Viga {} · vão {}", line.label, i + 1)
            } else {
                format!("Viga {}", line.label)
            };
            let stage = if span.cracked {
                "Estádio II (fissurada)"
            } else {
                "Estádio I (não fissurada)"
            };
            let mut checks = vec![
                Check::new(
                    "Altura",
                    span.height_min_m / span.height_m.max(1e-6),
                    if span.height_m + 1e-6 >= span.height_min_m {
                        format!(
                            "A altura de {} serve para o vão de {} m.",
                            cm(span.height_m),
                            decimal(span.length_m, 2)
                        )
                    } else {
                        format!(
                            "Aumente a viga para pelo menos {} de altura.",
                            cm(span.height_min_m)
                        )
                    },
                    format!(
                        "h = {} para L = {} m; mínima {} (L/10 em vão de extremidade ou isolado, L/12 em vão interno).",
                        cm(span.height_m),
                        decimal(span.length_m, 2),
                        cm(span.height_min_m)
                    ),
                ),
                Check::new(
                    "Flecha",
                    span.deflection_mm / span.limit_mm.max(1e-9),
                    if span.deflection_mm <= span.limit_mm {
                        format!(
                            "A viga deforma {} mm, dentro do limite de {} mm.",
                            decimal(span.deflection_mm, 1),
                            decimal(span.limit_mm, 1)
                        )
                    } else {
                        "A viga vai deformar demais: aumente a altura ou diminua o vão.".to_owned()
                    },
                    format!(
                        "Flecha diferida {} mm ≤ L/250 = {} mm (carga quase permanente, {}, Mr = {} kN·m, I_eq/Ic = {:.0}%, As = {} cm², 1 + αf = 2,32; NBR 6118 13.3 e 17.3.2.1).",
                        decimal(span.deflection_mm, 1),
                        decimal(span.limit_mm, 1),
                        stage,
                        decimal(span.cracking_moment_knm, 1),
                        span.i_eq_ratio * 100.0,
                        decimal(span.branson_as_cm2, 2)
                    ),
                ),
                Check::new(
                    "Largura",
                    0.12 / span.width_m.max(1e-6),
                    if span.width_m >= 0.12 - 1e-9 {
                        format!("A largura de {} atende ao mínimo.", cm(span.width_m))
                    } else {
                        "Alargue a viga para pelo menos 12 cm.".to_owned()
                    },
                    format!("b = {} ≥ 12 cm (NBR 6118 13.2.2).", cm(span.width_m)),
                ),
            ];
            if let Some(reinf) = reinforcement
                .beams
                .iter()
                .find(|r| r.beam_id == span.beam && r.span_index == i + 1)
            {
                let all_fit = reinf.bottom.fits_section
                    && reinf.top.left().fits_section
                    && reinf.top.right().fits_section;
                if let Some(e) = &reinf.frame_envelope {
                    checks.push(Check::new(
                        "Origem das armaduras", 0.5,
                        "Envoltória ELU do próprio pórtico; armadura superior contínua em todo o vão.".to_owned(),
                        format!(
                            "Md+ = {} kN·m ({}), Md− = {} kN·m ({}), Vsd = {} kN ({}); esforços já majorados.",
                            decimal(e.max_positive_moment_knm, 2), e.positive_combination,
                            decimal(e.min_negative_moment_knm, 2), e.negative_combination,
                            decimal(e.max_shear_kn, 2), e.shear_combination,
                        ),
                    ));
                }
                if !reinf.flexural_domain_ok {
                    checks.push(Check::fail(
                        "Armadura / Domínio",
                        "Momento acima do domínio de armadura simples: aumente a altura ou largura da viga."
                            .to_owned(),
                        format!(
                            "Flexão excede μ_lim = 0,2952 (x/d > 0,45); inferior {}, superiores {} / {} (NBR 6118 §14.6.4.3).",
                            reinf.bottom.notation(),
                            reinf.top.left().notation(),
                            reinf.top.right().notation()
                        ),
                    ));
                } else if !all_fit {
                    checks.push(Check::new(
                        "Armadura / Domínio",
                        0.95,
                        "As barras não cabem em uma camada com cobrimento de 3 cm: aumente a largura ou detalhe em duas camadas."
                            .to_owned(),
                        format!(
                            "Inferior {} ({}), superior esq. {} ({}), superior dir. {} ({}); espaçamento livre mínimo de 2 cm (NBR 6118 §18.3.2.2).",
                            reinf.bottom.notation(),
                            if reinf.bottom.fits_section { "cabe" } else { "não cabe" },
                            reinf.top.left().notation(),
                            if reinf.top.left().fits_section { "cabe" } else { "não cabe" },
                            reinf.top.right().notation(),
                            if reinf.top.right().fits_section { "cabe" } else { "não cabe" }
                        ),
                    ));
                } else {
                    checks.push(Check::new(
                        "Armadura / Domínio",
                        0.5,
                        format!(
                            "Armadura preliminar OK: inferior {}, superiores {} / {}.",
                            reinf.bottom.notation(),
                            reinf.top.left().notation(),
                            reinf.top.right().notation()
                        ),
                        format!(
                            "Domínio simples (x/d ≤ 0,45) e arranjo em 1 camada OK; ancoragem lb,nec = {:.0} cm (inf.) / {:.0} cm (sup.); massa {} kg (NBR 6118 §9.4).",
                            reinf.bottom_anchorage_m * 100.0,
                            reinf.top_anchorage_m * 100.0,
                            decimal(reinf.mass_kg, 1)
                        ),
                    ));
                }
                if reinf.stirrups.shear_capacity_ok {
                    checks.push(Check::new(
                        "Cortante (biela)",
                        0.5,
                        format!(
                            "Estribos {} atendem ao cortante preliminar.",
                            reinf.stirrups.notation()
                        ),
                        format!(
                            "Compressão diagonal VRd2 OK; estribos {} ({} un., compr. unit. {} m; NBR 6118 §17.4).",
                            reinf.stirrups.notation(),
                            reinf.stirrups.count,
                            decimal(reinf.stirrups.length_each_m, 2)
                        ),
                    ));
                } else {
                    checks.push(Check::fail(
                        "Cortante (biela)",
                        "Compressão diagonal excedida (VRd2): aumente a seção da viga.".to_owned(),
                        format!(
                            "Cortante de cálculo acima da resistência da biela comprimida VRd2; estribos {} não bastam (NBR 6118 §17.4.2.2).",
                            reinf.stirrups.notation()
                        ),
                    ));
                }
            }
            verdicts.push(Verdict {
                discipline: Discipline::Structure,
                element: name,
                checks,
            });
        }
    }
    for band in &report.masonry.bands {
        let bearing = band.bearing_left_m.min(band.bearing_right_m);
        let mut checks = vec![
            Check::new(
                "Apoio",
                crate::domain::masonry_structure::MIN_LINTEL_BEARING_M / bearing.max(1e-9),
                if bearing + 1e-9 >= crate::domain::masonry_structure::MIN_LINTEL_BEARING_M {
                    format!(
                        "{} apoia pelo menos {} em cada lado do vão.",
                        band.kind.label(),
                        cm(crate::domain::masonry_structure::MIN_LINTEL_BEARING_M)
                    )
                } else {
                    format!(
                        "Faltam apoios laterais: mova o vão ou prolongue {} até ter {} de cada lado.",
                        band.kind.label().to_lowercase(),
                        cm(crate::domain::masonry_structure::MIN_LINTEL_BEARING_M)
                    )
                },
                format!(
                    "Apoios esquerdo/direito = {}/{}; mínimo verificado {} e preferido 30 cm (NBR 16868-1, detalhamento de regiões junto a aberturas).",
                    cm(band.bearing_left_m),
                    cm(band.bearing_right_m),
                    cm(crate::domain::masonry_structure::MIN_LINTEL_BEARING_M)
                ),
            ),
            Check::new(
                "Vão",
                band.clear_span_m / crate::domain::masonry_structure::MAX_LINTEL_SPAN_M,
                if band.clear_span_m <= crate::domain::masonry_structure::MAX_LINTEL_SPAN_M {
                    format!(
                        "O vão de {} está dentro do método simples.",
                        decimal(band.clear_span_m, 2)
                    )
                } else {
                    "Vão grande demais para a verga em canaleta: use uma viga calculada por engenheiro."
                        .to_owned()
                },
                format!(
                    "L = {} m; o pré-dimensionamento de vergas do Trazo para em {} m.",
                    decimal(band.clear_span_m, 2),
                    decimal(crate::domain::masonry_structure::MAX_LINTEL_SPAN_M, 1)
                ),
            ),
        ];
        if band.kind == crate::domain::masonry_structure::MasonryBandKind::Lintel {
            checks.push(Check::new(
                "Flexão",
                band.flexure_ratio,
                if band.flexure_ratio <= 1.0 {
                    format!(
                        "A verga {}×{} com {} cobre a flexão preliminar.",
                        cm(band.width_m),
                        cm(band.height_m),
                        band.bars
                    )
                } else {
                    "A verga em canaleta não basta: aumente a altura ou use uma viga independente."
                        .to_owned()
                },
                format!(
                    "qk = {} kN/m; Mk = qk·L²/8 = {} kN·m; Md = 1,4 Mk, braço z = 0,9d, aço CA-50 {} (pré-dimensionamento conservador; NBR 16868-1 e NBR 6118).",
                    decimal(band.load_kn_m, 2),
                    decimal(band.moment_knm, 2),
                    band.bars
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("{} {}", band.kind.label(), band.label),
            checks,
        });
    }
    for panel in &report.masonry.panels {
        let mut checks = vec![
            Check::new(
                "Esbeltez",
                panel.slenderness / panel.slenderness_limit,
                if panel.slenderness <= panel.slenderness_limit {
                    format!(
                        "A parede de {} para {} m de altura tem esbeltez aceitável.",
                        cm(panel.thickness_m),
                        decimal(panel.height_m, 2)
                    )
                } else {
                    format!(
                        "Parede esbelta: aumente a espessura para pelo menos {} ou trave-a na altura.",
                        cm(panel.height_m / panel.slenderness_limit)
                    )
                },
                format!(
                    "hef/t = {} / {} = {}; limite {} para alvenaria {} (modelo simplificado NBR 16868-1).",
                    decimal(panel.height_m, 2),
                    decimal(panel.thickness_m, 2),
                    decimal(panel.slenderness, 1),
                    decimal(panel.slenderness_limit, 0),
                    panel.kind.label().to_lowercase()
                ),
            ),
            Check::new(
                "Amarração lateral",
                panel.unbraced_m / panel.unbraced_limit_m,
                if panel.unbraced_m <= panel.unbraced_limit_m {
                    format!(
                        "O maior painel sem travamento tem {} m e está amarrado.",
                        decimal(panel.unbraced_m, 2)
                    )
                } else {
                    format!(
                        "Acrescente pilar ou parede de amarração para limitar os painéis a {} m.",
                        decimal(panel.unbraced_limit_m, 2)
                    )
                },
                format!(
                    "Trecho sem retorno/pilar = {} m; limite preliminar {} m.",
                    decimal(panel.unbraced_m, 2),
                    decimal(panel.unbraced_limit_m, 2)
                ),
            ),
        ];
        if panel.kind == crate::domain::masonry_structure::MasonryWallKind::LoadBearing {
            checks.push(Check::new(
                "Compressão",
                panel.compression_ratio,
                if panel.compression_ratio <= 1.0 {
                    "A compressão preliminar fica abaixo da resistência reduzida do prisma."
                        .to_owned()
                } else {
                    "A parede estrutural está sobrecarregada: aumente a espessura/resistência ou redistribua as cargas."
                        .to_owned()
                },
                format!(
                    "fd,red = {} MPa; σd = {} MPa; uso {} %. Inclui peso próprio e faixa tributária conservadora de 1 m por pavimento (NBR 16868-1).",
                    decimal(panel.compression_capacity_mpa, 2),
                    decimal(panel.compression_mpa, 2),
                    decimal(panel.compression_ratio * 100.0, 0)
                ),
            ));
        }
        if let Some(wind) = &panel.wind {
            let jamb_note = if wind.opening_jambs.is_empty() {
                String::new()
            } else {
                let specs: Vec<String> = wind
                    .opening_jambs
                    .iter()
                    .map(|j| {
                        format!(
                            "abertura {} ({:.2} m): {}×{} cm {}",
                            j.opening_id,
                            j.opening_width_m,
                            (j.width_m * 100.0).round() as i32,
                            (j.depth_m * 100.0).round() as i32,
                            j.bars
                        )
                    })
                    .collect();
                format!(" Montantes grauteados sugeridos: {}.", specs.join("; "))
            };
            checks.push(Check::new(
                "Vento fora do plano",
                if wind.ok() { wind.ratio } else { wind.ratio.max(2.0) },
                if wind.ok() {
                    "O painel alto atende no modelo com cintas ligadas aos pilares.".to_owned()
                } else {
                    "Revise os apoios, aberturas e cintas do painel alto antes de construir.".to_owned()
                },
                format!(
                    "q = {} kPa; Md = {} kN·m/m; MRd = {} kN·m/m; uso {} %. Modelo unidirecional conservador, ftk = 0,10 MPa e γm = 2,0 (NBR 16868-1, Tabelas 2 e 3); pilares nas pontas: {}; fundação: {}; sem aberturas: {}.{}",
                    decimal(wind.q_kpa, 2),
                    decimal(wind.moment_knm_m, 2),
                    decimal(wind.capacity_knm_m, 2),
                    decimal(wind.ratio * 100.0, 0),
                    wind.end_columns,
                    wind.base_supported,
                    wind.openings_clear,
                    jamb_note
                ),
            ));
            if let Some(bond) = &wind.bond_beam {
                checks.push(Check::new(
                    "Cintas intermediárias",
                    if bond.ok() {
                        bond.flexure_ratio.max(bond.shear_ratio)
                    } else {
                        2.0
                    },
                    if bond.ok() {
                        format!(
                            "Preveja {} cintas intermediárias e uma de coroamento, ligadas aos pilares.",
                            bond.intermediate_count
                        )
                    } else {
                        "A cinta proposta não cabe na seção preliminar; aumente a seção ou acrescente pilares."
                            .to_owned()
                    },
                    format!(
                        "Passo = {} m, vão máximo = {} m, seção = {}×{} cm, M = {} kN·m, V = {} kN, As = {} cm² {}.",
                        decimal(bond.spacing_m, 2),
                        decimal(bond.max_span_m, 2),
                        bond.across_m.map_or("mais de 40".to_owned(), cm),
                        cm(bond.height_m),
                        decimal(bond.moment_knm, 2),
                        decimal(bond.shear_kn, 2),
                        decimal(bond.steel_cm2, 2),
                        bond.bars
                    ),
                ));
            }
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Alvenaria {}", panel.label),
            checks,
        });
    }
    for column in &report.columns {
        let needed = column.area_needed_cm2.max(360.0);
        let side = column.width_m.min(column.depth_m);
        let mut checks = vec![
            Check::new(
                "Seção",
                needed / column.area_cm2.max(1e-6),
                if column.area_cm2 + 1e-6 >= needed {
                    format!(
                        "A seção de {:.0} cm² aguenta a carga de {:.0} kN.",
                        column.area_cm2, column.load_kn
                    )
                } else {
                    format!("Aumente o pilar para pelo menos {needed:.0} cm² de seção.")
                },
                format!(
                    "Ac = 1,4·α·N / (0,85 fcd + 1,5 %·420 MPa) = {:.0} cm² (α = {} para pilar {}), mínimo 360 cm² (NBR 6118 13.2.3); N = {:.0} kN.",
                    column.area_needed_cm2,
                    decimal(column.position.factor(), 1),
                    column.position.label().to_lowercase(),
                    column.load_kn
                ),
            ),
            Check::new(
                "Menor lado",
                0.14 / side.max(1e-6),
                if side >= 0.14 - 1e-9 {
                    format!("O menor lado de {} atende ao mínimo.", cm(side))
                } else {
                    "O pilar precisa ter pelo menos 14 cm em cada lado.".to_owned()
                },
                format!(
                    "Menor dimensão {} ≥ 14 cm (NBR 6118 13.2.3, com coeficiente adicional γn).",
                    cm(side)
                ),
            ),
            slenderness_check(column),
        ];
        if column.position == crate::domain::structure::ColumnPosition::Isolated {
            checks.push(Check::fail(
                "Vigas",
                "Nenhuma viga chega a este pilar: ligue-o às vigas ou remova-o.".to_owned(),
                "Pilar sem vigas: a carga calculada é só o peso próprio.".to_owned(),
            ));
        }
        if let Some(reinf) = reinforcement
            .columns
            .iter()
            .find(|r| r.column_id == column.id)
        {
            let ratio_pct = reinf.steel_ratio * 100.0;
            checks.push(Check::new(
                "Taxa de aço",
                reinf.steel_ratio / 0.04,
                if reinf.steel_ratio <= 0.04 + 1e-9 {
                    format!(
                        "Taxa longitudinal de {}% ({}, estribos {}) dentro do limite.",
                        decimal(ratio_pct, 2),
                        reinf.longitudinal.notation(),
                        reinf.ties.notation()
                    )
                } else {
                    format!(
                        "Taxa longitudinal de {}% excede 4,0%: aumente a seção do pilar.",
                        decimal(ratio_pct, 2)
                    )
                },
                format!(
                    "ρ = {}% (mínimo 0,4%, limite de pré-detalhamento 4,0%; NBR 6118 §17.3.5.3); longitudinal {} com traspasse l0 = {:.0} cm, estribos {} ({} un., massa {} kg).",
                    decimal(ratio_pct, 2),
                    reinf.longitudinal.notation(),
                    reinf.lap_length_m * 100.0,
                    reinf.ties.notation(),
                    reinf.ties.count,
                    decimal(reinf.mass_kg, 1)
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Pilar {}", column.label),
            checks,
        });
    }
    for footing in &report.footings {
        let mut checks = Vec::new();
        if footing.column.is_none() {
            checks.push(Check::fail(
                "Pilar",
                "Esta sapata não tem pilar em cima: mova-a para baixo de um pilar.".to_owned(),
                "Sapata sem pilar: não há carga para dimensionar.".to_owned(),
            ));
        } else {
            let area = footing.width_m * footing.depth_m;
            let needed = footing.needed_width_m * footing.needed_depth_m;
            checks.push(Check::new(
                "Área",
                needed / area.max(1e-9),
                if area + 1e-6 >= needed {
                    format!(
                        "A sapata de {} × {} distribui bem a carga no solo.",
                        cm(footing.width_m),
                        cm(footing.depth_m)
                    )
                } else {
                    format!(
                        "Aumente a sapata para {} × {} (use Ajustar sapatas).",
                        cm(footing.needed_width_m),
                        cm(footing.needed_depth_m)
                    )
                },
                format!(
                    "A = 1,05·N / σadm = 1,05 × {:.0} kN / {:.0} kPa = {} m² (tem {} m²).",
                    footing.load_kn,
                    footing.soil_kpa,
                    decimal(needed, 2),
                    decimal(area, 2)
                ),
            ));
            checks.push(Check::new(
                "Pressão no solo",
                footing.max_pressure_kpa / footing.soil_kpa.max(1e-9),
                if footing.max_pressure_kpa <= footing.soil_kpa + 1e-3 {
                    "A pressão no solo fica dentro do admissível.".to_owned()
                } else {
                    "O solo fica sobrecarregado: centralize a sapata sob o pilar ou aumente-a.".to_owned()
                },
                format!(
                    "σmax = {:.0} kPa ≤ σadm = {:.0} kPa, excentricidade {} (regra do terço médio, NBR 6122).",
                    footing.max_pressure_kpa,
                    footing.soil_kpa,
                    cm(footing.eccentricity_m)
                ),
            ));
            checks.push(Check::new(
                "Punção",
                footing.punching_ratio,
                if footing.punching_ratio <= 1.0 {
                    "A sapata resiste à punção do pilar.".to_owned()
                } else {
                    "O pilar pode furar a sapata: aumente a altura dela.".to_owned()
                },
                format!(
                    "τSd/τRd = {} na face do pilar e a d/2 (NBR 6118 19.5); altura {}.",
                    decimal(footing.punching_ratio, 2),
                    cm(footing.height_m)
                ),
            ));
            if footing.overturning_fs.is_finite() {
                checks.push(Check::new(
                    "Tombamento",
                    1.5 / footing.overturning_fs.max(1e-9),
                    if footing.overturning_fs >= 1.5 {
                        "Não há risco de tombamento.".to_owned()
                    } else {
                        "Risco de tombar: centralize a sapata ou use uma viga de equilíbrio."
                            .to_owned()
                    },
                    format!(
                        "FS = {} ≥ 1,5 (NBR 6122).",
                        decimal(footing.overturning_fs, 2)
                    ),
                ));
            }
            let s_limit = crate::domain::soil::MAX_FOOTING_SETTLEMENT_MM;
            checks.push(Check::new(
                "Recalque",
                footing.settlement_mm / s_limit,
                if footing.settlement_mm <= s_limit + 1e-6 {
                    format!(
                        "Recalque elástico estimado de {} mm dentro do limite de {:.0} mm.",
                        decimal(footing.settlement_mm, 1),
                        s_limit
                    )
                } else {
                    "Recalque acima do limite: aumente a sapata ou mude para radier/estacas."
                        .to_owned()
                },
                format!(
                    "s = q·B·(1−ν²)·Is/Es = {} mm ≤ {:.0} mm (estimativa elástica imediata, NBR 6122 / B11).",
                    decimal(footing.settlement_mm, 1),
                    s_limit
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Sapata {}", footing.label),
            checks,
        });
    }
    for cap in &report.pile_caps {
        let mut checks = Vec::new();
        if cap.column.is_none() {
            checks.push(Check::fail(
                "Pilar",
                "Este bloco sobre broca não tem pilar em cima: mova-o para baixo de um pilar."
                    .to_owned(),
                "Bloco sobre broca sem pilar: não há carga para dimensionar.".to_owned(),
            ));
        } else {
            checks.push(Check::new(
                "Capacidade da broca (compressão)",
                cap.load_kn / cap.allowable_kn.max(1e-9),
                if cap.compression_ok() {
                    format!(
                        "{}× broca Ø{:.0} cm × {} m aguenta {} kN (capacidade {} kN).",
                        cap.pile_count,
                        cap.pile_diameter_m * 100.0,
                        decimal(cap.pile_depth_m, 1),
                        decimal(cap.load_kn, 1),
                        decimal(cap.allowable_kn, 1)
                    )
                } else {
                    format!(
                        "Carga de {} kN acima da capacidade das brocas ({} kN): aumente o diâmetro, a profundidade ou use mais brocas.",
                        decimal(cap.load_kn, 1),
                        decimal(cap.allowable_kn, 1)
                    )
                },
                format!(
                    "Décourt-Quaresma (1996) / NBR 6122: {}× Ø{:.0} cm × {} m (ponta {} kN + atrito {} kN na ruptura → admissível {} kN; recalque est. {} mm); bloco {} × {} × {}.",
                    cap.pile_count,
                    cap.pile_diameter_m * 100.0,
                    decimal(cap.pile_depth_m, 1),
                    decimal(cap.tip_rupture_kn, 1),
                    decimal(cap.shaft_rupture_kn, 1),
                    decimal(cap.allowable_kn, 1),
                    decimal(cap.settlement_mm, 1),
                    cm(cap.width_m),
                    cm(cap.depth_m),
                    cm(cap.height_m)
                ),
            ));
            if !cap.above_water_table {
                checks.push(Check::fail(
                    "Lençol freático",
                    format!(
                        "Broca de {} m atinge o lençol d'água ({} m): broca manual só funciona em solo seco — troque por estaca mecanizada.",
                        decimal(cap.pile_depth_m, 1),
                        decimal(cap.water_table_m.unwrap_or(0.0), 1)
                    ),
                    format!(
                        "Profundidade da broca {} m > NA {} m (NBR 6122 exige perfuração seca para brocas manuais).",
                        decimal(cap.pile_depth_m, 1),
                        decimal(cap.water_table_m.unwrap_or(0.0), 1)
                    ),
                ));
            }
            if cap.uplift_kn > 1e-6 {
                if !cap.uplift_anchored() {
                    checks.push(Check::fail(
                        "Arrancamento (vento)",
                        format!(
                            "Desça a ferragem do pilar por pelo menos 1,50 m dentro da broca (atual {} m) para resistir à tração de {} kN.",
                            decimal(cap.anchor_depth_m, 2),
                            decimal(cap.uplift_kn, 1)
                        ),
                        format!(
                            "Arranque atual {} m < 1,50 m mínimo para mobilizar atrito lateral sob vento ELU-EQU ({} kN).",
                            decimal(cap.anchor_depth_m, 2),
                            decimal(cap.uplift_kn, 1)
                        ),
                    ));
                } else {
                    checks.push(Check::new(
                        "Arrancamento (vento)",
                        cap.uplift_kn / cap.uplift_capacity_kn.max(1e-9),
                        if cap.uplift_ok() {
                            format!(
                                "Arranque de {} m na broca segura a tração de vento ({} kN ≤ {} kN).",
                                decimal(cap.anchor_depth_m, 2),
                                decimal(cap.uplift_kn, 1),
                                decimal(cap.uplift_capacity_kn, 1)
                            )
                        } else {
                            format!(
                                "Tração de vento ({} kN) excede o atrito lateral admissível ({} kN): aumente a profundidade ou o diâmetro da broca.",
                                decimal(cap.uplift_kn, 1),
                                decimal(cap.uplift_capacity_kn, 1)
                            )
                        },
                        format!(
                            "NBR 6122 §8.2.2: R_tração,adm = 0,70·Q_lat / 2,0 = {} kN (atrito {} kN × {}, arranque {} m ≥ 1,50 m).",
                            decimal(cap.uplift_capacity_kn, 1),
                            decimal(cap.shaft_rupture_kn, 1),
                            cap.pile_count,
                            decimal(cap.anchor_depth_m, 2)
                        ),
                    ));
                }
            }
            if !cap.uses_survey {
                checks.push(Check::new(
                    "Sondagem SPT",
                    0.90,
                    "Sem sondagem SPT cadastrada: confirme o tipo de solo e o lençol d'água antes de perfurar."
                        .to_owned(),
                    "Capacidade estimada conservadoramente a partir da tensão admissível configurada; informe o SPT para cálculo geotécnico completo."
                        .to_owned(),
                ));
            }
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Bloco sobre broca {}", cap.label),
            checks,
        });
    }
    for load in &report.concentrated_loads {
        let check = if !load.supported {
            Check::fail(
                "Caminho de carga",
                "A carga está sem apoio: mova-a sobre uma laje apoiada ou diretamente sobre vigas."
                    .to_owned(),
                format!(
                    "{} de {} kN em ({}, {}) não alcança laje ou viga.",
                    load.kind.label(),
                    decimal(load.load_kn, 1),
                    decimal(load.center.x, 2),
                    decimal(-load.center.y, 2)
                ),
            )
        } else if load.slab.is_some() && load.punching_ratio.is_none() {
            Check::new(
                "Verificação local",
                1.0,
                "A carga chega às vigas, mas a laje treliçada precisa de faixa maciça ou vigotas confirmadas pelo fabricante."
                    .to_owned(),
                format!(
                    "{}; momento local adicional {} kN·m/m. Punção não é verificável sem a geometria das nervuras.",
                    load.support,
                    decimal(load.local_moment_knm_m, 1)
                ),
            )
        } else if let Some(ratio) = load.punching_ratio {
            Check::new(
                "Punção local",
                ratio,
                if ratio <= 1.0 {
                    "A base da carga não excede a resistência preliminar à punção da laje."
                        .to_owned()
                } else {
                    "A carga pode perfurar a laje: aumente a base ou a espessura, ou apoie-a sobre vigas."
                        .to_owned()
                },
                format!(
                    "τSd/τRd = {} ao redor da base; momento local adicional {} kN·m/m (modelo preliminar NBR 6118 19.5).",
                    decimal(ratio, 2),
                    decimal(load.local_moment_knm_m, 1)
                ),
            )
        } else {
            Check::new(
                "Caminho de carga",
                0.0,
                "A carga está aplicada diretamente sobre uma viga e entra na análise.".to_owned(),
                format!(
                    "{}; carga característica {} kN.",
                    load.support,
                    decimal(load.load_kn, 1)
                ),
            )
        };
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Carga {} ({})", load.label, load.kind.label()),
            checks: vec![check],
        });
    }
    for roof in &report.roofs {
        for element in &roof.elements {
            let section = format!(
                "{} {:.0}×{:.0} cm a cada {} m",
                element.element.label(),
                element.section_cm.0,
                element.section_cm.1,
                decimal(element.spacing_m, 2)
            );
            verdicts.push(Verdict {
                discipline: Discipline::Structure,
                element: format!("Telhado {} · {}", roof.label, element.element.label()),
                checks: vec![
                    Check::new(
                        "Flexão",
                        element.bending_ratio,
                        format!("{section}, vão de {} m.", decimal(element.span_m, 2)),
                        format!(
                            "σ = Md / W ≤ fc0,d = kmod·fc0,k / 1,4 (NBR 7190), Md = 1,4·(g + q)·L²/8, q = {} kN/m.",
                            decimal(element.load_kn_m, 2)
                        ),
                    ),
                    Check::new(
                        "Cisalhamento",
                        element.shear_ratio,
                        format!("{section}."),
                        "τ = 1,5·Vd / (b·h) ≤ fv0,d = kmod·fv0,k / 1,8 (NBR 7190).".to_owned(),
                    ),
                    Check::new(
                        "Flecha",
                        element.deflection_ratio,
                        if element.deflection_ratio <= 1.0 + 1e-9 {
                            format!(
                                "{section}: flecha de {} mm, dentro de L/200 = {} mm; sem embarrigamento excessivo no modelo.",
                                decimal(element.deflection_m * 1000.0, 1),
                                decimal(element.deflection_limit_m * 1000.0, 1)
                            )
                        } else {
                            format!(
                                "{section}: flecha de {} mm passa de L/200 = {} mm; risco de embarrigamento, aumente a altura ou reduza o v\u{00e3}o.",
                                decimal(element.deflection_m * 1000.0, 1),
                                decimal(element.deflection_limit_m * 1000.0, 1)
                            )
                        },
                        "u = 5·(g + 0,2 q)·L⁴ / (384·Ec0,ef·I) ≤ L/200, Ec0,ef = kmod·Ec0,m (NBR 7190).".to_owned(),
                    ),
                ],
            });
        }
        let loose = roof.unsupported();
        let mut checks = vec![Check::new(
            "Inclinação",
            roof.min_slope_percent / roof.slope_percent.max(1e-6),
            if roof.slope_ok() {
                format!("{:.0} % atende à telha.", roof.slope_percent)
            } else {
                format!(
                    "Aumente a inclinação para pelo menos {:.0} %.",
                    roof.min_slope_percent
                )
            },
            format!(
                "Inclinação {:.0} % ≥ {:.0} % mínima usual da telha (confirme com o fabricante).",
                roof.slope_percent, roof.min_slope_percent
            ),
        )];
        if loose > 0 {
            checks.push(Check::fail(
                "Apoios",
                format!("{loose} ponta(s) de viga sem pilar, mourão ou viga embaixo."),
                "Cada viga do telhado descarrega metade do seu peso em cada ponta.".to_owned(),
            ));
        }
        if roof.overhang_m > 0.01 {
            checks.push(Check::new(
                "Beiral (balanço)",
                roof.overhang_bending_ratio,
                if roof.overhang_bending_ratio <= 1.0 {
                    format!("Beiral de {} m atende à flexão.", decimal(roof.overhang_m, 2))
                } else {
                    format!(
                        "Beiral de {} m excede a resistência: reduza o beiral ou aumente os caibros.",
                        decimal(roof.overhang_m, 2)
                    )
                },
                format!(
                    "Flexão em balanço no beiral ({} m): uso {:.0} % (NBR 7190).",
                    decimal(roof.overhang_m, 2),
                    roof.overhang_bending_ratio * 100.0
                ),
            ));
        }
        if let Some(truss) = &roof.truss {
            let max_ratio = truss
                .members
                .iter()
                .map(|m| m.ratio)
                .fold(0.0_f64, f64::max);
            checks.push(Check::new(
                "Tesouras",
                max_ratio,
                if truss.ok {
                    format!("{} ({} un.) atende à NBR 7190.", truss.kind, truss.count)
                } else {
                    "Tesoura acima do limite: reduza o espaçamento ou suba a classe da madeira."
                        .to_owned()
                },
                format!(
                    "{} · vão {} m · altura {} m · uso máx. {:.0} %.",
                    truss.kind,
                    decimal(truss.span_m, 2),
                    decimal(truss.rise_m, 2),
                    max_ratio * 100.0
                ),
            ));
        }
        if let Some(hip) = &roof.hip_rafters {
            let r = hip
                .bending_ratio
                .max(hip.shear_ratio)
                .max(hip.deflection_ratio);
            checks.push(Check::new(
                "Espigões",
                r,
                if hip.ok {
                    format!(
                        "Espigões {:.0}×{:.0} cm atendem às 4 águas.",
                        hip.section_cm.0, hip.section_cm.1
                    )
                } else {
                    "Espigões insuficientes: reduza o vão ou eleve a classe da madeira.".to_owned()
                },
                format!(
                    "4 espigões {:.0}×{:.0} cm · vão {} m · uso {:.0} % (NBR 7190).",
                    hip.section_cm.0,
                    hip.section_cm.1,
                    decimal(hip.span_m, 2),
                    r * 100.0
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Telhado {}", roof.label),
            checks,
        });
    }
    for column in &report.wind.columns {
        let mut checks = vec![Check::new(
            "Flexão (vento)",
            column.ratio,
            if column.ratio <= 1.0 + 1e-9 {
                format!(
                    "{} cm atravessando a parede seguram o vento ({}), {} cm² por face.",
                    decimal(column.depth_m * 100.0, 0),
                    if column.braced {
                        "topo travado pelo telhado"
                    } else {
                        "em balanço"
                    },
                    decimal(column.steel_cm2, 2)
                )
            } else {
                format!(
                    "Gire ou aumente o pilar: {} atravessando a parede{}.",
                    column
                        .needed_depth_m
                        .map_or("mais de 80 cm".to_owned(), |h| format!(
                            "{:.0} cm",
                            h * 100.0
                        )),
                    if column.braced {
                        ""
                    } else {
                        ", ou trave o topo com o telhado contraventado"
                    }
                )
            },
            format!(
                "NBR 6123: q = {} kN/m², Cp = 1,3, faixa de {} m, H = {} m; M = w·H²/{} = {} kN·m, Md = 1,4 M, x/d ≤ 0,45.",
                decimal(column.q_kpa, 2),
                decimal(column.tributary_m, 2),
                decimal(column.height_m, 2),
                if column.braced { 8 } else { 2 },
                decimal(column.moment_knm, 1)
            ),
        )];
        if let Some(footing_ratio) = column.footing_wind_ratio {
            let e_cm = column.footing_eccentricity_m.unwrap_or(0.0) * 100.0;
            let p_max = column.footing_max_kpa.unwrap_or(0.0);
            checks.push(Check::new(
                "Sapata sob vento",
                footing_ratio,
                if footing_ratio <= 1.0 + 1e-9 {
                    format!(
                        "A sapata absorve o momento do vento (e = {} cm, σmáx = {:.0} kPa).",
                        decimal(e_cm, 1),
                        p_max
                    )
                } else {
                    "A sapata sofre excentricidade ou tensão excessiva sob vento: aumente a sapata ou trave o topo do pilar.".to_owned()
                },
                format!(
                    "NBR 6122 sob vento (0,9·N + M_vento): e = {} cm (limite B/3), σmáx = {:.0} kPa (limite 1,3·σadm).",
                    decimal(e_cm, 1),
                    p_max
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Pilar {} · vento", column.label),
            checks,
        });
    }
    for roof in &report.wind.roofs {
        let light = if roof.lifts() {
            Light::Yellow
        } else {
            Light::Green
        };
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Telhado {} · vento", roof.label),
            checks: vec![Check {
                name: "Sucção".to_owned(),
                utilisation: 1.4 * roof.uplift_kpa / (0.9 * roof.dead_kpa).max(1e-9),
                light,
                simple: if roof.lifts() {
                    format!(
                        "O vento levanta o telhado: ancore cada ponta de viga para {} kN ({}) e amarre as telhas.",
                        decimal(roof.anchor_kn, 1),
                        roof.anchor_spec
                    )
                } else {
                    "O peso do telhado vence a sucção do vento.".to_owned()
                },
                detail: format!(
                    "NBR 6123: q = {} kN/m², Cpe = −1,0, Cpi = {}; 1,4·{} − 0,9·{} = {} kN/m²; ligação sugerida: {} (interação N+V NBR 8800 = {:.2}).",
                    decimal(roof.q_kpa, 2),
                    if roof.open { "+0,8 (lado aberto)" } else { "0" },
                    decimal(roof.uplift_kpa, 2),
                    decimal(roof.dead_kpa, 2),
                    decimal(roof.net_design_kpa, 2),
                    roof.anchor_spec,
                    roof.anchor_interaction
                ),
            }],
        });
    }
    for brace in &report.wind.bracing {
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Telhado {} · contraventamento", brace.label),
            checks: vec![Check::new(
                "Caminho lateral",
                if brace.path_ok { brace.capacity_kn.map_or(2.0, |kn| brace.demand_kn / kn.max(1e-9)) } else { 2.0 },
                if brace.ok() {
                    format!(
                        "A resistência informada cobre o esforço de cálculo até as fundações (sugestão: {}).",
                        brace.tie_spec
                    )
                } else {
                    format!(
                        "Verifique o contraventamento completo para {:.1} kN ({}) e ancore pelo menos dois apoios às fundações.",
                        brace.demand_kn,
                        brace.tie_spec
                    )
                },
                format!("Demanda {:.1} kN; capacidade verificada {}; tirante em X sugerido: {}. A resistência deve incluir plano do telhado, ligações, apoios e fundações.",
                    brace.demand_kn,
                    brace.capacity_kn.map_or("não informada".to_owned(), |kn| format!("{kn:.1} kN")),
                    brace.tie_spec),
            )],
        });
    }
    for global in &report.wind.global {
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Estabilidade global {}", global.direction),
            checks: vec![
                Check::new("Apoios e travamento", if global.supports_ok && global.bracing_ok { 0.0 } else { 2.0 },
                    if global.supports_ok && global.bracing_ok { "Caminho lateral e apoios declarados." } else { "Faltam fundações, apoios do telhado ou resistência de contraventamento." }.to_owned(),
                    "O esforço horizontal precisa chegar às fundações; o simples apoio vertical da cobertura não trava os pilares.".to_owned()),
                Check::new("Instabilidade global α", global.alpha / global.alpha_limit,
                    format!("α = {:.2}; limite {:.2} para até três pavimentos.", global.alpha, global.alpha_limit),
                    "Triagem conservadora de nós fixos (NBR 6118 §15.5.2), com 0,4 EcsIc dos pilares engastados; não substitui análise de segunda ordem.".to_owned()),
                Check::new("Deslocamento lateral", global.drift_mm / global.drift_limit_mm,
                    format!("Flecha lateral {:.1} mm; limite H/850 = {:.1} mm.", global.drift_mm, global.drift_limit_mm),
                    format!("Vento na fachada de {:.2} m × {:.2} m: {:.1} kN na direção {}.", global.facade_m, global.height_m, global.wind_kn, global.direction)),
                Check::new("Tombamento global", 1.0 / global.overturning_ratio.max(1e-9),
                    format!("Momento estabilizante {:.1} kN·m para {:.1} kN·m de vento de cálculo.", global.stabilizing_knm, 1.4 * global.overturning_knm),
                    format!("Triagem de corpo rígido com 0,9 G favorável, 1,4 W lateral e 1,4 U de sucção (U = {:.1} kN).", global.roof_uplift_kn)),
                Check::new("Deslizamento no solo", 1.50 / global.sliding_fs.max(1e-6),
                    if global.sliding_fs >= 1.50 - 1e-9 {
                        format!("Atrito na base (FS = {:.2} ≥ 1,50) resiste ao vento horizontal.", global.sliding_fs)
                    } else {
                        format!("Risco de deslizamento sob vento (FS = {:.2} < 1,50): aumente o peso das fundações ou preveja travamento/estacas.", global.sliding_fs)
                    },
                    format!("NBR 6122: μ = {:.2}, F_res = μ·(0,9·G − 1,4·U) = {:.1} kN para Wk = {:.1} kN → FS = {:.2} ≥ 1,50.", global.friction_mu, global.sliding_resistance_kn, global.wind_kn, global.sliding_fs)),
            ],
        });
    }
    for post in &report.timber_posts {
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Mourão {}", post.label),
            checks: vec![
                Check::new(
                    "Flexocompressão com vento",
                    post.ratio,
                    if post.ratio <= 1.0 + 1e-9 {
                        format!(
                            "{} {} cm em {} ({}) com {} m aguenta {} kN e o vento.",
                            if post.round { "Ø" } else { "Seção" },
                            decimal(post.size_m * 100.0, 0),
                            post.species.label(),
                            post.timber_class.label(),
                            decimal(post.height_m, 2),
                            decimal(post.load_kn, 1)
                        )
                    } else {
                        match post.needed_m {
                            Some(size) => format!("Use {} cm.", decimal(size * 100.0, 0)),
                            None => "Nem 30 cm resiste: diminua a altura ou a carga.".to_owned(),
                        }
                    },
                    format!(
                        "NBR 7190: uso axial/flambagem = {} e uso por vento = {}, soma = {}; λ = L0/i = {}; q = {} kN/m², Fw = {} kN e Mw = {} kN·m; N = {} kN.",
                        decimal(post.axial_ratio, 2),
                        decimal(post.wind_ratio, 2),
                        decimal(post.ratio, 2),
                        decimal(post.slenderness, 0),
                        decimal(post.wind_kpa, 2),
                        decimal(post.wind_force_kn, 1),
                        decimal(post.wind_moment_knm, 1),
                        decimal(post.load_kn, 1)
                    ),
                ),
                Check::new(
                    "Esbeltez",
                    post.slenderness / crate::domain::timber_posts::MAX_POST_SLENDERNESS,
                    format!("λ = {:.0}, no máximo 140.", post.slenderness),
                    "NBR 7190: peças comprimidas com λ ≤ 140.".to_owned(),
                ),
                Check::new(
                    "Durabilidade",
                    if post.treated { 0.0 } else { 2.0 },
                    if post.treated {
                        "Madeira externa tratada e afastada do solo pelo chumbador.".to_owned()
                    } else {
                        "Especifique tratamento preservativo antes de construir.".to_owned()
                    },
                    "Madeira externa deve receber tratamento compatível com a classe de uso; a espécie não substitui a comprovação do lote nem o tratamento e a peça não toca o solo.".to_owned(),
                ),
                Check::new(
                    "Base",
                    (post.base_kpa / post.soil_kpa.max(1e-6))
                        .max(post.base_stability_ratio),
                    format!(
                        "Bloco de concreto {}×{} cm; chumbador para {:.1} kN e madeira sem contato com o solo.",
                        decimal(post.base_m * 100.0, 0),
                        decimal(post.base_depth_m * 100.0, 0),
                        post.anchor_kn
                    ),
                    format!(
                        "Bloco + 1,05 N: σmáx = {:.0} kPa ≤ {:.0} kPa e 6 Mw/(Ntotal·B) = {}; chumbador = 1,4 Mw/(0,8 d).",
                        post.base_kpa,
                        post.soil_kpa,
                        decimal(post.base_stability_ratio, 2)
                    ),
                ),
            ],
        });
    }
    for beam in &report.eave_beams {
        let section = format!(
            "{:.0}×{:.0} cm entre {} e {}",
            beam.section_cm.0, beam.section_cm.1, beam.from_post, beam.to_post
        );
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Viga de madeira {}", beam.label),
            checks: vec![
                Check::new(
                    "Flexão",
                    beam.bending_ratio,
                    format!("{section}, vão de {} m.", decimal(beam.span_m, 2)),
                    "σ = 1,4 M / W ≤ fc0,d (NBR 7190), cargas das vigas do telhado e peso próprio.".to_owned(),
                ),
                Check::new(
                    "Cisalhamento",
                    beam.shear_ratio,
                    format!("{section}."),
                    "τ = 1,5·1,4 V / (b·h) ≤ fv0,d (NBR 7190).".to_owned(),
                ),
                Check::new(
                    "Flecha",
                    beam.deflection_ratio,
                    format!("{section}."),
                    "Soma das flechas máximas de cada carga e do peso próprio ≤ L/200, Ec0,ef = kmod·Ec0,m.".to_owned(),
                ),
                Check::new(
                    "Durabilidade",
                    if beam.treated { 0.0 } else { 2.0 },
                    if beam.treated {
                        format!(
                            "{} ({}) com tratamento preservativo.",
                            beam.species.label(),
                            beam.timber_class.label()
                        )
                    } else {
                        "Especifique tratamento preservativo para a viga externa.".to_owned()
                    },
                    "A espécie define uma classe preliminar; o lote entregue deve ser classificado e receber tratamento compatível com a classe de uso.".to_owned(),
                ),
            ],
        });
    }
    for foundation in &report.wall_foundations {
        let check = match foundation.kind {
            crate::domain::wall_foundations::WallFoundationKind::GroundBeam => {
                match foundation.height_m {
                    Some(h) => Check::new(
                        "Seção",
                        foundation.flexure_ratio.max(foundation.shear_ratio),
                        format!(
                            "Baldrame {}×{} cm com {} leva a parede até as sapatas.",
                            decimal(foundation.width_m * 100.0, 0),
                            decimal(h * 100.0, 0),
                            foundation.bars
                        ),
                        format!(
                            "q = {} kN/m (parede e peso próprio), M = {} kN·m, V = {} kN; viga contínua sobre os pilares, Md = 1,4 M, x/d ≤ 0,45; As = {} cm².",
                            decimal(foundation.load_kn_m, 1),
                            decimal(foundation.moment_knm, 1),
                            decimal(foundation.shear_kn, 1),
                            decimal(foundation.steel_cm2, 2)
                        ),
                    ),
                    None => Check::fail(
                        "Seção",
                        "Nenhum baldrame até 80 cm resiste: acrescente pilares nessa parede."
                            .to_owned(),
                        format!("M = {} kN·m.", decimal(foundation.moment_knm, 1)),
                    ),
                }
            }
            crate::domain::wall_foundations::WallFoundationKind::StripFooting => Check::new(
                "Pressão no solo",
                foundation.pressure_kpa / foundation.soil_kpa.max(1e-6),
                if foundation.pressure_kpa <= foundation.soil_kpa + 1e-6 {
                    format!(
                        "A sapata corrida de {} cm distribui o peso da parede no solo.",
                        decimal(foundation.width_m * 100.0, 0)
                    )
                } else {
                    format!(
                        "Alargue a sapata corrida para {} cm.",
                        decimal(foundation.needed_width_m * 100.0, 0)
                    )
                },
                format!(
                    "B = 1,05·q / σadm = 1,05 × {} kN/m / {:.0} kPa = {} cm (mínimo 40 cm); σ = {:.0} kPa.",
                    decimal(foundation.load_kn_m, 1),
                    foundation.soil_kpa,
                    decimal(foundation.needed_width_m * 100.0, 0),
                    foundation.pressure_kpa
                ),
            ),
        };
        let mut checks = vec![check];
        if foundation.unsupported {
            checks.push(Check::fail(
                "Apoios",
                "O baldrame não tem caminho de carga até dois apoios; revise as ligações ou mantenha a sapata corrida.".to_owned(),
                "A solicitação de baldrame foi substituída por sapata corrida porque a rede de apoios não alcança pilares suficientes.".to_owned(),
            ));
        }
        for (column_id, reaction) in &foundation.reactions {
            if *reaction < -1e-6 {
                checks.push(Check::fail(
                    "Apoio",
                    format!(
                        "O baldrame produz reação ascendente no pilar {}; revise a geometria e a ancoragem.",
                        column_id
                    ),
                    format!(
                        "Reação vertical do baldrame no pilar {} = {} kN (ascendente); verifique a ligação e a estabilidade com as demais cargas.",
                        column_id,
                        decimal(*reaction, 1)
                    ),
                ));
            }
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("{} {}", foundation.kind.label(), foundation.label),
            checks,
        });
    }
    for beam in &report.balance_beams {
        let min_fs = crate::domain::foundations::PAIR_OVERTURNING_MIN_FS;
        let overturning_util = if beam.pair_overturning_fs.is_infinite() {
            0.0
        } else {
            min_fs / beam.pair_overturning_fs.max(1e-6)
        };
        let strip_hint = beam
            .strip_alternative
            .as_ref()
            .map_or(String::new(), |alt| format!(" {}", alt.note));
        let mut checks = vec![Check::new(
            "Tombamento do par",
            overturning_util,
            if beam.uplift() {
                format!(
                    "O pilar {} é levantado pela viga (FS = {} < 1,5): ponha contrapeso nele ou troque por sapata corrida.{strip_hint}",
                    beam.anchor,
                    decimal(beam.pair_overturning_fs, 2)
                )
            } else if !beam.overturning_ok() {
                format!(
                    "Fator de segurança ao tombamento do par FS = {} abaixo de 1,5 (NBR 6122): aumente o contrapeso em {} ou adote sapata corrida.{strip_hint}",
                    decimal(beam.pair_overturning_fs, 2),
                    beam.anchor
                )
            } else {
                format!(
                    "O par {} → {} resiste ao tombamento sem levantar o apoio interno.",
                    beam.column, beam.anchor
                )
            },
            format!(
                "R1 = P1·l/(l − e) = {} kN (P1 = {} kN, l = {} m, e = {} cm); ΔP = {} kN, N2 = {} kN; FS = N2/ΔP = {} ≥ 1,5 (NBR 6122).",
                decimal(beam.reaction_kn, 1),
                decimal(beam.column_load_kn, 1),
                decimal(beam.span_m, 2),
                decimal(beam.eccentricity_m * 100.0, 0),
                decimal(beam.relief_kn, 1),
                decimal(beam.anchor_load_kn, 1),
                if beam.pair_overturning_fs.is_infinite() {
                    "∞".to_owned()
                } else {
                    decimal(beam.pair_overturning_fs, 2)
                }
            ),
        )];
        if beam.boundary_pressure_kpa.is_some() || beam.anchor_pressure_kpa.is_some() {
            checks.push(Check::new(
                "Solo do par",
                beam.pair_soil_ratio(),
                if beam.pair_soil_ok() {
                    "As duas sapatas do par ficam dentro da tensão admissível do solo.".to_owned()
                } else {
                    format!(
                        "Tensão no solo do par excede {:.0} kPa: aumente as sapatas (Ajustar sapatas).{strip_hint}",
                        beam.soil_kpa
                    )
                },
                format!(
                    "Divisa ({}) σ1 = {} kPa, apoio interno ({}) σ2 = {} kPa ≤ σadm = {:.0} kPa (com metade de ΔP descontada no apoio interno, NBR 6122).",
                    beam.column,
                    beam.boundary_pressure_kpa
                        .map_or("—".to_owned(), |v| format!("{v:.0}")),
                    beam.anchor,
                    beam.anchor_pressure_kpa
                        .map_or("—".to_owned(), |v| format!("{v:.0}")),
                    beam.soil_kpa
                ),
            ));
        }
        match beam.height_m {
            Some(h) => checks.push(Check::new(
                "Seção",
                beam.flexure_ratio.max(beam.shear_ratio),
                format!(
                    "Viga {}×{} cm com {} e estribos {} resiste ao momento de {} kN·m.",
                    decimal(beam.width_m * 100.0, 0),
                    decimal(h * 100.0, 0),
                    beam.bars,
                    beam.stirrups,
                    decimal(beam.moment_knm, 1)
                ),
                format!(
                    "M = P1·e = {} kN·m, V = {} kN; Md = 1,4 M, x/d ≤ 0,45, τwd ≤ τRd2; As = {} cm² (CA-50), estribos {}.",
                    decimal(beam.moment_knm, 1),
                    decimal(beam.shear_kn, 1),
                    decimal(beam.top_steel_cm2, 2),
                    beam.stirrups
                ),
            )),
            None => checks.push(Check::fail(
                "Seção",
                "Nenhuma seção até 120 cm resiste: aproxime o pilar interno ou alargue a viga."
                    .to_owned(),
                format!("M = {} kN·m.", decimal(beam.moment_knm, 1)),
            )),
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!(
                "Viga alavanca {} ({} → {})",
                beam.label, beam.column, beam.anchor
            ),
            checks,
        });
    }
    for misaligned in &report.misaligned_columns {
        let element = format!(
            "Pilar {} ({})",
            misaligned.column_label, misaligned.floor_name
        );
        let check = match &misaligned.transfer_beam {
            Some(beam) => Check::new(
                "Transição",
                1.0,
                format!("O pilar nasce sobre a viga {beam}: um engenheiro precisa detalhar essa viga de transição."),
                format!("Carga de {:.0} kN aplicada como carga concentrada em {beam}.", misaligned.load_kn),
            ),
            None => Check::fail(
                "Prumada",
                "Não há pilar nem viga embaixo deste pilar: alinhe-o com um pilar do andar de baixo.".to_owned(),
                format!("Carga de {:.0} kN sem apoio no pavimento inferior.", misaligned.load_kn),
            ),
        };
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element,
            checks: vec![check],
        });
    }
    for stair in &report.cantilevers.stairs {
        let mut checks = vec![Check::new(
            "Flecha (L/250)",
            stair.deflection_mm / stair.limit_mm.max(1e-6),
            if stair.deflection_mm <= stair.limit_mm {
                format!(
                    "Escada atende (espessura {}, As inf. {} cm²/m).",
                    cm(stair.waist_m),
                    decimal(stair.as_bottom_cm2_m, 2)
                )
            } else {
                "Aumente a espessura da laje da escada ou reduza o lance livre.".to_owned()
            },
            format!(
                "Lance {} m · h={} · Md={} kN·m/m · δ={} / {} mm (NBR 6118).",
                decimal(stair.flight_span_m, 2),
                cm(stair.waist_m),
                decimal(stair.moment_design_knm_m, 1),
                decimal(stair.deflection_mm, 1),
                decimal(stair.limit_mm, 1)
            ),
        )];
        if stair.flight_span_m > 5.5 {
            checks.push(Check::fail(
                "Vão",
                "Lance acima de 5,5 m: adicione patamar apoiado ou viga intermediária.".to_owned(),
                format!("Lance {} m > 5,5 m.", decimal(stair.flight_span_m, 2)),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Escada {} ({})", stair.label, stair.name),
            checks,
        });
    }
    for bal in &report.cantilevers.balconies {
        let mut checks = vec![
            Check::new(
                "Espessura",
                bal.thickness_min_m / bal.thickness_m.max(1e-6),
                if bal.thickness_m + 1e-6 >= bal.thickness_min_m {
                    format!(
                        "Espessura {} atende (armadura superior {} cm²/m, γn={}).",
                        cm(bal.thickness_m),
                        decimal(bal.as_top_cm2_m, 2),
                        decimal(bal.gamma_n, 2)
                    )
                } else {
                    format!(
                        "Aumente a espessura da laje em balanço para pelo menos {}.",
                        cm(bal.thickness_min_m)
                    )
                },
                format!(
                    "L_bal={} m · h={} (mín. {}) · γn={} (NBR 6118 Tab. 13.2) · Md−={} kN·m/m.",
                    decimal(bal.span_m, 2),
                    cm(bal.thickness_m),
                    cm(bal.thickness_min_m),
                    decimal(bal.gamma_n, 2),
                    decimal(bal.moment_d_neg_knm_m, 1)
                ),
            ),
            Check::new(
                "Flecha (L/125)",
                bal.deflection_mm / bal.limit_mm.max(1e-6),
                if bal.deflection_mm <= bal.limit_mm {
                    format!(
                        "Flecha em balanço {} mm dentro de L/125 ({} mm).",
                        decimal(bal.deflection_mm, 1),
                        decimal(bal.limit_mm, 1)
                    )
                } else {
                    "Flecha excessiva na ponta do balanço: aumente a espessura ou reduza o balanço."
                        .to_owned()
                },
                format!(
                    "δ_longo_prazo={} mm vs L/125={} mm (NBR 6118 Tab. 13.3).",
                    decimal(bal.deflection_mm, 1),
                    decimal(bal.limit_mm, 1)
                ),
            ),
        ];
        if bal.is_ribbed_warning {
            checks.push(Check::fail(
                "Sistema em balanço",
                "Laje treliçada em balanço exige projeto específico ou troca por laje maciça."
                    .to_owned(),
                "Balanços exigem armadura negativa superior contínua com cobrimento e caranguejos."
                    .to_owned(),
            ));
        }
        if !bal.back_span_ok {
            checks.push(Check::fail(
                "Contravão / tombamento",
                format!(
                    "Contravão ({} m) menor que 1,5 × balanço ({} m): garanta ancoragem ou engaste na viga.",
                    decimal(bal.back_span_m, 2),
                    decimal(bal.required_back_span_m, 2)
                ),
                "A armadura superior negativa precisa ancorar pelo menos 1,5·L_bal no vão adjacente."
                    .to_owned(),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Balanço {} ({})", bal.label, bal.source),
            checks,
        });
    }
    for cb in &report.cantilevers.beams {
        let mut checks = vec![
            Check::new(
                "Altura (L/5)",
                cb.height_min_m / cb.height_m.max(1e-6),
                if cb.height_m + 1e-6 >= cb.height_min_m {
                    format!(
                        "Viga em balanço {:.0}×{:.0} cm atende (As superior {} cm²).",
                        cb.width_m * 100.0,
                        cb.height_m * 100.0,
                        decimal(cb.as_top_cm2, 2)
                    )
                } else {
                    format!(
                        "Aumente a altura da viga em balanço para pelo menos {}.",
                        cm(cb.height_min_m)
                    )
                },
                format!(
                    "L_bal={} m · Md−={} kN·m · Vk={} kN · As,sup={} cm² (NBR 6118).",
                    decimal(cb.span_m, 2),
                    decimal(cb.moment_d_neg_knm, 1),
                    decimal(cb.shear_k_kn, 1),
                    decimal(cb.as_top_cm2, 2)
                ),
            ),
            Check::new(
                "Flecha (L/125)",
                cb.deflection_mm / cb.limit_mm.max(1e-6),
                if cb.deflection_mm <= cb.limit_mm {
                    format!(
                        "Flecha {} mm dentro de L/125 ({} mm).",
                        decimal(cb.deflection_mm, 1),
                        decimal(cb.limit_mm, 1)
                    )
                } else {
                    "Flecha excessiva na viga em balanço: aumente a altura da seção.".to_owned()
                },
                format!(
                    "δ={} mm vs L/125={} mm (NBR 6118 Tab. 13.3).",
                    decimal(cb.deflection_mm, 1),
                    decimal(cb.limit_mm, 1)
                ),
            ),
        ];
        if !cb.back_span_ok {
            checks.push(Check::fail(
                "Contravão",
                format!(
                    "Contravão ({} m) inferior a 1,5 × balanço ({} m).",
                    decimal(cb.back_span_m, 2),
                    decimal(cb.required_back_span_m, 2)
                ),
                "Garanta comprimento de ancoragem e equilíbrio contra tombamento no vão interno."
                    .to_owned(),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Viga em balanço {}", cb.label),
            checks,
        });
    }
    verdicts
}
