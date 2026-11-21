# ADR 0014 — Pós-processamento retro nativo com Rust e WGSL

- **Status:** PROPOSTA SH-0 — requer POC e aceite antes de implementação de produto.
- **Data:** 2026-11-21.
- **Baseline:** main 94949c680e973c889580588686503c575dbc5c5b.
- **Escopo:** filtros de apresentação de SG-1000, Master System e Game Gear no host Slint/WGPU.
- **Não escopo:** hardware, CPU, VDP, PSG, barramentos, MachineScheduler, clocks emulados ou estado da máquina.

## Contexto

O desktop utiliza Slint 1.18, WGPU 30 e backend Vulkan. O host recebe VideoFrame imutável, usa FrameMailbox, converte RGB888 para RGBA8 e atualiza a textura usada pela composição Slint.

A UI ainda identifica “Shaders / CRT / NTSC” como não integrado. Não há filtros nativos WGSL, processamento composto Rust ou histórico LCD conectado ao host atual.

## Objetivo visual: reconstrução CRT em 4K

O objetivo de qualidade visual desta arquitetura é reconstruir a aparência de um CRT de alta qualidade dos anos 1990, e não simplesmente ampliar um framebuffer de baixa resolução ou aplicar scanlines decorativas. A saída UHD (3840×2160) fornece amostragem espacial suficiente para representar com maior precisão a estrutura RGB dos fósforos (shadow mask ou aperture grille), a forma e a intensidade do feixe, a interação entre spot e fósforo, scanlines, blooming controlado, convergência e resposta temporal. A resolução 4K é espaço de reconstrução/apresentação: não aumenta a resolução, a quantidade de cores nem a informação originalmente produzida pelo VDP.

O modelo deve distinguir o sinal de origem (RGB direto ou composto) da reconstrução do display. A paleta limitada do SG-1000/SMS/GG e as regras reais de CRAM continuam sendo a fonte de cor; gamut amplo ou HDR não autorizam inventar cores. Máscara RGB deve ser calculada em coordenadas físicas estáveis da tela de saída, com fase/alinhamento explícitos, para evitar moiré, cintilação e padrões que mudem com resize. O shader deve integrar a energia luminosa em espaço linear e manter controle de headroom, sem elevar artificialmente toda a imagem.

A resolução de origem deve ser descrita por perfil de VDP/região e não generalizada como “240p” para todos os consoles e modos. O pipeline preserva a matriz ativa e os timings originais; somente a reconstrução do CRT é avaliada na resolução final 1080p/1440p/4K. Perfis RGB e composto devem permanecer distintos, pois não representam a mesma cadeia de sinal.

## VRR e cadência dos jogos PAL

O VRR é um recurso de apresentação para acompanhar a cadência real dos frames emulados dentro da faixa suportada pelo monitor, reduzindo tearing e judder decorrentes de desacordo entre produção e atualização do display. Para jogos europeus PAL com cadência nominal próxima de 50 Hz, isso pode tornar a apresentação mais estável e reduzir a sensação de irregularidade/arrasto causada por conversão inadequada para uma tela fixa de 60 Hz.

VRR não acelera a lógica do jogo, não converte 50 Hz em 60 Hz e não remove a menor cadência temporal original. O Scheduler continua executando os clocks e frames PAL originais. O host deve apresentar cada frame conforme pacing medido, sem descartar frames necessários a efeitos temporais e sem alterar áudio ou velocidade emulada. A faixa VRR do monitor, LFC, driver, compositor e modo de janela precisam ser verificados; abaixo da faixa suportada pode haver repetição de frames ou comportamento do driver, não uma garantia de suavidade.

## ## Decisão proposta

Adotar arquitetura híbrida nativa como caminho principal:

1. **CPU Rust na resolução original:** NTSC/PAL-M composto e Game Gear LCD frame blending, com buffers pré-alocados e estado temporal explícito.
2. **WGSL embarcado na GPU:** nearest, bilinear, sharp-bilinear, CRT scanlines/aperture grille e Game Gear LCD grid.
3. **Host Slint/WGPU:** administra recursos e apresenta o resultado; GUI escolhe perfis e parâmetros por comandos coarse-grained.
4. **Sem dependência obrigatória de librashader:** shaders WGSL próprios não precisam de parser .slangp ou runtime externo.
5. **librashader permanece opcional:** pode ser avaliado para compatibilidade com presets RetroArch/Slang. A documentação upstream atual classifica WGPU como suporte completo; isso corrige a classificação anterior nesta ADR. Ainda é necessário testar integração do produto e cada preset escolhido.
6. **Naga não é parser de Slang:** usá-lo para validar WGSL e entradas suportadas, não para interpretar diretamente .slangp.
7. **PAL-M é perfil próprio:** preset PAL padrão não deve ser rotulado como PAL-M.
8. **Separação de responsabilidades:** filtros ficam na camada de vídeo/apresentação, fora do VDP e Scheduler.

A arquitetura detalhada, protótipos, limitações e gates estão em [Arquitetura híbrida nativa de filtros](../architecture/native-video-filters-hybrid-pipeline.md).

## Invariantes obrigatórios

- Entrada somente de frames completos e imutáveis.
- Filtros nunca escrevem no framebuffer original nem alteram VRAM, CRAM, registradores, IRQs, clocks ou sequência de eventos.
- Nenhum callback por instrução/ciclo; fronteira é frame/apresentação.
- Filtros temporais declaram se consomem todos os frames emulados ou apenas os apresentados.
- Histórico GG não pode depender de FrameMailbox latest-only se a intenção for simular cada frame.
- Falha de shader retorna ao caminho direto/Nearest, sem parar ou atrasar runtime.
- Resize, troca de perfil e recriação GPU afetam somente recursos de apresentação.
- Nenhuma alegação de performance/latência sem benchmark reproduzível.

## Perfis de apresentação

Tela cheia terá três perfis de saída: 1920×1080, 2560×1440 e 3840×2160. Resolução de origem do VDP permanece independente. Preservar aspect ratio e recorte por perfil de console.

## Fases e gates

- **SH-0 — esta PR:** decisão arquitetural, matriz de filtros, contrato de cor, HDR/VRR, riscos e critérios. Nenhuma dependência Cargo ou código runtime.
- **SH-1 — POC WGSL:** shader embarcado no mesmo device/queue Slint, Vulkan confirmado, SDR, resize, lifecycle e fallback.
- **SH-2 — filtros espaciais GPU:** nearest, bilinear, sharp-bilinear e CRT; golden images em 1080p/1440p/4K.
- **SH-3 — filtros de sinal CPU:** NTSC e PAL-M, modelo de fase, vetores de referência e benchmark.
- **SH-4 — Game Gear LCD:** ghosting CPU, grade WGSL, histórico por frame, reset e snapshot.
- **SH-5 — HDR/VRR:** seleção explícita de color space, fallback SDR, pacing e sincronização medidos no Windows 11/Vulkan.
- **SH-6 — UI:** perfis por sistema, parâmetros, persistência e telemetria.

## HDR e VRR

WGPU 30 possui SurfaceConfiguration::color_space e capacidades de espaço de cor por formato. scRGB requer ExtendedSrgbLinear com formato compatível; HDR10/PQ exige Bt2100Pq e suporte anunciado. Rgba16Float isoladamente não prova HDR. HDR10+ dinâmico e Dolby Vision não fazem parte desta proposta.

VRR depende de driver, superfície, modo de janela, compositor e monitor. Mailbox/Immediate não são garantias de VRR e Immediate pode permitir tearing. A escolha deve ser baseada em capacidades e validação real. Pacing de vídeo não pode alterar tempo emulado nem alegar preservar pitch por simplesmente ajustar período do frame.

## Catálogo Slang

O catálogo em [catalog.toml](../shaders/catalog.toml) é pesquisa complementar. Nenhum preset, shader, LUT ou textura upstream é embarcado nesta etapa. Cada arquivo e dependência precisa de auditoria de licença antes de redistribuição.

## Referências

- [WGPU 30 HDR surfaces](https://wgpu.rs/doc/wgpu/documentation/color/hdr_surfaces/)
- [WGPU 30 SurfaceConfiguration](https://wgpu.rs/doc/wgpu/api/surface/type.SurfaceConfiguration.html)
- [librashader — Supported Render APIs](https://github.com/SnowflakePowered/librashader#supported-render-apis)
- [Windows 11 windowed-game optimizations](https://support.microsoft.com/windows/optimizations-for-windowed-games-in-windows-11)
- [Game Gear Hardware Reference Manual](https://segaretro.org/images/1/16/Sega_Game_Gear_Hardware_Reference_Manual.pdf)
- [ADR 0011 — Slint + WGPU + CPAL](0011-slint-wgpu-cpal-desktop-platform.md)
