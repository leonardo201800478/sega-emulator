# Auditoria SMS VDP — R8 e H-counter em boundaries temporais

**Data:** 2026-11-06  
**Baseline:** `671a91cce990f4e69480b8680c741228afaa5cc7`  
**Branch:** `feat/sms-vdp-r8-hcounter-temporal-latches-20261106`  
**Escopo:** latch horizontal de R8 e latch de H-counter por TH no caminho temporal SMS.

## 1. Frente documental — SMS Power

O projeto [SmsVdpTest](https://github.com/FluBBaOfWard/SmsVdpTest) fornece uma ROM de diagnóstico e suas notas de referência:

- O H-counter do port `$7F` deve permanecer constante em condições normais.
- A transição TH `0 -> 1` captura o H-counter corrente.
- A transição pode ser provocada pelo joystick ou pelo software via port `$3F`.
- A nota de referência descreve o contador por HClock/MClock e registra o latch de HScroll na região de HCount `$F3`.
- O código de teste usa `IO_Ctrl = $55` seguido de `IO_Ctrl = $FF` antes de ler `$7F`: TH é primeiro dirigido para baixo e depois liberado como entrada alta.

Fontes:
- [SmsVdpTest — VDPTest-ReadMe.txt](https://github.com/FluBBaOfWard/SmsVdpTest/blob/main/VDPTest-ReadMe.txt)
- [SmsVdpTest — testmain.asm](https://github.com/FluBBaOfWard/SmsVdpTest/blob/main/testmain.asm)
- [SMS Power — SMS Official Docs](https://www.smspower.org/Development/SMSOfficialDocs)

As notas do SmsVdpTest são referência de teste de hardware de terceiros; esta execução não rodou a ROM em console físico.

## 2. Frente de engenharia reversa — MAME

O código MAME para o VDP 315-5124 fornece um modelo independente:

- `line_315_5124` define `XSCROLL_HPOS = 21`.
- A rotina `hcount()` usa o HPos anterior, faz wrap pela largura horizontal, subtrai 46 e divide por dois.
- `sms_io_control_w` captura H-counter quando TH passa de saída baixa para entrada alta.
- `sms_ctrl1_th_input` e `sms_ctrl2_th_input` capturam na borda ascendente da entrada TH.

Fontes:
- [MAME — 315_5124.cpp, timing e H-counter](https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp)
- [MAME — sms_m.cpp, TH e I/O control](https://github.com/mamedev/mame/blob/master/src/mame/sega/sms_m.cpp)

O MAME é um modelo de emulação comparativo, não uma prova física isolada. A concordância com a ROM de diagnóstico do SmsVdpTest aumenta a confiança, mas não substitui a execução em SMS1/SMS2 real.

## 3. Conversão temporal

O perfil SMS usa 342 posições horizontais por linha e 228 Z80 T-states por linha:

`342 / 228 = 3/2` posições horizontais por T-state.

### R8 — HScroll

- O latch de HScroll é copiado para a nova linha no boundary de scanline.
- Uma escrita em R8 enquanto `HPos <= 21` (inclusive) atualiza a linha corrente.
- Uma escrita após HPos 21 só afeta a próxima linha.
- HPos 21 corresponde a Z80 T-state 14, não 7.
- Escritas durante VBlank atualizam o latch imediatamente para o próximo período visível.

A implementação usa `latched_scroll_x`; o renderer recebe uma cópia dos registradores com R8 substituído pelo valor latched. A escrita em R8 continua atualizando o registrador de leitura, mas só altera o latch corrente dentro da janela documentada.

### H-counter

A conversão segue o modelo 315-5124 do MAME:

1. `hpos = floor(tstates_into_line * 3 / 2) % 342`;
2. `previous_hpos = hpos - 1`, com wrap para 341 quando HPos é zero;
3. `HCounter = ((previous_hpos - 46) >> 1) & 0xFF`.

Pontos de regressão usados:

| Z80 T-state | HPos | H-counter |
|---:|---:|---:|
| 0 | 0 | `$93` |
| 1 | 1 | `$E9` |
| 31 | 46 | `$FF` |
| 32 | 48 | `$00` |
| 136 | 204 | `$4E` |

O valor lido em `$7F` é o último valor capturado; não é um contador livre que muda a cada leitura.

### TH — condição de latch

No caminho temporal, uma escrita ímpar em `$01..$3F` é aplicada depois de o Scheduler avançar o VDP até o timestamp de conclusão do ciclo de barramento. O barramento compara os níveis TH antes/depois e só chama `latch_h_counter()` quando:
- houve uma transição ascendente em TH1/TH2; e
- o respectivo pino está configurado como entrada no novo valor de I/O control.

Assim, no caminho atualmente implementado:
- saída baixa -> entrada alta por escrita em I/O control: captura;
- saída baixa -> saída alta: não captura;
- transição descendente: não captura.

A borda ascendente de uma entrada TH externa enquanto o pino já está configurado como entrada também é exigida pelo modelo MAME, mas ainda não possui um evento de periférico dedicado no `SmsIo/SmsTemporalBus`; permanece NOT RUN. O Light Phaser continua usando seu evento sintético interno para atualizar o latch.

A ROM de teste integrada executa o Z80 temporal real para TH1 e TH2 e lê o valor por `IN A,($7F)`. Também verifica que saída-alta e borda descendente não substituem o valor capturado.

## 4. Integração arquitetural

- O VDP compartilhado está em `native/video_core/src/sms/vdp.rs`.
- O caminho ativo é `SmsTemporalMachine -> SmsTemporalBus -> SmsVdp`.
- `SmsTemporalBus::dispatch_next` avança os domínios do Scheduler até a conclusão do evento antes de aplicar a escrita de porta.
- O latch de R8 é decidido no momento da escrita do registrador, usando o HPos atual.
- O latch de H-counter é aplicado no commit da escrita de I/O control, sem callback por instrução e sem sincronização paralela em Python.
- Os campos `latched_scroll_x` e `h_counter_latch` fazem parte do estado clonado do VDP e, portanto, dos snapshots em memória do barramento.

A proposta antiga de adicionar `synchronize_tstates` ao caminho de CPU atômica não foi portada: o caminho temporal já fornece os boundaries corretos. A proposta antiga de R8 em 7 T-states foi corrigida para HPos 21 / T14. A detecção de qualquer transição TH foi restringida à borda ascendente em entrada.

## 5. Testes adicionados

### `sega-video`
- R8 escrito exatamente em T14 atualiza a linha corrente.
- R8 escrito em T15 preserva a linha corrente e atualiza a próxima.
- H-counter nos pontos T0, T1, T31, T32 e T136.
- H-counter permanece estável enquanto o VDP continua avançando.

### `sega-sms-core`
- TH1: `$DD -> $DF` captura.
- TH2: `$77 -> $7F` captura.
- Saída baixa -> saída alta não captura.
- Borda descendente não substitui o valor capturado.
- Todos os casos passam pelo `SmsTemporalMachine`, `SmsTemporalBus` e Scheduler.

## 6. Limitações e classificação

| Item | Estado |
|---|---|
| R8 HPos 21 / T14 no modelo de referência | Implementado; CI pendente |
| H-counter latched por TH ascendente | Implementado; CI pendente |
| TH1 e TH2 pelo caminho temporal | Testes adicionados; CI pendente |
| Persistência em snapshot/restore em memória | Incluída via clone do VDP; teste dedicado pendente |
| Execução da SmsVdpTest ROM em console físico | NOT RUN |
| Confirmação física SMS1 e SMS2 | NOT RUN |
| Fase absoluta do H-counter no reset | Modelo MAME; validação física pendente |
| Raster/fetch por dot e efeitos de outros registradores mid-scanline | Fora deste incremento |
| Save states persistentes via serde | Fora deste incremento; continua gate separado |
| Light Phaser | Mantida a conversão aproximada existente; validação elétrica separada |

## 7. Decisão

Este incremento corrige os dois limites temporais sem introduzir eventos por dot:
- R8 possui uma janela de latch horizontal explícita;
- H-counter possui latch persistente disparado por TH ascendente no boundary de barramento.

Não altera o limite físico de sprites, o pipeline de pixels, a temporização do Scheduler ou a lógica de CPU. A validação física continua explicitamente pendente.
