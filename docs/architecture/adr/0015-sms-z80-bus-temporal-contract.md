# ADR-0015 — contrato temporal SMS/Z80/barramento e resolução de safe points

- **Estado:** Proposta para revisão; decisão de runtime NÃO aprovada.
- **Data:** 2026-11-02.
- **Base:** PR #360, head `b2079083d5e19ac1b1936ba98cf40ae98c7b7f9b`.
- **Branch desta ADR:** `docs/adr-0015-sms-z80-bus-temporal-contract`.
- **Dependências:** ADR-0010, ADR-0012, ADR-0013, ADR-0014 e `docs/architecture/z80-temporal-bus-model.md`.
- **Escopo:** primeira fronteira de integração temporal do SMS, com foco no Z80, bus adapter e compatibilidade com a timeline genérica.
- **Não faz:** implementar CPU retomável, arbitrar o barramento, alterar timing VDP/PSG, adicionar sub-tick dispatch ou declarar `TEMPORAL_PASS`.

## 1. Contexto e estado remoto

A stack de infraestrutura temporal está em revisão e não foi integrada à `main`:

| PR | Base | Head | Estado no início desta ADR |
|---|---|---|---|
| #358 | `main` | `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607` | OPEN / Draft |
| #359 | branch #358 | `a544fe367f67c1f6c6d622efcdb528e222ab53c3` | OPEN / Draft |
| #360 | branch #359 | `b2079083d5e19ac1b1936ba98cf40ae98c7b7f9b` | OPEN / Draft |

`main` foi confirmada em `0da3098893cce4d80f7a22291d00488e4e7d0f9b`. A PR #358 usa esse SHA como base. Os heads e checks da stack permanecem associados aos SHAs exatos registrados no ledger. Nenhuma PR foi mergeada nesta etapa.

O Scheduler atual fornece timeline inteira, domínios racionais, preservação de fase, ordenação determinística, snapshot/restore, codec e replay nos cenários cobertos. `ClockEdgeTime` calcula posições racionais de borda, mas `EventQueue` despacha em `MachineTime` inteiro. `HardwarePhase` e `PlatformPriority` são dimensões ordinais de ordenação, não instantes físicos ou fases elétricas.

O runtime SMS existente permanece em `native/sms_core` e `native/z80_core`. A implementação Scheduler candidata está em `native/machine_scheduler` na stack #358. Não existe ainda ligação entre os dois.

## 2. Evidência atual no caminho Z80/SMS

### 2.1 CPU e barramento

- `native/sms_core/src/cpu.rs`: `Z80::step` executa uma instrução completa e retorna T-states agregados. A atualização de `total_tstates` ocorre no caminho da instrução, não como avanço cooperativo do Scheduler.
- `native/z80_bus/src/lib.rs`: `CpuBus` expõe operações lógicas de memória e I/O e hooks opcionais de observação temporal.
- `native/sms_core/src/machine.rs`: `SmsMachineBus` aplica leituras/escritas ao mapa SMS durante a execução da instrução.
- O barramento não recebe atualmente um deadline do Scheduler, não suspende a CPU por WAIT e não devolve uma transação pendente retomável.

### 2.2 Trace temporal já existente — não duplicar

`docs/architecture/z80-temporal-bus-model.md` documenta que já existe trace M-cycle-level e amostragem por T-state para oito instruções ED de transferência/comparação sem DD/FD:

- LDI, LDD, LDIR, LDDR;
- CPI, CPD, CPIR, CPDR.

O trace cobre endereço/dados e RD/WR/MREQ/IORQ no perfil de observação testado. A fase de refresh é inferida; M1, RFSH e WAIT não são pinos independentes modelados nesse trace. O trace é passivo e não controla o Scheduler.

A instrumentação de instrução JSONL também é observacional e não é um trace completo de transações. Portanto, a primeira implementação não deve criar um segundo mecanismo concorrente de trace: deve inventariar, normalizar e estender os tipos/harness existentes.

### 2.3 SMS Machine, VDP e PSG

- `SmsMachine::step` executa CPU e depois avança o VDP pela duração agregada retornada.
- O adaptador SMS aplica as leituras/escritas de memória e portas durante as chamadas síncronas do CPU.
- Writes do PSG entram numa fila com timestamp nominal; a fila é consumida retrospectivamente após a instrução. Isso não equivale a efetivação temporal online.
- O VDP é um modelo por scanline, com 228 T-states internos por linha; sua própria documentação de código não reivindica precisão por dot.
- A sincronização VDP→INT é atualizada no avanço do VDP e observada pelo CPU em fronteiras de instrução.
- A fronteira PyO3 ainda expõe `step()` e `run_frame(max_steps)`; não deve ser ampliada com callbacks por transação.

### 2.4 Mapper e efeitos colaterais

O SMS memory map já aplica alterações de bancos e controles no adaptador de memória. O modelo temporal futuro precisa distinguir o momento em que o endereço/dado é apresentado, o instante em que a escrita se torna efetiva e o primeiro acesso que observa o novo banco. Não há evidência suficiente nesta ADR para fixar essas fases físicas.

## 3. Fontes de hardware e limites de inferência

### 3.1 Z80 genérico — fabricante

ZiLOG, *Z80 CPU User Manual*, UM008011-0816 (08/2016), seção Timing, pp. 7–11, Figuras 4–7:

- M1 é um ciclo de fetch com quatro T-states no modelo descrito;
- ciclos comuns de memória têm três T-states nominais;
- WAIT é amostrado na borda descendente do clock durante T2 e TW;
- o ciclo de I/O descrito pelo manual insere um TW automático;
- no fetch, o endereço muda para refresh e MREQ/RFSH têm comportamento próprio.

Esses são fatos do Z80 descrito pelo fabricante. Não determinam por si só waits externos, decodificação, contenção ou prioridade no Master System.

Referência: https://www.zilog.com/docs/z80/um0080.pdf

### 3.2 Master System — conectores e linhas

SMS Power, *SMS Cartridge / Cartridge Slot*:
- https://www.smspower.org/Development/SMSCartridge
- https://www.smspower.org/Development/CartridgeSlot

A documentação do slot lista sinais como INT, WAIT, BUSREQ, BUSACK, RFSH, M1, IORQ, MREQ, RD e WR. Isso confirma que sinais relevantes estão expostos na interface de cartucho; não documenta, por si só, a forma de onda completa, o produtor de WAIT, as condições de contenção ou a precedência elétrica em todas as revisões.

### 3.3 Clocks e perfis

A documentação técnica secundária consultada para o manual de serviço SMS1 relata valores nominais distintos para NTSC e PAL no clock de VDP e CPU. Há uma aparente diferença de arredondamento entre a divisão aritmética dos valores publicados; não será inferida uma razão exata nem fase inicial a partir desses números sem consultar a fonte primária e a revisão de placa correspondente.

Nenhuma constante de clock, divisor, fase CPU↔VDP, fase PSG, wait state ou prioridade de barramento é fixada por esta ADR.

### 3.4 Classificação de evidência

| Afirmação | Classe | Uso permitido |
|---|---|---|
| M/T-cycles, M1, WAIT e TW de I/O | Manual primário do Z80 genérico | Modelo base da CPU; não extrapolar eletricamente para SMS sem validação |
| Sinais disponíveis no slot SMS | Documentação de pinout SMS Power | Inventário de sinais; não prova forma de onda |
| Relação de clock em placa/revisão SMS | Manual de serviço / medição por confirmar | Hipótese de perfil; não fixar ratio até resolver fonte e tolerância |
| Comportamento Ares/Gearsystem/MAME | Implementação de emulador | Comparação diferencial, não autoridade de hardware |
| Trace SingleStepTests/Z80 | Corpus externo com ancestralidade documentada no Ares | Regressão de configuração de teste; não oráculo independente do Ares |
| Trace físico de placa | Ainda não coletado nesta etapa | Necessário para afirmações elétricas não documentadas |

## 4. Decisão proposta

### 4.1 Separar três níveis

1. **CPU microsequencer:** representa a continuação da instrução, incluindo opcode/prefixos, estado do decoder, micro-op atual e transação pendente.
2. **SMS bus transaction layer:** traduz a micro-operação em ciclo Z80, resolve memória/I/O/mapper e decide os instantes de amostragem/efetivação conforme o perfil documentado.
3. **Machine Scheduler:** avança domínios e despacha eventos tipados até limites autorizados. Não conhece instruções, endereços SMS, portas, VDP ou prioridades elétricas.

A CPU e o barramento continuam genéricos por monomorfização. Não introduzir `dyn Bus` no hot path e não fazer o Scheduler chamar callbacks arbitrários de dispositivo.

### 4.2 Safe points mínimos a representar

O desenho de CPU retomável deve cobrir, sem presumir que todos tenham a mesma duração ou sequência:

- M1/opcode fetch e refresh;
- leitura e escrita de memória;
- leitura e escrita de I/O;
- ciclos internos;
- stack e operações read-modify-write;
- prefixos repetidos/ignorados e instruções indexadas;
- instruções repetidas de bloco, com fronteira por iteração;
- HALT;
- NMI, INT, acknowledge e push/fetch associado;
- WAIT/TW, quando o perfil e a fonte determinarem sua inserção;
- eventos simultâneos CPU/VDP e efeitos clear-on-read/latch.

Uma instrução não pode ser suspensa em um ponto que perca estado causal. A conclusão da instrução é um estado explícito, não a simples soma retrospectiva de ciclos.

### 4.3 Contrato conceitual, não API congelada

O contrato futuro deve permitir:

- CPU emitir a próxima operação necessária sem efetivar antecipadamente o efeito externo;
- bus adapter aceitar a operação, determinar seu plano temporal e devolver uma conclusão;
- Scheduler avançar a timeline até a próxima fronteira autorizada;
- bus adapter efetivar/amostrar a transação e devolver dados/resultado;
- CPU retomar com estado serializável;
- snapshot/restore capturar uma transação pendente exatamente uma vez.

Os tipos Rust concretos, ownership dos dados e divisão entre `CpuMicroOp`, `BusRequest` e `BusCompletion` permanecem para protótipo e revisão. Esta ADR não congela assinaturas públicas.

### 4.4 Preservar o trace existente

O trace ED existente será usado como primeiro corpus de validação do microsequencer, sem ampliar imediatamente para todos os opcodes. A instrumentação passiva deve continuar opcional e provar que desligada não altera estado, contagem ou comportamento.

A primeira fatia de execução deve gerar trace a partir do caminho real da CPU/bus, não fabricar trace offline a partir do opcode e do total agregado.

## 5. Granularidade temporal — decisão ainda bloqueada por evidência

A solução exige combinação de:
- micro-operações para suspensão/retomada;
- T-states/M-cycles para o modelo nominal Z80;
- timestamp absoluto da máquina para sincronização dos componentes.

A escolha entre grade inteira comum e timestamp racional na fila ainda NÃO está fechada.

Critério:
1. identificar as bordas observáveis necessárias no manual Z80 e no SMS real;
2. estabelecer as relações exatas de clocks e fases por perfil/revisão;
3. verificar se todas as bordas cabem em uma unidade inteira comum por LCM dos denominadores;
4. somente se isso não for demonstrável, propor extensão de timestamp racional para despacho;
5. não usar `HardwarePhase` como substituto de sub-tick.

A grade inteira comum é preferível se representar exatamente todos os eventos exigidos sem overflow ou denominadores impraticáveis. `ClockEdgeTime` pode ser usado para cálculo e prova; não se deve fingir que ele já é aceito pela EventQueue.

## 6. Interação com ADR-0014 e roadmap existente

A ADR-0014 estabelece safe points, separação entre Scheduler genérico e perfil/barramento, e sugere uma sequência de integração que inclui um componente simples e o YM2413 antes da CPU.

O roadmap consolidado `docs/roadmaps/sg1000-sms-consolidation-roadmap.md` posiciona a implementação do Scheduler na Fase 8, após fechamento funcional e robustez; o roadmap PDF também descreve o Scheduler como implementação tardia, mantendo contratos temporais antes disso.

O pedido de continuidade de 2026-11-02 prioriza agora o desenho da integração SMS começando pelo Z80 e barramento. Esta ADR registra essa frente como **trabalho de design e prototipagem isolada**, não como autorização para antecipar a integração de runtime nem para reordenar automaticamente as fases normativas. A mudança formal da sequência do roadmap deve ser aprovada explicitamente antes de abrir PRs de runtime que antecipem a Fase 8.

## 7. Plano de validação

| Etapa | Trace/teste | Oráculo e limite |
|---|---|---|
| CPU fetch | M1, opcode, refresh, PC/R | ZiLOG + trace passivo; pinos ausentes permanecem não modelados |
| Memória | read/write, stack, RMW | ZiLOG para sequência genérica; SMS para decodificação |
| I/O | IN/OUT imediato e ED | ZiLOG para TW automático genérico; pinout SMS não prova waits externos |
| Mapper | write control + primeiro read do banco novo | Teste sintético de bus; hardware timing pendente |
| VDP status | clear-on-read em fronteira de raster | VDP documentation/medição; emuladores só diferenciais |
| IRQ/NMI | evento durante instrução, reconhecimento e acknowledge | ZiLOG para Z80 genérico; fonte SMS específica pendente |
| H/VBlank | evento raster e linha INT | Perfil VDP + trace físico/oráculo independente pendente |
| PSG | write, fase/divisor e avanço | modelo SN76489 e evidência de clock SMS pendentes |
| WAIT | amostragem e repetição TW | ZiLOG genérico + captura de WAIT no SMS necessária |
| Snapshot | salvar em cada safe point e em transação pendente | comparação com execução contínua |
| Replay | mesmas entradas/eventos/trace | hash e primeira divergência de evento |
| Regressão | SG-1000 e SMS funcional | golden masters e suites atuais no SHA candidato |

A comparação com Ares, Gearsystem e MAME deve registrar SHA/versão e divergência, sem eleger um deles como autoridade absoluta. O corpus SingleStepTests deve continuar identificado como derivado do Ares.

## 8. Fatiamento proposto após revisão da ADR

| Fatia | Escopo | Critério de aceitação |
|---|---|---|
| T0 | Corrigir documentação/ledger do trace atual e consolidar matriz de opcodes × cobertura temporal | Nenhum trace existente duplicado; gaps listados por família |
| T1 | Protótipo headless de safe-point e snapshot da CPU, sem ligar hardware real | Suspensão/retomada e restore reproduzem estado e sequência |
| T2 | Adaptador de barramento com transações sintéticas e relógio virtual | Ordem e efeitos determinísticos; WAIT configurável apenas em fixtures |
| T3 | Microsequencer Z80 para subconjunto já coberto pelo trace ED | Trace runtime bate com corpus existente no escopo declarado |
| T4 | Ampliar famílias de transações com testes por opcode/micro-op | Primeira divergência, não apenas ciclos totais |
| T5 | Provar a unidade temporal SMS por perfil/revisão e decidir despacho inteiro/sub-tick | Fonte/medição e oracle independente para cada relação |
| T6 | Primeira integração CPU/bus/Scheduler em SMS, sem mudar semântica não documentada | Snapshot/replay e traces integrados; SG-1000 preservado |
| T7 | Sincronização VDP/PSG/IRQ progressiva | H/VBlank, I/O, áudio e waits cobertos por traces/oráculos |

Cada fatia de runtime exige branch própria, PR Draft, gates completos no SHA exato e separação entre mudança estrutural e correção semântica. Não alterar `main`, não rebasear a stack #358–#360 automaticamente e não declarar `TEMPORAL_PASS` antes da integração e dos oráculos.

## 9. Critérios de saída desta ADR

- [ ] Revisão da compatibilidade com ADR-0014.
- [ ] Decisão explícita sobre a sequência do roadmap (manter Fase 8 tardia ou antecipar integração).
- [ ] Inventário de cobertura dos traces Z80 existente por família.
- [ ] Evidência primária ou captura reproduzível para relação de clocks/fases SMS.
- [ ] Modelo de transação e snapshot revisado, sem API pública prematura.
- [ ] Plano de testes com oráculos e ancestralidade documentados.

**Status atual:** design proposto; nenhuma implementação autorizada por esta ADR. `INTEGRATION_PENDING` permanece. `TEMPORAL_PASS` não declarado.

## Referências

- ZiLOG, *Z80 CPU User Manual*, UM008011-0816 (08/2016), pp. 7–11, Figs. 4–7: https://www.zilog.com/docs/z80/um0080.pdf
- SMS Power, SMS Cartridge: https://www.smspower.org/Development/SMSCartridge
- SMS Power, Cartridge Slot: https://www.smspower.org/Development/CartridgeSlot
- `docs/architecture/z80-temporal-bus-model.md`
- `docs/architecture/adr/0014-temporal-device-execution-and-synchronization.md`
- `docs/architecture/adr/0012-event-ordering-dimensions.md`
- `docs/architecture/adr/0013-deterministic-scheduler-snapshot-restore.md`
- `docs/roadmaps/sg1000-sms-consolidation-roadmap.md`
