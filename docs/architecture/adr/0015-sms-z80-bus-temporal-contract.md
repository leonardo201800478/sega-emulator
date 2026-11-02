# ADR-0015 — contrato temporal SMS/Z80/barramento e resolução de safe points

- **Estado:** Direção arquitetural aprovada para antecipação incremental; implementação de runtime sujeita a PRs e gates.
- **Data:** 2026-11-02.
- **Base:** PR #360, head `b2079083d5e19ac1b1936ba98cf40ae98c7b7f9b`.
- **Branch desta ADR:** `docs/adr-0015-sms-z80-bus-temporal-contract`.
- **Dependências:** ADR-0010, ADR-0011, ADR-0012, ADR-0013, ADR-0014, `docs/architecture/z80-temporal-bus-model.md` e `docs/architecture/sms-vdp-cycle-accurate-design.md` (arquitetura-alvo em revisão na PR #342).
- **Escopo:** contrato da integração temporal do SMS entre Z80, bus adapter, Machine Scheduler e o **novo VDP temporal em construção**, conforme a arquitetura aprovada na ADR-0011 e detalhada na PR #342.
- **Não faz:** integrar o Z80 ao VDP legado `native/sms_core/src/vdp.rs`, tratá-lo como destino temporal, implementar CPU retomável, arbitrar o barramento, fixar timing sem evidência, adicionar sub-tick dispatch ou declarar `TEMPORAL_PASS`.

## 1. Contexto e estado remoto

A stack de infraestrutura temporal está em revisão e não foi integrada à `main`:

| PR | Base | Head | Estado no início desta ADR |
|---|---|---|---|
| #358 | `main` | `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607` | OPEN / Draft |
| #359 | branch #358 | `a544fe367f67c1f6c6d622efcdb528e222ab53c3` | OPEN / Draft |
| #360 | branch #359 | `b2079083d5e19ac1b1936ba98cf40ae98c7b7f9b` | OPEN / Draft |

`main` foi confirmada em `0da3098893cce4d80f7a22291d00488e4e7d0f9b`. A PR #358 usa esse SHA como base. Os heads e checks da stack permanecem associados aos SHAs exatos registrados no ledger. Nenhuma PR foi mergeada nesta etapa.

O Scheduler atual fornece timeline inteira, domínios racionais, preservação de fase, ordenação determinística, snapshot/restore, codec e replay nos cenários cobertos. `ClockEdgeTime` calcula posições racionais de borda, mas `EventQueue` despacha em `MachineTime` inteiro. `HardwarePhase` e `PlatformPriority` são dimensões ordinais de ordenação, não instantes físicos ou fases elétricas.

O runtime SMS atualmente executável permanece em `native/sms_core` e `native/z80_core`; seu `SmsVdp` scanline-based é o VDP legado e **não é o alvo da integração temporal**. A arquitetura do novo VDP está especificada em `docs/architecture/sms-vdp-cycle-accurate-design.md` (PR #342): `TimingGenerator`, `FetchPipeline`, `SpriteEngine`, `PixelPipeline`, `RegisterFile/latches`, VRAM/CRAM e `InterruptController`, todos sincronizados pelo único Machine Scheduler. A implementação Scheduler candidata está em `native/machine_scheduler` na stack #358. O novo VDP ainda está em construção e não há ligação integrada entre Z80/barramento/Scheduler/novo VDP.

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

### 2.3 Runtime legado — inventário, não destino

Os itens abaixo descrevem exclusivamente o runtime legado para localizar os pontos que serão substituídos/desacoplados; não são requisitos a preservar no novo modelo temporal:

- `SmsMachine::step` executa CPU e depois avança o VDP legado pela duração agregada retornada.
- O adaptador SMS aplica leituras/escritas de memória e portas durante chamadas síncronas do CPU.
- Writes do PSG entram numa fila com timestamp nominal; a fila é consumida retrospectivamente após a instrução. Isso não equivale a efetivação temporal online.
- `native/sms_core/src/vdp.rs` renderiza por scanline, com 228 T-states internos por linha. A auditoria documenta ausência de fetch pipeline, pixel stepping, visibilidade intralinha geral e bordas subinstrução de IRQ. Esse modelo é reconhecidamente insuficiente para a integração temporal pretendida.
- A sincronização VDP→INT legada é atualizada no avanço do VDP e observada pelo CPU em fronteiras de instrução.
- A fronteira PyO3 ainda expõe `step()` e `run_frame(max_steps)`; não deve ser ampliada com callbacks por transação.

O novo VDP deverá ser implementado conforme a arquitetura temporal dedicada: gerador de raster, fetch pipeline, avaliação/fetch de sprites, pixel pipeline, latches/registradores temporais, VRAM/CRAM e IRQ. A API de saída pode manter compatibilidade com a apresentação existente, mas a implementação scanline legada não será a autoridade de hardware nem receberá as transações temporais do novo Z80.

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
3. **Novo SMS VDP temporal:** mantém seu próprio estado de raster/fetch/sprites/pixels/latches/IRQ e anuncia próximos eventos observáveis; não possui scheduler nem clock mestre autônomo.
4. **Machine Scheduler:** avança domínios e despacha eventos tipados até limites autorizados. Não conhece instruções, endereços SMS, portas, VDP ou prioridades elétricas.

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

## 6. Integração obrigatória com o novo VDP e interação com ADR-0014

A ADR-0011 e o documento `sms-vdp-cycle-accurate-design.md` estabelecem o novo VDP temporal como arquitetura-alvo: o scanline renderer legado não pode ser a autoridade de raster, fetch, sprite evaluation, pixel output ou IRQ. A nova implementação deve avançar até timestamps solicitados pelo Scheduler e sincronizar antes de aplicar transações CPU/barramento. O Z80 não deve ser conectado ao VDP legado como atalho transitório.

A ADR-0014 estabelece safe points, separação entre Scheduler genérico e perfil/barramento, e sugere uma sequência de integração que inclui um componente simples e o YM2413 antes da CPU.

O roadmap consolidado `docs/roadmaps/sg1000-sms-consolidation-roadmap.md` posiciona a implementação do Scheduler na Fase 8, após fechamento funcional e robustez; o roadmap PDF também descreve o Scheduler como implementação tardia, mantendo contratos temporais antes disso.

A decisão explícita do mantenedor em 2026-11-02 autoriza antecipar a integração temporal do Scheduler. O trabalho de runtime pode avançar por PRs incrementais, começando pela reconciliação dos contratos de CPU retomável, transações de barramento e coordenação da máquina. A autorização não aprova automaticamente APIs, fases elétricas, clocks, WAITs, arbitragem ou semânticas ainda sem evidência.

A antecipação não significa declarar concluídas as fases funcionais nem conectar o Z80 ao VDP legado. O roadmap único coloca a fundação Scheduler-driven dentro da fase de barramento e integração; a fase final permanece reservada à comprovação sistêmica de `TEMPORAL_PASS`. O novo VDP temporal continua sendo o destino de raster/IRQ.

## 7. Plano de validação

| Etapa | Trace/teste | Oráculo e limite |
|---|---|---|
| CPU fetch | M1, opcode, refresh, PC/R | ZiLOG + trace passivo; pinos ausentes permanecem não modelados |
| Memória | read/write, stack, RMW | ZiLOG para sequência genérica; SMS para decodificação |
| I/O | IN/OUT imediato e ED | ZiLOG para TW automático genérico; pinout SMS não prova waits externos |
| Mapper | write control + primeiro read do banco novo | Teste sintético de bus; hardware timing pendente |
| VDP status | clear-on-read em fronteira de raster | Novo VDP temporal; documentação/medição; emuladores só diferenciais |
| IRQ/NMI | evento durante instrução, reconhecimento e acknowledge | ZiLOG para Z80 genérico; fonte SMS específica pendente |
| H/VBlank | evento raster e linha INT | TimingGenerator/InterruptController do novo VDP + trace físico/oráculo independente pendente |
| PSG | write, fase/divisor e avanço | modelo SN76489 e evidência de clock SMS pendentes; não usar o VDP legado como relógio de referência |
| WAIT | amostragem e repetição TW | ZiLOG genérico + captura de WAIT no SMS necessária |
| Snapshot | salvar em cada safe point e em transação pendente | comparação com execução contínua |
| Replay | mesmas entradas/eventos/trace | hash e primeira divergência de evento |
| Regressão | SG-1000 e SMS funcional | golden masters e suites atuais no SHA candidato |

A comparação com Ares, Gearsystem e MAME deve registrar SHA/versão e divergência, sem eleger um deles como autoridade absoluta. O corpus SingleStepTests deve continuar identificado como derivado do Ares.

## 8. Fatiamento proposto após revisão da ADR

| Fatia | Escopo | Critério de aceitação |
|---|---|---|
| T0 | Reconciliar a PR #396 e os contratos temporais existentes com o microsequenciador já integrado; inventariar o trace atual sem duplicá-lo | Uma única definição de transação/continuação; lacunas e ownership documentados |
| T1 | Completar o estado arquitetural retomável e seus pontos de snapshot, preservando o executor atômico como referência diferencial | Suspensão/retomada/restore reproduzem registradores, micro-op, índices M/T e transação pendente |
| T2 | Implementar o coordenador da máquina sobre o `MachineScheduler`, sem lógica de dispositivo dentro do Scheduler | Um único `MachineTime`, deadlines explícitos, avanço monotônico e consumo determinístico de dispatches |
| T3 | Integrar um adaptador de barramento transacional com operações sintéticas, efeitos exatamente-uma-vez e WAIT apenas em fixtures controladas | Ordem causal, suspensão sem busy-wait e replay idêntico; sem alegar timing elétrico SMS ainda não demonstrado |
| T4 | Primeira fatia vertical Z80/bus/Scheduler em SMS usando somente operações cuja sequência temporal esteja coberta | Trace originado do caminho real, comparação diferencial e snapshot/replay; VDP legado excluído |
| T5 | Expandir famílias do microsequenciador e transações (M1/refresh, memória, I/O, stack, RMW, prefixos, interrupções) com oráculos por escopo | Primeira divergência por operação, não apenas total de T-states; UNKNOWN preservado onde faltar evidência |
| T6 | Provar unidade temporal e perfil SMS por revisão; integrar o novo VDP temporal e eventos IRQ/raster progressivamente | Relações de clock/fase sustentadas por fonte/medição; sem sub-tick até decisão formal específica |
| T7 | Integrar PSG/áudio, WAIT/arbitragem e snapshots/replay da máquina completa | CPU/bus/VDP/PSG causalmente integrados; SG-1000 e SMS regressivos preservados |

Cada fatia de runtime exige branch própria, PR Draft, gates completos no SHA exato e separação entre mudança estrutural e correção semântica. Não alterar `main`, não rebasear a stack #358–#360 automaticamente e não declarar `TEMPORAL_PASS` antes da integração e dos oráculos.

## 8.1 Registro de execução T0–T4 (2026-11-02)

| Etapa | Entrega | Evidência | Estado |
|---|---|---|---|
| T0 | Revisão de compatibilidade PR #396 versus microsequenciador existente | Review #5740657756; contratos não integrados sem índices M/T fiéis | REVIEWED; integração dos contratos pendente |
| T1 | Contadores checked e snapshot binário versionado do estado retomável implementado | PRs #412/#413; consolidado e revalidado no CI #41496284832 | IMPLEMENTED; SHORT_PASS na fatia |
| T2 | Coordenador SMS sobre MachineScheduler com escala explícita | PR #414; revalidado no SHA T4 | IMPLEMENTED; QUICK_PASS |
| T3 | Barramento de memória transacional com efeitos no deadline e WAIT de fixture | Stack #415/#416; revalidado no SHA T4 | IMPLEMENTED; QUICK_PASS |
| T4 | Execução vertical Z80/bus/Scheduler/memória, trace, snapshot/restore e replay | PR #416; head `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`; main `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`; CI #41499228458 | SHORT_PASS / INTEGRATED |

A fatia usa somente o subconjunto atual do microsequenciador. A escala temporal é configurada explicitamente e não deve ser interpretada como prova de clock/fase SMS NTSC/PAL. A ausência de VDP/PSG/IRQ e de oráculo físico independente mantém o sistema completo em `INTEGRATION_PENDING`.

## 9. Critérios de saída desta ADR

- [ ] Revisão da compatibilidade com ADR-0014.
- [x] Decisão explícita do mantenedor: antecipar a integração temporal por fatias incrementais, sem declarar o marco final concluído.
- [ ] Inventário de cobertura dos traces Z80 existente por família.
- [ ] Evidência primária ou captura reproduzível para relação de clocks/fases SMS.
- [ ] Modelo de transação e snapshot revisado, sem API pública prematura.
- [ ] Plano de testes com oráculos e ancestralidade documentados.
- [ ] Novo VDP temporal implementado ao menos até os contratos necessários de raster/fetch/IRQ antes da integração da CPU; VDP legado explicitamente excluído do caminho temporal.

**Status atual (2026-11-02, pós-merge):** T0–T4 executados incrementalmente; a primeira fatia vertical obteve `SHORT_PASS` no head `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`, integrada em `main` no merge SHA `3f4a71d7b7bbc1863c20cd3293466405cb3f579b` (PR #416, CI #41499228458). Isso não aprova a PR #396 nem declara `TEMPORAL_PASS`.

- T0: revisão técnica registrada na PR #396 (review #5740657756). Os contratos genéricos continuam Draft e sem merge; não são ligados artificialmente ao microsequenciador porque ele ainda não expõe índices M-cycle/T-state estáveis.
- T1: PRs #412/#413 implementam aritmética checked e snapshot binário versionado com checksum para o subconjunto retomável existente. Não representam o estado arquitetural completo do Z80.
- T2: PR #414 implementa `SmsTemporalCoordinator` sobre `MachineScheduler`, com escala de T-state explícita pelo perfil chamador.
- T3: adaptador de memória transacional, efeitos no deadline, acknowledgement único e WAIT de fixture foram incorporados à stack que culmina na PR #416.
- T4: PR #416 integra microsequenciador, scheduler, memória SMS/mapper, ciclo interno, trace runtime opcional, snapshot/restore in-memory, replay e comparação diferencial com o executor atômico para as instruções cobertas.

No SHA T4, Rust fmt/clippy/test, Python Ruff/pytest, Windows x64 vídeo/áudio e Scheduler deterministic stress (perfil short) passaram. Execução local: NOT RUN; a evidência é o CI remoto do SHA indicado. O resultado não cobre VDP, PSG, I/O, interrupções, HALT, DMA, arbitragem, WAIT físico, clock/fase regional ou codec persistente da máquina completa. CPU/bus/VDP/PSG do runtime SMS completo continuam `INTEGRATION_PENDING`; `TEMPORAL_PASS` não declarado.

## Referências

- ZiLOG, *Z80 CPU User Manual*, UM008011-0816 (08/2016), pp. 7–11, Figs. 4–7: https://www.zilog.com/docs/z80/um0080.pdf
- SMS Power, SMS Cartridge: https://www.smspower.org/Development/SMSCartridge
- SMS Power, Cartridge Slot: https://www.smspower.org/Development/CartridgeSlot
- `docs/architecture/z80-temporal-bus-model.md`
- `docs/architecture/adr/0011-cycle-accurate-temporal-model.md`
- `docs/architecture/sms-vdp-cycle-accurate-design.md` (PR #342; arquitetura-alvo do novo VDP)
- `docs/architecture/adr/0014-temporal-device-execution-and-synchronization.md`
- `docs/architecture/adr/0012-event-ordering-dimensions.md`
- `docs/architecture/adr/0013-deterministic-scheduler-snapshot-restore.md`
- `docs/roadmaps/sg1000-sms-consolidation-roadmap.md`


## 10. Estado após integração T4 em main (2026-11-02)

A PR #416 foi integrada por merge commit `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`. O SHA de head validado foi `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`; o CI de PR #41499228458 executou no merge ref `cea276159efd382e8289085be4b5f67dccecc753`, composto pela base `bf09712659a102d6e4c02f2ff5d97a99487ac166` e pelo head validado. Todos os quatro jobs foram PASS, incluindo Windows x64 vídeo/áudio e Scheduler deterministic stress short.

A auditoria encontrou e corrigiu duas invariantes antes do merge: (1) completion de barramento já efetivada permanece latched até acknowledgement, enquanto a CPU valida a continuação em cópia; (2) o ciclo interno da fatia atual aceita somente o 1 T-state exposto pelo microsequenciador, rejeitando durações divergentes sem mutação. O diferencial foi ampliado para instruções representativas do subconjunto suportado.

A implementação T4 cobre somente opcode fetch, leituras/escritas de memória, ciclos internos e WAIT de fixture. O trace real observado para INC/DEC (HL) é fetch 4, read 3, internal 1, write 3 T-states (11 no total), agrupável em (4,4,3), sem fundir o evento interno no trace. O caminho temporal não referencia o VDP scanline legado.

**Próximo incremento T5a:** adicionar índices M-cycle/T-state estáveis às operações já suportadas e mapear os tipos do microsequenciador para os contratos genéricos da PR #396. A PR #396 permanece Draft até que o mapeamento seja demonstrado sem duplicar estado de continuação. Depois disso, expandir uma família de instruções por vez com vetores diferenciais.

O CI pós-merge #41499594576 é uma execução separada: o job Scheduler stress aparece SKIPPED no evento push; isso não invalida o PASS do stress short no CI de PR #41499228458. O CI pós-merge deve ser registrado por job sem converter SKIPPED em PASS.

O estado global permanece `INTEGRATION_PENDING`; não declarar `TEMPORAL_PASS`.
