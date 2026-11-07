# ADRs normativas do produto — Rust-only / SMS temporal

Este diretório contém as decisões arquiteturais que definem a migração e o produto Rust-only. Consulte também o [índice das ADRs técnicas](../architecture/adr/README.md).

| ADR | Decisão | Estado |
|---|---|---|
| [0006 — SG-1000 Rust standalone](0006-standalone-rust-sg1000-core.md) | Registro histórico da implementação SG-1000 standalone | Histórica; não define runtime ativo inicial |
| [0007 — política Rust-first](0007-rust-first-development-policy.md) | Política de desenvolvimento Rust-first | Referência histórica; aplicar as regras mais recentes da ADR 0010 e do roadmap |
| [0008 — hardware nativo Rust](0008-rust-native-hardware-only.md) | Hardware e lógica de emulação em Rust | Vigente naquilo que não conflita com ADR 0010 |
| [0009 — aplicação Rust + egui/eframe](0009-full-rust-application-egui-eframe.md) | Aplicação desktop nativa Rust com egui/eframe | Vigente, refinada pela ADR 0010 |
| [0010 — Rust-only e SMS temporal primeiro](0010-rust-only-sms-temporal-first.md) | Produto 100% Rust, egui/eframe e SMS temporal como primeiro runtime | **Normativa principal vigente** |
| [Z80-001 — execução retomável](ADR-Z80-001-resumable-execution.md) | Contrato de execução suspensível/retomável do Z80 | Aplicar conforme o caminho temporal observável e ADR-0016 |

## Precedência

1. ADR 0010 define a arquitetura do produto e a prioridade SMS temporal.
2. ADR-0016 (`docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md`) define a política de fidelidade e granularidade temporal.
3. Roadmap mestre define fases e gates.
4. ADRs antigas preservam decisões e contexto; quando conflitarem com as decisões vigentes, não prevalecem.

## Estado operacional

Uma ADR aceita não comprova implementação. Para o estado efetivo, consultar [status atual](../project-status-2026-11-07.md), [roadmap](../project-roadmap.md) e [status dos componentes](../component-status.md).
