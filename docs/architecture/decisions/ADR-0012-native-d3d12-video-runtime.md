# ADR-0012 — Runtime de vídeo nativo em Rust com Direct3D 12

- **Status:** SUPERSEDED — substituída para a arquitetura desktop-alvo pela ADR 0011 em 2026-11-11. Mantida exclusivamente como registro histórico da decisão anterior.
- **Data:** 2026-11-02
- **Escopo:** Windows x64; caminho de vídeo do Sega Emulator, começando pela integração SMS1
- **Decisões relacionadas:** ADR-0016 (fidelidade observável e precisão temporal proporcional); proposta `native-video-runtime.md`

## Contexto

A proposta de migração de vídeo integrada pela PR #333 deixou a API gráfica em aberto e previa Qt como proprietário da janela/surface. A direção aprovada para esta etapa é mais restrita: o VDP SMS1, o Scheduler, a composição do frame e a apresentação Direct3D 12 devem executar em Rust. Python/PySide6 é somente frontend e tooling; não pode participar do caminho de pixels, do stepping da máquina, do frame pacing nem da sincronização com a GPU.

O novo `Sms1Vdp` já possui comportamento funcional por scanline, mas não é ainda o VDP integrado à máquina nos caminhos temporais de gameplay. Qualquer refinamento deve seguir ADR-0016 e a matriz de evidências SMS1, priorizando efeitos de raster/registradores que jogos realmente observem e mantendo processamento por scanline/evento quando suficiente. Um renderer de apresentação não pode definir nem corrigir comportamento de hardware.

## Decisão

1. **Backend gráfico único inicial: Direct3D 12 nativo.** A implementação Windows x64 usará D3D12/DXGI através de bindings Rust para Win32. A dependência concreta e suas features serão escolhidas e travadas no Cargo.lock durante a implementação; a versão consultada de `windows` é referência de pesquisa, não pin obrigatório.
2. **Propriedade Rust end-to-end.** Um host/runtime Rust será proprietário do HWND de apresentação, D3D12 device, direct command queue, command allocators/lists, descriptor heaps, resources, upload buffers, root signature/PSO, DXGI swap chain, fences, waitable handle e ciclo de vida gráfico.
3. **Python/PySide6 fora do caminho crítico.** Python pode iniciar/controlar o runtime por comandos coarse-grained, exibir menus, preferências e telemetria. Não recebe framebuffer por frame, não cria QImage/QPixmap por frame, não chama PyO3 por scanline/instrução e não controla o relógio da máquina. Não haverá dependência PyO3 no crate de vídeo nem no caminho SMS1→Scheduler→frame→DX12. A janela Qt de controle não executa `step_frame`; comandos de execução contínua devem ser consumidos por um host/worker nativo Rust.
4. **VDP e Scheduler são lógica de emulação, não lógica de GPU.** O VDP SMS1 gera pixels e eventos digitais determinísticos em Rust. O Scheduler é o único proprietário do tempo da máquina e ordena os dispositivos segundo contratos de hardware documentados. O D3D12 apenas transfere/apresenta um frame já produzido; shader de apresentação não implementa registradores, fetch, sprites, colisão, IRQ, H/V counters ou timing do VDP.
5. **Contrato de frame independente de backend.** O VDP/máquina publica um frame imutável com dimensões, formato de pixel explícito, pitch/stride, sequência de frame e metadados de região/orientação quando aplicáveis. O contrato inicial suporta RGB565 e RGBA8888; a conversão, se necessária, pertence ao renderer Rust. Ownership e sincronização entre produtor e consumidor são explícitos. O frame não contém referências mutáveis ao estado interno do VDP.
6. **Apresentação desacoplada do tempo emulado.** A frequência de apresentação, VSync/VRR, resize, minimize/restore e stalls da GPU nunca alteram a quantidade de ciclos/ticks executados pela máquina. A máquina avança pelo Scheduler; o host apresenta o frame mais recente concluído, respeitando uma política explícita de fila limitada e sem descartar trabalho de emulação.
7. **Frame pacing e latência mensuráveis.** Usar swap chain DXGI flip-model (`FLIP_DISCARD` ou `FLIP_SEQUENTIAL`, conforme compatibilidade demonstrada), fence para limitar frames in flight e waitable object quando suportado. O número de frames em voo será configurável/medido; não se assume que triple buffering seja automaticamente a menor latência. Não usar `Present(1, 0)` sem limitar o avanço do CPU por fence.
8. **Falha e ciclo de vida.** Resize requer liberar referências aos back buffers antes de `ResizeBuffers`; device removed/reset, perda do HWND, encerramento e restauração devem ter estados explícitos. Fallback para software ou outra API não faz parte desta decisão e exige ADR separada.
9. **SG-1000 preservado.** A migração SMS1 não altera o comportamento TMS9918A/SG-1000. Os dois sistemas podem compartilhar contrato de frame e backend de apresentação, mas mantêm seus próprios VDPs e regras de hardware.

## Arquitetura-alvo

```text
Rust machine runtime
  ├── Master Scheduler (único owner de MachineTime)
  ├── Z80 + bus + memory + mappers
  ├── SMS1 VDP
  │     ├── register/port interface
  │     ├── timing generator + fetch pipeline
  │     ├── sprite evaluation + pixel composition
  │     └── IRQ/status/VRAM contention
  └── completed immutable VideoFrame
            │
            ▼
Rust video host (Windows x64)
  ├── native HWND + message/event pump
  ├── DXGI swap chain
  ├── D3D12 device + direct queue
  ├── upload resources + presentation pipeline
  ├── fences + bounded frames-in-flight
  └── Present
            │
            ▼
         Display

Python / PySide6 frontend
  └── launch, menus, settings, commands, logs, coarse telemetry
      (no frame pixels, no emulation timing, no GPU ownership)
```

O runtime da máquina não depende de D3D12. O renderer consome somente o contrato de frame. Isso permite testes headless e mantém determinismo de emulação independente do driver, GPU, refresh rate ou janela.

## Contratos e limites de implementação

- `Sms1Vdp` não recebe tipos D3D12, HWND, DXGI ou dependências de host.
- O Scheduler não chama a GPU nem espera por fences para avançar o tempo lógico.
- O renderer não consulta ou modifica registradores/VRAM/CRAM nem decide quando o VDP busca dados.
- A transferência de frame deve ter ownership e backpressure definidos. A fila é limitada; a política de apresentação não pode retroagir nem modificar o estado da máquina.
- Save states serializam estado digital do VDP e Scheduler; handles COM, recursos GPU, fences e HWND não são serializados. Após load state, o renderer recria recursos de apresentação sem alterar o estado emulado.
- Headless rendering/frame capture deve ser possível sem criar device D3D12.
- O runtime Rust pode expor uma API de controle coarse-grained ao frontend, mas nenhum callback por instrução/scanline/framebuffer é permitido através de Python.

## Plano de execução e gates

1. **T0 — Contratos e evidências:** reconciliar esta ADR com ADR-0016, matriz SMS1 T0.1 e `native-video-runtime.md`; documentar incógnitas temporais e não inferir clocks/fases.
2. **T1 — VDP com precisão proporcional:** implementar o comportamento funcional e os eventos/estados temporais de que jogos SMS realmente dependam (por exemplo, raster, status/IRQ e efeitos intralinha demonstrados). Usar scanline/evento como granularidade quando suficiente; não exigir pipeline por dot/pixel sem evidência. Não migrar a máquina para o VDP novo antes de cobrir os requisitos de aceitação.
3. **T2 — Frame contract/headless:** publicar frame imutável e testes de golden frame/determinismo sem API gráfica.
4. **T3 — D3D12 host mínimo:** janela HWND Rust, device/queue, swap chain flip-model, upload e apresentação de padrão sintético; validar resize, DPI, minimize/restore e shutdown.
5. **T4 — Integração:** ligar a máquina SMS1 ao frame sink Rust e o sink ao host D3D12; Python somente inicia e controla.
6. **T5 — Validação:** Rust fmt, Clippy com warnings negados, testes workspace, testes Windows x64 com GPU real, replay determinístico, captura/golden frames, device loss e medições de frame pacing/latência.
7. **T6 — Remoção do legado:** somente após equivalência funcional/temporal demonstrada, regressão SG-1000 aprovada e validação Windows. Remoção do caminho Python de vídeo não precede a substituição funcional.

### Critérios de aceitação

- Nenhuma dependência Python/PyO3/Qt nos crates SMS core, Scheduler, frame contract ou D3D12 host.
- Nenhum pixel/framebuffer atravessa Python no caminho normal.
- Resultado de emulação idêntico em replay headless e com D3D12, para a mesma entrada e estado inicial.
- O tamanho dos intervalos de avanço do Scheduler não altera o estado final quando o contrato temporal exige equivalência.
- Testes de VDP usam expected values/oráculos independentes e evidências rastreáveis; o VDP legado não é oráculo.
- A janela/DXGI/GPU nunca altera MachineTime, CPU cycles, IRQ ou estado do VDP.
- CI verde no SHA candidato; testes de hardware e latência registrados separadamente dos testes headless.

## Consequências

**Positivas:** caminho de pixels nativo, ownership explícito, menor acoplamento com Python/Qt, teste headless, apresentação desacoplada do hardware emulado e controle mensurável da fila de frames.

**Custos e riscos:** D3D12 exige gestão explícita de recursos, transições, descriptors, fences e device loss; o host de janela Rust requer integração deliberada com o frontend; testes de GPU real são necessários além do CI.

**Não objetivos:** Vulkan, D3D11, wgpu, renderização do VDP em compute shader, JIT, frame skip, alteração de timing para acompanhar refresh rate e fallback gráfico automático.

## Referências técnicas

- Microsoft — [Direct3D 12 swap chains](https://learn.microsoft.com/en-us/windows/win32/direct3d12/swap-chains): flip-model, vida útil dos back buffers, ResizeBuffers e sincronização de frames in flight.
- Microsoft — [DXGI flip model performance](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model): flip-model, tearing e frame latency waitable object.
- Microsoft — [IDXGISwapChain2::GetFrameLatencyWaitableObject](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject): sincronização antes de iniciar o próximo frame.
- Rust for Windows — [windows crate](https://docs.rs/crate/windows/latest): bindings Win32/Direct3D12 em Rust.

## Estado de implementação

Esta ADR registra uma decisão arquitetural, não uma implementação. No momento da aprovação documental, o novo SMS1 VDP ainda é scanline-functional, a integração temporal com o Scheduler está pendente e não existe backend D3D12 no runtime. Nenhum estado QUICK_PASS, SHORT_PASS, LONG_PASS ou TEMPORAL_PASS é atribuído por esta ADR.
