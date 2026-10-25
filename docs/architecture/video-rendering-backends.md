# Arquitetura dos backends de renderização

## Objetivo

Planejar uma saída gráfica acelerada por GPU com Direct3D 11, Direct3D 12 e Vulkan, apresentação de baixa latência, suporte condicionado a VRR/G-SYNC, filtros básicos, proporções configuráveis e uma cadeia de shaders inspirada no formato RetroArch.

## Separação de responsabilidades

1. **Núcleo determinístico:** CPU, VDP, clocks e estado de vídeo do console.
2. **Framebuffer de origem:** pixels produzidos pelo VDP, sem depender de API gráfica.
3. **Camada de renderização:** upload da textura, escala, proporção, filtros e cadeia de shaders.
4. **Backend GPU:** recursos e comandos específicos de D3D11, D3D12 ou Vulkan.
5. **Apresentação:** swapchain, pacing, sincronização, modo de janela e diagnóstico de latência.

A taxa de atualização do monitor e o ritmo de apresentação nunca devem alterar o clock lógico do VDP. A renderização pode descartar/reapresentar quadros segundo política explícita, mas não pode pular execução de hardware emulado.

## Backends

- **Direct3D 11:** candidato inicial no Windows pela maturidade e menor complexidade relativa de integração.
- **Direct3D 12:** opção de backend com controle mais explícito sobre filas e recursos; requer gerenciamento correto de fences e quadros em voo.
- **Vulkan:** opção explícita e multiplataforma, com consulta às capacidades da superfície e aos modos de apresentação suportados.

Não assumir que um backend será sempre mais rápido. A escolha padrão será revisada após um protótipo medido no Windows; a seleção manual deve permanecer disponível quando implementada.

## Apresentação, VRR e baixa latência

- Detectar capacidades de apresentação e tearing em runtime.
- Em DXGI, usar os flags e swap effects adequados para tearing/VRR apenas quando suportados; a combinação correta depende do modo de janela e do intervalo de sincronização.
- Em Vulkan, consultar os modos de apresentação da superfície. FIFO é obrigatório; MAILBOX e IMMEDIATE podem não estar disponíveis em todas as superfícies.
- G-SYNC/VRR depende também do monitor, driver, configuração do sistema e caminho de apresentação. Mostrar capacidade detectada e estado configurado, não apenas uma caixa marcada.
- Limitar frames em voo, inicialmente a 1 quando o backend permitir, para evitar fila de renderização excessiva. Medir estabilidade e throughput.
- Separar **frame delay** (adiar a submissão/apresentação para reduzir quadros de fila) de **run-ahead** (executar previsões de frames futuros e voltar ao estado anterior). Run-ahead exige estado completo restaurável e determinismo; deve permanecer desativado até existir save-state interno confiável, testes de equivalência e tratamento de entradas/áudio.
- Não prometer latência de ponta a ponta sem medições no dispositivo real.

## Filtros e proporção

- **Nearest / pixel-art:** amostragem sem suavização, adequada para pixels nítidos.
- **Bilinear:** suavização simples; pode borrar pixels em escalas não inteiras.
- **CRT:** filtro/shader opcional; pode combinar máscara, scanlines, curvatura e resposta de cor. O preset inicial deve ser leve e os efeitos avançados serão ajustáveis.
- **Pixel-perfect:** escala inteira da imagem de origem quando houver espaço; preservar proporção da imagem e centralizar, com barras quando necessário.
- **4:3:** ajustar a imagem à proporção de apresentação 4:3.
- **16:9:** esticar ou ajustar ao viewport 16:9 conforme política escolhida; a opção de esticar pode deformar a geometria e deve ser explícita.

Para o SG-1000, a imagem do VDP é 256×192. Pixel-perfect é uma política de escala inteira; não é sinônimo de proporção de pixel histórica. Correção de aspecto e integer scaling devem ser opções separadas internamente, mesmo que a UI apresente os três modos pedidos.

## Shaders e compatibilidade RetroArch

O RetroArch documenta presets **Slang** (.slangp) e **GLSL** (.glslp), com Slang recomendado para backends modernos como Vulkan e Direct3D 10/11/12. HLSL é a linguagem nativa relevante para Direct3D, mas um shader HLSL arbitrário não equivale a um preset RetroArch nem garante compatibilidade com os metadados, passagens, parâmetros e recursos do sistema Slang.

Implementação recomendada:
1. filtros internos nearest/bilinear e um shader CRT simples, sem dependência externa;
2. cadeia de múltiplos passes e parâmetros;
3. compatibilidade de presets Slang e GLSL com declaração explícita de backend e recursos suportados;
4. suporte HLSL nativo como formato adicional para shaders próprios do backend Direct3D, sem alegar compatibilidade automática com presets RetroArch.

A licença e atribuição dos shaders externos devem ser verificadas antes de redistribuir arquivos. O carregador precisa validar formatos e limitar recursos; falhas de compilação devem gerar diagnóstico e fallback para o filtro básico.

## Janela e proporção

Oferecer janela, janela sem bordas e tela cheia. O backend deve recriar/ajustar swapchain e recursos conforme exigido ao alterar tamanho ou modo. A interface deve preservar a configuração anterior caso a transição falhe e apresentar erro explícito.

## Diagnóstico futuro e RetroAchievements

Planejar um painel de diagnóstico gráfico com backend ativo, GPU/dispositivo, modo de apresentação, resolução de saída, filtro/shader, quadros em voo, tempos CPU/GPU disponíveis, frames descartados e estado VRR reportado pelo backend. O diagnóstico não deve perturbar o caminho crítico de renderização.

A integração com RetroAchievements é uma iniciativa futura separada. Exige identificar ROMs com hashes aceitos, integração autorizada ao serviço, gestão segura de credenciais e conformidade com as regras de Hardcore. Run-ahead, save states, cheats e outras funções podem conflitar com o modo Hardcore; devem ser explicitamente desativados ou impedir a ativação desse modo conforme as regras vigentes.

## Critérios de aceitação

- A execução do VDP e o framebuffer de referência são idênticos independentemente do backend gráfico.
- Testes headless validam configurações, cálculo de viewport/proporção, escala inteira e cadeia de filtros sem GPU.
- Cada backend consulta capacidades em runtime e trata modo de apresentação não suportado.
- A troca entre janela, borderless e tela cheia não perde o dispositivo silenciosamente nem corrompe recursos.
- A CI não exige GPU; testes de integração de backend são executados separadamente no Windows e em ambientes com Vulkan compatível.
- Medições de latência reportam hardware, driver, modo de janela, VRR, modo de apresentação, quadros em voo e configurações de renderização.

## Referências

- Microsoft, [Variable refresh rate displays](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/variable-refresh-rate-displays).
- Microsoft, [Direct3D 12 swap chains](https://learn.microsoft.com/en-us/windows/win32/direct3d12/swap-chains).
- Khronos, [Vulkan presentation modes](https://github.khronos.org/Vulkan-Site/refpages/latest/refpages/source/VkPresentModeKHR.html).
- Libretro, [Shader development overview](https://docs.libretro.com/development/shader/shader-overview/).
- Libretro, [Slang shader development](https://docs.libretro.com/development/shader/slang-shaders/).
- RetroArch, [Achievements](https://www.retroarch.com/achievements.php).
