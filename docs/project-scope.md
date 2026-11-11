# Escopo do projeto — Sega Emulator

**Baseline auditada:** main SHA 392a5c67e85a11a5a980d8f7055bcc8a12fe3784 (2026-11-10).
**Arquitetura desktop alvo:** ADR 0011 — Slint + WGPU + CPAL.
**Runtime:** ADR 0010 e ADR-0016 permanecem vigentes.

## Objetivo

Construir um emulador Sega multissistema em Rust, modular, determinístico e temporalmente fiel. A replataforma desktop substitui gradualmente GUI e adaptadores host, preservando runtime e componentes de hardware validados.

## Arquitetura

| Camada | Tecnologia/responsabilidade |
|---|---|
| Runtime | Rust: CPU, memória, barramentos, mappers, VDP, áudio, interrupções, Scheduler, timing e estados |
| GUI alvo | Slint 1.18.x |
| Renderização | WGPU 30.x; Vulkan preferencial no Windows x64 |
| Saída física | CPAL 0.18.2 |
| Input futuro | SDL3 após auditoria |
| Baseline transitória | egui/eframe + D3D12 + WASAPI/XAudio2 até paridade e corte |

Estrutura de referência: native/sega_desktop, native/video_output_wgpu e native/audio_output. A estrutura real do repositório prevalece.

## Escopo ativo

- Preservar Z80 compartilhado, MachineScheduler, runtime SMS e SG-1000, VDPs, áudio, barramentos, memórias, mappers e testes.
- Construir POC isolada Slint/WGPU/Vulkan.
- Manter integração coarse-grained.
- Transferir frames/áudio sem conceder à GUI propriedade sobre hardware.
- Validar nova stack em Release e Windows x64.
- Manter ADRs, roadmap, sprint e evidências sincronizados.

## Fora do escopo da R1

Não integrar SmsTemporalWorker, FrameMailbox ou ROM real. Não alterar CPU, VDP, PSG, barramento, Scheduler ou timing. Não implementar menus completos, HUD, debugger, biblioteca, CPAL, SDL3, HDR ou VRR. Não remover egui/eframe nem hosts antigos. Não ampliar a POC para antecipar fases.

## Trilha de replataforma

- **R1:** janela Slint, WGPU 30, Vulkan explícito, device/queue compartilhados, textura externa animada e composição na mesma janela.
- **R2:** frames SMS reais, preservando SmsTemporalWorker, FrameMailbox e Scheduler.
- **R3:** CPAL 0.18.2, dispositivos, formatos, streams, reprodução e underruns.
- **R4:** auditar/integrar SDL3 para teclado e gamepads.
- **R5:** consolidar launcher, biblioteca, configurações, menus, HUD, logs e telemetria; remover baseline somente após paridade e validação.

Uma janela que abre não comprova device/queue compartilhados; compilação não comprova execução Vulkan; textura de teste não comprova integração SMS. UI, renderização e áudio físico não podem alterar clocks ou ordem causal do runtime.

Não usar JIT, frame skipping, hacks temporais ou otimizações especulativas. Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.
