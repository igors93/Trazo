# Baseline de desempenho (T-07)

> **Por que o desempenho importa.** O Trazo é feito para famílias que vão construir a própria casa com poucos recursos (veja [a proposta](proposta.md)). Muitas delas usam computadores antigos ou modestos, então o Trazo precisa abrir e responder bem num notebook com 4 GB de RAM e vídeo integrado (item #164 do [roadmap](../ROADMAP.md)). Estas medições servem para garantir isso.

O comando `trazo --bench` executa um cenário sintético e determinístico, sem abrir
janela ou carregar arquivos do usuário. O modelo contém três pavimentos, 585
paredes no total (195 no pavimento ativo), 4 cômodos, 88 aberturas e 414 objetos.
Ele mede construção do modelo, índice espacial, consultas de snap e seleção,
edição incremental, undo/redo, análise estrutural, quantitativos, gravação e
leitura do contêiner ZIP e exportação vetorial SVG.

```sh
cargo run -- --bench
cargo run -- --bench --bench-out target/bench.json
```

`--bench-out` grava os mesmos resultados em JSON para comparação automatizada.
Falhas de execução ou gravação retornam código de saída diferente de zero.
Os tempos são diagnósticos, não limites rígidos de CI: compare sempre no mesmo
equipamento, perfil de compilação e versão do Rust.

## Medição inicial — 24/11/2026

Mediana de três execuções consecutivas após o build, no perfil `dev`, com
`CARGO_INCREMENTAL=0`. Máquina: AMD Athlon 3000G (2 núcleos / 4 threads), 15,9 GiB
de RAM, Windows 11 Pro 10.0.26300, `rustc 1.99.1`.

| Etapa | Mediana |
| --- | ---: |
| Construção do modelo | 473,56 ms |
| Índice espacial XY | 2,34 ms |
| Snap + picking (20 consultas) | 23,33 ms (1,17 ms/consulta) |
| Edição e validação incremental | 4,36 ms |
| Undo + redo com compartilhamento estrutural | 3,87 ms |
| Relatório estrutural | 301,75 ms |
| Quantitativos dos três pavimentos | 3,80 ms |
| Salvar `.trazo` ZIP | 115,14 ms |
| Abrir `.trazo` ZIP | 64,17 ms |
| Exportação SVG vetorial | 24,53 ms |

O JSON do modelo tem 229.691 bytes; o contêiner `.trazo` tem 48.625 bytes
(redução de 4,72 vezes). A exportação SVG mede a geração do desenho vetorial,
não o tempo de pintura da GPU na janela.

## Regressões e ritmo de frames

`cargo test --all-targets --all-features` inclui os testes headless com
`egui_kittest` para Página Inicial, Estrutura, simulador “E se?”, Elétrica,
Segurança, Prancha, Quantitativos e Modelos. Também inclui a varredura hostil
de todas as ferramentas MCP com entradas inválidas e o teste do cenário de
585 paredes. Esses testes rodam na CI nas três plataformas.

No aplicativo, animações contínuas pedem repintura pelo `FramePacer`; eventos
de entrada continuam acordando a interface imediatamente. O driver mantém
vsync por padrão. `TRAZO_VSYNC=0` o desativa para diagnóstico;
`TRAZO_MAX_FPS=30` limita a cadência das animações; `TRAZO_RENDERER=llvmpipe`
(ou `WARP`, por exemplo) ativa a redução de carga para renderizadores por
software. A medição do pacer considera o custo da UI na CPU, não o tempo de
apresentação da GPU.
