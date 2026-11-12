# Arquitetura — Sega Emulator

**Baseline:** main `3ad1781f84b7824b5aa889a583ae6d4545987ec1` (2026-11-07).  
**Decisões normativas:** [ADR 0010 — Rust-only/SMS temporal](adr/0010-rust-only-sms-temporal-first.md) e [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## Produto

O produto é nativo Rust. A interface desktop é egui/eframe. Python, PySide6/Qt, PyO3 e Maturin não fazem parte do produto nem dos fluxos ativos de desenvolvimento.

## Camadas

| Camada | Responsabilidade |
|---|---|
| Hardware Rust | CPU Z80 compartilhada, ALU, barramento, memória, mapper, VDP, PSG e interrupções |
| Tempo Rust | MachineScheduler e primitives temporais; autoridade única de clocks/eventos |
| Runtime SMS | `rust/runtime/sms-runtime`; worker dono da máquina e comandos coarse-grained |
| Hosts desktop | Slint 1.18 + WGPU 30/Vulkan; CPAL 0.18.2 (R3); SDL3 após auditoria |
| Aplicação egui | Menus, apresentação, estado de sessão e comandos; sem lógica de hardware |

## Estado integrado

O worker SMS temporal é preservado. O desktop Slint/WGPU recebe frames reais do runtime pelo contrato de frame e não controla o tempo emulado. A saída física CPAL e o input SDL3 permanecem etapas posteriores (R3/R4). O estado de migração e os componentes transitórios estão documentados no plano de corte dos hosts anteriores.

## Regras

- Uma única implementação compartilhada do Z80.
- Uma única máquina SMS temporal ativa; nenhum fallback atômico.
- A GUI envia comandos de sessão, não instruções, ciclos ou timestamps de hardware.
- O Scheduler define causalidade e ordenação de eventos.
- Áudio/vídeo físicos não podem alterar a timeline emulada.
- A precisão temporal é proporcional ao comportamento observável, conforme ADR-0016.
- Sem JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Validação

A compilação de uma crate não comprova fidelidade funcional. São obrigatórios os gates Rust, os testes semânticos/determinísticos e a validação física Vulkan, CPAL e SDL3 quando aplicáveis. A campanha longa do Scheduler tem [evidência própria](evidence/scheduler-stress-long-2026-11-07.md), mas não prova integração temporal SMS. ZEXDOC/ZEXALL permanecem NOT RUN até o fechamento E7 e a conclusão de harness Rust.
