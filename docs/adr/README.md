# ADRs normativas do produto — Rust-only / SMS temporal

Este diretório contém as decisões arquiteturais que definem o produto e sua evolução. Consulte também o [índice das ADRs técnicas](../architecture/adr/README.md).

| ADR | Decisão | Estado |
|---|---|---|
| [0001 — apresentação Vulkan nativa](0001-native-vulkan-presentation.md) | Decisão histórica de Vulkan via ash e fallback D3D12 | **Supersedida pela ADR 0011** |
| [0006 — SG-1000 Rust standalone](0006-standalone-rust-sg1000-core.md) | Registro histórico da implementação SG-1000 standalone | Histórica; não define runtime ativo inicial |
| [0007 — política Rust-first](0007-rust-first-development-policy.md) | Política de desenvolvimento Rust-first | Referência histórica; aplicar as regras mais recentes da ADR 0010 e ADR 0011 |
| [0008 — hardware nativo Rust](0008-rust-native-hardware-only.md) | Hardware e lógica de emulação em Rust | Vigente naquilo que não conflita com ADR 0010/0011 |
| [0009 — aplicação Rust + egui/eframe](0009-full-rust-application-egui-eframe.md) | Aplicação desktop nativa Rust com egui/eframe | **Supersedida na escolha de UI pela ADR 0011** |
| [0010 — Rust-only e SMS temporal primeiro](0010-rust-only-sms-temporal-first.md) | Produto Rust-only e prioridade SMS temporal | Normativa para runtime, hardware e roadmap; escolha de UI supersedida pela ADR 0011 |
| [0011 — Slint + WGPU + CPAL](0011-slint-wgpu-cpal-desktop-platform.md) | Nova plataforma desktop e adaptadores de host | **Normativa para a replataforma; produção condicionada ao POC** |
| [Z80-001 — execução retomável](ADR-Z80-001-resumable-execution.md) | Contrato de execução suspensível/retomável do Z80 | Aplicar conforme o caminho temporal observável e ADR-0016 |

## Precedência

1. ADR 0010 continua definindo propriedade do hardware/runtime, prioridade SMS temporal, preservação SG-1000 e regras de determinismo.
2. ADR 0011 define a tecnologia da aplicação desktop, API gráfica, saída de áudio, fronteiras host e sequência de replataforma.
3. ADR-0016 (`docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md`) define a política de fidelidade e granularidade temporal.
4. Roadmap mestre define fases e gates de hardware/plataforma.
5. ADRs antigas preservam decisões e contexto; quando conflitarem com decisões mais recentes, não prevalecem.

Uma ADR aceita não comprova implementação. Para o estado efetivo, consultar [status atual](../project-status-2026-11-07.md), [roadmap](../project-roadmap.md) e [status dos componentes](../component-status.md).
