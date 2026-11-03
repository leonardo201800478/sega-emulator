# Estado dos componentes — SG-1000 + Master System

**Base de referência:** `main` em `b2c8e6533d8ffb698e14a213b324de5c9cbab108` (2026-11-02, após PR #421/T6a).
**Natureza:** inventário documental do estado observado no repositório e das auditorias disponíveis. Não substitui testes no SHA candidato nem declara precisão ciclo a ciclo.

Este documento responde o que já existe, o que permanece como componente válido e o que ainda precisa ser integrado ou concluído. Componentes existentes não devem ser apagados só porque a arquitetura-alvo muda; a migração deve preservar APIs e testes até haver substituição comprovada.

## 1. Plataformas

| Componente | Estado observado | O que permanece / pendência |
|---|---|---|
| SG-1000 | Runtime Rust ativo em `native/sg1000_core`, integrado à extensão e ao frontend | Preservar como baseline; ampliar regressões e validação independente |
| Master System | Runtime Rust ativo em `native/sms_core`, composto pela camada nativa `native/z80_core` e fachada Python | Fechar cobertura funcional, mappers, diferenças de revisão e integração temporal |
| Game Gear | Não implementado como máquina independente | Fora do escopo ativo até M4 |
| Mega Drive/Genesis, Sega CD, 32X e sistemas integrados | Não implementados como runtimes neste marco | Fora do escopo ativo até revisão pós-M4 |

## 2. Componentes de hardware

| Componente | Implementação / localização observada | Estado e trabalho restante |
|---|---|---|
| Z80 — execução | Executores atômicos legados permanecem nos cores SG-1000/SMS; `native/z80_cpu` contém microsequenciador retomável consumido pela fatia temporal SMS | Microsequenciador ainda restrito ao subconjunto testado; executor atômico preservado como oráculo diferencial. Não é ainda o executor compartilhado completo em produção |
| Z80 — ALU | `native/z80_alu` | Biblioteca compartilhada existente; continuar auditoria de flags/opcodes e cobertura independente |
| Z80 — bus | `native/z80_bus` e adaptador `SmsTemporalBus` em `native/sms_core/src/temporal_bus.rs` | Fetch/read/write de memória são agendados por deadline; T5a expõe índices M-cycle/T-state no subconjunto retomável e mapeia transações para `BusTransaction`; conclusão de barramento e ciclo interno ficam latched até commit/acknowledgement; WAIT continua sendo fixture |
| Memória e barramentos SG-1000 | `native/sg1000_core` | Funcionais no runtime; ampliar testes de mapa, espelhamento, RAM e cartuchos |
| Memória e barramentos SMS | `native/sms_core/src/memory.rs`, `temporal_bus.rs`, `temporal_machine.rs` | Caminho temporal CPU→Scheduler→memória SMS para fetch/read/write está implementado e `SHORT_PASS`; WAIT é fixture, I/O temporal, arbitragem e integração completa permanecem pendentes |
| TMS9918A | `native/sg1000_core` | VDP ativo do SG-1000; preservar regressões e validar efeitos de hardware com cenas/oráculos |
| SMS VDP / Mode 4 | `native/sms_core/src/vdp.rs` e `vdp_sms1.rs` | Caminho legado permanece funcional e fora de `SmsTemporalMachine`; pipeline temporal por dot/fetch/IRQ ainda não integrado |
| SMS1 VDP 315-5124 | `native/sms_core/src/vdp_sms1.rs` | Componente SMS1 e estado de H Counter latch presentes; integração completa com máquina, pipeline raster/fetch, IRQ e Scheduler permanece pendente |
| PSG SN76489 | `native/sg1000_core`, `native/sms_core`, `native/audio_runtime` | Núcleo/geração digital e runtime de áudio existentes; perfis por máquina e validação de ruído/atenuação/timing ainda precisam de fechamento |
| YM2413 | Não há runtime funcional integrado identificado no baseline | Permanece como componente futuro específico do SMS, não requisito para declarar o PSG implementado |
| Mappers SMS | `native/sms_core` | Implementações existentes permanecem; cobertura e registry explícito sem fallback precisam ser auditados por perfil |
| I/O, input e IRQ | Cores de máquina e `native/input_runtime` | Caminhos atuais existem; integração temporal, transições elétricas e IRQ do VDP ainda não estão fechadas |
| Save states | `native/z80_cpu/src/snapshot.rs` e DTOs em `native/sms_core/src/temporal*.rs` | Codec binário versionado + CRC32 cobre o estado do microsequenciador suportado; estado integrado da máquina é DTO in-memory. Codec persistente e restore de todos os dispositivos seguem pendentes |

## 3. Componentes de infraestrutura e aplicação

| Componente | Implementação / localização observada | Estado e trabalho restante |
|---|---|---|
| Temporal Core | `native/temporal_core` | Contratos e tipos temporais existem; contrato não significa execução temporal integrada |
| Machine Scheduler | `native/machine_scheduler`, `SmsTemporalCoordinator` e `SmsTemporalMachine` | SMS consome o Scheduler para a fatia CPU/memória T4; não é ainda autoridade temporal de VDP/PSG/I/O/IRQ da máquina completa |
| Video Runtime | `native/video_runtime` | Runtime de apresentação e backend Windows/D3D12 existem; falta ligação de frames do SMS1 VDP e sincronização com o Scheduler |
| Audio Runtime | `native/audio_runtime` | Sessão DSP/PCM Rust existe; o pump ainda depende do fluxo `run_frame()`/Qt, portanto produção/pacing autônomos permanecem pendentes |
| Input Runtime | `native/input_runtime` | Crate de runtime existe; autonomia completa do host Rust e integração final ao engine permanecem pendentes |
| Engine standalone | Não há binário/engine autônomo equivalente ao desenho proposto confirmado no workspace atual | P4: sessão, thread, comandos, pacer, host ports e executável Rust |
| Host-null / testkit de golden masters | Não identificados como implementação completa no workspace atual | P0/P1: captura repetível, comparação de framebuffer/PCM/estado e oráculos |
| PyO3/Maturin | `native/z80_core` | Extensão ativa que compõe os cores e runtimes; fronteira ainda expõe ciclo de execução por frame, não apenas controle/telemetria |
| Frontend Qt/PySide6 | `src/sega_emulator` | Aplicação ativa, carregamento, configuração, apresentação e diagnóstico; deve evoluir para launcher/painel sem avanço temporal |
| Workspace Cargo | `Cargo.toml` | Inclui SG-1000, SMS, ALU, bus, áudio, input, temporal, scheduler e vídeo; `native/z80_core`, `native/sega_emu_core` e `native/audio_output` estão excluídos do workspace root |

## 4. PRs e integração temporal registrada em 2026-11-02

- **PR #411 — ADR-0015/roadmap T0–T4:** integrada à main em `bf09712659a102d6e4c02f2ff5d97a99487ac166`.
- **PR #416 — T1–T4 runtime SMS:** integrada à main em `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`; head validado `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`; CI #41499228458 PASS nos quatro jobs.
- **PR #412:** head é ancestral do merge #416 e foi reconhecida como merged pelo GitHub.
- **PR #413/#414:** fechadas como supersedidas; seus entregáveis funcionais de snapshot/codec e coordinator estão no diff integrado de #416, mas os heads divergentes não são ancestrais de main.
- **PR #415:** fechada como supersedida; seu head é ancestral do merge #416, portanto o histórico desse head está contido em main.
- **PR #396 — contratos temporais SMS1:** encerrada sem merge, supersedida pela PR #418. Os contratos temporais canônicos foram absorvidos pela #418; não reabrir nem duplicar essa proposta.
- **PR #421 — P6/T6a, Sega mapper no SmsTemporalBus:** integrada à main no merge SHA `b2c8e6533d8ffb698e14a213b324de5c9cbab108`; HEAD validado `9bffc3015ecb55893808519cdaf9b8aef468b976`; CI da PR `41545972767` 4/4 PASS. CI pós-merge `41546391641`: Rust/Python/Windows PASS; Scheduler stress SKIPPED no evento push. Auditoria técnica do Agente 2 limitada à issue #420 aprovada no comentário `5744939391`.
- **PR #423 — P6/T6b, barramento de memória temporal SG-1000:** aberta em Draft na branch `feat/sg1000-temporal-memory-bus`; código SHA `b4330713e329dbb44149d9589f9d5053afdd3db9`; CI `41548085870` 4/4 PASS. É candidata de branch, não parte da main; não implementa ainda `Sg1000TemporalMachine` nem liga o Z80 retomável ao SG-1000.

## 5. Componentes que permanecem no projeto

Permanecem válidos e não devem ser removidos por esta consolidação:

- cores SG-1000 e SMS e suas implementações de CPU, ALU, memória, bus, I/O e interrupções;
- VDP TMS9918A e VDP SMS, incluindo SMS1 VDP e registradores/latches já implementados;
- PSG SN76489 e perfis específicos;
- mappers e perfis de cartucho já implementados;
- bibliotecas Z80 ALU/bus e testes/vetores;
- Temporal Core e Machine Scheduler;
- Audio Runtime, Input Runtime e Video Runtime;
- extensão PyO3/Maturin e frontend Qt/PySide6;
- ADRs, especificações de hardware, auditorias, testes e evidências que descrevem comportamento real.

A consolidação remove somente documentos que funcionavam como roadmaps/cronogramas concorrentes. Não remove componentes, código de hardware, testes, contratos ou evidências técnicas.

## 6. Critérios para atualizar este inventário

Toda PR que alterar um componente deve atualizar sua linha e indicar:
1. caminho/camada de implementação;
2. estado formal (`IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING`, `TEMPORAL_PASS`);
3. testes e comandos executados;
4. SHA candidato e links de workflow;
5. limitações conhecidas e o que não foi testado.

Não elevar um estado com base apenas em documentação, compilação isolada, boot ou teste ignorado. O estado de uma PR aberta não deve ser descrito como parte da main.


## 7. Estado formal após P6/T6a — main `b2c8e6533d8ffb698e14a213b324de5c9cbab108`

| Componente | Estado formal | Evidência / limite |
|---|---|---|
| Z80 microsequencer retomável (subconjunto) | IMPLEMENTED / QUICK_PASS | Codec versionado, CRC32, WAIT/T-states checked, diferencial contra executor atômico; não é CPU completa |
| SMS temporal coordinator | IMPLEMENTED / QUICK_PASS | `MachineScheduler`, `MachineTime` e escala master-ticks/T-state explícita |
| SMS temporal bus | IMPLEMENTED / SHORT_PASS | Fetch/read/write memória; Sega mapper `$FFFD/$FFFE/$FFFF` no deadline; janela fixa 1 KiB, RAM mirror, WAIT fixture, exactly-once, snapshot/restore/replay; PR #421 |
| SMS temporal vertical slice T4 | IMPLEMENTED / SHORT_PASS | PR #416, CI #41499228458; trace, snapshot/restore in-memory, replay e diferencial |
| SG-1000 legado | IMPLEMENTED / QUICK_PASS | Runtime legado preservado; sem golden master/homebrew smoke nesta execução |
| SG-1000 temporal memory bus | INTEGRATION_PENDING | PR #423 candidata em branch, HEAD de código `b4330713e329dbb44149d9589f9d5053afdd3db9`, CI 4/4 PASS; ainda não integrada à main e sem Z80TemporalMachine |
| SMS completo (CPU/VDP/PSG/I/O/IRQ) | INTEGRATION_PENDING | VDP legado excluído; novo VDP temporal, PSG, I/O e interrupções ainda não integrados |
| TEMPORAL_PASS | NOT DECLARED | Faltam oráculos independentes/físicos, WAIT físico, arbitragem e integração sistêmica |

**T5a — PR #418 integrada:** índices M-cycle/T-state para o subconjunto retomável; restore valida fase/opcode, endereço efetivo `(HL)`, payload e progresso temporal da instrução; codec binário Z80 versão 3; `SmsTemporalMachineState` versão 6 preserva `instruction_started_at`; conclusão latched até commit/ack da CPU. HEAD da PR `6e73c0655521941c9e4ab7384f4508954306561a`, CI da PR `41538464875`: 4/4 jobs PASS. Merge em `main`: `6c8e18b1c0a134ee56e8282d1cbc7660373100f8`. CI pós-merge `41540515497`: Rust, Python e Windows PASS; Scheduler deterministic stress SKIPPED (não contar como PASS). Auditoria técnica pós-merge no SHA integrado: APROVADA TECNICAMENTE para o escopo limitado T5a, registrada no comentário 6590621514 da PR #418; não representa aprovação arquitetural da ADR-001 nem `TEMPORAL_PASS`. Limites preservados; não declarar `TEMPORAL_PASS`.
## 5. T5a — PR #418 integrada

- Fonte canônica dos contratos temporais: a linha T5a (#418), que incorpora `BusTransaction`, `BusOperationResult`, os tipos de progresso e a ADR-001 originalmente propostos na PR #396. A #396 é supersedida e não deve ser integrada em paralelo.
- Código funcional validado no SHA `0c178ed6c8da6aeddf8e25682a2ebc620eec786f`, run `41536695747` 4/4 PASS, incluindo restore de endereço `(HL)`, CRC válido e coerência `instruction_tstates`/Scheduler. HEAD final da PR `6e73c0655521941c9e4ab7384f4508954306561a`, run `41538464875` 4/4 PASS. Merge SHA `6c8e18b1c0a134ee56e8282d1cbc7660373100f8`; CI pós-merge `41540515497` com Scheduler deterministic stress SKIPPED. Execução local: NOT RUN.
- Restore do sequenciador valida fase/opcode/request, destino, endereço efetivo `(HL)`, duração nominal e payload esperado; `MemoryWrite` persiste endereço/valor e o codec Z80 é versão 3. `SmsTemporalMachineState` versão 6 preserva o início da instrução e valida `instruction_tstates` contra o tempo decorrido no Scheduler, inclusive completion latched. Restore do barramento valida deadline, escala de T-state e watermark de dispatch; restore da máquina exige exatamente 1 T-state para o ciclo interno. Conclusões permanecem latched até commit/ack.
- `BusTransaction.timestamp` conserva o início da requisição em Setup e Complete; `BusOperationResult::Completed.completed_at` expõe a conclusão separadamente. O descriptor de ciclo interno conserva seu timestamp de início mesmo quando a conclusão está latched.
- P2 de payload de escrita tratado na branch: a fase `MemoryWrite` preserva endereço/valor esperados, o restore compara a requisição exata e o codec versão 3 inclui esses campos. Testes negativos DTO para `LD (HL),n` e `INC (HL)`, round-trip do codec e teste de rejeição binária com CRC recalculado cobrem o payload adulterado. Também há regressão para o timestamp inicial de ciclo interno durante conclusão latched.
- Limites: apenas subconjunto atual de opcodes no SMS; WAIT é fixture; SG-1000, VDP/PSG/I/O/IRQ e semântica elétrica não estão integrados nesta fatia. `TEMPORAL_PASS` não concedido.
- A integração está concluída. A auditoria técnica pós-merge do Agente 2 para o SHA `6c8e18b1c0a134ee56e8282d1cbc7660373100f8` foi aprovada tecnicamente no escopo limitado de T5a (comentário 6590621514); a revisão arquitetural independente da ADR-001 permanece pendente; o CI da PR passou 4/4 no HEAD `6e73c0655521941c9e4ab7384f4508954306561a`. O CI pós-merge teve Scheduler deterministic stress SKIPPED; não classificar esse job como PASS. Execução local: NOT RUN.


## 8. Evidência P6 — T6a integrada e T6b em validação

### T6a — issue #420 / PR #421 (main)

- Merge SHA: `b2c8e6533d8ffb698e14a213b324de5c9cbab108`; HEAD validado da PR: `9bffc3015ecb55893808519cdaf9b8aef468b976`.
- CI da PR `41545972767`: Rust fmt/Clippy, SMS core focado, workspace, Ruff, pytest, Windows x64 e Scheduler deterministic stress short PASS (4/4 jobs).
- Testes SMS core focados: 171; Scheduler short: 2 PASS, 1 teste de perfil longo filtrado. Pytest: 214 PASS, 21 SKIPPED por ausência de PySide6, extensão PyO3 e NumPy; SKIPPED não é PASS.
- CI pós-merge `41546391641`: Rust/Python/Windows PASS; Scheduler deterministic stress SKIPPED no evento push. Não contar como PASS pós-merge.
- Auditoria técnica do Agente 2 limitada à issue #420: APROVADA TECNICAMENTE no comentário #5744939391 (COMMENTED, não review formal APPROVE). Sem TEMPORAL_PASS.

### T6b — issue #422 / PR #423 (branch candidata)

- Branch: `feat/sg1000-temporal-memory-bus`; código validado no SHA `b4330713e329dbb44149d9589f9d5053afdd3db9`; base `main` em `b2c8e6533d8ffb698e14a213b324de5c9cbab108`.
- CI `41548085870`: 4/4 jobs PASS. Rust fmt e Clippy PASS; `cargo test -p sega-sms-core --locked` PASS; `cargo test -p sega-sg1000-core --locked` PASS (57 testes); workspace PASS; Ruff PASS; pytest 214 PASS / 21 SKIPPED; Windows D3D12/video/WASAPI/XAudio2 PASS; Scheduler stress short 2 PASS, 1 teste longo filtrado.
- Escopo validado: transações de memória, ROM mirror, RAM interna 1 KiB/8 KiB, cartridge RAM 1/2/8 KiB, deadline, WAIT fixture, exactly-once, snapshot/restore e rejeição explícita de PortRead/PortWrite.
- Limite: PR #423 continua aberta; o barramento ainda não compõe o Z80MicroSequencer. Não contar como implementação da main até merge. Execução local: NOT RUN.


## 10. P7 — expansão do Z80 retomável: ALU com fonte em registrador, PR #425

**Branch candidata:** `feat/z80-sms-alu-register-matrix`  
**SHA validado:** `6ed6cc3b42942f791d04f4366db2b35e3cafa985`  
**PR:** https://github.com/leonardo201800478/sega-emulator/pull/425  
**CI:** run `41562585658` — Rust, Python e Windows PASS; Scheduler deterministic stress SKIPPED.

| Componente | Estado formal | Evidência / limite |
|---|---|---|
| ALU BASE com fonte em registrador no microsequenciador | IMPLEMENTED / QUICK_PASS | 56 encodings: ADD/ADC/SUB/SBC/AND/XOR/OR/CP com B,C,D,E,H,L,A; vetores fixos de resultado/flags, teste estrutural e diferencial no SmsTemporalMachine/Scheduler |
| ALU com fonte `(HL)` | INTEGRATION_PENDING | Requer fase de leitura de memória e validação do snapshot codec |
| ALU imediata | INTEGRATION_PENDING | Requer fase de leitura do operando imediato, avanço de PC e validação temporal |
| Z80 retomável completo | INTEGRATION_PENDING | Prefixos CB/ED/DD/FD, HALT, interrupções, I/O, instruções 16-bit, refresh elétrico e estado arquitetural completo permanecem pendentes |
| TEMPORAL_PASS | NOT DECLARED | A cobertura desta PR é uma fatia CPU isolada; não prova execução completa de jogos nem integração temporal sistêmica |

A implementação reutiliza a ALU compartilhada `sega-z80-alu` e preserva o executor atômico como referência. O teste CP confirma preservação de A e flags incluindo X/Y. Não se declara cobertura de operandos imediatos/memória, comparação diferencial abrangente, ZEXDOC/ZEXALL ou boot de jogos. A PR #425 permanece Draft e não faz parte da main.

## 11. P7 — Z80 ALU com operando imediato e indireto (HL), PR #426

**Branch candidata:** feat/z80-sms-alu-immediate-memory  
**SHA de código validado:** 35372cdd6705cbae57992fbc770b03982b4f10eb  
**PR:** https://github.com/leonardo201800478/sega-emulator/pull/426  
**Base:** feat/z80-sms-alu-register-matrix (PR #425; empilhada, não integrada à main).  
**CI:** run 41564679857 falhou somente no step Formatting; Windows x64 e Python PASS; Scheduler deterministic stress SKIPPED. A correção de formatação foi aplicada no SHA candidato atual, mas a execução CI desse SHA ainda não foi localizada/confirmada.

| Componente | Estado formal | Evidência / limite |
|---|---|---|
| ALU imediata A,n | IMPLEMENTED / QUICK_PASS | 8 opcodes C6/CE/D6/DE/E6/EE/F6/FE; leitura de operando agendada, PC avança na conclusão; CI PASS |
| ALU com fonte (HL) | IMPLEMENTED / QUICK_PASS | 8 opcodes 86/8E/96/9E/A6/AE/B6/BE; leitura de memória agendada; CI PASS |
| Continuação retomável e snapshot | IMPLEMENTED / QUICK_PASS | Fases ImmediateReadAlu/MemoryReadAlu, codec com tags próprias e testes de round-trip pendente; CI PASS |
| SMS temporal state | IMPLEMENTED / QUICK_PASS | Versão do estado SMS temporal incrementada; comparação diferencial via Scheduler PASS |
| WAIT e timing nominal | IMPLEMENTED / QUICK_PASS | Testes de 7 T nominais e WAIT PASS |
| Z80 retomável completo | INTEGRATION_PENDING | Prefixos CB/ED/DD/FD, HALT, interrupções, I/O, instruções 16-bit, refresh elétrico e estado arquitetural completo permanecem pendentes |
| TEMPORAL_PASS | NOT DECLARED | Sem CI verde e sem execução in-game; não inferir fidelidade sistêmica total |

**Critérios implementados no código:** M1 4 T + leitura 3 T; leitura imediata usa PC e incrementa PC somente ao concluir; (HL) solicita o byte pelo barramento temporal; WAIT entra no contador da instrução; snapshot preserva fase/request/WAIT. A suíte diferencial compara os oito grupos de operação com o executor atômico em registradores, refresh e tempo do Scheduler.

**Validação:** no SHA de código 35372cdd6705cbae57992fbc770b03982b4f10eb, cargo fmt, Clippy, testes direcionados SMS/SG-1000 e testes do workspace: PASS; Ruff formatação/lint e regressão Python: PASS; Windows x64 D3D12, testes de vídeo, WASAPI/XAudio2 e testes de áudio: PASS. Scheduler deterministic stress: SKIPPED. Execução local NOT RUN. A fatia recebe QUICK_PASS de validação automatizada; manter INTEGRATION_PENDING quanto à integração sistêmica/in-game.

A PR #426 permanece Draft e empilhada sobre #425. Nenhum merge foi realizado.


## 12. P7 — Q-latch e SCF/CCF no Z80 retomável, PR #427

**Branch candidata:** `feat/z80-sms-accumulator-flags`  
**SHA candidato validado:** `cb040c50bfec9867a0c97068ad4de4bd9e866de9`  
**PR:** https://github.com/leonardo201800478/sega-emulator/pull/427  
**CI:** run `41569544969` — Rust, Python e Windows x64 PASS; Scheduler deterministic stress SKIPPED.

| Componente | Estado formal | Evidência / limite |
|---|---|---|
| RLCA/RRCA/RLA/RRA, DAA e CPL | IMPLEMENTED / QUICK_PASS | Shared ALU, M1 4 T, testes unitários e diferencial Scheduler/atomic; CI run `41568918251` PASS em Rust, Python e Windows x64 |
| Q-latch do microsequenciador | IMPLEMENTED / QUICK_PASS | `q` e `q_previous` atualizados na fronteira de instrução; instruções que escrevem F atualizam Q; instruções sem escrita de F deixam Q=0 |
| SCF/CCF | IMPLEMENTED / QUICK_PASS pendente de CI atual | Fórmula X/Y usa `(A | (Q_previous XOR F)) & 0x28`; CCF alterna C/H; teste após CP e instrução sem escrita de flags |
| Snapshot Z80 | IMPLEMENTED / QUICK_PASS pendente de CI atual | Codec binário versão 5 serializa Q/Q_previous; teste de round-trip/restauração com M1 pendente; CI `41569544969` PASS |
| SMS temporal machine state | IMPLEMENTED / QUICK_PASS pendente de CI atual | Versão incrementada de 7 para 8 para refletir o novo estado embutido do Z80; CI `41569544969` PASS |
| Z80 retomável completo | INTEGRATION_PENDING | Prefixos CB/ED/DD/FD, HALT, interrupções, I/O, instruções 16-bit, refresh elétrico e estado arquitetural completo permanecem pendentes |
| TEMPORAL_PASS | NOT DECLARED | Sem oráculos independentes/físicos, WAIT físico e integração sistêmica completa |

O executor atômico existente é a referência para SCF/CCF: no início de cada instrução ele captura `q_previous = q` e limpa `q`; `set_flags` grava F e Q. O microsequenciador reproduz essa fronteira e persiste ambos os latches. CI `41569544969`: Rust Formatting, Clippy, testes direcionados SMS/SG-1000, workspace, Ruff, pytest e Windows x64 vídeo/áudio PASS; Scheduler deterministic stress SKIPPED. Execução local NOT RUN. Não inferir validação in-game nem cobertura ZEXDOC/ZEXALL.

## 13. P7 — Z80 BASE: LD rr,nn no microsequenciador retomável

**Branch candidata:** `feat/z80-sms-ld-register-pairs-immediate`  
**SHA de código candidato:** `f416efef960cdd20816fd46c5e4a8509bff36ee5`  
**PR:** pendente de abertura  
**Dependência:** PR #427 (empilhada; ainda não integrada à main).  
**CI:** primeira execução `41570758837` falhou apenas em `cargo fmt --all -- --check`; a primeira correção de formatação foi incompleta; nova correção aplicada no SHA de código `917eb97b3c5ec0dc0ada79d8c590c422a538197c`; CI desse SHA ainda pendente; o run `41571022447` confirmou Formatting PASS, mas Clippy encontrou ausência dos dois novos arms no encoder `write_phase`; corrigido em `f416efef960cdd20816fd46c5e4a8509bff36ee5`.

| Componente | Estado formal | Evidência / limite |
|---|---|---|
| LD BC,nn / LD DE,nn / LD HL,nn / LD SP,nn | IMPLEMENTED / QUICK_PASS pendente de CI | Quatro opcodes BASE (01/11/21/31); leitura little-endian em dois M-cycles de memória agendados após M1 |
| Suspensão/retomada e snapshot | IMPLEMENTED / QUICK_PASS pendente de CI | Fases distintas para byte baixo e alto; validação de opcode, par, endereço, M-cycle e duração; codec Z80 versão 6 |
| Integração temporal SMS | IMPLEMENTED / QUICK_PASS pendente de CI | Testes diferenciais planejados no SmsTemporalMachine/Scheduler contra o executor atômico; estado temporal SMS versão 9 |
| Z80 retomável completo | INTEGRATION_PENDING | Cobertura ainda parcial; instruções BASE restantes, prefixos CB/ED/DD/FD, HALT, interrupções, I/O e estados/pinos completos pendentes |
| TEMPORAL_PASS | NOT DECLARED | Sem comprovação elétrica independente ou integração sistêmica completa |

**Critérios da fatia:** M1 4 T + leitura do byte baixo 3 T + leitura do byte alto 3 T = 10 T nominais. PC avança apenas quando cada leitura conclui; o par de registradores só é atualizado após a leitura do byte alto. WAIT de cada transação deve ser contabilizado no Scheduler. O executor atômico legado permanece como oráculo diferencial e não é removido.

**Validação:** CI NOT RUN nesta atualização; execução local NOT RUN. Atualizar SHA e estados somente após workflow verde. Scheduler deterministic stress deve ser reportado separadamente; SKIPPED não é PASS.
