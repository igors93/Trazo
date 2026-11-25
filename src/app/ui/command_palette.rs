//! Universal quick-command palette (`Ctrl+K`) inspired by BIMCraft `find.rs`
//! and CreaBIM command search.
//!
//! Searches every drawing tool, engineering/documentation dialog, file/view
//! action, discipline, floor level and named room in the current project with
//! multi-word order-independent matching and live shortcut badges.

use eframe::egui::{
    self, Align2, Color32, Key, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, vec2,
};

use super::super::{
    DocDialog, FloorPlanApp, RibbonTab, Selection, Tool, theme::*, tools::DrawKind,
};
use super::icons::{self, Icon};
use super::widgets::truncated_galley;
use crate::domain::{discipline::Discipline, model::OpeningKind};

#[derive(Clone)]
enum PaletteAction {
    Tool(Tool, RibbonTab),
    Dialog(DocDialog),
    Help,
    Mcp,
    StartPage,
    NewProject,
    OpenProject,
    SaveProject,
    SaveTemplate,
    Print,
    ExportPdf,
    ExportPng,
    ExportSvg,
    ExportDxf,
    ExportCsv,
    Undo,
    Redo,
    FitView,
    CenterPlan,
    DetectRooms,
    ToggleGrid,
    ToggleSnap,
    ToggleOrtho,
    ToggleDimensions,
    ToggleBrowser,
    ToggleProperties,
    ToggleRibbon,
    SetDiscipline(Discipline),
    SelectRoom(u64),
    SwitchFloor(u64),
}

struct PaletteItem {
    category: &'static str,
    label: String,
    shortcut: &'static str,
    keywords: &'static str,
    icon: Icon,
    action: PaletteAction,
}

fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| match c.to_ascii_lowercase() {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn match_score(item: &PaletteItem, full_query: &str, terms: &[String]) -> Option<u8> {
    if terms.is_empty() {
        return Some(0);
    }
    let label = normalize(&item.label);
    if label == full_query {
        return Some(0);
    }
    if label.starts_with(full_query) {
        return Some(1);
    }
    if terms.iter().all(|term| label.contains(term.as_str())) {
        return Some(2);
    }
    let cat_label = format!("{} {label}", normalize(item.category));
    if terms.iter().all(|term| cat_label.contains(term.as_str())) {
        return Some(3);
    }
    let hay = format!(
        "{cat_label} {} {}",
        normalize(item.shortcut),
        normalize(item.keywords)
    );
    if terms.iter().all(|term| hay.contains(term.as_str())) {
        return Some(4);
    }
    None
}

impl FloorPlanApp {
    pub(in crate::app) fn open_command_palette(&mut self) {
        self.show_command_palette = true;
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
    }

    fn build_palette_items(&self) -> Vec<PaletteItem> {
        let mut items = vec![
            // --- Ferramentas principais ---
            PaletteItem {
                category: "Desenhar",
                label: "Selecionar / Mover".to_owned(),
                shortcut: "V",
                keywords: "select move cursor",
                icon: Icon::Select,
                action: PaletteAction::Tool(Tool::Move, RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Parede".to_owned(),
                shortcut: "P",
                keywords: "wall alvenaria muro",
                icon: Icon::Wall,
                action: PaletteAction::Tool(Tool::Wall, RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Cômodo retangular".to_owned(),
                shortcut: "C",
                keywords: "room ambiente sala quarto cozinha",
                icon: Icon::Room,
                action: PaletteAction::Tool(Tool::Room, RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Cômodo em polígono livre".to_owned(),
                shortcut: "L",
                keywords: "polygon livre forma irregular",
                icon: Icon::RoomPolygon,
                action: PaletteAction::Tool(Tool::RoomPolygon, RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Terreno / Lote".to_owned(),
                shortcut: "T",
                keywords: "site lot terreno recuo",
                icon: Icon::Terrain,
                action: PaletteAction::Tool(Tool::Terrain, RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Porta".to_owned(),
                shortcut: "R",
                keywords: "door abertura folha",
                icon: Icon::Door,
                action: PaletteAction::Tool(Tool::Opening(OpeningKind::Door), RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Janela".to_owned(),
                shortcut: "J",
                keywords: "window abertura esquadria peitoril",
                icon: Icon::Window,
                action: PaletteAction::Tool(Tool::Opening(OpeningKind::Window), RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Portão".to_owned(),
                shortcut: "",
                keywords: "gate garagem entrada",
                icon: Icon::Gate,
                action: PaletteAction::Tool(Tool::Opening(OpeningKind::Gate), RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Vão livre".to_owned(),
                shortcut: "",
                keywords: "gap passagem abertura",
                icon: Icon::Gap,
                action: PaletteAction::Tool(Tool::Opening(OpeningKind::Gap), RibbonTab::Architecture),
            },
            PaletteItem {
                category: "Arquitetura",
                label: "Medir distância (Régua)".to_owned(),
                shortcut: "M",
                keywords: "measure ruler trena distancia",
                icon: Icon::Ruler,
                action: PaletteAction::Tool(Tool::Measure, RibbonTab::Architecture),
            },
            // --- Desenho 2D ---
            PaletteItem {
                category: "Desenho 2D",
                label: "Linha".to_owned(),
                shortcut: "",
                keywords: "line segmento cad",
                icon: Icon::Glyph(super::Glyph::Line),
                action: PaletteAction::Tool(Tool::Draw(DrawKind::Line), RibbonTab::Drafting),
            },
            PaletteItem {
                category: "Desenho 2D",
                label: "Polilinha".to_owned(),
                shortcut: "",
                keywords: "polyline cad",
                icon: Icon::Glyph(super::Glyph::Polyline),
                action: PaletteAction::Tool(Tool::Draw(DrawKind::Polyline), RibbonTab::Drafting),
            },
            PaletteItem {
                category: "Desenho 2D",
                label: "Retângulo".to_owned(),
                shortcut: "",
                keywords: "rectangle caixa cad",
                icon: Icon::Glyph(super::Glyph::Rectangle),
                action: PaletteAction::Tool(Tool::Draw(DrawKind::Rectangle), RibbonTab::Drafting),
            },
            PaletteItem {
                category: "Desenho 2D",
                label: "Círculo".to_owned(),
                shortcut: "",
                keywords: "circle raio cad",
                icon: Icon::Glyph(super::Glyph::Circle),
                action: PaletteAction::Tool(Tool::Draw(DrawKind::Circle), RibbonTab::Drafting),
            },
            PaletteItem {
                category: "Desenho 2D",
                label: "Arco".to_owned(),
                shortcut: "",
                keywords: "arc curva cad",
                icon: Icon::Glyph(super::Glyph::Arc),
                action: PaletteAction::Tool(Tool::Draw(DrawKind::Arc), RibbonTab::Drafting),
            },
            // --- Engenharia & Simulação ---
            PaletteItem {
                category: "Engenharia",
                label: "Pré-dimensionamento estrutural (Vigas, Pilares, Lajes, Fundações, Telhado, What-if)".to_owned(),
                shortcut: "",
                keywords: "calculo estrutural dimensionamento estrutura structure viga pilar laje sapata portico grelha telhado vento whatif e se",
                icon: Icon::SectionView,
                action: PaletteAction::Dialog(DocDialog::Structure),
            },
            PaletteItem {
                category: "Engenharia",
                label: "Projeto elétrico e Quadro de cargas (NBR 5410)".to_owned(),
                shortcut: "",
                keywords: "calculo eletrico eletrica electrical circuito disjuntor fio eletroduto tomada iluminacao",
                icon: Icon::Bolt,
                action: PaletteAction::Dialog(DocDialog::Electrical),
            },
            PaletteItem {
                category: "Engenharia",
                label: "Semáforo de segurança e limites".to_owned(),
                shortcut: "",
                keywords: "safety seguranca semaforo alerta crea cau revisao",
                icon: Icon::Warning,
                action: PaletteAction::Dialog(DocDialog::Safety),
            },
            PaletteItem {
                category: "Engenharia",
                label: "Quantitativos da obra (Blocos, Argamassa, Acabamentos)".to_owned(),
                shortcut: "",
                keywords: "quantities quantitativo bloco tijolo reboco piso orcamento",
                icon: Icon::Table,
                action: PaletteAction::Dialog(DocDialog::Quantities),
            },
            PaletteItem {
                category: "Engenharia",
                label: "Verificar circulação, ergonomia e interferências".to_owned(),
                shortcut: "",
                keywords: "ergonomia circulacao acessibilidade nbr9050 colisao",
                icon: Icon::CirculationCheck,
                action: PaletteAction::Dialog(DocDialog::Ergonomics),
            },
            // --- Documentação & Modelo ---
            PaletteItem {
                category: "Documentação",
                label: "Camadas (Layers)".to_owned(),
                shortcut: "",
                keywords: "layers camadas penas cores visibilidade",
                icon: Icon::Layers,
                action: PaletteAction::Dialog(DocDialog::Layers),
            },
            PaletteItem {
                category: "Documentação",
                label: "Cotas associativas".to_owned(),
                shortcut: "",
                keywords: "dimensions cotas medidas",
                icon: Icon::Ruler,
                action: PaletteAction::Dialog(DocDialog::Dimensions),
            },
            PaletteItem {
                category: "Documentação",
                label: "Textos, notas e símbolos técnicos".to_owned(),
                shortcut: "",
                keywords: "notes textos anotacoes simbolos",
                icon: Icon::TextNote,
                action: PaletteAction::Dialog(DocDialog::Notes),
            },
            PaletteItem {
                category: "Documentação",
                label: "Quadros técnicos (Esquadrias, Áreas, Cômodos)".to_owned(),
                shortcut: "",
                keywords: "schedules tabelas quadros",
                icon: Icon::Table,
                action: PaletteAction::Dialog(DocDialog::Schedules),
            },
            PaletteItem {
                category: "Documentação",
                label: "Cortes, fachadas e detalhes".to_owned(),
                shortcut: "",
                keywords: "views cortes fachadas elevacoes detalhes",
                icon: Icon::SectionView,
                action: PaletteAction::Dialog(DocDialog::Views),
            },
            PaletteItem {
                category: "Documentação",
                label: "Prancha técnica (A4–A0, carimbo ABNT)".to_owned(),
                shortcut: "",
                keywords: "sheet prancha folha carimbo escala",
                icon: Icon::Plan,
                action: PaletteAction::Dialog(DocDialog::Sheet),
            },
            PaletteItem {
                category: "Documentação",
                label: "Conjunto de pranchas e revisões".to_owned(),
                shortcut: "",
                keywords: "sheet set pranchas revisoes lista mestra",
                icon: Icon::ExportPdf,
                action: PaletteAction::Dialog(DocDialog::SheetSet),
            },
            PaletteItem {
                category: "Modelo",
                label: "Pavimentos e níveis".to_owned(),
                shortcut: "",
                keywords: "floors pavimentos andares niveis sobrado",
                icon: Icon::Floors,
                action: PaletteAction::Dialog(DocDialog::Floors),
            },
            PaletteItem {
                category: "Modelo",
                label: "Escadas, rampas e desníveis".to_owned(),
                shortcut: "",
                keywords: "stairs escada rampa circulacao",
                icon: Icon::Stairs,
                action: PaletteAction::Dialog(DocDialog::Circulation),
            },
            PaletteItem {
                category: "Modelo",
                label: "Biblioteca de móveis, blocos e símbolos".to_owned(),
                shortcut: "",
                keywords: "library biblioteca moveis blocos mobiliario",
                icon: Icon::Fixture(crate::domain::fixtures::FixtureKind::Sofa),
                action: PaletteAction::Dialog(DocDialog::Library),
            },
            PaletteItem {
                category: "Referências",
                label: "Imagem / PDF de fundo (Underlay)".to_owned(),
                shortcut: "",
                keywords: "underlay fundo imagem pdf planta",
                icon: Icon::ExportImage,
                action: PaletteAction::Dialog(DocDialog::Underlay),
            },
            PaletteItem {
                category: "Referências",
                label: "Referências externas (XREF)".to_owned(),
                shortcut: "",
                keywords: "xref referencia externa dxf vinculo",
                icon: Icon::DuplicateTab,
                action: PaletteAction::Dialog(DocDialog::Xref),
            },
            PaletteItem {
                category: "Referências",
                label: "Comparar versões do projeto".to_owned(),
                shortcut: "",
                keywords: "compare diff comparar versoes",
                icon: Icon::SplitView,
                action: PaletteAction::Dialog(DocDialog::Compare),
            },
            // --- Arquivo & Exportação ---
            PaletteItem {
                category: "Arquivo",
                label: "Página inicial (Recentes e modelos)".to_owned(),
                shortcut: "",
                keywords: "start home inicio recentes",
                icon: Icon::Plan,
                action: PaletteAction::StartPage,
            },
            PaletteItem {
                category: "Arquivo",
                label: "Nova planta em branco".to_owned(),
                shortcut: "Ctrl+N",
                keywords: "new novo criar",
                icon: Icon::New,
                action: PaletteAction::NewProject,
            },
            PaletteItem {
                category: "Arquivo",
                label: "Novo a partir de modelo ou exemplo".to_owned(),
                shortcut: "",
                keywords: "templates modelos exemplos area gourmet",
                icon: Icon::Plan,
                action: PaletteAction::Dialog(DocDialog::Templates),
            },
            PaletteItem {
                category: "Arquivo",
                label: "Abrir projeto ou modelo...".to_owned(),
                shortcut: "Ctrl+O",
                keywords: "open abrir trazo trazot json",
                icon: Icon::Open,
                action: PaletteAction::OpenProject,
            },
            PaletteItem {
                category: "Arquivo",
                label: "Salvar projeto".to_owned(),
                shortcut: "Ctrl+S",
                keywords: "save salvar gravar",
                icon: Icon::Save,
                action: PaletteAction::SaveProject,
            },
            PaletteItem {
                category: "Arquivo",
                label: "Salvar como modelo (.trazot)...".to_owned(),
                shortcut: "",
                keywords: "save template modelo trazot",
                icon: Icon::Save,
                action: PaletteAction::SaveTemplate,
            },
            PaletteItem {
                category: "Arquivo",
                label: "Imprimir em escala...".to_owned(),
                shortcut: "Ctrl+P",
                keywords: "print imprimir escala",
                icon: Icon::Print,
                action: PaletteAction::Print,
            },
            PaletteItem {
                category: "Exportar",
                label: "Exportar PDF".to_owned(),
                shortcut: "",
                keywords: "export pdf prancha",
                icon: Icon::ExportPdf,
                action: PaletteAction::ExportPdf,
            },
            PaletteItem {
                category: "Exportar",
                label: "Exportar imagem PNG".to_owned(),
                shortcut: "",
                keywords: "export png imagem",
                icon: Icon::ExportImage,
                action: PaletteAction::ExportPng,
            },
            PaletteItem {
                category: "Exportar",
                label: "Exportar SVG editável".to_owned(),
                shortcut: "",
                keywords: "export svg vetor",
                icon: Icon::Export,
                action: PaletteAction::ExportSvg,
            },
            PaletteItem {
                category: "Exportar",
                label: "Exportar DXF (AutoCAD)".to_owned(),
                shortcut: "",
                keywords: "export dxf autocad cad",
                icon: Icon::Export,
                action: PaletteAction::ExportDxf,
            },
            PaletteItem {
                category: "Exportar",
                label: "Exportar quantitativos CSV".to_owned(),
                shortcut: "",
                keywords: "export csv excel planilha",
                icon: Icon::Table,
                action: PaletteAction::ExportCsv,
            },
            // --- Editar & Planta ---
            PaletteItem {
                category: "Editar",
                label: "Desfazer".to_owned(),
                shortcut: "Ctrl+Z",
                keywords: "undo voltar",
                icon: Icon::Undo,
                action: PaletteAction::Undo,
            },
            PaletteItem {
                category: "Editar",
                label: "Refazer".to_owned(),
                shortcut: "Ctrl+Y",
                keywords: "redo avancar",
                icon: Icon::Redo,
                action: PaletteAction::Redo,
            },
            PaletteItem {
                category: "Planta",
                label: "Centralizar planta no terreno".to_owned(),
                shortcut: "",
                keywords: "center centralizar terreno",
                icon: Icon::Center,
                action: PaletteAction::CenterPlan,
            },
            PaletteItem {
                category: "Planta",
                label: "Detectar cômodos automaticamente".to_owned(),
                shortcut: "",
                keywords: "detect auto rooms detectar comodos",
                icon: Icon::Room,
                action: PaletteAction::DetectRooms,
            },
            // --- Exibir & Disciplinas ---
            PaletteItem {
                category: "Exibir",
                label: "Enquadrar tudo na tela".to_owned(),
                shortcut: "F",
                keywords: "zoom fit enquadrar",
                icon: Icon::ZoomFit,
                action: PaletteAction::FitView,
            },
            PaletteItem {
                category: "Exibir",
                label: "Alternar grade".to_owned(),
                shortcut: "F7",
                keywords: "grid grade",
                icon: Icon::Grid,
                action: PaletteAction::ToggleGrid,
            },
            PaletteItem {
                category: "Exibir",
                label: "Alternar encaixe magnético (OSNAP)".to_owned(),
                shortcut: "F3",
                keywords: "snap osnap encaixe",
                icon: Icon::Snap,
                action: PaletteAction::ToggleSnap,
            },
            PaletteItem {
                category: "Exibir",
                label: "Alternar modo orto".to_owned(),
                shortcut: "F8",
                keywords: "ortho orto 90",
                icon: Icon::Ortho,
                action: PaletteAction::ToggleOrtho,
            },
            PaletteItem {
                category: "Exibir",
                label: "Alternar cotas automáticas".to_owned(),
                shortcut: "",
                keywords: "dimensions cotas",
                icon: Icon::Ruler,
                action: PaletteAction::ToggleDimensions,
            },
            PaletteItem {
                category: "Exibir",
                label: "Mostrar / ocultar Navegador do Projeto".to_owned(),
                shortcut: "",
                keywords: "browser navegador painel esquerdo",
                icon: Icon::PanelLeft,
                action: PaletteAction::ToggleBrowser,
            },
            PaletteItem {
                category: "Exibir",
                label: "Mostrar / ocultar Propriedades".to_owned(),
                shortcut: "",
                keywords: "inspector propriedades painel direito",
                icon: Icon::PanelRight,
                action: PaletteAction::ToggleProperties,
            },
            PaletteItem {
                category: "Exibir",
                label: "Minimizar / expandir Faixa de Opções (Ribbon)".to_owned(),
                shortcut: "",
                keywords: "ribbon faixa compactar maximizar tela",
                icon: Icon::ChevronUp,
                action: PaletteAction::ToggleRibbon,
            },
            // --- Disciplinas ---
            PaletteItem {
                category: "Disciplina",
                label: "Vista Geral (todas as disciplinas)".to_owned(),
                shortcut: "",
                keywords: "disciplina tudo geral",
                icon: Icon::Plan,
                action: PaletteAction::SetDiscipline(Discipline::All),
            },
            PaletteItem {
                category: "Disciplina",
                label: "Disciplina: Arquitetura".to_owned(),
                shortcut: "",
                keywords: "disciplina arquitetura",
                icon: Icon::Room,
                action: PaletteAction::SetDiscipline(Discipline::Architecture),
            },
            PaletteItem {
                category: "Disciplina",
                label: "Disciplina: Elétrica".to_owned(),
                shortcut: "",
                keywords: "disciplina eletrica",
                icon: Icon::Bolt,
                action: PaletteAction::SetDiscipline(Discipline::Electrical),
            },
            PaletteItem {
                category: "Disciplina",
                label: "Disciplina: Estrutura".to_owned(),
                shortcut: "",
                keywords: "disciplina estrutura",
                icon: Icon::SectionView,
                action: PaletteAction::SetDiscipline(Discipline::Structure),
            },
            PaletteItem {
                category: "Ajuda",
                label: "Atalhos de teclado e guia rápido".to_owned(),
                shortcut: "F1",
                keywords: "help ajuda atalhos",
                icon: Icon::Keyboard,
                action: PaletteAction::Help,
            },
            PaletteItem {
                category: "Ajuda",
                label: "Conectar uma IA (MCP)".to_owned(),
                shortcut: "",
                keywords: "mcp ia ai claude assistente",
                icon: Icon::Ai,
                action: PaletteAction::Mcp,
            },
        ];

        // Dynamic entries: named rooms in current plan (click to select & frame!)
        for room in &self.project.rooms {
            items.push(PaletteItem {
                category: "Ir para cômodo",
                label: room.display_name().to_owned(),
                shortcut: "",
                keywords: "comodo sala quarto banheiro cozinha selecionar",
                icon: Icon::Room,
                action: PaletteAction::SelectRoom(room.id),
            });
        }

        // Dynamic entries: floors in multi-storey building
        for floor in &self.project.floors.levels {
            items.push(PaletteItem {
                category: "Pavimento",
                label: floor.label_with_elevation(),
                shortcut: "",
                keywords: "andar nivel piso pavimento",
                icon: Icon::Floors,
                action: PaletteAction::SwitchFloor(floor.id),
            });
        }

        items
    }

    fn execute_palette_action(&mut self, action: PaletteAction) {
        self.show_command_palette = false;
        match action {
            PaletteAction::Tool(tool, tab) => {
                self.show_start_page = false;
                self.ribbon_tab = tab;
                self.set_tool(tool);
            }
            PaletteAction::Dialog(dialog) => {
                self.active_doc_dialog = dialog;
            }
            PaletteAction::Help => self.show_help = true,
            PaletteAction::Mcp => self.show_mcp_dialog = true,
            PaletteAction::StartPage => self.show_start_page = true,
            PaletteAction::NewProject => self.request_new_project(),
            PaletteAction::OpenProject => self.request_open(),
            PaletteAction::SaveProject => {
                self.save();
            }
            PaletteAction::SaveTemplate => {
                self.save_as_template();
            }
            PaletteAction::Print => self.open_print_dialog(),
            PaletteAction::ExportPdf => self.export_pdf(),
            PaletteAction::ExportPng => self.export_png(),
            PaletteAction::ExportSvg => self.export_svg(),
            PaletteAction::ExportDxf => self.export_dxf(),
            PaletteAction::ExportCsv => self.export_csv(),
            PaletteAction::Undo => self.undo(),
            PaletteAction::Redo => self.redo(),
            PaletteAction::FitView => self.fit_view_requested(),
            PaletteAction::CenterPlan => self.center_content_on_terrain(),
            PaletteAction::DetectRooms => self.detect_enclosed_rooms(),
            PaletteAction::ToggleGrid => self.toggle_grid(),
            PaletteAction::ToggleSnap => self.toggle_snap(),
            PaletteAction::ToggleOrtho => self.toggle_ortho(),
            PaletteAction::ToggleDimensions => self.show_dimensions = !self.show_dimensions,
            PaletteAction::ToggleBrowser => self.show_browser = !self.show_browser,
            PaletteAction::ToggleProperties => self.show_properties = !self.show_properties,
            PaletteAction::ToggleRibbon => self.ribbon_collapsed = !self.ribbon_collapsed,
            PaletteAction::SetDiscipline(d) => self.set_discipline(d),
            PaletteAction::SelectRoom(id) => {
                self.show_start_page = false;
                self.set_selection(Some(Selection::Room(id)));
                self.zoom_to_selection(Selection::Room(id));
            }
            PaletteAction::SwitchFloor(id) => {
                self.show_start_page = false;
                let _ = self.project.switch_floor(id);
                self.active_tab_floor_id = self.project.floors.active_floor_id;
            }
        }
    }

    pub(super) fn command_palette(&mut self, ctx: &egui::Context) {
        if !self.show_command_palette {
            return;
        }

        let items = self.build_palette_items();
        let full_query = normalize(self.command_palette_query.trim());
        let terms: Vec<String> = full_query.split_whitespace().map(str::to_owned).collect();
        let mut scored: Vec<(usize, u8)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| match_score(item, &full_query, &terms).map(|s| (i, s)))
            .collect();
        scored.sort_by_key(|&(_, score)| score);
        let matched: Vec<usize> = scored.into_iter().take(12).map(|(i, _)| i).collect();

        if matched.is_empty() {
            self.command_palette_selected = 0;
        } else {
            self.command_palette_selected = self.command_palette_selected.min(matched.len() - 1);
        }

        let (close_key, move_up, move_down, confirm) = ctx.input(|i| {
            (
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter),
            )
        });
        if close_key {
            self.show_command_palette = false;
            return;
        }
        if !matched.is_empty() {
            if move_up {
                self.command_palette_selected = if self.command_palette_selected == 0 {
                    matched.len() - 1
                } else {
                    self.command_palette_selected - 1
                };
            }
            if move_down {
                self.command_palette_selected = (self.command_palette_selected + 1) % matched.len();
            }
        }

        let mut chosen: Option<PaletteAction> = None;
        if confirm && let Some(&idx) = matched.get(self.command_palette_selected) {
            chosen = Some(items[idx].action.clone());
        }

        let screen = ctx.content_rect();
        let palette_w = 500.0_f32.min((screen.width() - 32.0).max(280.0));

        let response = egui::Modal::new(egui::Id::unique("trazo_command_palette"))
            .backdrop_color(Color32::from_rgba_unmultiplied(15, 23, 42, 70))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER_STRONG))
                    .corner_radius(10)
                    .shadow(egui::Shadow {
                        offset: [0, 10],
                        blur: 32,
                        spread: 0,
                        color: Color32::from_rgba_unmultiplied(15, 23, 42, 60),
                    })
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.set_width(palette_w);

                // Search input header
                egui::Frame::new()
                    .fill(CONTROL)
                    .stroke(Stroke::new(1.5, ACCENT_RING))
                    .corner_radius(7)
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            icons::paint(
                                ui.painter(),
                                icon_rect,
                                Icon::Search,
                                ACCENT_TEXT,
                                ACCENT,
                            );
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.command_palette_query)
                                    .frame(egui::Frame::NONE)
                                    .font(regular(14.0))
                                    .desired_width(ui.available_width())
                                    .hint_text(
                                        RichText::new(
                                            "Buscar ferramentas, cálculos, cômodos ou atalhos...",
                                        )
                                        .color(MUTED),
                                    ),
                            );
                            edit.request_focus();
                        });
                    });

                ui.add_space(8.0);

                if matched.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Nenhum comando encontrado para esta busca.")
                            .size(12.5)
                            .color(MUTED),
                    );
                    ui.add_space(12.0);
                } else {
                    egui::ScrollArea::vertical()
                        .max_height((screen.height() * 0.55).clamp(180.0, 380.0))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (pos, &item_idx) in matched.iter().enumerate() {
                                let item = &items[item_idx];
                                let is_selected = pos == self.command_palette_selected;
                                let (rect, row_resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 30.0),
                                    Sense::click(),
                                );
                                let row_resp =
                                    row_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                if row_resp.hovered() {
                                    self.command_palette_selected = pos;
                                }
                                let painter = ui.painter();
                                if is_selected {
                                    painter.rect(
                                        rect,
                                        6,
                                        ACCENT_SOFT,
                                        Stroke::new(1.0, ACCENT_RING),
                                        StrokeKind::Inside,
                                    );
                                } else if row_resp.hovered() {
                                    painter.rect_filled(rect, 6, ACCENT_SOFTER);
                                }
                                icons::paint(
                                    painter,
                                    Rect::from_center_size(
                                        Pos2::new(rect.left() + 16.0, rect.center().y),
                                        vec2(15.0, 15.0),
                                    ),
                                    item.icon,
                                    if is_selected { ACCENT_TEXT } else { TEXT_2 },
                                    ACCENT,
                                );
                                let cat_galley = painter.layout_no_wrap(
                                    format!("{} › ", item.category),
                                    regular(11.5),
                                    MUTED,
                                );
                                let sc_w = if item.shortcut.is_empty() {
                                    0.0
                                } else {
                                    painter
                                        .layout_no_wrap(
                                            item.shortcut.to_owned(),
                                            strong(10.5),
                                            MUTED,
                                        )
                                        .size()
                                        .x
                                        + 18.0
                                };
                                let x_cat = rect.left() + 32.0;
                                painter.galley(
                                    Pos2::new(x_cat, rect.center().y - cat_galley.size().y / 2.0),
                                    cat_galley.clone(),
                                    MUTED,
                                );
                                let x_lbl = x_cat + cat_galley.size().x;
                                let max_lbl_w = (rect.right() - 10.0 - sc_w - x_lbl).max(32.0);
                                let lbl_color = if is_selected { ACCENT_TEXT } else { TEXT };
                                let lbl_galley = truncated_galley(
                                    painter,
                                    &item.label,
                                    if is_selected {
                                        strong(12.5)
                                    } else {
                                        regular(12.5)
                                    },
                                    lbl_color,
                                    max_lbl_w,
                                );
                                painter.galley(
                                    Pos2::new(x_lbl, rect.center().y - lbl_galley.size().y / 2.0),
                                    lbl_galley,
                                    lbl_color,
                                );
                                if !item.shortcut.is_empty() {
                                    let sc_galley = painter.layout_no_wrap(
                                        item.shortcut.to_owned(),
                                        strong(10.5),
                                        MUTED,
                                    );
                                    let pill_w = sc_galley.size().x + 10.0;
                                    let pill = Rect::from_min_size(
                                        Pos2::new(
                                            rect.right() - 8.0 - pill_w,
                                            rect.center().y - 9.0,
                                        ),
                                        vec2(pill_w, 18.0),
                                    );
                                    painter.rect(
                                        pill,
                                        4,
                                        CONTROL,
                                        Stroke::new(1.0, BORDER),
                                        StrokeKind::Inside,
                                    );
                                    painter.text(
                                        pill.center(),
                                        Align2::CENTER_CENTER,
                                        item.shortcut,
                                        strong(10.5),
                                        MUTED,
                                    );
                                }
                                if row_resp.clicked() {
                                    chosen = Some(item.action.clone());
                                }
                            }
                        });
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("↑↓ navegar  ·  Enter executar  ·  Esc fechar")
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            });

        if let Some(action) = chosen {
            self.execute_palette_action(action);
        } else if response.should_close() {
            self.show_command_palette = false;
        }
    }
}
