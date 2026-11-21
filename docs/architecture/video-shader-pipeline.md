# Pipeline de filtros de vídeo

**Estado:** preparação documental SH-0. Nenhum filtro está integrado.
**Arquitetura proposta:** CPU Rust para sinal/efeitos temporais na resolução nativa; WGSL embarcado para efeitos espaciais na resolução de apresentação.

## 1. Caminho nativo atual

1. VDP/runtime produz VideoFrame completo e imutável.
2. FrameMailbox entrega o frame mais recente; frames pendentes podem ser substituídos por frames novos.
3. Host Slint recebe wgpu::Device e wgpu::Queue em RenderingSetup, verifica Vulkan e atualiza textura RGBA8.
4. Slint compõe a imagem na janela.

Não há filtros nativos Rust/WGSL conectados ao host atual. Não há runtime librashader integrado.

## Objetivo visual: CRT de referência reconstruído em 4K

O objetivo não é fazer upscale convencional de um jogo de baixa resolução. É usar a densidade de amostragem de 4K para reconstruir os fenômenos espaciais de um CRT de alta qualidade: máscara RGB de fósforos (shadow mask/aperture grille), perfil do feixe/spot, scanlines, espalhamento e bloom controlados, convergência e resposta temporal. O framebuffer original permanece intacto; a resolução 4K pertence apenas à etapa de reconstrução e apresentação.

A máscara RGB deve ser posicionada em coordenadas físicas do display, com alinhamento e frequência estáveis, evitando moiré e cintilação em escalas fracionárias. A energia luminosa deve ser processada em espaço linear. A paleta de origem permanece estritamente determinada pelo VDP/CRAM do SG-1000, SMS ou Game Gear. Wide gamut/HDR não deve inventar cores inexistentes no hardware.

Não generalizar todos os modos como 240p: dimensões ativas, totais e cadência devem vir do perfil de hardware e região. Manter perfis separados para RGB direto e vídeo composto.

## VRR para PAL europeu

VRR pode acompanhar a cadência próxima de 50 Hz dos jogos PAL quando a frequência estiver dentro da faixa suportada pelo display. Isso reduz tearing e irregularidade de frame pacing ao evitar a conversão forçada para uma atualização fixa de 60 Hz, podendo amenizar a sensação de movimento irregular ou arrastado. Não aumenta a velocidade do jogo nem remove a cadência original de 50 Hz. O Scheduler e o áudio permanecem vinculados ao tempo emulado; VRR altera somente a apresentação.

## ## 2. Arquitetura-alvo

    VDP / runtime (frame completo, imutável)
                     |
              fronteira de frame
                     |
        +------------+-------------+
        |                          |
        v                          v
    CPU Rust                 Host Slint + WGPU 30
    - NTSC / PAL-M                  |
    - GG LCD ghosting               v
        |                     textura RGBA8
        +-------------------------->|
                                   v
                              WGSL embarcado
                              - Nearest / Bilinear
                              - Sharp Bilinear
                              - CRT / Aperture
                              - Game Gear LCD Grid
                                   |
                                   v
                              SDR / HDR surface

O desenho é lógico: a posição exata do filtro CPU em relação ao FrameMailbox depende do requisito temporal. Filtros espaciais podem operar no frame mais recente; filtros com estado (ghosting) precisam consumir todos os frames emulados ou declarar que modelam apenas frames apresentados. O mailbox latest-only não é histórico completo.

## 3. Matriz por sistema

| Sistema/perfil | Origem lógica | Filtros propostos |
|---|---:|---|
| SG-1000 | Perfil VDP TMS9918A | RGB direto, escala, CRT; composto experimental após validar encoder |
| Master System NTSC | Perfil SMS NTSC | RGB direto, NTSC composto, CRT |
| Master System PAL-M | Perfil SMS PAL-M | RGB direto, PAL-M composto, CRT |
| Master System PAL | Perfil SMS PAL | RGB direto, PAL composto, CRT |
| Game Gear nativo | Janela 160×144 | LCD ghosting CPU, LCD grid WGSL, escala |
| Game Gear SMS-compat | Modo SMS real do hardware | Filtros SMS conforme região e modo ativo |

Não selecionar perfil exclusivamente pela extensão da ROM. Modo real da máquina e registradores determinam o perfil.

## 4. Perfis de tela cheia

Saídas alvo: 1920×1080, 2560×1440 e 3840×2160. Textura fonte mantém resolução lógica original. Shader calcula UV crop, aspect ratio e posição física dos efeitos. A interface não oferece outras resoluções de tela cheia nesta primeira política; modos de janela/redimensionamento seguem contrato separado.

## 5. Filtros CPU

### NTSC / PAL-M

Processamento em cópia do frame concluído e buffers pré-alocados. Modelo declara RGB→YUV/YIQ, modulação, fase por pixel/linha/campo, burst, separação Y/C, demodulação e reconstrução RGB.

Subportadora NTSC-M de referência: 3,579545 MHz; PAL-M: aproximadamente 3,575611 MHz. Fase não pode ser inferida apenas da razão pixel/subportadora. Burst, blanking, sincronismo e fase por linha/campo também precisam ser especificados.

PAL-M usa família 525/60 e não deve ser confundido com PAL 625/50. Alternância de fase V e linha de atraso devem seguir modelo de encoder/decoder e vetores de referência.

O filtro de três taps fornecido como protótipo não é separador Y/C de referência: [0.25, 0.5, 0.25] não anula a subportadora amostrada a 240° por pixel. Não registrar precisão ou tempo inferior a 0,15 ms sem testes e benchmarks.

### Game Gear LCD ghosting

Mistura temporal proposta usa aritmética inteira e histórico RGB na região ativa 160×144. Coeficiente é parâmetro de aparência, não especificação física. Estado deve ser atualizado em todos os frames emulados e resetado em troca de ROM/perfil/reset/restore.

Histórico não deve ser alimentado somente pelo frame apresentado quando mailbox descarta frames. Usar estágio CPU antes da coalescência ou fila limitada com semântica de perda explícita, sem backpressure no Scheduler.

## 6. GPU WGSL

Shaders próprios podem ser embarcados com include_str!. Modos propostos:

| Modo | Efeito | Sistema |
|---:|---|---|
| 0 | Nearest | Todos |
| 1 | Bilinear | Todos |
| 2 | Sharp Bilinear | Todos |
| 3 | CRT scanlines + aperture grille | SG-1000 / SMS |
| 4 | LCD subpixel grid | Game Gear |

Protótipo WGSL precisa de validação Naga e imagem antes de ser aceito. Verificar UV crop, bordas, alinhamento da máscara em pixels físicos, derivadas, smoothstep com limites crescentes, uniforms e escalas fracionárias. Medir GPU em 1080p, 1440p e 4K.

## 7. Cor e CRAM

Conversão para RGBA8 pode ser compartilhada após decodificação específica de perfil:

- SG-1000: paleta fixa TMS9918A.
- SMS: 32 entradas CRAM de 6 bits.
- Game Gear: 32 palavras de 12 bits em 64 bytes; byte par fica no latch e byte ímpar efetiva o par.
- GG SMS-compat: semântica de cor SMS, condicionada ao modo real do hardware.

Latch CRAM GG é estado do VDP e precisa de snapshot. Cache RGBA8 é derivado e atualizado somente após escrita efetiva. Esta PR documenta o contrato; não modifica VDP/CRAM.

## 8. Saída SDR/HDR

No WGPU 30, consultar Surface::get_capabilities, format_capabilities e Surface::display_hdr_info(). Configurar formato e SurfaceColorSpace em conjunto:

- SDR: Srgb.
- scRGB: Rgba16Float + ExtendedSrgbLinear, se suportado.
- HDR10: Bt2100Pq e formato compatível, se suportados.

Não inferir HDR apenas por Rgba16Float. Shader precisa escrever codificação/gamut corretos; WGPU não executa tone mapping automático para a aplicação. HDR10+ dinâmico e Dolby Vision não são declarados suportados.

## 9. VRR, pacing e áudio

Present mode é escolha de apresentação, não comando de habilitação VRR. Consultar capabilities e validar no Windows 11/Vulkan/driver/monitor reais. Fifo é fallback garantido; Mailbox pode descartar frames e Immediate pode causar tearing.

Usar deadlines absolutos com relógio monotônico para evitar deriva, mas medir temporizador, jitter e custo do spin-wait. Não prometer precisão sub-15 µs sem evidência.

Período de apresentação não pode alterar velocidade do Scheduler ou clock emulado. Servo PI que modifica frame period pode mudar relação entre tempo emulado e tempo real e não preserva automaticamente pitch. Definir autoridade de pacing e avaliar áudio como referência, resampling assíncrono ou correção limitada antes de implementar.

## 10. Critérios de validação

- Golden images e padrões sintéticos para cada filtro.
- Testes de fase/linha/campo para NTSC e PAL-M.
- Testes do latch e atualização da CRAM GG em etapa própria de VDP.
- Testes de sequência completa, reset e restore para ghosting.
- Testes WGSL/Naga e render real Vulkan.
- Benchmark Release CPU e GPU em 1080p/1440p/4K.
- Medição separada de CPU, upload, GPU, fila e present.
- Resize, alt-tab, troca de perfil, perda de frames e fallback.

## Referências

- [Arquitetura híbrida nativa de filtros](native-video-filters-hybrid-pipeline.md)
- [ADR 0014](../adr/0014-retro-shader-pipeline-librashader-wgpu.md)
- [WGPU 30 HDR surfaces](https://wgpu.rs/doc/wgpu/documentation/color/hdr_surfaces/)
- [Game Gear Hardware Reference Manual](https://segaretro.org/images/1/16/Sega_Game_Gear_Hardware_Reference_Manual.pdf)
- [TI TVP5147M1](https://www.ti.com/lit/ds/symlink/tvp5147m1.pdf)
