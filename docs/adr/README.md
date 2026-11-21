# ADRs normativas do produto — Rust-only / SMS temporal

Este diretório contém decisões arquiteturais que definem o produto e sua evolução. Consulte também o [índice das ADRs técnicas](../architecture/adr/README.md).

| ADR | Decisão | Estado |
|---|---|---|
| [0001 — apresentação Vulkan nativa](0001-native-vulkan-presentation.md) | Decisão histórica de Vulkan via ash e fallback D3D12 | **Supersedida pela ADR 0011** |
| [0006 — SG-1000 Rust standalone](0006-standalone-rust-sg1000-core.md) | Registro histórico da implementação SG-1000 standalone | Histórica; não define runtime ativo inicial |
| [0007 — política Rust-first](0007-rust-first-development-policy.md) | Política de desenvolvimento Rust-first | Referência histórica; aplicar regras mais recentes da ADR 0010 e ADR 0011 |
| [0008 — hardware nativo Rust](0008-rust-native-hardware-only.md) | Hardware e lógica de emulação em Rust | Vigente naquilo que não conflita com ADR 0010/0011 |
| [0009 — aplicação Rust + egui/eframe](0009-full-rust-application-egui-eframe.md) | Aplicação desktop nativa Rust com egui/eframe | **Supersedida na escolha de UI pela ADR 0011** |
| [0010 — Rust-only e SMS temporal primeiro](0010-rust-only-sms-temporal-first.md) | Produto Rust-only e prioridade SMS temporal | Normativa para runtime, hardware e roadmap; escolha de UI supersedida pela ADR 0011 |
| [0011 — Slint + WGPU + CPAL](0011-slint-wgpu-cpal-desktop-platform.md) | Nova plataforma desktop e adaptadores de host | **Normativa para a replataforma; produção condicionada ao POC** |
| [0012 — política anterior de panic/crash](0012-panic-crash-diagnostics-policy.md) | Hook privado consentido para crash diagnostics | **Supersedida pela ADR 0013** |
| [0013 — diagnóstico RAM-first sem crash files automáticos](0013-no-automatic-runtime-crash-files.md) | Trace essencial em RAM, consentimento para persistência/export e preservação do panic hook padrão | **Normativa; política vigente de DP0** |
| [0014 — pós-processamento retro nativo](0014-retro-shader-pipeline-librashader-wgpu.md) | Filtros de sinal em Rust e apresentação espacial em WGSL; librashader opcional | **Proposta SH-0; depende de POC e aceite** |
| [Z80-001 — execução retomável](ADR-Z80-001-resumable-execution.md) | Contrato de execução suspensível/retomável do Z80 | Aplicar conforme caminho temporal observável e ADR-0016 |

## Precedência

1. ADR 0010 define propriedade do hardware/runtime, prioridade SMS temporal, preservação SG-1000 e determinismo.
2. ADR 0011 define tecnologia desktop, API gráfica, áudio, fronteiras host e replataforma.
3. ADR-0016 define fidelidade e granularidade temporal.
4. ADR 0013 define política vigente de diagnóstico RAM-first e consentimento.
5. A proposta ADR 0014 define somente futuro pós-processamento; não altera runtime, hardware, Scheduler ou ownership temporal e não é normativa antes do aceite.
6. Roadmap mestre define fases e gates de hardware/plataforma.
7. ADRs antigas preservam histórico e não prevalecem quando conflitarem com decisões mais recentes.

Uma ADR aceita não comprova implementação. Para estado efetivo, consultar [status atual](../project-status-2026-11-07.md), [roadmap](../project-roadmap.md) e [status dos componentes](../component-status.md).
