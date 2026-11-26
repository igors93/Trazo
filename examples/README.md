# Sample projects

Ready-made Trazo architectural plans you can open immediately — either by double-clicking the `.json` file, via *Arquivo › Abrir...*, or directly from *Arquivo › Novo a partir de modelo...* inside Trazo.

| File | Description | Floors | Sheet |
| --- | --- | --- | --- |
| [`casa-terrea.json`](casa-terrea.json) | Single-storey house (2 bedrooms + master suite, living/dining, kitchen, laundry, 2 bathrooms) on a 10 × 20 m lot with setbacks, hatches, fixtures and technical symbols. | 1 | A3 1:50 (NBR 6492) |
| [`sobrado.json`](sobrado.json) | Two-storey house with U-shaped stair, social and service areas on the ground floor, and 3 bedrooms (1 suite) plus 2 bathrooms on the upper floor. | 2 | A3 1:50 (NBR 6492) |
| [`apartamento.json`](apartamento.json) | 2-bedroom apartment with balcony, integrated living/dining room, kitchen, laundry, suite and social bathroom. | 1 | A3 1:50 (NBR 6492) |
| [`area-gourmet.json`](area-gourmet.json) | Structural reference: a semi-open gourmet area against the lot boundary, with a soil survey, ground beams, balance beams, a tall boundary wall under wind, timber posts and a mono-pitch roof, computed end to end. | 1 | — |

**Learning with the samples.** Open `casa-terrea.json`, go to the *Estrutura* tab, run **Gerar estrutura**, then open **Segurança** and read each result: the traffic light, the sentence on what to change and the hypotheses at the end. Then ask a connected AI assistant to explain each result in plain words. This is how a family's own house will be checked.

All files in this folder are validated by `cargo test` (schema version, geometry repair checks, SVG and CSV exports).
