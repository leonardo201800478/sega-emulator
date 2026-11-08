# Composição da GUI nativa — egui/eframe + D3D12

## Objetivo

Manter a barra principal, os menus suspensos e os painéis de configuração acessíveis enquanto o jogo é apresentado por uma janela filha D3D12.

A GUI continua responsável exclusivamente por apresentação e interação. Nenhuma lógica de CPU, barramento, Scheduler, vídeo ou áudio é transferida para a interface.

## Hierarquia de janelas

A janela principal do eframe é a janela raiz. O backend D3D12 cria um HWND filho para apresentar o jogo. O pai usa `WS_CLIPCHILDREN`, e a janela D3D12 usa `WS_CLIPSIBLINGS`.

A barra superior permanece na janela raiz, fora do retângulo ocupado pelo HWND D3D12. Os menus e painéis são renderizados em um viewport egui nativo separado, criado por `Context::show_viewport_immediate`. Esse viewport é:

- filho da janela raiz;
- opaco, sem decoração e sem botão próprio na taskbar;
- posicionado junto ao botão que o abriu;
- limitado à área cliente disponível;
- rolável quando a área disponível é menor que o conteúdo.

Não se utiliza uma camada transparente de tela inteira. A janela do menu cobre somente a região ocupada pelo próprio painel.

## Ordem Z do D3D12

A janela de vídeo é posicionada no fim da ordem Z dos filhos com `SetWindowPos(HWND_BOTTOM, ...)` durante a atualização dos seus limites.

Isso mantém os viewports nativos egui irmãos acima da janela D3D12. `WS_CLIPSIBLINGS` impede que o desenho da janela de vídeo invada a área de irmãos que estejam acima dela.

A chamada não oculta, pausa ou recria o jogo. Ela altera somente a ordem Z e os limites da janela host.

## Posicionamento e redimensionamento

A posição do popup é calculada a partir do retângulo da janela raiz e do retângulo do botão que abriu o menu. O resultado é limitado ao client rect e preserva a faixa superior da barra de menus.

O tamanho do popup é limitado à área disponível. O conteúdo usa rolagem horizontal e vertical para que a redução da janela não torne controles inacessíveis.

O retângulo do viewport D3D12 continua sendo calculado pela GUI e aplicado ao HWND filho. Em redimensionamento, maximização e tela cheia, a posição do popup é recalculada e o HWND D3D12 continua sendo reposicionado ao fim da ordem Z.

## Foco e entrada

A leitura de teclado é feita no contexto egui do viewport que está focado. A entrada de gamepad continua sendo atualizada pelo backend existente.

Comandos que afetam a janela principal, como tela cheia e encerramento, são enviados explicitamente para `ViewportId::ROOT`; eles não devem alterar ou fechar acidentalmente o viewport do menu.

## Diagnósticos acessíveis

Diagnostics possui um acesso próprio na barra superior. O painel é um viewport nativo independente e pode ser aberto durante a execução da ROM. A evolução das métricas e da infraestrutura de logs ocorre nas entregas seguintes da sprint de observabilidade.

## Testes automatizados

- `menu_popup_is_clamped_to_root_client_bounds`: verifica os limites do posicionamento do popup.
- `menu_popup_viewport_is_opaque_undecorated_and_non_resizable`: verifica as propriedades do viewport.
- `child_viewport_stays_below_native_egui_popup_siblings`: cria HWNDs irmãos reais no Windows e verifica a ordem Z após posicionar a janela D3D12 no fundo.

O CI compila e executa os testes da aplicação egui e do backend de vídeo no Windows.

## Validação manual obrigatória

Em Windows com uma ROM homebrew carregada:

1. Abrir Arquivo, Vídeo, Som e Opções.
2. Abrir os submenus e Diagnostics.
3. Confirmar que cada painel aparece acima do jogo e permanece interativo.
4. Confirmar que o jogo continua visível fora da área do popup.
5. Testar teclado e gamepad com o popup focado e sem foco.
6. Redimensionar a janela, maximizar, restaurar e alternar F11.
7. Confirmar que o popup não cria uma janela independente na taskbar e acompanha a janela principal.

A execução automatizada não substitui a inspeção visual real em Windows/D3D12. O merge depende de CI verde no HEAD final e da validação manual acima.

## Referências

- [egui 0.37 — Context::show_viewport_immediate](https://docs.rs/egui/0.37.0/egui/struct.Context.html#method.show_viewport_immediate)
- [egui 0.37 — ViewportBuilder](https://docs.rs/egui/0.37.0/egui/viewport/struct.ViewportBuilder.html)
- [Microsoft — SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)
- [Microsoft — Window Styles (WS_CLIPCHILDREN / WS_CLIPSIBLINGS)](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-styles)
