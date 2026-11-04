# Z80 E7.2E — Auditoria de snapshots, restauração e retomada

**Data:** 2026-11-04  
**Baseline:** `main` SHA `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`  
**Branch de trabalho:** `feat/z80-e7-2e-snapshot-restore-20261104`  
**Escopo desta fatia:** auditoria estrutural do codec/restore e evidência de retomada temporal para interrupções, WAIT, EI delay e HALT/M1.  
**Estado:** IN_PROGRESS — não declara E7.2E concluída.

---

## 1. Auditoria estrutural

Arquivos examinados na baseline:

- `native/z80_cpu/src/sequencer.rs`
- `native/z80_cpu/src/snapshot.rs`
- `native/z80_cpu/src/micro_ops.rs`
- `native/z80_cpu/src/lib.rs`
- `native/sms_core/src/temporal_machine.rs`
- `native/sms_core/src/temporal_bus.rs`

### 1.1 Fases e codec

O enum `Z80MicroSequencerPhase` contém 70 variantes na baseline. A inspeção estática confirmou:

- 70 variantes contempladas por `write_phase()`;
- 70 variantes reconstruídas por `read_phase()`;
- 70 variantes contempladas na validação de `restore_state()`;
- validação separada dos índices M-cycle/T-state;
- validação cruzada entre fase, opcode, prefixo, request pendente, operação, endereço, payload e estado arquitetural, conforme aplicável.

Esta verificação comprova cobertura estrutural dos match arms, não que cada combinação válida/inválida tenha teste dinâmico.

### 1.2 Versão e tags

- `Z80_MICRO_SEQUENCER_STATE_VERSION = 31`.
- As fases IM0/IM1/IM2 já estão representadas pelas tags 65–69; NMI usa 61–63.
- Nenhuma nova representação serializada foi introduzida nesta fatia.
- **Decisão:** manter schema 31. Não alterar tags existentes nem incrementar versão sem mudança real no formato.

O codec CPU encode/decode é versionado e valida CRC, tamanho, tags e estado reconstruído. A política atual rejeita versões diferentes da versão corrente; compatibilidade de snapshots antigos não é presumida.

### 1.3 Handshake

`Z80BusHandshakeState` preserva:

- `next_request_id`;
- request pendente completo (ID, operação, endereço e duração base);
- WAIT acumulado do request.

O restore rejeita request com ID não anterior ao próximo ID, duração base zero e WAIT sem request pendente. A conclusão exige o ID correspondente e valida presença/ausência de dado conforme operação.

### 1.4 Snapshot da máquina SMS

`SmsTemporalMachineState` preserva CPU, timestamp de início da instrução, estado do barramento/memória/I/O, coordenador e fila de eventos, request/completion pendente, ciclo interno pendente, IDs de continuação e estado M1/WAIT.

`SmsTemporalMachine::restore_state()` verifica correspondência entre request CPU/barramento, IDs de ciclo interno, deadlines, WAIT acumulado, eventos agendados, estado M1 e timeline da instrução.

**Limite importante:** `SmsTemporalMachineState` é atualmente um DTO de estado em memória com `export_state()/restore_state()`; esta auditoria não encontrou um codec binário versionado para a máquina SMS inteira. O codec binário exercitado nesta fatia é o do estado CPU Z80. Não descrever isso como save-state persistente completo do console.

---

## 2. Evidência existente consultada

- `native/z80_cpu/tests/nmi_e7.rs`: round-trip binário e restore em NMI response, NMI internal e nos dois estágios de stack write, incluindo WAIT.
- `native/z80_cpu/src/sequencer.rs`, módulo E7.2C: round-trip binário durante dispatch IM, stack IM1 e leituras low/high de IM2.
- `native/z80_cpu/src/sequencer.rs`, módulo E7.2D: round-trip binário do EI delay ativo.
- `native/sms_core/src/temporal_machine.rs`: snapshots de acknowledge INT e testes de restauração de requests, ciclos internos e timeline; a cobertura de integração não percorria ainda todas as fases de interrupção com snapshot em cada safe point.

Essas evidências não substituem uma matriz de retomada para todas as 70 fases.

---

## 3. Novos testes desta fatia

Arquivo: `native/sms_core/tests/snapshot_restore_e7_2e.rs`.

O helper de teste:

1. inicia a execução pela API temporal SMS;
2. agenda cada request ou ciclo interno no Scheduler;
3. codifica/decodifica o estado CPU;
4. restaura o estado completo da máquina;
5. continua o evento;
6. compara execução contínua com execução que restaura em cada safe point.

Cenários adicionados:

- NMI response, ciclo interno e stack high/low com WAIT;
- IM0 com opcode RST fornecido no acknowledge;
- IM1 com dispatch e stack high/low;
- IM2 com stack e leitura low/high da tabela de vetores;
- EI delay preservado entre EI, instrução seguinte e INT;
- HALT M1/hold com WAIT e byte de barramento ignorado.

Os testes comparam o resultado terminal, o DTO completo da máquina, o estado da CPU, memória relevante e tempo absoluto.

---

## 4. Limites ainda abertos nesta E7.2E

- Executar CI do SHA candidato e corrigir qualquer falha.
- Completar a matriz de round-trip/restore/continuação para as demais fases BASE, CB, ED, DD/FD e DDCB/FDCB, sem inferir cobertura a partir apenas de enum/codec.
- Verificar sistematicamente estados inválidos por fase, inclusive índices M/T, payload, endereço e request ID.
- Verificar equivalência em WAIT zero, WAIT múltiplo e fronteiras de transação para as famílias restantes.
- Separar a evidência de CPU codec da evidência de restauração da máquina SMS.
- Scheduler deterministic stress permanece uma suíte independente; SKIPPED não é PASS.

Nenhum LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS é declarado. E7 global permanece INTEGRATION_PENDING.

---

## 5. Gates

Resultados desta branch devem ser preenchidos após execução do workflow:

| Suíte | Estado nesta auditoria |
|---|---|
| Rust fmt | NOT RUN localmente |
| Rust Clippy | NOT RUN localmente |
| Rust workspace tests | NOT RUN localmente |
| Python pytest | NOT RUN localmente |
| Ruff check | NOT RUN localmente |
| Ruff format | NOT RUN localmente |
| Windows x64 D3D12/video | NOT RUN |
| Windows WASAPI/XAudio2 | NOT RUN |
| Scheduler deterministic stress | NOT RUN |
| Testes E7.2E direcionados | Aguardando CI |

A branch foi criada sobre `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`; a validação oficial deve ser vinculada ao SHA final da PR.
