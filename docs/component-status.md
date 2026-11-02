# Estado dos componentes — SG-1000 + Master System

**Base de referência:** `main` em `6c8e18b1c0a134ee56e8282d1cbc7660373100f8` (2026-11-02, após PR #418/T5a).
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


## 7. Estado formal após T4 — main `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`

| Componente | Estado formal | Evidência / limite |
|---|---|---|
| Z80 microsequencer retomável (subconjunto) | IMPLEMENTED / QUICK_PASS | Codec versionado, CRC32, WAIT/T-states checked, diferencial contra executor atômico; não é CPU completa |
| SMS temporal coordinator | IMPLEMENTED / QUICK_PASS | `MachineScheduler`, `MachineTime` e escala master-ticks/T-state explícita |
| SMS temporal bus | IMPLEMENTED / QUICK_PASS | Fetch/read/write memória, deadline, efeito único, completion latched, WAIT de fixture |
| SMS temporal vertical slice T4 | IMPLEMENTED / SHORT_PASS | PR #416, CI #41499228458; trace, snapshot/restore in-memory, replay e diferencial |
| SG-1000 legado | IMPLEMENTED / QUICK_PASS | 42 testes unitários no workspace no SHA candidato; sem golden master/homebrew smoke nesta execução |
| SMS completo (CPU/VDP/PSG/I/O/IRQ) | INTEGRATION_PENDING | VDP legado excluído; novo VDP temporal, PSG, I/O e interrupções ainda não integrados |
| TEMPORAL_PASS | NOT DECLARED | Faltam oráculos independentes/físicos, WAIT físico, arbitragem e integração sistêmica |

**T5a — PR #418 integrada:** índices M-cycle/T-state para o subconjunto retomável; restore valida fase/opcode, endereço efetivo `(HL)`, payload e progresso temporal da instrução; codec binário Z80 versão 3; `SmsTemporalMachineState` versão 6 preserva `instruction_started_at`; conclusão latched até commit/ack da CPU. HEAD da PR `6e73c0655521941c9e4ab7384f4508954306561a`, CI da PR `41538464875`: 4/4 jobs PASS. Merge em `main`: `6c8e18b1c0a134ee56e8282d1cbc7660373100f8`. CI pós-merge `41540515497`: Rust, Python e Windows PASS; Scheduler deterministic stress SKIPPED (não contar como PASS). Auditoria técnica pós-merge no SHA integrado: PENDENTE. Limites preservados; não declarar `TEMPORAL_PASS`.
## 5. T5a — PR #418 integrada

- Fonte canônica dos contratos temporais: a linha T5a (#418), que incorpora `BusTransaction`, `BusOperationResult`, os tipos de progresso e a ADR-001 originalmente propostos na PR #396. A #396 é supersedida e não deve ser integrada em paralelo.
- HEAD de código `0c178ed6c8da6aeddf8e25682a2ebc620eec786f`: run `41536695747` PASS nos quatro jobs; inclui testes de restore para endereço `(HL)`, CRC válido e `instruction_tstates` incompatível com a timeline do Scheduler. Execução local: NOT RUN.
- Restore do sequenciador valida fase/opcode/request, destino, endereço efetivo `(HL)`, duração nominal e payload esperado; `MemoryWrite` persiste endereço/valor e o codec Z80 é versão 3. `SmsTemporalMachineState` versão 6 preserva o início da instrução e valida `instruction_tstates` contra o tempo decorrido no Scheduler, inclusive completion latched. Restore do barramento valida deadline, escala de T-state e watermark de dispatch; restore da máquina exige exatamente 1 T-state para o ciclo interno. Conclusões permanecem latched até commit/ack.
- `BusTransaction.timestamp` conserva o início da requisição em Setup e Complete; `BusOperationResult::Completed.completed_at` expõe a conclusão separadamente. O descriptor de ciclo interno conserva seu timestamp de início mesmo quando a conclusão está latched.
- P2 de payload de escrita tratado na branch: a fase `MemoryWrite` preserva endereço/valor esperados, o restore compara a requisição exata e o codec versão 3 inclui esses campos. Testes negativos DTO para `LD (HL),n` e `INC (HL)`, round-trip do codec e teste de rejeição binária com CRC recalculado cobrem o payload adulterado. Também há regressão para o timestamp inicial de ciclo interno durante conclusão latched.
- Limites: apenas subconjunto atual de opcodes no SMS; WAIT é fixture; SG-1000, VDP/PSG/I/O/IRQ e semântica elétrica não estão integrados nesta fatia. `TEMPORAL_PASS` não concedido.
- A integração está concluída. A auditoria técnica independente pós-merge do SHA `6c8e18b1c0a134ee56e8282d1cbc7660373100f8` permanece pendente; o CI da PR passou 4/4 no HEAD `6e73c0655521941c9e4ab7384f4508954306561a`. O CI pós-merge teve Scheduler deterministic stress SKIPPED; não classificar esse job como PASS. Execução local: NOT RUN.
