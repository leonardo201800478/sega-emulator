# Sega Emulator — Rust / Slint + WGPU + CPAL

Emulador Sega multissistema com runtime de hardware integralmente em Rust. A arquitetura desktop de destino é **Slint 1.18.x + WGPU 30.x + CPAL 0.18.2**, com Vulkan como backend gráfico inicial preferencial no Windows x64.

**Arquitetura-alvo atual:** backend Rust determinístico; desktop Slint 1.18.x; apresentação WGPU 30.x com Vulkan inicial no Windows x64; áudio físico CPAL 0.18.2; SDL3 após auditoria. A migração está em R2, com vídeo SMS integrado ao caminho Slint/WGPU. CPAL (R3), SDL3 (R4) e consolidação final (R5) permanecem pendentes.

## Migração

- **R1:** POC isolada Slint/WGPU/Vulkan: janela, device/queue compartilhados, textura externa atualizada e composição na mesma janela.
- **R2:** frames reais SMS, preservando SmsTemporalWorker, FrameMailbox e Scheduler.
- **R3:** saída física CPAL 0.18.2.
- **R4:** auditoria e integração SDL3 para teclado/gamepads.
- **R5:** frontend Slint completo, launcher, biblioteca, configurações, HUD e telemetria; corte da baseline após paridade.

Consulte [status da migração e corte](docs/architecture/migrations/legacy-host-retirement.md), [escopo](docs/project-scope.md), [roadmap](docs/project-roadmap.md), [sprint R1](docs/sprints/2026-11-r1-slint-wgpu-vulkan-poc.md) e [ADR 0011](docs/adr/0011-slint-wgpu-cpal-desktop-platform.md).

## Arquitetura

**Runtime Rust preservado:** CPU Z80 compartilhada, memória, barramentos, mappers, VDP, PSG, interrupções, MachineScheduler, timing e estados. O Scheduler é a autoridade temporal. A GUI não executa instruções, avança clocks ou controla ciclos.

**Desktop alvo:** Slint 1.18.x para GUI; WGPU 30.x para renderização; Vulkan via WGPU no Windows x64; CPAL 0.18.2 para áudio; SDL3 para input após auditoria.

Estrutura de referência: native/sega_desktop, native/video_output_wgpu e native/audio_output. A estrutura real prevalece; não duplicar crates/componentes.

A arquitetura anterior permanece apenas como caminho transitório durante a migração controlada. Inventário, dependências remanescentes e critérios de retirada estão em [plano de corte dos hosts anteriores](docs/architecture/migrations/legacy-host-retirement.md). Não confundir documentação histórica ou código ainda necessário ao host anterior com arquitetura-alvo.

## Build e validação

Builds do aplicativo e avaliações de desempenho devem usar Release. Não usar Debug como referência de desempenho. Conferir formatação, Clippy e testes direcionados antes de enviar ao workflow.

Gates:
- cargo fmt --all -- --check
- cargo clippy --release --workspace --all-targets --locked -- -D warnings
- cargo test --release --workspace --locked

Validar workspaces standalone e targets específicos separadamente. SKIPPED é NOT RUN, nunca PASS. Todo resultado deve corresponder ao SHA exato.

Não usar JIT, frame skipping, hacks temporais ou otimizações especulativas. Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos. Projeto independente, sem afiliação à SEGA.
