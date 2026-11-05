# Runtime de vídeo nativo em Rust — migração Direct3D 12

## Estado e decisão

**Decisão aprovada:** [ADR-0012 — Runtime de vídeo nativo em Rust com Direct3D 12](../decisions/ADR-0012-native-d3d12-video-runtime.md).

A API gráfica inicial para Windows x64 é **Direct3D 12/DXGI**. O backend, a janela de apresentação e todo o caminho de pixels são Rust-owned. Python/PySide6 permanece somente como frontend e tooling. Esta decisão substitui a avaliação aberta de Vulkan, D3D11/12 e wgpu registrada na proposta original integrada pela PR #333.

Esta documentação não declara que o port esteja implementado. O runtime atual só deve ser substituído após implementação, testes e validação Windows/GPU nos gates definidos na ADR-0012.

## Objetivo

Transferir para Rust o caminho completo de vídeo: contrato de frame, recursos, filtros/transformações, upload, sincronização, frame pacing e apresentação Direct3D 12. O VDP SMS1 e o Scheduler também são exclusivamente Rust e não dependem do renderer.

## Fronteiras obrigatórias

- **SMS1 VDP:** lógica digital de hardware, registradores, VRAM/CRAM, fetch, sprites, composição de pixels, status e IRQ. Não conhece D3D12, DXGI, HWND ou Python.
- **Master Scheduler:** único proprietário de MachineTime e da ordenação temporal dos dispositivos. Não espera GPU/fences e não ajusta ciclos ao refresh da tela.
- **VideoFrame:** frame imutável com dimensões, formato, pitch/stride e sequência. Formatos iniciais: RGB565 e RGBA8888. Ownership e backpressure explícitos.
- **Rust video host:** proprietário do HWND, D3D12 device, direct queue, command allocators/lists, resources, descriptors, shaders/PSO, upload, DXGI swap chain, fences e apresentação.
- **Python/PySide6:** launcher, menus, configuração, comandos coarse-grained, logs e telemetria. Não recebe pixels por frame, não cria QImage/QPixmap por frame, não participa do stepping nem do frame pacing e não possui recursos gráficos do caminho principal.

O renderer apenas apresenta um frame já produzido. Nenhum shader pode implementar registradores, fetch, sprite evaluation, colisão, IRQ, H/V counters ou timing do VDP. O runtime da máquina deve continuar testável headless, sem device gráfico.

## Arquitetura-alvo

```text
Rust machine runtime
  Master Scheduler -> Z80/bus -> SMS1 VDP -> immutable VideoFrame
                                                    |
                                                    v
Rust video host (Windows x64)
  native HWND -> DXGI flip-model swap chain -> D3D12 upload/pipeline -> Present
                                                    |
                                                    v
                                                  Display

Python/PySide6 frontend: launch, menus, settings, commands, logs, telemetry
                         (no frame pixels, no emulation timing, no GPU ownership)
```

A janela de apresentação é criada/gerenciada pelo host Rust. O frontend pode iniciar o runtime e enviar comandos de interface, mas não é proprietário da superfície DXGI nem faz intermediação do framebuffer.

## Plano incremental

1. **Contratos/evidências:** reconciliar ADR-0012 com ADR-0011 e matriz SMS1; manter incógnitas temporais bloqueadas até haver fonte/oráculo.
2. **VDP temporal:** implementar TimingGenerator, fetch pipeline, sprite evaluation, pixel pipeline, status/IRQ e transações de barramento no Rust Scheduler. Não migrar a máquina antes da cobertura.
3. **Frame contract/headless:** criar frame imutável e testes determinísticos/golden sem API gráfica.
4. **D3D12 mínimo:** HWND nativo Rust, device/queue, flip-model swap chain, upload e apresentação de padrão sintético.
5. **Integração SMS1:** conectar máquina → VideoFrame → DX12 sem Python/PyO3 no caminho.
6. **Validação Windows:** resize/DPI/fullscreen windowed/minimize/restore/shutdown, device loss, fences, frame pacing, replay e regressão SG-1000.
7. **Remoção do legado:** somente após equivalência funcional/temporal demonstrada e gates aprovados.

## Requisitos D3D12

- DXGI flip-model: `FLIP_DISCARD` ou `FLIP_SEQUENTIAL`, conforme a matriz de compatibilidade.
- Fence obrigatório para limitar frames em voo e impedir CPU run-ahead/fila de apresentação sem limite.
- Avaliar `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT` e `SetMaximumFrameLatency`; medir a política escolhida, sem presumir que triple buffering minimize latência.
- Tratar transições de estado de recursos e garantir back buffer em `PRESENT` antes de Present.
- Em resize, liberar referências aos buffers antes de `ResizeBuffers`; tratar device removed/reset e recriar recursos.
- O crate `windows`/bindings e suas features devem ser escolhidos conforme o toolchain suportado e travados no Cargo.lock. Não fixar versão por este documento.
- Nenhuma dependência D3D12 deve vazar para `sms_core`, `temporal_core` ou para o contrato headless.

## Critérios de aceitação

- Zero dependência Python/PyO3/Qt nos crates do VDP, Scheduler, frame contract e DX12 host.
- Nenhum framebuffer transferido para Python/QImage por frame no caminho normal.
- Replay headless e replay com DX12 produzem o mesmo estado de máquina e os mesmos pixels para a mesma entrada.
- GPU, VSync, VRR, janela e stalls de apresentação não alteram MachineTime ou ciclos emulados.
- Testes funcionais/temporais usam expected values independentes; o VDP legado não é oráculo.
- SG-1000/TMS9918A preserva seus limites e comportamento; não herda semântica SMS1.
- CI no SHA candidato sem warnings, mais validação Windows x64 com GPU real e evidência visual/temporal.

## Não objetivos

- Vulkan, D3D11 ou wgpu como backends paralelos nesta primeira implementação.
- VDP ou Scheduler em shaders/compute.
- Python/PyO3 como transporte de pixels, clock ou eventos de hardware.
- JIT, frame skip ou otimização especulativa.
- Ajustar o clock da máquina para casar com refresh da tela.
- Remover o frontend Qt ou migrar menus/configuração para a janela de apresentação.

## Referências

- [Microsoft — Direct3D 12 swap chains](https://learn.microsoft.com/en-us/windows/win32/direct3d12/swap-chains)
- [Microsoft — DXGI flip model performance](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model)
- [Microsoft — Frame latency waitable object](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject)
- [Rust for Windows — windows crate](https://docs.rs/crate/windows/latest)

## Situação

**Estado na publicação original desta proposta:** DOCUMENTAÇÃO / DECISÃO ARQUITETURAL; o backend D3D12 ainda não estava implementado. O histórico posterior está registrado na seção Implementação publicada abaixo. O VDP SMS1 permanece scanline-functional e a integração temporal com Scheduler permanece pendente. Não atribuir QUICK_PASS, SHORT_PASS, LONG_PASS ou TEMPORAL_PASS a esta decisão documental.


## Implementação publicada — 2026-11-05

A implementação do host D3D12 avançou e foi integrada à `main` pelas PRs #546, #547, #550, #553, #555 e #558. O contrato funcional público do novo VDP SMS1 recebeu testes externos na #560. As entregas incluem fila FIFO lossless com backpressure, sessão Rust-owned em worker dedicado, retenção de frames durante minimize/restore, DPI por janela PMv2/PMv1, waits finitos com detecção de device loss e recuperação one-shot para `DXGI_ERROR_DEVICE_REMOVED` / `DXGI_ERROR_DEVICE_RESET`.

A recuperação retenta uma única vez o mesmo frame imutável; não avança nem reinicia o Scheduler. Falha de recuperação é reportada explicitamente. A recriação também substitui o HWND, portanto a preservação exata de posição ainda requer validação.

**Estado atualizado:** o host/sessão de apresentação está implementado e CI passou nos HEADs integrados. Ainda não há wiring da máquina SMS1 ativa para `VideoFrame`/D3D12. `SmsMachine` continua usando o VDP legado e `SmsTemporalMachine` não conecta VDP/PSG ao Scheduler. O VDP SMS1 permanece scanline-functional; fase, H-counter, fetch schedule, arbitragem, WAIT e IRQ temporal continuam bloqueados pela evidência T0/T0.1. GPU física, device-loss forçado, mixed-DPI real e replay headless-vs-GPU permanecem `NOT RUN`.

Registro detalhado: [auditoria de merges e validação de 2026-11-05](../../audits/video-runtime-d3d12-session-merge-audit-2026-11-05.md).
