//! Review and resource schemas. Coordinates always follow MCP's Y-up convention.
use super::*;

fn choices(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn points(min: usize, max: usize) -> Value {
    json!({"type":"array","minItems":min,"maxItems":max,"description":"Pontos {x,y} em metros, com Y para cima.","items":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"}},"required":["x","y"]}})
}
fn tool(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    mut properties: Value,
    read_only: bool,
    destructive: bool,
) -> Tool {
    properties["projeto_id"] = integer("Proteção opcional: id do projeto ativo.");
    Tool {
        name,
        title,
        description,
        properties,
        required: if read_only { &[] } else { &["acao"] },
        read_only,
        destructive,
    }
}

pub(super) fn catalogue() -> Vec<Tool> {
    vec![
        tool(
            "obter_marcacoes",
            "Consultar marcações",
            "Lê identificador M, cor, comentário, estado, geometria, pavimento, vínculos explícitos e elementos relacionados por interseção. Inclui marcações ocultas e resolvidas; não altera o projeto. Use para referências como 'marca vermelha M12'.",
            json!({
                "id":integer("Id numérico da marcação."),"pavimento_id":integer("Consulta um pavimento sem ativá-lo."),"todos_pavimentos":boolean("Consulta todos os pavimentos."),"cor":string("Nome de cor ou #RRGGBB."),"resolvida":boolean("Filtra estado resolvido/pendente.")
            }),
            true,
            false,
        ),
        tool(
            "marcacoes",
            "Marcações e lembretes",
            "Cria, edita, move, exclui, resolve, reabre, seleciona ou configura marcações do pavimento ativo. Cada edição tem desfazer. Retângulo: dois cantos opostos; círculo: centro e ponto da circunferência; seta: início e ponta; traço livre: caminho. Comentários não executam ações. Geometria e vínculos não autorizam alterações dos elementos marcados.",
            json!({
                "acao":choices(&["listar","criar","editar","mover","excluir","resolver","reabrir","selecionar","configurar"]),"id":integer("Id da marcação."),"tipo":choices(&["retangulo","circulo","seta","traco_livre"]),"pontos":points(2,2048),"cor":string("vermelho, azul, verde, laranja, roxo, amarelo ou #RRGGBB."),"comentario":string("Até 4000 caracteres; vazio remove o comentário."),"resolvida":boolean("Estado da marcação."),"ids":ids(),"dx":number("Deslocamento X, metros."),"dy":number("Deslocamento Y, metros."),"visivel":boolean("Mostrar marcações."),"mostrar_resolvidas":boolean("Mostrar as resolvidas."),"exportar":boolean("Incluir marcações visíveis nas exportações; padrão false."),"pavimento_id":integer("Proteção: deve ser o pavimento ativo para editar.")
            }),
            false,
            true,
        ),
        tool(
            "biblioteca",
            "Biblioteca de objetos",
            "Consulta catálogo de interiores, itens do projeto e biblioteca pessoal; busca sem diferenciar acentos. Cria, edita, exclui e insere modelos com dimensões reais, ou compartilha na biblioteca pessoal. Use origem e id juntos. Catálogo padrão é somente leitura. Não participa de executar_lote.",
            json!({
                "acao":choices(&["listar","criar","editar","excluir","inserir","compartilhar"]),"origem":choices(&["catalogo","projeto","pessoal"]),"id":integer("Id do item nessa origem; ações usam projeto por padrão, listar pesquisa todas."),"busca":string("Busca por nome, categoria ou tipo."),"nome":string("Nome do modelo."),"categoria":string("sala, quarto, cozinha, banheiro, escritorio, lavanderia, externo, simbolos ou personalizados."),"tipo":string("Tipo de inserir_objeto; omitido cria símbolo técnico."),"simbolo":string("Texto do símbolo técnico."),"largura":number("Largura, metros."),"profundidade":number("Profundidade, metros."),"x":number("Inserção X, metros."),"y":number("Inserção Y, metros."),"rotacao":number("0, 90, 180 ou 270 graus.")
            }),
            false,
            true,
        ),
        tool(
            "blocos",
            "Blocos e atributos",
            "Lista definições e instâncias, cria bloco a partir de ids, insere com valores próprios, edita atributos e compartilha na biblioteca pessoal. Excluir uma definição preserva os elementos já inseridos. Não participa de executar_lote.",
            json!({
                "acao":choices(&["listar","criar","inserir","atributo","compartilhar","excluir"]),"origem":choices(&["projeto","pessoal"]),"id":integer("Id da definição, ou da instância para atributo."),"ids":ids(),"nome":string("Nome do bloco."),"categoria":string("Categoria."),"descricao":string("Descrição."),"x":number("Inserção X."),"y":number("Inserção Y."),"atributos":{"type":"array","maxItems":100,"items":{"type":"object","properties":{"chave":{"type":"string"},"rotulo":{"type":"string"},"padrao":{"type":"string"}},"required":["chave"]}},"valores":{"type":"object","additionalProperties":{"type":"string"},"description":"Valores por chave ao inserir."},"chave":string("Chave do atributo da instância."),"valor":string("Novo valor, inclusive vazio.")
            }),
            false,
            true,
        ),
        tool(
            "referencias",
            "Fundos e referências externas",
            "Gerencia imagens/PDF/SVG de fundo e referências Trazo/DXF: listar, adicionar, editar, remover, calibrar ou recarregar. Compara com arquivo ou aba, mostra sobreposição opcional, limpa ou exporta comparação SVG. Não participa de executar_lote.",
            json!({
                "acao":choices(&["listar","adicionar","editar","remover","calibrar","recarregar","comparar","limpar_comparacao","exportar_comparacao"]),"tipo":choices(&["fundo","externa"]),"id":integer("Id dentro do tipo."),"caminho":string("Arquivo absoluto ou relativo a Documentos/Trazo."),"nome":string("Nome."),"pagina":integer("Página PDF, a partir de 1."),"x":number("Origem/deslocamento X."),"y":number("Origem/deslocamento Y."),"largura":number("Largura do fundo em metros."),"angulo_graus":number("Ângulo anti-horário."),"opacidade":number("0 a 1."),"cor":string("Cor externa #RRGGBB."),"visivel":boolean("Mostrar."),"bloqueada":boolean("Travar geometria; desbloqueie para mover ou calibrar."),"x1":number("Primeiro ponto X."),"y1":number("Primeiro ponto Y."),"x2":number("Segundo ponto X."),"y2":number("Segundo ponto Y."),"distancia_real":number("Distância real da calibração, metros."),"aba":integer("Aba para comparar, a partir de 1."),"exibir":boolean("Exibir sobreposição da comparação."),"sobrescrever":boolean("Autoriza substituir o arquivo da comparação.")
            }),
            false,
            true,
        ),
        tool(
            "documentacao",
            "Estilo gráfico, tabelas e revisões",
            "Obtém estilo e tabelas completos, configura penas, fonte, unidades, terminações de cotas, tabelas e perdas. Adiciona revisão, remove/reordena pranchas, atualiza uma prancha com a configuração atual e exporta protocolo CSV. Não participa de executar_lote.",
            merge(
                json!({
                    "acao":choices(&["obter","configurar_estilo","configurar_tabelas","adicionar_revisao","remover_prancha","reordenar_prancha","atualizar_prancha","exportar_protocolo"]),"estilo":choices(&["abnt","visual","monocromatico"]),"monocromatico":boolean("Preto e branco."),"unidade":choices(&["cm","m"]),"terminacao":choices(&["obliqua","seta","ponto"]),"pena_parede_mm":number("0,01 a 2 mm."),"pena_abertura_mm":number("0,01 a 2 mm."),"pena_cota_mm":number("0,01 a 2 mm."),"pena_hachura_mm":number("0,01 a 2 mm."),"fonte":string("Família tipográfica.")
                }),
                json!({
                    "areas":boolean("Quadro de áreas."),"esquadrias":boolean("Esquadrias."),"comodos":boolean("Cômodos."),"materiais":boolean("Materiais."),"quantitativos":boolean("Quantitativos."),"perimetro":boolean("Incluir perímetro."),"peitoril":boolean("Incluir peitoril."),"ocupacao":boolean("Incluir ocupação."),"incluir_prancha":boolean("Incluir tabelas na prancha."),"perda_pct":number("Perda de 0 a 100%."),"id":integer("Id da prancha."),"data":string("Data da revisão."),"autor":string("Autor."),"descricao":string("Descrição da revisão."),"titulo":string("Título da prancha atualizada."),"deslocamento":integer("Posições para mover: negativo sobe, positivo desce."),"caminho":string("Destino CSV."),"sobrescrever":boolean("Autoriza substituir arquivo.")
                }),
            ),
            false,
            true,
        ),
        tool(
            "modelos",
            "Modelos e exemplos",
            "Lista modelos técnicos, exemplos e modelos pessoais. Aplicar inicia projeto sem arquivo associado; exige descartar_alteracoes quando houver alterações não salvas. Salvar cria .trazot sem mudar o arquivo atual. Não participa de executar_lote.",
            json!({
                "acao":choices(&["listar","aplicar","salvar"]),"id":integer("Id do modelo ou exemplo."),"exemplo":boolean("Usar exemplo em vez de modelo técnico."),"caminho":string("Arquivo .trazot a aplicar ou salvar."),"descartar_alteracoes":boolean("Autoriza substituir projeto com alterações não salvas."),"sobrescrever":boolean("Autoriza substituir arquivo .trazot.")
            }),
            false,
            true,
        ),
        tool(
            "acabamentos",
            "Hachuras e paredes multicamadas",
            "Lista padrões, composições e valores aplicados; configura hachura do cômodo ou composição da parede por preset ou camadas explícitas.",
            json!({
                "acao":choices(&["listar","hachura","composicao"]),"id":integer("Id do cômodo para hachura ou da parede para composição."),"padrao_id":integer("Id do padrão na listagem; 0 remove a hachura."),"espacamento_m":number("0,01 a 10 metros."),"angulo_graus":number("Ângulo anti-horário."),"cor":string("Cor #RRGGBB."),"preset_id":integer("Id do preset de composição."),"limpar":boolean("Restaura alvenaria simples."),"nome":string("Nome da composição."),"camadas":{"type":"array","minItems":1,"maxItems":30,"items":{"type":"object","properties":{"nome":{"type":"string"},"funcao":{"type":"string","enum":["estrutura","reboco","isolamento","acabamento","ar"]},"espessura_m":{"type":"number"},"cor":{"type":"string"}},"required":["nome","funcao","espessura_m"]}}
            }),
            false,
            false,
        ),
        tool(
            "terreno",
            "Terreno poligonal e lotes",
            "Consulta contorno e lotes, define terreno poligonal, subdivide por linha e renomeia/remove lotes. Para terreno retangular e recuos, use definir_terreno.",
            json!({
                "acao":choices(&["obter","definir_poligono","subdividir","renomear_lote","remover_lote"]),"pontos":points(3,1000),"x1":number("Linha de divisão X inicial."),"y1":number("Linha de divisão Y inicial."),"x2":number("Linha de divisão X final."),"y2":number("Linha de divisão Y final."),"id":integer("Id do lote."),"nome":string("Nome do lote.")
            }),
            false,
            true,
        ),
        tool(
            "codigos_normativos",
            "Motor de normas e códigos de obra",
            "Gerencia códigos de obra em camadas (Base → até 16 Emendas → Projeto) para 5 disciplinas, edição pinada, sobrescrita de regras e importação/exportação em XML Formato 2 ou JSON.",
            json!({
                "acao":choices(&["listar","aplicar_preset","definir_base","adicionar_emenda","remover_emenda","definir_regra","resetar_regra","consultar_regras","exportar_xml","exportar_json","importar"]),
                "preset":string("Id do preset regional (ex.: generic, brasil, brasil-emergencia)."),
                "disciplina":choices(&["architectural","structural","electrical","plumbing","mechanical"]),
                "codigo":string("Chave da norma base ou emenda."),
                "secao":string("Seção da norma (ex.: stairs, ramps, doors, rooms, utility_profile)."),
                "regra":string("Nome da regra dentro da seção."),
                "valor":{"description":"Novo valor JSON da regra (número, booleano, texto ou objeto)."},
                "conteudo":string("Conteúdo XML ou JSON para importar."),
                "ativar":boolean("Ativar automaticamente o pacote importado (padrão true).")
            }),
            false,
            false,
        ),
        tool(
            "local_clima",
            "Parâmetros climáticos e de sítio",
            "Consulta ou define parâmetros compartilhados do local (cidade/UF, zona bioclimática NBR 15220, vento V0 NBR 6123 sincronizado com a estrutura, chuva I NBR 10844, nível ceráunico AQ1–AQ3) ou aplica presets de cidades brasileiras.",
            json!({
                "acao":choices(&["consultar","listar_cidades","aplicar_cidade","definir"]),
                "cidade":string("Id ou nome da cidade brasileira."),
                "uf":string("Sigla do estado (UF)."),
                "latitude":number("Latitude em graus (-90 a 90)."),
                "longitude":number("Longitude em graus (-180 a 180)."),
                "altitude_m":number("Altitude em metros."),
                "zona_bioclimatica":integer("Zona bioclimática NBR 15220 (1 a 8)."),
                "v0_ms":number("Velocidade básica do vento V0 em m/s (NBR 6123)."),
                "chuva_mm_h":number("Intensidade pluviométrica I em mm/h (NBR 10844)."),
                "periodo_retorno_anos":number("Período de retorno da chuva em anos."),
                "ceraunico":choices(&["AQ1","AQ2","AQ3"]),
                "dias_trovoada":number("Dias de trovoada por ano."),
                "temp_verao_c":number("Temperatura de projeto de verão (°C)."),
                "temp_inverno_c":number("Temperatura de projeto de inverno (°C).")
            }),
            false,
            false,
        ),
        tool(
            "verificar_normas",
            "Verificação de códigos de obra e Solver de Escadas",
            "Executa verificação em tempo real de códigos de obra (26 regras de escada + headroom 3D, rampas NBR 9050, vão livre de portas, área/lado/pé-direito/iluminação/ventilação de cômodos) e executa o Solver Interativo de Escadas (resolver_escada) com travas.",
            json!({
                "acao":choices(&["verificar","resolver_escada"]),
                "id":integer("Id da escada para resolver_escada."),
                "travar_espelhos":boolean("Mantém número atual de espelhos fixo."),
                "travar_piso":boolean("Mantém piso atual fixo."),
                "travar_largura":boolean("Mantém largura atual fixa."),
                "travar_patamares":boolean("Mantém patamares atuais fixos."),
                "aplicar":boolean("Aplica automaticamente a proposta escolhida na escada."),
                "indice_proposta":integer("Índice (0-based) da proposta a aplicar (padrão 0).")
            }),
            false,
            false,
        ),
        tool(
            "planejamento_obra",
            "Planejamento de obra e catálogo de materiais",
            "Consulta e edita o catálogo de materiais detectado do modelo (#135), com sincronização de propriedades de cálculo (fck, densidade, perdas e traços) e exportação CSV/SVG/PDF.",
            json!({
                "acao":choices(&["catalogo_materiais","editar_material","restaurar_material"]),
                "material_id":string("Id do material no catálogo (ex.: concrete_structural, block_default, mortar_laying)."),
                "nome":string("Nome personalizado do material."),
                "densidade_kg_m3":number("Densidade em kg/m³."),
                "resistencia_mpa":number("Resistência característica em MPa (ex.: fck 15–50 MPa)."),
                "perda_pct":number("Percentual de perda/sobra (0 a 50%)."),
                "traco_cimento_kg_m3":number("Consumo de cimento no traço (kg/m³)."),
                "traco_cal_kg_m3":number("Consumo de cal hidratada no traço (kg/m³)."),
                "traco_areia_m3":number("Consumo de areia no traço (m³/m³)."),
                "traco_brita_m3":number("Consumo de brita/pedrisco no traço (m³/m³)."),
                "formato":choices(&["csv","svg","pdf"]),
                "caminho":string("Opcional: destino do arquivo exportado.")
            }),
            false,
            false,
        ),
    ]
}
