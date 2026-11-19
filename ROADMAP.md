# Roadmap do Trazo

Atualizado em 19/11/2026, a partir do commit `64fabbc`. As lacunas técnicas abaixo foram conferidas no código-fonte, não só deduzidas da documentação.

## 1. Visão

> Um CAD arquitetônico 2D inteligente, com a simplicidade de um editor visual e a precisão de uma ferramenta de engenharia.

O Trazo não deve virar uma cópia do AutoCAD. O diferencial é servir às duas pontas:

- **Quem está começando** cria uma casa inteira sem precisar entender coordenadas, camadas ou comandos CAD: desenha cômodos, arrasta portas e móveis, digita medidas e recebe orientação visual.
- **Quem é profissional** produz uma planta técnica detalhada: controla geometria, cotas, camadas, escalas, quantitativos e intercâmbio com outros programas CAD.

### Princípios

1. **Um só motor e um só arquivo.** Iniciante e profissional trabalham no mesmo projeto, com as mesmas medidas reais. O que muda é quanto controle a interface mostra, não a qualidade da planta. Uma porta criada no modo simples continua sendo uma porta editável pelo profissional.
2. **Inteligência arquitetônica antes de quantidade de comandos.** Mover uma parede e ver os dois cômodos, as portas e as cotas se ajustarem vale mais do que reproduzir centenas de comandos genéricos.
3. **Pensado para a obra.** A planta tem que sair pronta para o canteiro: medidas em cm, vãos cotados, alvenaria modulada, quantitativos e impressão em escala.
4. **Evoluir, não reescrever.** A separação atual entre domínio, interface, arquivos, exportação e MCP é boa. Cada módulo evolui quando uma entrega exigir.
5. **Facilidade como valor.** Isso inclui a conexão com IA. A proteção deve chegar sem acrescentar passos para quem usa (ver [seção 6](#6-segurança-do-mcp)).

## 2. Onde o Trazo está hoje

Já entregue e documentado no [README](README.md):

| Área | Recursos |
| --- | --- |
| Desenho | Terreno, paredes e cômodos com medidas reais; grade de 0,25 m; encaixe em pontas de parede; modo orto; edição numérica; arraste; setas para mover |
| Aberturas | Porta de giro, porta vai-e-vem, janela e vão livre; lado de abertura e dobradiça invertíveis; largura, altura e peitoril editáveis |
| Alvenaria | Tijolos e blocos prontos ou personalizados (2 dimensões, junta e reboco); espessura real da parede; modulação em meio bloco; 1ª fiada por parede; estimativa de blocos descontando os vãos |
| Interiores | 11 tipos de objeto (bancadas, pias, fogões, churrasqueira, geladeira, lavatórios, bacias, boxes, mesas, prateleiras), encostados na face da parede |
| Documentação | Cotas automáticas em cm (cadeias em cima e à esquerda); etiquetas "P1 80/210" e "J1 120/100"; quadro de áreas, de esquadrias e de alvenaria; taxa de ocupação; modo medir (régua) |
| Arquivos | Projeto JSON com gravação atômica e `.bak`; recuperação após falha; preferências salvas; exportação em PDF vetorial e PNG |
| IA | Servidor MCP com 21 ferramentas (HTTP local e stdio), túnel público opcional, cada ação da IA vira um passo de desfazer |
| Qualidade | 115 testes de unidade; `cargo fmt` e `clippy` limpos |

O ponto mais valioso a preservar: o Trazo sabe o que é uma parede, uma porta e um cômodo. Isso o aproxima de um editor arquitetônico paramétrico, e não de um CAD genérico de linhas.

## 3. Lacunas estruturais (conferidas no código)

| Área | Hoje | O que precisa evoluir |
| --- | --- | --- |
| Precisão | Coordenadas `f32` ([geometry.rs](src/domain/geometry.rs)) | Núcleo geométrico em `f64` com tolerâncias controladas |
| Geometria | Paredes são segmentos; cômodos nascem como retângulos de dois cantos (`add_room`); terreno é um retângulo `min`/`max` | Polígonos livres, arcos, cômodos em L/U, terrenos irregulares |
| Paredes compartilhadas | `resize_room_dimensions` recusa redimensionar cômodo com parede compartilhada ([model.rs](src/domain/model.rs)) | Mover a parede entre dois cômodos e ajustar os dois |
| Encontros de parede | Pontas quadradas estendidas em meia espessura: cantos ortogonais ficam corretos, mas não há junção topológica nem ângulos quaisquer | Encontros L, T e X calculados, em qualquer ângulo e com espessuras diferentes |
| Encaixe | Grade, pontas de parede, modulação; faces de parede só no modo medir ([interaction.rs](src/app/interaction.rs)) | Ponto médio, interseção, perpendicular, tangente, centro, prolongamento |
| Entrada de medidas | Números no painel depois de desenhar; terreno com largura × comprimento | Digitar distância, ângulo e coordenadas durante o desenho |
| Edição | Mover, arrastar, girar objetos 90° | Copiar, colar, espelhar, girar, offset, aparar, estender; seleção múltipla |
| Cotas | Calculadas a cada quadro a partir das paredes ([annotations.rs](src/domain/annotations.rs)); só cadeias externas | Cotas como entidades do projeto: associativas, internas, de vãos, editáveis |
| Organização | Navegador por tipo de elemento | Camadas, grupos, blocos e filtros |
| Exportação | SVG a 100 px/m, reduzido para caber em 5000 px, convertido em PDF/PNG ([export.rs](src/io/export.rs)) | Pranchas (A4–A0), escalas exatas (1:50, 1:100…), carimbo |
| Intercâmbio | Só o JSON do Trazo | DXF (e depois DWG), SVG editável, CSV |
| Histórico | Guarda cópias inteiras do projeto, com limite ([history.rs](src/domain/history.rs)) | Transações ou comandos reversíveis quando os projetos crescerem |
| Pavimentos | Um único plano, um pé-direito por projeto | Pavimentos, níveis, desníveis, escadas |
| Distribuição | Sem CI, sem licença, sem instalador | CI, licença, instalador e atualização |

## 4. Roadmap por fases

Prioridades: **P0** fundamental, **P1** importante, **P2** evolução. Os números (#) remetem ao [backlog](#5-backlog-completo).

### Fase 1 — Desenhar qualquer planta residencial

Base geométrica, edição natural e precisão CAD.

- Entregas: #01, #02, #03, #04, #05, #09, #10, #11, #12, #13
- **Pronto quando:** dá para desenhar uma casa com um cômodo em L e uma parede inclinada; digitar "3,47" durante o desenho; mover 40 cm a parede entre dois quartos e ver áreas, portas, janelas e cotas se ajustarem sem reconstruir nada; e os testes de geometria cobrirem esses casos.

### Fase 2 — Projeto 2D profissional e planta de obra

Documentação que pode ser impressa, medida com escalímetro e enviada a outros profissionais.

- Entregas: #07, #26, #27, #28, #29, #30, #31, #32, #34, #37, #38, #40, #41, #42, #53, #55, #57
- **Pronto quando:** a planta da casa sai em A3 a 1:50 com carimbo, cotas, vãos cotados e quadro de esquadrias, e confere com o escalímetro no papel; e o DXF abre no AutoCAD/QCAD com camadas e unidades corretas.

### Fase 3 — Inteligência arquitetônica e quantitativos

Menos trabalho repetitivo e informações mais confiáveis tiradas do próprio desenho.

- Entregas: #06, #08, #14–#23, #33, #39, #43, #54, #56, #65, #66
- **Pronto quando:** um sobrado de dois pavimentos com escada, terreno irregular e recuos pode ser desenhado; a paginação da alvenaria e o quantitativo de obra (blocos, vergas, argamassa) saem direto da planta; e uma pessoa sem experiência completa o fluxo "Minha primeira casa" sem ajuda.

### Fase 4 — Assistência inteligente

IA para analisar, propor e revisar, com as decisões sempre nas mãos de quem projeta.

- Entregas: #24, #25, #35, #36, #44, #45, #48–#52, #67

### Em paralelo, desde a Fase 1

- **Segurança do MCP:** #46, #47 (ver [seção 6](#6-segurança-do-mcp)).
- **Qualidade e distribuição:** #58–#64. CI e licença (#58, #59) já no início.

## 5. Backlog completo

### A. Motor de desenho e precisão CAD

| # | Funcionalidade | Prioridade | Fase | Hoje |
| --- | --- | --- | --- | --- |
| 01 | Coordenadas e cálculos em `f64`, com tolerâncias controladas | P0 | 1 | Feito |
| 02 | Cômodos poligonais (L, U, irregulares), desenhados clicando nos cantos | P0 | 1 | Feito |
| 03 | Encaixe avançado: ponto médio, interseção, perpendicular, tangente, centro, prolongamento | P0 | 1 | Feito |
| 04 | Digitar distância, ângulo, coordenadas e medidas durante o desenho | P0 | 1 | Feito |
| 05 | Copiar, colar, espelhar, girar, deslocar (offset), aparar e estender | P0 | 1 | — |
| 06 | Dividir, unir, alinhar, distribuir, agrupar e repetir objetos | P1 | 3 | — |
| 07 | Entidades CAD genéricas: linha, polilinha, arco, círculo, retângulo, spline | P1 | 2 | Feito |
| 08 | Restrições: paralelismo, perpendicularidade, coincidência, distâncias fixas | P1 | 3 | — |
| 09 | Seleção múltipla, por janela, por cruzamento e por filtros | P1 | 1 | — |
| 10 | Edição de vértices e manipulação direta da geometria | P1 | 1 | Feito |

O #10 foi antecipado para a Fase 1 porque cômodos poligonais (#02) só ficam úteis se os vértices puderem ser editados.

### B. Ferramentas arquitetônicas inteligentes

| # | Funcionalidade | Prioridade | Fase | Hoje |
| --- | --- | --- | --- | --- |
| 11 | Encontros automáticos de paredes em L, T e X, em qualquer ângulo | P0 | 1 | Feito |
| 12 | Mover paredes compartilhadas atualizando os cômodos, aberturas e cotas ligados | P0 | 1 | Feito |
| 13 | Paredes referenciadas pela face interna, externa ou pelo eixo | P0 | 1 | Feito |
| 14 | Ambientes detectados automaticamente pelo fechamento das paredes | P1 | 3 | Feito |
| 15 | Terrenos irregulares, divisas inclinadas e desmembramento de lotes | P1 | 3 | Feito |
| 16 | Recuos, afastamentos e limites de implantação configuráveis | P1 | 3 | Feito |
| 17 | Área útil, construída, de projeção e do terreno calculadas separadamente | P1 | 3 | Parcial |
| 18 | Paredes multicamadas: alvenaria, revestimento, isolamento, acabamento | P1 | 3 | Parcial |
| 19 | Portas de correr, pivotantes, duplas, sanfonadas e outras tipologias | P1 | 3 | Parcial |
| 20 | Tipos de janela (correr, basculante, maxim-ar…) com representação técnica | P1 | 3 | — |
| 21 | Escadas, rampas, patamares, guarda-corpos e desníveis | P1 | 3 | — |
| 22 | Vários pavimentos, com níveis, pé-direito e referência do pavimento de baixo | P1 | 3 | — |
| 23 | Biblioteca de móveis, equipamentos e símbolos personalizáveis | P1 | 3 | Parcial |
| 24 | Revestimentos, acabamentos, rodapés e especificações por cômodo | P2 | 4 | — |
| 25 | Verificação de circulação, interferências e área de uso do mobiliário | P2 | 4 | — |

O #12 é o mais importante deste grupo. Dois quartos separados por uma parede: o usuário arrasta a parede 40 cm e o Trazo ajusta as áreas dos dois, mantém portas e janelas e atualiza as cotas.

### C. Documentação e apresentação

| # | Funcionalidade | Prioridade | Fase | Hoje |
| --- | --- | --- | --- | --- |
| 26 | Cotas associativas: internas, externas, alinhadas, angulares, editáveis | P0 | 2 | Feito |
| 27 | Camadas com cor, espessura, tipo de linha, visibilidade e bloqueio | P0 | 2 | Feito |
| 28 | Pranchas A4, A3, A2, A1, A0 e tamanhos personalizados | P0 | 2 | Feito |
| 29 | Impressão em escala exata: 1:25, 1:50, 1:75, 1:100, 1:200 | P0 | 2 | Feito |
| 30 | Carimbo editável, legenda, identificação do projeto e paginação | P1 | 2 | — |
| 31 | Textos, notas, chamadas, linhas de chamada e símbolos | P1 | 2 | Feito |
| 32 | Hachuras e padrões gráficos por material | P1 | 2 | Feito |
| 33 | Modelos de planta baixa, implantação e detalhamento | P1 | 3 | — |
| 34 | Quadros de áreas, esquadrias, ambientes e materiais configuráveis | P1 | 2 | Parcial |
| 35 | Cortes, fachadas, detalhes e referências entre desenhos | P2 | 4 | — |
| 36 | Conjunto de pranchas com revisões e controle de documentação | P2 | 4 | — |
| 37 | Estilos de documentação baseados nas normas brasileiras | P1 | 2 | — |

As cotas automáticas e os quadros de áreas e de esquadrias já existem: a proposta é ampliá-los, não recriá-los. Para o #37, a referência é a **ABNT NBR 6492:2021** (documentação técnica de projetos arquitetônicos e urbanísticos). O Trazo pode trazer padrões prontos, mas deve deixar claro que conferir as exigências aplicáveis continua sendo responsabilidade de quem projeta.

### D. Compatibilidade com o mercado

| # | Funcionalidade | Prioridade | Fase | Hoje |
| --- | --- | --- | --- | --- |
| 38 | Importar e exportar DXF preservando geometria, camadas e unidades | P0 | 2 | — |
| 39 | DWG, por biblioteca ou conversor confiável | P1 | 3 | — |
| 40 | Imagens e PDFs como referência de fundo, com calibração de escala | P1 | 2 | — |
| 41 | Exportação SVG editável | P1 | 2 | Feito |
| 42 | Exportação CSV de áreas, esquadrias e quantitativos | P1 | 2 | Feito |
| 43 | Blocos reutilizáveis com atributos e biblioteca pessoal | P1 | 3 | — |
| 44 | Referências externas (outros desenhos) | P2 | 4 | — |
| 45 | Comparação visual entre versões | P2 | 4 | — |

O DXF é o que permite mandar o projeto a outro profissional sem obrigá-lo a instalar o Trazo. O #40 também ajuda iniciantes: muitos começam de um rascunho fotografado.

### E. Inteligência artificial e automação

| # | Funcionalidade | Prioridade | Fase | Hoje |
| --- | --- | --- | --- | --- |
| 46 | Modo de aprovação opcional: a IA propõe, a planta mostra a prévia e o usuário confirma | P1 | paralelo | — |
| 47 | Endereço do túnel protegido por token automático, sem passo extra | P0 | paralelo | Feito |
| 48 | Comandos em linguagem natural para criar e modificar ambientes | P1 | 4 | Parcial |
| 49 | Validação automática da planta, com explicação dos problemas | P1 | 4 | — |
| 50 | Propostas de distribuição de ambientes | P2 | 4 | — |
| 51 | Histórico separado das ações da IA, com comparação e reversão | P1 | 4 | Parcial |
| 52 | Assistente que ensina a usar o editor | P1 | 4 | — |

O #48 já funciona hoje por meio de assistentes externos (ChatGPT, Claude) conectados pelo MCP. O #51 é parcial porque cada ação da IA já vira um passo de desfazer, mas ainda não existe um histórico próprio das ações da IA.

### F. Obra e execução (novo)

O objetivo declarado é usar o Trazo no dia a dia da obra. Estes itens vêm do trabalho já feito em alvenaria, cotas e esquadrias.

| # | Funcionalidade | Prioridade | Fase |
| --- | --- | --- | --- |
| 53 | Cotas dos vãos: posição e largura de cada porta e janela na parede, e cotas internas de cada cômodo | P0 | 2 |
| 54 | Planta de 1ª fiada e paginação das paredes: blocos inteiros, meios, cortes, vergas e contravergas | P1 | 3 |
| 55 | Planta de locação: cotas acumuladas a partir de um ponto de origem no terreno, para marcar a obra no gabarito | P1 | 2 |
| 56 | Quantitativo de obra: blocos com perda, meio-bloco e canaleta, vergas e contravergas por vão, argamassa de assentamento, reboco, piso e revestimento por cômodo | P1 | 3 |
| 57 | "Prancha de obra" em um clique: A3 a 1:50 com cotas, vãos, quadro de esquadrias e legenda | P1 | 2 |

### G. Qualidade, distribuição e manutenção (novo)

| # | Funcionalidade | Prioridade | Fase |
| --- | --- | --- | --- |
| 58 | CI no GitHub: `cargo fmt --check`, `clippy` e testes a cada push | P0 | paralelo — **Feito** |
| 59 | Licença explícita no repositório | P0 | paralelo |
| 60 | Testes de propriedade da geometria (interseções, polígonos, tolerâncias) | P1 | paralelo — **Feito** |
| 61 | Testes de regressão visual da exportação PDF/PNG | P1 | paralelo |
| 62 | Desempenho com plantas grandes (centenas de paredes, histórico longo) | P1 | paralelo |
| 63 | Instalador para Windows, associação de arquivos e atualização automática | P1 | paralelo |
| 64 | Versão do formato de arquivo com migrações testadas | P1 | paralelo — **Feito** |

### H. Experiência progressiva (novo)

| # | Funcionalidade | Prioridade | Fase |
| --- | --- | --- | --- |
| 65 | Níveis de interface Simples, Intermediário e Profissional, no mesmo projeto e no mesmo arquivo | P1 | 3 |
| 66 | Fluxo guiado "Minha primeira casa": terreno, ambientes desejados, cômodos ajustáveis, aberturas e conferência de medidas | P1 | 3 |
| 67 | Fluxo "Montar documentação": conjunto de pranchas com quadros, detalhes e escalas, revisado antes de exportar | P2 | 4 |

O nível escolhido não pode ser uma barreira. O iniciante abre uma ferramenta avançada quando precisar, e o profissional continua usando as ações simples.

Exemplo: "Criar quarto 3,00 × 4,00 m".

- **Simples:** digitar as medidas ou arrastar o mouse; o Trazo cria paredes, nome e área.
- **Profissional:** o mesmo quarto, com referência da parede pela face, cotas associativas e camada definida.

## 6. Segurança do MCP

**Decisão atual:** o MCP prioriza a facilidade de conexão. O endereço local (`127.0.0.1`) não pede autenticação, e isso continua assim.

O ponto que pede atenção é o túnel: enquanto ele está aberto, qualquer pessoa que conheça o endereço pode ler e alterar a planta e gravar arquivos. A proposta protege esse caso sem somar passos para o usuário:

1. **[x] #47 — Token automático no túnel.** Ao abrir o túnel, o Trazo gera um caminho secreto (`https://….trycloudflare.com/mcp/<token>`). O botão "Copiar" já entrega o endereço completo, então o usuário continua só colando um link. Um novo token é gerado a cada vez que o túnel abre.
2. **Somente leitura**, ligado por um interruptor em *Conectar uma IA*, para quando a IA só deve analisar a planta.
3. **#46 — Aprovação opcional.** A IA propõe a alteração, a planta mostra a prévia e o usuário confirma. Fica desligado por padrão.
4. **Registro das ações da IA**, com o nome de cada ferramenta e horário, no painel do projeto (base para o #51).

## 7. Evolução da arquitetura de software

| Componente | Evolução |
| --- | --- |
| [domain/geometry.rs](src/domain/geometry.rs) | Núcleo geométrico em `f64`: interseções, arcos, polígonos, tolerâncias robustas |
| [domain/model.rs](src/domain/model.rs) | Separar documento, objetos, relações topológicas (parede ↔ cômodo ↔ abertura), pavimentos e propriedades |
| [domain/annotations.rs](src/domain/annotations.rs) | Cotas e anotações como entidades persistentes vinculadas à geometria |
| [domain/masonry.rs](src/domain/masonry.rs) | Base da paginação (#54) e do quantitativo de obra (#56) |
| [app/interaction.rs](src/app/interaction.rs) | Arquitetura de comandos (desenhar, selecionar, modificar) com entrada numérica e encaixes |
| [app/canvas.rs](src/app/canvas.rs) e [io/export.rs](src/io/export.rs) | Um único estilo de desenho (camadas, espessuras, hachuras) para a tela e a exportação; pranchas e escalas na saída |
| [domain/history.rs](src/domain/history.rs) | De cópias inteiras para transações reversíveis quando o tamanho dos projetos pedir |
| [mcp/](src/mcp/) | Token do túnel, modo somente leitura, aprovação e registro de ações |

## 8. As 10 prioridades absolutas

| Ordem | Entrega | Itens | Resultado |
| --- | --- | --- | --- |
| 1 | Motor geométrico robusto | #01 | Base confiável para todo o resto |
| 2 | Cômodos de formato livre | #02, #10 | Liberdade arquitetônica |
| 3 | Paredes conectadas e encontros automáticos | #11, #12, #13 | Editar sem reconstruir ambientes |
| 4 | Encaixe e entrada de medidas CAD | #03, #04 | Precisão profissional |
| 5 | Ferramentas de edição CAD | #05, #09 | Produtividade |
| 6 | Cotas associativas e cotas dos vãos | #26, #53 | Documentação confiável para a obra |
| 7 | Camadas e estilos gráficos | #27, #32 | Organização técnica |
| 8 | Pranchas e impressão em escala | #28, #29, #30, #57 | Entrega do projeto |
| 9 | Importação e exportação DXF | #38 | Integração com outros profissionais |
| 10 | Experiência guiada para iniciantes | #65, #66 | Diferenciação e acessibilidade |

Em paralelo a tudo isso: token do túnel (#47), CI (#58) e licença (#59).

## 9. Fora do escopo por enquanto

Para manter o foco em um CAD arquitetônico 2D acessível, ficam para depois:

- renderização 3D fotorrealista;
- modelagem BIM completa;
- cálculo estrutural;
- projetos avançados de instalações (elétrica, hidráulica);
- colaboração em nuvem em tempo real.

Podem fazer sentido no futuro, mas nenhum deles é necessário para o Trazo ser um editor 2D excelente.

## Conclusão

A maior distância até um CAD 2D profissional não está no número de ferramentas. Está na capacidade de **representar, modificar, documentar e trocar qualquer planta baixa com precisão e consistência**. A evolução tem dois objetivos:

- **Iniciante:** criar uma casa inteira sem entender coordenadas, camadas ou comandos CAD.
- **Profissional:** produzir uma planta técnica detalhada, ajustar elementos conectados, tirar quantitativos e entregar documentação que outros profissionais possam continuar usando.

A simplicidade tem que ser uma vantagem para o iniciante, sem virar limitação para o profissional.
