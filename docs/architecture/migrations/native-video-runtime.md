## Objetivo
Transferir o caminho de vídeo para Rust nativo: renderer, API gráfica, recursos, filtros, sincronização e apresentação. Qt/PySide6 permanece como shell de janela, menus e configuração.

## Escopo
- Inventariar Qt Quick/QSG, QImage/QQuickImageProvider, cópias de framebuffer, filtros Python/NumPy, frame pacing, VSync/VRR e APIs gráficas usadas.
- Avaliar Vulkan, D3D11/12 e opções Rust (incluindo wgpu quando adequado) com documentação oficial, capacidades e interoperabilidade com janela Qt.
- Rust possui device, queue/context, swapchain/surface, recursos, shaders, upload, sincronização e apresentação; Qt fornece apenas janela/surface handle e eventos de UI com contrato explícito de thread/lifetime.
- Migrar filtros e transformações para Rust, sem cópias integrais por frame para Python/QImage.
- Mover execução do frame, frame pacing e coordenação A/V para runtime Rust; Python recebe estado/telemetria e comandos de UI.
- Não impor backend único sem benchmark e matriz de compatibilidade.

## Critérios de aceitação
- Nenhum framebuffer copiado para QImage/Python por frame no caminho normal.
- Renderer/backend efetivos são Rust-owned; diagnóstico informa solicitado/ativo/fallback/motivo/capacidades.
- Janela, resize, DPI, fullscreen, minimize/restore, device loss e shutdown testados.
- Filtros têm vetores determinísticos e equivalência visual documentada.
- VSync/VRR/frame pacing e sincronização A/V medidos em Windows; distinguir FPS lógico, apresentação e latência física.
- Testes unitários, integração gráfica e evidência visual em hardware real.

## Plano
1. Inventário e decisão técnica de API/interoperabilidade.
2. Criar/adaptar camada host-video Rust sem duplicar componentes.
3. Migrar filtros e recursos de framebuffer.
4. Migrar apresentação e sincronização; reduzir Qt a janela/shell.
5. Remover caminhos Python ativos e validar em Windows.

## Estado
Draft de migração. Este commit estabelece escopo e aceitação; não declara o port implementado nem validado.