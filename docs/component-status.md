# Estado dos componentes — SG-1000 + Master System

**Base de referência:** `main` em `3a510ae68028228065f7e60350a821ef9f76c882` (2026-11-02).
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
| Z80 — execução | Executores presentes nos cores SG-1000/SMS, com extensão em `native/z80_core` | Não há ainda um único executor compartilhado em produção. Extração e adaptadores comuns pertencem a P3 |
| Z80 — ALU | `native/z80_alu` | Biblioteca compartilhada existente; continuar auditoria de flags/opcodes e cobertura independente |
| Z80 — bus | `native/z80_bus` | Contratos/base compartilhada existentes; execução de transações ainda não é microsequenciada/interruptível |
| Memória e barramentos SG-1000 | `native/sg1000_core` | Funcionais no runtime; ampliar testes de mapa, espelhamento, RAM e cartuchos |
| Memória e barramentos SMS | `native/sms_core` | Implementados para o runtime atual; WAIT físico, arbitragem e transações temporais completas permanecem pendentes |
| TMS9918A | `native/sg1000_core` | VDP ativo do SG-1000; preservar regressões e validar efeitos de hardware com cenas/oráculos |
| SMS VDP / Mode 4 | `native/sms_core` | Caminho VDP SMS existente; não confundir renderização atual/legada com pipeline temporal por dot |
| SMS1 VDP 315-5124 | `native/sms_core/src/vdp_sms1.rs` | Componente SMS1 e estado de H Counter latch presentes; integração completa com máquina, pipeline raster/fetch, IRQ e Scheduler permanece pendente |
| PSG SN76489 | `native/sg1000_core`, `native/sms_core`, `native/audio_runtime` | Núcleo/geração digital e runtime de áudio existentes; perfis por máquina e validação de ruído/atenuação/timing ainda precisam de fechamento |
| YM2413 | Não há runtime funcional integrado identificado no baseline | Permanece como componente futuro específico do SMS, não requisito para declarar o PSG implementado |
| Mappers SMS | `native/sms_core` | Implementações existentes permanecem; cobertura e registry explícito sem fallback precisam ser auditados por perfil |
| I/O, input e IRQ | Cores de máquina e `native/input_runtime` | Caminhos atuais existem; integração temporal, transições elétricas e IRQ do VDP ainda não estão fechadas |
| Save states | Estruturas e snapshots distribuídos pelos cores | Persistência coordenada/versionada de todos os dispositivos e restore atômico ainda precisam de auditoria e integração |

## 3. Componentes de infraestrutura e aplicação

| Componente | Implementação / localização observada | Estado e trabalho restante |
|---|---|---|
| Temporal Core | `native/temporal_core` | Contratos e tipos temporais existem; contrato não significa execução temporal integrada |
| Machine Scheduler | `native/machine_scheduler` | Scheduler determinístico existe como crate; a máquina SMS ainda não o consome como autoridade temporal ativa |
| Video Runtime | `native/video_runtime` | Runtime de apresentação e backend Windows/D3D12 existem; falta ligação de frames do SMS1 VDP e sincronização com o Scheduler |
| Audio Runtime | `native/audio_runtime` | Sessão DSP/PCM Rust existe; o pump ainda depende do fluxo `run_frame()`/Qt, portanto produção/pacing autônomos permanecem pendentes |
| Input Runtime | `native/input_runtime` | Crate de runtime existe; autonomia completa do host Rust e integração final ao engine permanecem pendentes |
| Engine standalone | Não há binário/engine autônomo equivalente ao desenho proposto confirmado no workspace atual | P4: sessão, thread, comandos, pacer, host ports e executável Rust |
| Host-null / testkit de golden masters | Não identificados como implementação completa no workspace atual | P0/P1: captura repetível, comparação de framebuffer/PCM/estado e oráculos |
| PyO3/Maturin | `native/z80_core` | Extensão ativa que compõe os cores e runtimes; fronteira ainda expõe ciclo de execução por frame, não apenas controle/telemetria |
| Frontend Qt/PySide6 | `src/sega_emulator` | Aplicação ativa, carregamento, configuração, apresentação e diagnóstico; deve evoluir para launcher/painel sem avanço temporal |
| Workspace Cargo | `Cargo.toml` | Inclui SG-1000, SMS, ALU, bus, áudio, input, temporal, scheduler e vídeo; `native/z80_core`, `native/sega_emu_core` e `native/audio_output` estão excluídos do workspace root |

## 4. PRs e trabalho em andamento na fotografia

- **PR #393 — H Counter latch:** integrada à main antes da base deste inventário. O estado do latch existe no SMS1 VDP; não significa que todas as transições TH/VDP estejam conectadas ao caminho elétrico completo.
- **PR #394 — Scheduler/SMS1 VDP/D3D12:** integrada à main como consolidação documental/arquitetural. Não significa que Scheduler, SMS1 VDP e D3D12 estejam integrados em runtime.
- **PR #396 — contratos temporais SMS1:** aberta em Draft na fotografia consultada. Define contratos compartilhados, mas não implementa microsequenciador Z80, WAIT físico, arbitragem ou Scheduler integrado. Status e checks devem ser revalidados antes de qualquer integração.
- **PR #397 — consolidação documental:** branch desta atualização. A exclusão dos roadmaps antigos e a atualização do documento canônico só passam a valer na main após merge e verificação dos links.

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
