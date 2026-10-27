# Contrato de filtros de apresentação e sincronização de vídeo

## Fronteira determinística

O VDP produz o framebuffer original. O pipeline de apresentação recebe somente uma cópia concluída e não pode alterar registradores, VRAM, clocks, interrupções, sequência de eventos ou estado determinístico da máquina.

## Modos de filtro

- **Nativo / Nearest (implementado):** pixels sem suavização; referência de comparação digital.
- **Bilinear (implementado):** interpolação na escala de apresentação.
- **Dither Blending (implementado):** detector conservador de padrões 2×2 em xadrez. Os pares diagonais precisam ser semelhantes e ter contraste entre si; somente tiles aceitos são substituídos pela média das quatro cores. Não há desfoque global. Padrões desalinhados com a grade de 2×2 podem não ser detectados.
- **CRT / NTSC (implementado, aproximação espacial):** scanlines, máscara triádica de fósforo, curvatura e seleção RGB/Composite/RF. Composite usa um low-pass horizontal simples de crominância em espaço YIQ; RF acrescenta suavização de luminância e reduz crominância. A implementação é determinística e compartilhada pelos backends, via CPU/NumPy, sem shader específico.

**Limitações do CRT/NTSC:** não modela subportadora e fase por linha, burst de cor, comb filtering temporal, persistência de fósforo, sincronismo, ruído RF aleatório ou a cadeia elétrica real. Composite/RF são efeitos visuais aproximados, não simulação fisicamente precisa. Curvatura pode criar bordas pretas. O caminho de filtro só recebe cópias RGB concluídas e nunca altera framebuffer, VDP, clocks ou estado da máquina.

## Validação

Os testes sintéticos cobrem xadrez, bordas sólidas, cópia do framebuffer, modos de sinal, scanlines, máscara, curvatura, determinismo e rejeição de parâmetros inválidos. A equivalência visual entre APIs gráficas e o custo no Windows ainda requer execução manual nos backends reais.

## Backends e sincronização

A configuração é backend-independente; o backend efetivo precisa ser reportado separadamente da preferência. A superfície Qt Quick usa o RHI do Qt e o caminho conhecido de fallback é Qt Raster. Não declarar D3D11/D3D12 ou qualquer modo de apresentação como validado sem selecionar e testar realmente esses backends.

- **Pacing:** regula quando o frontend começa o próximo quadro; não é sinônimo de VSync.
- **VSync:** controla apresentação/swapchain; requer suporte explícito do Qt/backend.
- **VRR/G-Sync:** o driver e o monitor negociam a taxa variável. A preferência da aplicação não demonstra ativação.
- **Sincronismo A/V:** modo de baixa latência mantém a política atual. O modo pelo relógio de áudio só pode controlar apresentação quando existirem telemetria do endpoint WASAPI, underruns/overflows e timestamps de apresentação. Não alterar o scheduler ou clocks de hardware para compensar o host.

## Validação necessária

1. Testar valores padrão, enumerações inválidas, persistência e valores antigos/corrompidos.
2. Testes Qt offscreen verificam controles e restauração dos padrões.
3. Para filtros a implementar: padrões sintéticos de pixels alternados, xadrez, bordas, texto, gradientes e movimento.
4. No Windows: comparar VSync On/Off/Adaptive com timings de apresentação, tearing observado, refresh nativo e VRR no driver; medir underruns e deriva A/V por 10 minutos ou mais.
5. Distinguir sempre preferência solicitada, suporte detectado e modo efetivo. Não afirmar G-Sync ativo sem evidência do runtime/driver.
