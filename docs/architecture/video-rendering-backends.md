# Backends de apresentação de vídeo

## Estado desta implementação

O frontend Qt solicita **Vulkan pelo Qt Quick Scene Graph/RHI** para apresentar o framebuffer SG-1000. Não há chamadas Vulkan escritas manualmente nesta fatia: a criação do dispositivo, swapchain, comandos e apresentação ficam a cargo do Qt. Se a inicialização do scene graph falhar ou a API efetiva não for Vulkan, o frontend troca para o caminho raster Qt já conhecido e registra o motivo.

O VDP, o framebuffer e a sequência de execução da máquina continuam pertencendo ao emulador. O backend gráfico recebe apenas uma cópia do quadro concluído; não lê nem modifica registradores, VRAM ou clocks.

## Caminho de dados

1. A máquina conclui um quadro e expõe o framebuffer RGB de 256 × 192.
2. O frontend cria uma cópia imutável do quadro, de modo que o buffer interno possa ser reutilizado.
3. Um `QQuickImageProvider` thread-safe publica a imagem para a cena Qt Quick. A URL da imagem é versionada por quadro e o cache de imagem fica desabilitado.
4. Qt Quick usa Vulkan para compor e apresentar a textura na janela, mantendo a proporção 4:3, a escala inteira opcional e o filtro nearest/bilinear.
5. Se Vulkan não inicializar, o mesmo quadro é exibido pela implementação raster baseada em `QImage`/`QPixmap`.

O framebuffer SG-1000 é pequeno; esta etapa prioriza uma integração de apresentação correta e isolada. O caminho via `QQuickImageProvider` ainda faz upload de imagem por quadro. Não se afirma que isso seja zero-copy, nem que produza ganho de desempenho sem benchmark.

## Decisões e limites

- API solicitada: Vulkan por `QQuickWindow.setGraphicsApi(QSGRendererInterface.Vulkan)`, antes de criar a primeira janela Qt Quick.
- Integração de UI: `QQuickView` é embutida na janela QWidget com `QWidget.createWindowContainer()`; não usar `QQuickWidget` como substituto, pois a exigência é apresentar pela janela Qt Quick com backend Vulkan.
- Fallback: renderização raster Qt em caso de erro de criação da cena ou se a API efetiva reportada não for Vulkan. O fallback não altera o framebuffer ou o estado da máquina.
- Filtros: nearest desativa suavização; bilinear ativa suavização na textura. A escala inteira prefere múltiplos inteiros quando cabem na área de apresentação.
- Threading: a imagem do provider é protegida por lock e retornada como cópia. A callback de solicitação de imagem não avança hardware nem consulta a máquina.
- O backend atual não expõe D3D11/D3D12 como escolhas do usuário; essas opções não devem ser exibidas até que exista seleção e validação explícitas.

## Critérios de validação

### Automatizados

- O provider devolve snapshots independentes do buffer de origem.
- Limpar o framebuffer retorna imagem vazia ao provider.
- A UI preserva o contrato existente de `set_framebuffer`, `clear_framebuffer`, filtro nearest/bilinear e escala inteira.
- `pytest`, `ruff check` e `ruff format --check` devem passar na revisão candidata.

### Windows com GPU real

- Confirmar no log `vulkan_video_initialized` e a API efetiva Vulkan.
- Verificar que o fallback ocorre e é registrado quando Vulkan não está disponível.
- Comparar capturas do caminho raster e Vulkan com a mesma sequência de quadros; a apresentação não deve alterar os pixels de origem. Diferenças de interpolação em escalas fracionárias devem ser avaliadas separadamente.
- Medir custo de cópia/upload e tempo de apresentação em condições idênticas antes de alegar ganho de desempenho.
- Testar redimensionamento, tela cheia, alt-tab, minimização, restauração e encerramento da janela.
- Testar em máquinas com GPU dedicada e integrada; a disponibilidade de Vulkan depende do sistema, driver e runtime instalados.

## Referências oficiais

- Qt 6.8, [QQuickWindow — setGraphicsApi e erros do scene graph](https://doc.qt.io/qt-6.8/qquickwindow.html).
- Qt for Python 6.8, [QQuickView](https://doc.qt.io/qtforpython-6.8/PySide6/QtQuick/QQuickView.html).
- Qt for Python, [QQuickImageProvider](https://doc.qt.io/qtforpython-6.10/PySide6/QtQuick/QQuickImageProvider.html).
- Qt 6.8, [Qt Quick Scene Graph Renderer](https://doc.qt.io/qt-6.8/qtquick-visualcanvas-scenegraph-renderer.html).
