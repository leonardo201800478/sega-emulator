# Registro de evidências — precisão temporal SG-1000/SMS

**Estado:** backlog de pesquisa e validação. Nenhuma linha abaixo autoriza inferência automática de comportamento não documentado.  
**Regra:** cada fato implementado deve apontar para fonte/versão/página/trecho e teste reproduzível.

## 1. Classificação da evidência

| Classe | Significado | Uso |
|---|---|---|
| PRIMARY_DOC | Manual/documentação primária de hardware ou fabricante | Base preferencial |
| MEASURED_HARDWARE | Captura/medição de hardware real com setup e método publicados | Base experimental |
| INDEPENDENT_TEST | Teste/homebrew com resultado esperado e origem verificável | Oráculo de comportamento |
| EMULATOR_SOURCE | Código de emulador | Comparação; não prova isolada |
| COMMUNITY_DOC | Documento técnico comunitário com referências | Apoio, exige triangulação |
| INFERENCE | Dedução a partir de fatos | Hipótese explicitamente rotulada |
| UNKNOWN | Evidência insuficiente ou conflitante | Não implementar como fato |

A ancestralidade de oráculos deve ser registrada. Dois emuladores derivados do mesmo core não contam como duas confirmações independentes.

## 2. Fontes já registradas no repositório

| Fonte | Escopo conhecido | Limite conhecido |
|---|---|---|
| ZiLOG Z80 CPU User Manual UM008011-0816 | T/M cycles, timing diagrams, WAIT, refresh, interrupções e exemplos de instruções | Descreve Z80; não determina sozinho a integração elétrica/temporal específica do SMS |
| SingleStepTests/Z80, SHA fixado no documento Z80 temporal | Amostras de bus cycles para instruções e sinais selecionados | Corpus tem configuração simplificada de memória/refresh e ancestralidade no Ares; não é oráculo físico independente |
| SMS Power — Software Reference Manual for SEGA Mark III | Contrato programável de registradores e VDP | Não fornece necessariamente todas as fases elétricas/sub-T-state |
| SMS Power — VDPRegisters/Modes/Sprites | Registros, modos, sprites e comportamento observado | Documentação técnica comunitária; verificar cada afirmação e revisão |
| MAME 315_5124 | Implementação de referência e timers/raster | Código de emulador; não prova pixel/cycle accuracy por si |
| Ares Master System VDP | Interleaving de processamento horizontal/CPU/VDP em caminhos inspecionados | Código de emulador; versão/commit deve ser fixado antes de usar como oráculo |
| MEKA / GearSystem | Renderização, latches e raster comparativos | Não presumir precisão intralinha; inspecionar fluxo de chamadas e versão |

## 3. Perguntas abertas prioritárias

### P0 — Base temporal e clocks

- Qual unidade inteira mínima representa exatamente as bordas e fases do Master Clock, Z80, VDP e PSG para NTSC/PAL?
- Quais clocks são derivados, quais são independentes e qual é a fase após power-on/reset?
- Há diferenças de clock/fase por revisão ASIC, região ou console?
- Qual sequência de eventos ocorre quando bordas relevantes coincidem?

**Entregável:** tabela fonte→clock→razão→fase/reset→fonte/página→teste.

### P1 — Z80 e barramento

- Validar fases de M1, memória read/write, I/O read/write, interrupt acknowledge, refresh e HALT.
- Documentar o instante de amostragem de endereço/dados e transições de MREQ/IORQ/RD/WR/M1/RFSH/WAIT.
- Determinar como waits SMS são gerados e se existem stalls/contenda por VDP.
- Mapear cada família/prefixo do Z80 a sequência de micro-op e trace de referência.
- Determinar o instante em que INT é amostrada e como EI delay/IM0/1/2 interagem com ciclos.

**Entregável:** trace por T-state com origem, setup, versão e primeiro diff.

### P2 — SMS VDP timing/raster

- H/V counter: frequência, offset, wrap, latches, diferenças NTSC/PAL e ASIC.
- Fronteiras active display/HBlank/VBlank e bordas de IRQ.
- Relação entre CPU I/O write e fase horizontal do VDP.
- Fetch schedule de tile/name/pattern/attributes e acesso à VRAM.
- Sprite evaluation, fetch, limites, overflow e colisão: em que fase cada flag muda?
- Quando writes em R0–R10 passam a valer? Quais registradores são imediatos, latched por linha, por blanking ou por fetch?
- Quando writes em CRAM afetam a saída DAC/pixel?
- Acesso CPU a VRAM: waits, bloqueios, contenda e read-ahead.
- Diferenças reais 315-5124 versus 315-5246, com testes por revisão.

**Entregável:** tabela dot/phase→operação interna→observabilidade→fonte→teste.

### P3 — PSG e áudio digital

- Clock e fase do SN76489 no SG-1000/SMS por revisão/perfil.
- Divisores tone/noise, shift register, latch/data writes e instante de mudança de saída.
- Ordem entre write CPU e atualização do contador do PSG no mesmo timestamp.
- Separar saída digital do chip, filtros analógicos e resampling host.

**Entregável:** traces de transições digitais e vetores PCM com cadeia de origem.

### P4 — SG-1000 preservação

- Confirmar clocks e relação Z80/TMS9918A/SN76489.
- A mesma abstração temporal é válida para TMS9918A ou exige contrato específico?
- Quais eventos e fetches são observáveis e testáveis no SG-1000?
- Golden masters e regressões SG-1000 não podem ser sacrificados para acelerar SMS.

**Entregável:** perfil temporal independente SG-1000 e testes de regressão.

## 4. Solicitação de pesquisa externa (Gemini Plus)

Pedir respostas separadas por tópico. Para cada afirmação, exigir:
1. URL direta e fonte primária quando possível;
2. nome exato do documento, revisão, página/seção/figura;
3. citação curta ou descrição do diagrama;
4. distinguir explicitamente fato documentado, medição e inferência;
5. diferenças entre SMS1 315-5124 e SMS2 315-5246;
6. indicar conflitos entre fontes;
7. propor teste sintético que confirme/falsifique a hipótese;
8. não preencher lacunas com suposição.

Perguntas prioritárias:
- **Pesquisa A — SMS VDP raster:** reconstruir sequência de fetch do Mode 4 por posição horizontal; VRAM contention; sprite evaluation; overflow/collision; scroll R8/R9; CRAM writes e DAC; H/V counters; HINT/VINT; timings NTSC/PAL; diferenças 315-5124/315-5246.
- **Pesquisa B — Z80/SMS bus:** decomposição T-state/M-cycle de todos os ciclos relevantes; sinais MREQ/IORQ/RD/WR/M1/RFSH/WAIT; I/O wait; interrupt acknowledge; refresh; relação temporal com o VDP SMS.
- **Pesquisa C — clocks:** valores exatos e relações de fase Master Clock/Z80/VDP/PSG para SG-1000 e SMS, NTSC/PAL, incluindo reset phase e event ordering.
- **Pesquisa D — PSG:** clock real, divisores, ruído, atualização digital, latch e sincronização com Z80.
- **Pesquisa E — oráculos:** homebrew/test ROMs de domínio público para raster effects, VRAM contention, sprite overflow/collision, H-counter, IRQ e CPU bus timing; licenças, checksums e resultado esperado.

Não é necessário aguardar toda a pesquisa para definir tipos genéricos e testes de invariantes. É obrigatório obter evidência antes de fixar offsets, prioridades, fases, latches ou diferenças de ASIC.

## 5. Manifest por comportamento (modelo)

Cada regra temporal deverá ser rastreada em um registro com campos equivalentes a:

- ID estável;
- componente e perfil/revisão;
- comportamento observado;
- timestamp/fase/unidade;
- classe de evidência;
- fonte, versão, página/seção/commit;
- nível de confiança e limitações;
- fontes contraditórias;
- teste que reproduz;
- implementação e SHA;
- status: UNKNOWN / HYPOTHESIS / DOCUMENTED / MEASURED / VERIFIED.

O manifest de oráculos também deve preservar licença, origem e ancestralidade. Não incluir ROMs comerciais ou BIOS proprietárias no Git.

## 6. Gate de entrada em implementação

- Não inventar prioridades de eventos simultâneos.
- Não inferir timings SMS a partir de um manual Z80 genérico sem validar a plataforma.
- Não inferir ASIC 315-5246 a partir de fonte exclusiva 315-5124.
- Não chamar código de emulador de documentação primária.
- Não atualizar golden master apenas para silenciar divergência.
- Se a evidência for desconhecida, isolar o comportamento em hipótese configurável e manter o gate temporal bloqueado.

## Referências internas

- [Modelo temporal do sistema](cycle-accurate-system-model.md)
- [Arquitetura temporal do VDP SMS](sms-vdp-cycle-accurate-design.md)
- [Contrato Z80/barramento](z80-temporal-bus-model.md)
- [Auditoria raster SMS 2026-10-29](../audits/sms-vdp-raster-timing-investigation-2026-10-29.md)
- [Auditoria latches/raster SMS 2026-10-30](../audits/sms-vdp-latches-raster-audit-2026-10-30.md)


## 7. Atualização — acesso temporal VRAM/CRAM SMS (2026-11-01)

A pesquisa complementar e a auditoria de integração foram preservadas em [Auditoria de integração — acesso temporal à VRAM do SMS](../audits/sms-vram-access-rust-integration-audit-2026-11-01.md).

### Evidências e limites registrados

- A pesquisa fornecida pelo usuário relata corrupção em acessos rápidos no SMS2 e intervalo medido de aproximadamente 26 T-states entre escritas durante Active Display. O setup experimental completo e os documentos originais citados indiretamente ainda precisam ser arquivados para reprodução independente.
- Referências indiretas a manuais Mark III/Game Gear mencionam 29/16 e 28 T-states, respectivamente. Esses números permanecem em conflito com a medição de 26 T-states; não unificar os valores nem generalizá-los para SMS1/CRAM/VBlank.
- Não há evidência suficiente para afirmar WAIT states efetivos, slots em 32 dots/4 dots, fase de slot, buffer físico de escrita com substituição ou política de acesso tardio.
- A existência de read-ahead de um byte é preservada como comportamento documentado; o runtime SMS já mantém `read_buffer` e implementa leitura do byte anterior seguida de pré-carregamento e incremento do endereço.
- O fetch dot-by-dot do Mode 4 SMS e diferenças de arbitragem 315-5124/315-5246 permanecem UNKNOWN.

### Auditoria de integração Rust

- O tipo canônico integrado é `sega_temporal_core::MachineTime(u128)`; não criar um segundo `MachineTime(u64)`.
- `native/temporal_core` fornece domínios racionais e fila determinística, mas não equivale por si só ao Machine Scheduler integrado.
- A PR #341 permanece proposta aberta/Draft na fotografia desta auditoria; sua API não é contrato de `main`.
- O Z80 SMS atual executa por instrução e o contrato `CpuBus` não fornece timestamp por transação I/O. A máquina avança o VDP após a instrução completa.
- Portanto, um contador confiável de espaçamento entre escritas ainda não pode ser ligado ao runtime sem primeiro introduzir transações temporais observáveis no Z80/barramento.

**Decisão atual:** não implementar slots, WAIT, perda/corrupção artificial, política Slotted ou contador de violações no runtime. Preservar o comportamento funcional atual; evoluir primeiro os contratos temporais e a instrumentação de barramento. Nenhum TEMPORAL_PASS é declarado.

## 7. SMS1 VDP 315-5124 — T0 characterization (2026-11-02)

Detailed evidence ledger: [SMS1 VDP T0 characterization](../audits/sms1-315-5124-vdp-t0-characterization-2026-11-02.md).

Newly recorded source facts:
- SMS Power distinguishes ASIC 315-5124 and 315-5246; known primary differences include SMS1 legacy VRAM address-mask behavior and SMS2-only 224/240-line modes. This is COMMUNITY_DOC, not an exhaustive electrical specification.
- Sega Master System Service Manual lists IC5 pin 30 at 10.738635 MHz ±100 Hz and pin 34 at 3.579545 MHz ±30 Hz. This is PRIMARY_DOC/OCR; it does not establish edge phase, duty cycle, reset phase, or bus arbitration.
- The Mark III Software Reference Manual documents 16 Z80A T-states between VDP accesses in VBLANK and 29 during active video, R9 latched in VBlank, R10 H-Line interrupt at HBLANK with one-interrupt update delay, and status behavior. The access-rate rule is not proof of automatic Z80 WAIT injection.
- SMS Power documents 315-5124 address mask bits and the SMS1 sprite horizontal-zoom limitation; timing offsets for evaluation/fetch/overflow/collision remain UNKNOWN.

Scope decision: first new VDP implementation is 315-5124 only. 315-5246 comparison follows later. ASIC identity and NTSC/PAL remain independent dimensions. Do not import Game Gear H-counter ratios/timings into SMS1.

Status: T0_PARTIAL. Documentary evidence supports register/address/functional behavior and test design. It does not yet authorize H-counter offsets, raster phase constants, per-dot fetch schedules, WAIT generation, or simultaneous-event priorities.
- Candidate test corpus: FluBBaOfWard/SmsVdpTest at inspected commit `490c00b38c83012dee5a7a582097f080c210a3e7`. Covers VDP ports, counters, IRQs, sprite flags/collision, and latch effects. Repository declares no license; do not vendor source/binary before permission review. Its own TODO leaves several hardware latch tests incomplete; 228 CPU cycles/line is a stated test precondition, not yet an independently confirmed 315-5124 measurement.