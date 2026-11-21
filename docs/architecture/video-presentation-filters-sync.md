# Contrato de apresentação de vídeo e filtros

**Baseline verificada:** main 94949c680e973c889580588686503c575dbc5c5b — 2026-11-21.

## Caminho de vídeo implementado

1. Runtime publica VideoFrame completo, imutável e com metadados de formato/dimensões/stride.
2. FrameMailbox mantém o frame mais recente. Frames pendentes podem ser substituídos por frames novos; isso não altera execução do Scheduler.
3. Host desktop Slint 1.18 obtém device/queue WGPU 30 e confirma backend Vulkan.
4. Host converte RGB888 em RGBA8 e atualiza textura usada pela composição Slint.
5. Slint compõe imagem com a UI.

A UI mantém “Shaders / CRT / NTSC” como **Não integrado**. Não há filtros nativos Rust/WGSL, carregador .slangp ou runtime librashader conectado ao host nesta baseline.

## Arquitetura proposta

Arquitetura híbrida: filtros de sinal e resposta temporal na CPU Rust, na resolução de origem; filtros espaciais e dependentes da resolução de saída em WGSL embarcado. Consulte [Arquitetura híbrida nativa](native-video-filters-hybrid-pipeline.md).

## Objetivo visual e resolução de reconstrução

A saída em 4K é escolhida para reconstruir melhor a estrutura RGB dos fósforos de um CRT de alta qualidade, não para alegar que o console produzia resolução 4K. O shader deve usar a densidade de pixels físicos para modelar shadow mask/aperture grille, spot do feixe, scanlines, espalhamento luminoso e convergência, preservando a resolução e a paleta nativas. A máscara é ancorada em coordenadas físicas da saída e validada contra moiré, cintilação e mudanças de escala. RGB direto e sinal composto são perfis distintos; nenhum deve ser confundido com o outro.

O pipeline não assume que todo modo dos consoles seja exatamente 240p. Dimensões ativas e cadência vêm do perfil real do VDP/região. HDR pode representar níveis luminosos e headroom de forma mais ampla, mas não amplia artificialmente a paleta original nem substitui calibração.

## VRR e jogos PAL

Para jogos europeus PAL de aproximadamente 50 Hz, VRR pode sincronizar a apresentação à cadência nativa e reduzir tearing/judder de uma tela fixa de 60 Hz, melhorando a regularidade percebida do movimento. Não acelera a execução nem elimina a diferença temporal entre 50 Hz e 60 Hz. Scheduler, áudio e clocks emulados permanecem inalterados; o VRR atua exclusivamente no host de apresentação e precisa ser validado dentro da faixa operacional do monitor.

## ## O que não deve ser considerado implementado

- NTSC/PAL-M composto por subportadora.
- Ghosting LCD do Game Gear.
- Shader WGSL CRT, aperture grille ou LCD grid.
- HDR scRGB/HDR10 ou VRR validado.
- Presets RetroArch/Slang ou carregamento .slangp.
- Perfis de tela cheia 1080p/1440p/4K ligados a filtros na UI.

## Invariantes de apresentação

- Pipeline recebe somente frames concluídos e não muta frame original.
- Nenhum filtro altera registradores, VRAM, CRAM, interrupções, clocks, barramentos, sequência de eventos ou estado do runtime.
- GUI não controla ciclos de hardware e não faz callbacks por instrução.
- Resize/recriação de recursos afeta somente apresentação.
- Falhas de shader voltam ao caminho direto, sem interromper a máquina.
- Efeitos temporais declaram se consomem todos os frames emulados ou apenas os apresentados. FrameMailbox latest-only não equivale a sequência completa.

## Próximas etapas

- [ADR 0014 — pós-processamento retro nativo](../adr/0014-retro-shader-pipeline-librashader-wgpu.md)
- [Arquitetura híbrida nativa](native-video-filters-hybrid-pipeline.md)
- [Pipeline de filtros de vídeo](video-shader-pipeline.md)
- [Catálogo de candidatos Slang](../shaders/catalog.toml)

A PR SH-0 é documental. Nenhuma dependência, asset, shader ou funcionalidade de runtime foi adicionada ao binário.
