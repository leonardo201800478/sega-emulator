# Proposta de migração do runtime de vídeo para Rust

## Objetivo

Avaliar e planejar a transferência progressiva do caminho de vídeo para Rust nativo, incluindo renderer, recursos gráficos, filtros, sincronização e apresentação. Qt/PySide6 permanece como shell de janela, menus, configuração e integração de eventos de interface.

Este documento é uma proposta arquitetural. **Não declara que a migração esteja implementada**, nem autoriza remover o caminho de vídeo atual antes de existir uma implementação substituta validada.

## Escopo de investigação

- Inventariar Qt Quick/QSG, QImage/QQuickImageProvider, cópias de framebuffer, filtros Python/NumPy, frame pacing, VSync/VRR e APIs gráficas efetivamente utilizadas.
- Avaliar Vulkan, D3D11/12 e opções Rust (incluindo wgpu quando adequado), considerando capacidades, compatibilidade e interoperabilidade com a janela Qt.
- Definir um contrato em que Rust seja proprietário do device, queue/context, surface/swapchain, recursos, shaders, upload, sincronização e apresentação; Qt fornece a janela/surface handle e eventos de UI, com contratos explícitos de thread e lifetime.
- Migrar filtros e transformações para Rust, evitando cópias integrais por frame para Python/QImage no caminho normal.
- Avaliar a transferência da execução de frame, frame pacing e coordenação A/V para o runtime Rust; Python recebe comandos de UI e telemetria coarse-grained.
- Não impor backend gráfico único sem benchmark reproduzível e matriz de compatibilidade.

## Critérios de aceitação para uma futura implementação

- Nenhum framebuffer é copiado para QImage/Python por frame no caminho normal, salvo fallback explicitamente documentado.
- Renderer/backend efetivos são Rust-owned; diagnóstico informa backend solicitado e ativo, fallback, motivo e capacidades.
- Janela, resize, DPI, fullscreen, minimize/restore, device loss e shutdown têm testes de integração.
- Filtros possuem vetores determinísticos e equivalência visual documentada.
- VSync/VRR/frame pacing e sincronização A/V são medidos em Windows; FPS lógico, apresentação e latência física são métricas distintas.
- Testes unitários, integração gráfica e evidência visual em hardware real são publicados para o SHA candidato.

## Plano incremental

1. Inventariar o pipeline atual e decidir API/interoperabilidade com base em evidências.
2. Criar/adaptar a camada host-video Rust, reutilizando componentes existentes e evitando uma segunda implementação paralela.
3. Migrar filtros e recursos de framebuffer com vetores determinísticos.
4. Migrar apresentação e sincronização, mantendo Qt como shell.
5. Remover caminhos Python ativos somente após equivalência funcional e validação em Windows.

## Limites e não objetivos

- Esta proposta não altera o runtime atual.
- Não declara backend Vulkan, D3D ou wgpu selecionado.
- Não declara zero-copy, frame pacing nativo, VSync/VRR ou sincronização A/V implementados.
- Não substitui a validação Windows/in-game por CI headless.
- Não autoriza JIT, frame skip ou otimizações especulativas.
- O comportamento SG-1000/SMS deve permanecer preservado durante a migração, salvo mudanças semânticas aprovadas em trabalho separado.

## Estado

**DOCUMENTAÇÃO / PROPOSTA ARQUITETURAL.** Nenhum port foi implementado ou validado por este documento.
