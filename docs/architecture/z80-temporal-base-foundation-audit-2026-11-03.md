# Auditoria de dependências do Z80 e da integração temporal SMS

**Data:** 2026-11-03  
**Base auditada:** `main` — `eba8faae3f87baed2e64dfbe96f2009474d9f212`  
**PR DD/FD em andamento:** #447 — HEAD `405944de94b7a90b921867a1ce8a827e658f08b9`  
**Tipo de auditoria:** inspeção estática do código e reconciliação com contratos/documentação existentes.  
**Limite:** não foram executados testes locais nesta auditoria; nenhum novo QUICK_PASS, SHORT_PASS, LONG_PASS ou TEMPORAL_PASS é declarado.

## 1. Conclusão executiva

O runtime SMS atualmente utilizado e a fatia temporal experimental são caminhos distintos:

- `SmsMachine::step()` em `native/sms_core/src/machine.rs` chama `self.cpu.step(&mut bus)`, executa a instrução atomicamente e depois avança o VDP pelo total agregado de T-states.
- `SmsTemporalMachine` em `native/sms_core/src/temporal_machine.rs` usa o microsequenciador e o Scheduler, mas sua própria documentação exclui VDP, PSG, portas, interrupções, DMA e WAIT físico.
- O microsequenciador `native/z80_cpu/src/sequencer.rs` declara-se uma fatia retomável pequena. Seu decoder BASE cobre um subconjunto, não o conjunto BASE completo.
- O barramento temporal SMS atual aceita somente OpcodeFetch, MemoryRead e MemoryWrite. PortRead e PortWrite são rejeitados.
- A integração temporal de ciclos internos aceita somente `MemoryIncDecInternal` com duração de exatamente um T-state.

Portanto, não é correto atribuir automaticamente os bugs do runtime SMS tradicional à integração temporal experimental: essa integração ainda não substituiu o runtime tradicional. Entretanto, a coexistência dos dois caminhos e a cobertura parcial do sequenciador são riscos arquiteturais concretos e impedem declarar o Z80 temporalmente integrado.

**Decisão:** interromper a expansão de novas famílias de instruções até concluir a auditoria e estabelecer os gates BASE. PRs de CB/ED/DD/FD já integradas não são consideradas prova de que essas famílias funcionem no caminho temporal SMS.

## 2. Evidências de código

### 2.1 Runtime SMS tradicional continua atômico

Em `native/sms_core/src/machine.rs`, método `SmsMachine::step()`:

1. constrói `SmsMachineBus`;
2. chama `self.cpu.step(&mut bus)?`;
3. obtém o total agregado de T-states;
4. avança o VDP por `self.vdp.advance(u64::from(cycles))`;
5. processa eventos PSG acumulados dentro do intervalo da instrução.

O próprio método documenta que o total retornado é nominal, não temporização de máquina ciclo a ciclo.

### 2.2 Fatia temporal experimental ainda não é a máquina SMS completa

O cabeçalho de `native/sms_core/src/temporal_machine.rs` define a implementação como primeira fatia temporal e exclui explicitamente VDP legado, PSG, portas, interrupções, DMA e WAIT físico.

A classe `SmsTemporalMachine` compõe:
- `Z80MicroSequencer`;
- `SmsTemporalBus`;
- `MachineScheduler` via `SmsTemporalCoordinator`;
- modelo explícito de fases M1;
- snapshot in-memory e trace de eventos da fatia.

Ela não é usada por `SmsMachine::step()`.

### 2.3 Cobertura BASE parcial no microsequenciador

O decoder BASE em `native/z80_cpu/src/sequencer.rs`, dentro de `complete_bus_cycle_inner()`, contém handlers para:

- NOP;
- RLCA, RRCA, RLA, RRA, DAA, CPL, SCF e CCF;
- JP (HL) e EX DE,HL;
- LD (BC),A / LD (DE),A e LD A,(BC) / LD A,(DE);
- LD rr,nn;
- LD r,n para os registradores cobertos pelo padrão implementado;
- LD (HL),n;
- ALU imediata;
- INC/DEC de registradores e (HL);
- LD r,r, incluindo leituras/escritas em (HL);
- ALU entre registrador e A, ou operando em (HL).

Prefixos CB/ED são reconhecidos como caminhos separados. DD/FD ainda não têm fallback geral na fatia da PR #447: opcode indexado não coberto pode terminar em `UnsupportedOpcode`.

Famílias BASE ausentes no dispatch atual do microsequenciador incluem, entre outras:

- EX AF,AF', EXX e EX (SP),HL;
- DI, EI, HALT e LD SP,HL;
- INC/DEC rr e ADD HL,rr;
- LD (nn),HL / LD HL,(nn), LD (nn),A / LD A,(nn);
- DJNZ, JR e JR condicional;
- JP nn e JP condicional;
- PUSH, POP, CALL, CALL condicional, RET, RET condicional e RST;
- IN A,(n) e OUT (n),A.

A lista deve ser reconciliada automaticamente com os 256 bytes BASE do executor atômico. Este inventário é evidência estática dos handlers observados, não uma contagem certificada de cobertura semântica.

### 2.4 Barramento temporal não cobre todas as transações exigidas

Em `native/sms_core/src/temporal_bus.rs`:

- `begin_request()` aceita OpcodeFetch, MemoryRead e MemoryWrite;
- PortRead e PortWrite são rejeitados como `UnsupportedOperation`;
- a conclusão de leitura/escrita ocorre no deadline agendado, com resposta latched até acknowledgement;
- WAIT é fornecido por fixture determinística; o código declara que não amostra nem representa o sinal físico WAIT do SMS.

O ciclo de memória é agendado como uma duração agregada `base_tstates + fixture_wait_tstates`. Isso não constitui, por si só, um trace de cada T-state do ciclo.

### 2.5 Ciclos internos temporalmente restritos

Em `native/sms_core/src/temporal_machine.rs`:

- `pending_internal_transaction()` expõe apenas `MemoryIncDecInternal`;
- `schedule_internal_cycle()` aceita apenas essa fase e exige `tstates == 1`;
- as demais fases internas retornadas pelo microsequenciador não podem ser conduzidas por esse caminho temporal atual.

Isso bloqueia, por exemplo, a execução temporal completa de instruções com intervalos internos de múltiplos T-states, mesmo quando a fase correspondente existe no sequenciador.

## 3. Estado do trabalho DD/FD

PR #447 permanece OPEN, base `main`, HEAD `405944de94b7a90b921867a1ce8a827e658f08b9`. O workflow CI #3938 do HEAD consultado terminou SUCCESS.

Esse resultado não altera as seguintes limitações declaradas na PR:
- diferencial completo para os oito ADD IX/IY ainda pendente;
- matriz integral de instruções DD/FD afetadas e não afetadas pendente;
- DD/FD CB e rastros de barramento pendentes;
- snapshots/replay em todas as fases e integração com a máquina SMS integral pendentes.

A PR #447 não declara FUNCTIONAL_PASS, STATE_PASS ou TEMPORAL_PASS global do Z80. Não deve ser promovida a integração temporal completa.

## 4. Ordem de implementação obrigatória

### Fase A — inventário e baseline (sem expansão funcional)

1. Gerar matriz canônica dos 256 bytes BASE, vinculando cada encoding ao handler atômico, handler microsequenciado, operações de barramento, fases internas, flags/estado afetado e evidência de teste.
2. Classificar cada caso como instrução BASE, prefixo, alias, reservado ou comportamento específico da variante-alvo.
3. Identificar todas as chamadas diretas de memória/I/O no executor atômico e todas as operações correspondentes no sequenciador.
4. Mapear para cada instrução os M-cycles, T-states, transações, commits, WAIT e safe points.
5. Registrar diferenças entre runtime tradicional, microsequenciador e máquina temporal.

### Fase B — completar o substrato temporal

Antes de ampliar famílias de instruções, o contrato CPU ↔ bus ↔ Scheduler deve suportar:

- M1, refresh e fetches de opcode/prefixo;
- leitura e escrita de memória;
- leitura e escrita de portas;
- ciclos internos de duração variável;
- acknowledge de interrupção e operações de stack decorrentes;
- WAIT associado à transação ativa, sem duplicar tempo ou efeitos colaterais;
- request ID, completion e acknowledgement exatamente uma vez;
- suspensão, retomada e restore em todos os safe points suportados;
- ordem determinística de eventos e invariância em diferentes partições de avanço.

O adaptador não deve rejeitar operações que a CPU declara suportar e tampouco simular essas operações como acessos de memória fictícios.

### Fase C — totalizar BASE funcionalmente

1. Implementar todos os handlers BASE faltantes no microsequenciador.
2. Preservar o executor atômico como oráculo interno de compatibilidade durante a migração.
3. Comparar estado arquitetural completo após cada instrução: AF/BC/DE/HL, bancos alternativos, IX/IY, SP/PC, I/R, IFF, IM, HALT, EI delay, WZ, Q e demais latches comprovados.
4. Comparar memória, portas, bytes consumidos, flags, efeitos colaterais e T-states nominais.
5. Cobrir caminhos condicionais tomados/não tomados, wraparound, limites de SP/PC e endereços 16-bit.
6. Testar suspensão e restore em cada safe point de cada família BASE.

Prefixos CB/ED/DD/FD devem ser reconhecidos e preservados como contexto, mas a conclusão desta fase não implica a implementação de suas famílias subsequentes.

### Fase D — validar BASE temporalmente

Para cada família BASE:
- trace ordenado de M1, memória, I/O, ciclos internos e commits;
- teste WAIT em cada M-cycle relevante;
- teste de não repetição de leitura/escrita após restore;
- comparação de execução contínua e particionada;
- snapshot/restore em todos os safe points;
- replay determinístico com primeira divergência identificável;
- execução integrada ao Scheduler, sem fallback atômico silencioso.

Só depois desse gate o caminho BASE pode ser considerado pronto para integrar o runtime SMS.

### Fase E — extensões em dependência

Após BASE funcional e temporalmente validado, avançar nesta ordem:

1. CB;
2. ED;
3. DD/FD;
4. DDCB/FDCB;
5. HALT, NMI, INT IM0/1/2 e interleavings completos, caso não tenham sido fechados como parte dos gates transversais BASE.

Cada família exige os mesmos critérios funcionais, de estado e temporais, não apenas testes de dispatch.

### Fase F — integração vertical SMS

Somente após o Z80 temporal completo:
- ligar o runtime SMS ao microsequenciador como caminho único;
- integrar mapper, VDP, PSG, portas, IRQ e WAIT conforme perfil de hardware e evidência;
- validar concorrência temporal, raster, replay, snapshots e regressões SG-1000/SMS.

## 5. Gates e regras de bloqueio

Estados devem ser registrados por família e por SHA:

- IMPLEMENTED;
- QUICK_PASS;
- SHORT_PASS;
- LONG_PASS;
- INTEGRATION_PENDING;
- TEMPORAL_PASS.

Gates globais:
- **FUNCTIONAL_PASS:** todas as instruções/aliases/prefixos do escopo têm semântica diferencial validada.
- **STATE_PASS:** estado arquitetural e continuations são restauráveis em todos os safe points.
- **TEMPORAL_PASS:** cronologia, M/T, WAIT, barramento, interrupções e componentes integrados têm evidência temporal independente.

Não promover uma família por compilação, dispatch smoke test ou total agregado de T-states. Teste ausente, SKIPPED ou executado em SHA anterior não é PASS.

## 6. Critério de saída desta auditoria preliminar

Esta auditoria confirma a separação dos runtimes e as lacunas explícitas no caminho temporal. Ela não substitui o inventário automatizado completo dos 256 opcodes BASE nem a execução dos testes exigidos.

Próximo incremento recomendado: implementar o inventário automatizado e o relatório de cobertura BASE a partir do executor atômico e do microsequenciador, sem alterar ainda a semântica do runtime. Esse inventário será o baseline para as PRs pequenas e sequenciais seguintes.

## 7. Arquivos-fonte examinados

- `native/sms_core/src/machine.rs`
- `native/sms_core/src/temporal_machine.rs`
- `native/sms_core/src/temporal_bus.rs`
- `native/sms_core/src/temporal.rs`
- `native/z80_cpu/src/cpu.rs`
- `native/z80_cpu/src/sequencer.rs`
- `docs/z80-completion-contract.md`
- `docs/architecture/z80-bus-cycle-scheduler-gate.md`
- `docs/architecture/z80-opcode-space-completion-audit.md`
- `docs/architecture/z80-semantic-matrix.md`
- `docs/architecture/scheduler/validation-requirements.md`
