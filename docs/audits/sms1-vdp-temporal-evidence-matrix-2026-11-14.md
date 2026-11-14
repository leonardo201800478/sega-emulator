# SMS1 VDP — matriz de evidências temporais

**Data:** 14/11/2026  
**Baseline:** `main` em `23c77a21ae427feecfb70fa6e78692e588f72416`  
**ASIC-alvo:** Sega 315-5124 / SMS1  
**Estado:** auditoria em andamento; sem liberação in-game.

## 1. Como interpretar as evidências

As fontes não têm o mesmo peso nem representam o mesmo tipo de evidência:

| Classe | Fonte | O que sustenta | O que não sustenta |
|---|---|---|---|
| Documentação pública | [SMS Power — Master System VDP](https://www.smspower.org/Development/VDP) | Registradores, portas, comportamento funcional documentado | Não fornece, para todos os eventos, timestamps elétricos dot-a-dot |
| Documentação de periférico | [SMS Power — Light Phaser](https://www.smspower.org/Development/LightPhaser) | Relação observável entre TH, Light Phaser e H-counter | Não fecha sozinho a borda física e o atraso interno de captura |
| ROM de diagnóstico | [FluBBaOfWard/SmsVdpTest](https://github.com/FluBBaOfWard/SmsVdpTest) | Expectativas e procedimentos do teste para H-counter, HINT, VINT, VCounter e overflow | Este projeto ainda não executou a ROM no runtime nem em hardware físico |
| Modelo comparativo | [MAME — SMS VDP implementation](https://github.com/mamedev/mame/blob/master/src/mame/sega/315_5124.cpp) e [SMS machine/peripheral model](https://github.com/mamedev/mame/blob/master/src/mame/sega/sms_m.cpp) | Uma implementação independente e seus limites/hipóteses | Não é medição física independente |
| CPU | Manuais Zilog Z80 anexados ao projeto | Semântica Z80, ciclos de barramento e amostragem de INT | Não determina os latches internos do VDP |
| Testes do projeto | Workflows Rust/Release | Determinismo e consistência do modelo implementado | Não comprovam, isoladamente, equivalência elétrica com um SMS real |

**Medições físicas executadas neste projeto: nenhuma.** Os testes automatizados validam o modelo temporal e sua integração, não substituem uma captura de hardware ou a execução da ROM de diagnóstico em um SMS real.

## 2. Divergência central de timing

O modelo SMS1 atual usa boundaries CPU-visíveis derivados da descrição do SmsVdpTest:

| Evento | Modelo atual | Base de timing |
|---|---:|---|
| VINT pendente | HPos 20 | T13 |
| HINT/R10 | HPos 21 | T14 |
| VCounter latch | HPos 23 | T15 |
| VBlank flag / overflow | HPos 24 | T16 |
| Fechamento da área ativa / HBlank modelado | HPos 319 | T213 |

O modelo MAME 315-5124 usa, em sua tabela de timing, VINT_HPOS 24, VINT_FLAG_HPOS 24, HINT_HPOS 26, VCOUNTER_HPOS 23 e OVERFLOW_HPOS 24; também lista XSCROLL_HPOS 21 e COLLISION_HPOS 59. Isso diverge principalmente dos eventos VINT/HINT adotados pelo código atual.

Os valores MAME são uma referência comparativa de implementação, não uma medição. Os valores do SmsVdpTest são a base diagnóstica adotada no código, mas a ROM ainda não foi executada neste projeto. Portanto, HPos 20/T13 e HPos 21/T14 permanecem **hipóteses de trabalho testadas internamente**, não timing físico fechado.

Não converter HClock em HPos por simples igualdade: as referências usam origens e convenções de coordenadas distintas.

## 3. Matriz técnica

| Recurso | Implementação atual | Evidência externa | Teste existente | Pendência |
|---|---|---|---|---|
| HCounter / porta $7F | Conversão HPos/T-state e latch por transição TH via software; leitura espelhada mantém valor capturado | SMS Power descreve latch relacionado ao TH do Light Phaser; SmsVdpTest documenta o teste TH 0→1; MAME implementa sua própria conversão e latch | Leitura estável por $7F, espelhamento, captura TH software e invariância de chunking | Borda e atraso do latch por TH externo; HCounter dot-phase; validação física |
| HBlank | T213/HPos319 fecha o span de 256 pixels; T212 não fecha o último pixel | A documentação pública consultada não fecha o dot físico exato; MAME é apenas comparador | T212/T213, pixel 255, colisão e leitura de status | Boundary elétrico real, border e fetch pipeline |
| HINT / R10 | R10 decrementa/recarrega e gera IRQ em T14/HPos21 | SmsVdpTest documenta HINT próximo de HClock -17; tabela MAME usa HINT_HPOS26 | Antes/depois T13/T14, snapshot/restore, chegada à linha INT do Z80 | Resolver a diferença MAME/SmsVdpTest por execução da ROM e/ou medição física; latch físico de R0/R10 |
| VBlank / VINT | IRQ pendente em T13/HPos20; VBlank flag em T16/HPos24 | SmsVdpTest documenta VINT próximo de HClock -18 e flag mais tarde; MAME usa VINT_HPOS24 e VINT_FLAG_HPOS24 | T12/T13/T16, snapshot/restore, contínuo versus blocos, serviço de VBlank pelo Z80 | Resolver o deslocamento de VINT entre fontes; confirmar boundary físico e ordem de acknowledge |
| Sprite overflow | SAT é avaliada por scanline; índice/status latched em T16/HPos24 | SmsVdpTest e MAME fornecem referências de overflow, mas não constituem medição feita neste projeto | Boundary T16, índice, buffers Mode 4 e legacy/TMS | Índice físico exato, interação com fetch SAT e seleção em casos-limite |
| Sprite collision | Composição identifica pixels opacos sobrepostos; status é publicado quando o commit alcança o pixel; status read reconhece | SmsVdpTest fornece testes funcionais; MAME usa um boundary de colisão próprio no modelo | Colisão por pixel, último pixel ativo, status read e limpeza | Pipeline por dot, instante físico exato e interação com fetch-ahead |
| CRAM timing | Spans preservam prefixo/sufixo de paleta em escritas relevantes | SMS Power descreve CRAM funcionalmente; fontes consultadas não fecham o latch dot-a-dot | Escritas intralinha, mudança de backdrop e spans | CRAM dot, instante físico de latch e comportamento durante fetch |
| VRAM / pattern fetch | Escritas commitam o prefixo; tiles/bitplanes ainda são consultados na composição | Documentação descreve VRAM e padrões, mas não fornece timestamps individuais de fetch para o modelo atual | Escritas intralinha, transições de modo e regressões de renderização | Fetch-ahead de tiles, bitplanes, padrões de sprites e WAIT states reais |
| SAT evaluation | Buffers separados Mode 4 e legacy/TMS; metadados de sprites capturados para a próxima scanline | Documentação descreve SAT e limites funcionais; MAME oferece modelo comparativo | Seleção, overflow, preservação de metadados e transição de modo | Timestamps por entrada SAT/X/tile/pattern e fetch real de padrões |
| Scheduler / CPU INT | VDP avança pelo Scheduler; linha INT é sincronizada após boundaries | Manuais Z80 sustentam semântica de CPU/barramento, não o timestamp físico do VDP | HINT T14 até Z80 INT, VINT, chunking, restore e stress do Scheduler | Resolver empates CPU/VDP com evidência física e expandir equivalência para todos os eventos |

## 4. Evidência automatizada integrada

No HEAD da PR #843, antes do merge:

- 98 testes `sega-video`.
- 131 testes `sega-sms-core`.
- 14 testes de integração SMS.
- Testes direcionados de VINT contínuo versus blocos, snapshot em T12, IRQ em T13 e flag em T16.
- Testes de HINT/R10 antes/depois de T14, restore e propagação até a linha INT do Z80.
- Z80 focused, input focused, Scheduler deterministic stress e workspace regression aprovados.
- Rustfmt, Clippy, runtime Release e build Windows x64 Release aprovados.

Esses resultados provam consistência do modelo nos cenários cobertos. Não provam que os timestamps escolhidos são os mesmos do ASIC físico.

## 5. Próximos passos técnicos

1. Executar a ROM SmsVdpTest no runtime SMS1 e registrar os resultados por teste, sem assumir que o README equivale à execução.
2. Investigar a diferença VINT/HINT entre SmsVdpTest e a tabela MAME 315-5124; não alterar os boundaries antes de evidência melhor.
3. Validar H-counter externo/Light Phaser: TH low/high, instante de latch e atraso; não importar o default MAME de 19 HPos como constante universal.
4. Obter medição física ou referência de hardware reproduzível para HBlank, CRAM dot e fetch-ahead.
5. Expandir testes de clone/restore e chunking para overflow, colisão e escritas CRAM/VRAM em todos os boundaries relevantes.

**Conclusão:** VDP SMS1 continua NÃO LIBERADO PARA TESTE IN-GAME. A temporalidade CPU-visível está mais bem coberta e determinística, mas há divergências entre fontes e pendências de fetch/latch físico que impedem declarar fidelidade temporal suficiente.
