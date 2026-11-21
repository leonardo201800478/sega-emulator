# Arquitetura híbrida nativa de filtros de vídeo

**Estado:** proposta técnica SH-0; documentação apenas. Nenhum filtro ou shader é declarado implementado.
**Baseline:** main 94949c680e973c889580588686503c575dbc5c5b (2026-11-21).
**Escopo:** SG-1000, Master System e Game Gear; tela cheia em 1080p, 1440p e 4K.

## 1. Decisão arquitetural proposta

Adotar arquitetura híbrida com filtros nativos próprios como caminho padrão, sem dependência obrigatória de runtimes de shaders de terceiros:

1. **CPU / Rust nativo, na resolução de origem:** filtros que modelam sinal ou resposta temporal de baixa resolução, como NTSC/PAL-M composto e persistência LCD do Game Gear.
2. **GPU / WGSL embarcado:** filtros espaciais dependentes da resolução física de apresentação, como nearest, bilinear, sharp-bilinear, scanlines CRT, máscara de fósforo e grade de subpixels LCD.
3. **Host Slint/WGPU/Vulkan:** seleciona perfil e parâmetros, prepara recursos e apresenta a textura final. A GUI não executa hardware nem controla ciclos do VDP ou do Scheduler.

Os módulos ficam na camada de vídeo/apresentação, fora dos runtimes de hardware. O filtro recebe frame concluído e não altera VRAM, CRAM, registradores, IRQs, clocks, barramentos ou estado do console.

librashader permanece como alternativa futura para compatibilidade com presets Slang/RetroArch, não como dependência obrigatória. Correção da revisão: a documentação upstream classifica WGPU como suporte completo, não secundário. Mesmo assim, cada preset e a integração real Slint/WGPU/Vulkan precisam de testes.

## Objetivo de reconstrução CRT em alta resolução

O alvo visual é aproximar a experiência de um CRT de alta qualidade da década de 1990, aproveitando a resolução física de uma tela 4K para reconstruir com precisão a estrutura luminosa que um framebuffer 240p-like não consegue descrever sozinho. A saída 3840×2160 não cria detalhe do console: ela oferece amostras suficientes para modelar a máscara RGB de fósforos (shadow mask/aperture grille), a distribuição do feixe sobre os fósforos, scanlines, blooming, convergência e resposta temporal com alinhamento subpixel estável.

O shader deve tratar a máscara RGB como estrutura física da tela, em coordenadas de saída, e não como textura decorativa presa aos pixels de origem. A frequência, fase e orientação da máscara precisam ser calibráveis para a densidade física do display; resize, escala fracionária e mudança de resolução não podem introduzir moiré, cintilação ou deriva do padrão. A energia do feixe, o espalhamento e o bloom são calculados em espaço linear, com limites para evitar clipping. O RGB de origem continua limitado pela paleta/CRAM real do sistema: HDR e wide gamut representam luminância e espaço de saída, não expansão artificial da paleta.

A arquitetura separa três conceitos: (1) raster/frame nativo e temporização do VDP; (2) codificação do sinal, incluindo RGB direto ou composto NTSC/PAL-M/PAL; e (3) reconstrução eletro-óptica do CRT. Não presumir que todos os modos dos três sistemas sejam genericamente “240p”: preservar dimensões ativas, totais e cadência de cada perfil de hardware. A reconstrução é calculada na resolução final de apresentação (1080p, 1440p ou 4K), sem alterar o raster emulado.

Perfis de referência devem permitir calibrar shadow mask, aperture grille e monitor profissional, além de intensidade do feixe, largura do spot, scanlines, bloom, convergência, curvatura e resposta temporal. Os parâmetros precisam ser independentes por perfil de sistema/região e não podem modificar VDP, CRAM, Scheduler ou estado da máquina.

## VRR para cadências PAL de 50 Hz

Jogos PAL europeus frequentemente operam em cadência nominal próxima de 50 Hz. Em monitores fixos de 60 Hz, conversões ou pacing inadequados podem produzir repetição irregular de frames, tearing ou judder, reforçando a percepção de movimento lento/irregular. VRR pode acompanhar a cadência real de aproximadamente 50 Hz (quando dentro da faixa operacional do monitor), reduzindo esses artefatos de apresentação e deixando o movimento mais consistente.

Isso não acelera o jogo PAL, não altera sua temporização original e não transforma 50 atualizações por segundo em 60. O MachineScheduler mantém os clocks, totais e sequência originais do perfil PAL; VRR apenas coordena o instante de apresentação. A implementação deve medir frame pacing e validar faixa VRR/LFC, driver, compositor e modo de janela. Fallback fixo precisa manter cadência sem deriva e não pode descartar frames exigidos por filtros temporais. VRR também não substitui correção de áudio nem autoriza servo que altere o período do hardware emulado.

## ## 2. Resoluções de saída

| Perfil | Resolução de saída |
|---|---:|
| Full HD | 1920 × 1080 |
| QHD | 2560 × 1440 |
| UHD / 4K | 3840 × 2160 |

A resolução de origem não muda. SMS/SG-1000 usam o framebuffer lógico definido pelo VDP; Game Gear usa a janela ativa 160×144. A resolução de saída determina somente viewport e densidade espacial dos efeitos. SMS/SG-1000 preservam aspect ratio 4:3. No Game Gear, 160×144 em pixels quadrados resulta em 10:9; considerando PAR horizontal 1,2:1, a área física resulta em 4:3. Se o host usar framebuffer compartilhado 256×192, o crop proposto é offset (48/256, 24/192) e scale (160/256, 144/192), ainda sujeito à confirmação do contrato real do renderer GG.

Não dimensionar buffers de sinal composto para 1080p/1440p/4K. O sinal composto e o ghosting são calculados na matriz de origem; shaders operam no viewport final. Preservar aspect ratio, overscan e bordas, sem esticar a imagem.

## 3. Distribuição dos filtros

| Filtro | Estágio recomendado | Domínio |
|---|---|---|
| RGB direto / pixels puros | GPU sampler/shader mínimo | Textura de origem |
| Bilinear / sharp-bilinear | GPU WGSL ou sampler | Escala de apresentação |
| NTSC composto | CPU Rust | Linha ativa na resolução nativa |
| PAL-M composto | CPU Rust | Linha ativa + estado da linha anterior |
| LCD frame blending / ghosting GG | CPU Rust | Janela ativa 160×144 |
| CRT scanlines | GPU WGSL | Resolução física do viewport |
| Aperture grille / shadow mask | GPU WGSL | Pixels físicos de saída |
| Curvatura e vinheta | GPU WGSL | Resolução física do viewport |
| LCD grid / subpixels GG | GPU WGSL | Janela GG recortada e viewport |

A divisão é uma hipótese de arquitetura, não uma medição. Valores como menos de 0,15 ms na CPU ou 0,2 ms na GPU não são resultados medidos nem garantias.

## 4. CPU: NTSC e PAL-M

O filtro composto opera sobre uma cópia RGBA8 do frame concluído, com buffers de linha pré-alocados e sem alocação por frame. O modelo experimental contém conversão RGB para YUV/YIQ, modulação em quadratura, fase horizontal/por linha/por campo, separação Y/C, filtragem horizontal, demodulação e reconstrução RGB.

Referências de frequência: 3,579545 MHz para NTSC-M e aproximadamente 3,575611 MHz para PAL-M. O clock de pixel nominal citado para SMS é aproximadamente 5,369317 MHz; em relação à subportadora NTSC isso corresponde a 1,5 amostra de pixel por ciclo e avanço nominal de 240° por pixel. Para PAL-M, calcular a relação com a frequência PAL-M efetiva, sem arredondar a sequência de fase para um ciclo idêntico ao NTSC. A relação entre clock de pixel e subportadora deve ser derivada do perfil real do VDP/encoder e do modelo de amostragem. A razão matemática isolada não determina toda a fase: blanking, burst, sincronismo, alinhamento de linha/campo e origem temporal também importam.

**NTSC:** especificar fase da subportadora por linha/campo, burst e política de separação Y/C. Alternar fase arbitrariamente por linha/quadro não basta para declarar fidelidade NTSC.

**PAL-M:** preservar a família 525 linhas/60 campos e subportadora PAL-M; implementar alternância de fase V e linha de atraso conforme a sequência efetivamente documentada. Não reutilizar PAL de 625 linhas como se fosse PAL-M.

O protótipo de 3 taps enviado é apenas um ponto de partida visual. O filtro [0.25, 0.50, 0.25] não anula matematicamente uma subportadora amostrada a 240° por pixel; portanto, não é separador Y/C completo. O protótipo também não modela integralmente burst, blanking, sincronismo, ruído, banda real do encoder/decoder ou tolerâncias do hardware.

As tabelas de fase devem ser determinísticas. Não depender de trigonometria em tempo de execução. A geração por const fn só deve ser usada após confirmar precisão e suporte na toolchain Rust fixada; tabela explícita ou geração reproduzível também são opções. SIMD é otimização posterior: primeiro estabelecer baseline escalar, vetores de teste e benchmark Release. Não introduzir std::simd ou crate SIMD sem verificar estabilidade, MSRV e custo de dependência.

## 5. CPU: Game Gear LCD ghosting

O efeito de persistência atua somente na janela ativa GG (160×144) dentro do buffer compartilhado, caso o contrato de vídeo mantenha uma superfície lógica 256×192 com offset documentado. O offset proposto X=48..207 e Y=24..167 precisa ser confirmado no contrato real do VDP antes de ser fixado.

Mistura inteira sugerida:

    out = (current * (256 - persistence) + previous * persistence + 128) >> 8

O coeficiente é parâmetro de aparência, não especificação física comprovada. A atualização recursiva do histórico representa um filtro IIR simplificado; não é automaticamente o comportamento eletro-óptico completo de um LCD passivo.

**Regra temporal:** ghosting precisa consumir cada frame emulado na ordem correta. O FrameMailbox atual é latest-only e pode substituir frames pendentes. Portanto, não atualizar o histórico somente a partir dos frames apresentados se a intenção for simular persistência por frame emulado. A arquitetura futura deve processar o histórico antes da coalescência ou introduzir fila de frames concluídos com política explícita de overflow. Essa fila nunca pode aplicar backpressure ao MachineScheduler.

Resetar histórico ao resetar máquina, trocar ROM/sistema/perfil ou restaurar save state, conforme política documentada. Se o estado do filtro fizer parte de replay visual determinístico, serializar histórico e metadados; caso contrário, declarar estado transitório e reiniciá-lo previsivelmente.

## 6. GPU: WGSL embarcado

Para efeitos próprios, preferir poucos shaders WGSL mantidos no repositório e embarcados com include_str! ou compilação equivalente:

- **Modo 0 — Nearest:** amostragem no centro do texel.
- **Modo 1 — Bilinear:** interpolação linear.
- **Modo 2 — Sharp Bilinear:** núcleo nítido e transição subpixel suavizada.
- **Modo 3 — CRT:** scanlines, máscara RGB/aperture grille e bloom moderado.
- **Modo 4 — Game Gear LCD Grid:** separação de subpixels e bordas da matriz LCD.

Fullscreen triangle sem vertex buffer é opção adequada. Um único sampler linear pode ser mantido se nearest calcular o centro correto do texel e os testes cobrirem bordas, recorte UV e coordenadas fora da área ativa.

O shader enviado é protótipo, não código aprovado. Validar:
- fórmula sharp-bilinear em escalas inteiras e fracionárias;
- máscara alinhada à posição física do pixel de saída, não somente ao texel lógico;
- ausência de smoothstep com limites invertidos, pois WGSL não garante resultado nesses casos;
- divisão por zero e parâmetros fora de faixa;
- filtragem nas bordas do UV crop do Game Gear;
- layout/alinhamento dos uniforms Rust/WGSL;
- validação WGSL/Naga e testes de imagem em Vulkan real.

A GUI Slint apenas escolhe perfil e parâmetros. A atualização de uniforms e troca de pipeline ocorrem no host de apresentação, em fronteira segura de frame.

## 7. Cor e CRAM por sistema

O pipeline de rasterização pode compartilhar leitura de paleta RGBA8 pré-decodificada, desde que decodificação de CRAM e estado de hardware permaneçam específicos do perfil.

| Perfil | CRAM | Decodificação |
|---|---|---|
| SG-1000 / TMS9918A | Paleta fixa de 16 cores | Índice TMS9918A |
| Master System | 32 entradas × 6 bits | RGB com 2 bits por canal |
| Game Gear nativo | 32 palavras de 12 bits (64 bytes) | RGB com 4 bits por canal |
| Game Gear em modo SMS | Semântica de paleta SMS | RGB com 2 bits por canal |

No Game Gear nativo, o byte de endereço par fica retido no latch e o byte ímpar conclui a escrita da palavra de 12 bits. A CRAM só deve ser considerada atualizada quando o par for efetivado; o latch é estado de hardware e integra snapshots. Não confundir esse latch com o read buffer da VRAM.

Um cache [u32; 32] em RGBA8 é otimização plausível, mas precisa respeitar ordem de bytes, alpha, paleta inicial e invalidação em cada escrita efetiva. Não duplicar lógica de tiles/sprites por console.

O perfil Game Gear SMS-compat não deve ser ativado apenas por extensão/header da ROM: a seleção precisa refletir o estado real do hardware/registrador de sistema. Esta seção documenta requisitos; não autoriza modificar VDP nesta PR SH-0.

## 8. SDR, HDR e gerenciamento de cor no WGPU 30

WGPU 30 expõe SurfaceConfiguration::color_space. A seleção HDR deve consultar Surface::get_capabilities e capacidades de espaço de cor por formato (format_capabilities), não apenas procurar Rgba16Float em uma lista simples. Surface::display_hdr_info() fornece dados consultivos do display, como luminância de pico e branco SDR quando disponíveis.

- **SDR sRGB:** caminho universal. Para formato *Srgb, shader escreve luz linear e formato aplica codificação sRGB; para formato não-sRGB, aplicação entrega valores codificados conforme o espaço selecionado.
- **scRGB:** Rgba16Float + ExtendedSrgbLinear, quando anunciado pelo backend. Shader escreve valores lineares estendidos.
- **HDR10/PQ:** Bt2100Pq e formato compatível, somente quando a combinação for anunciada. Aplicação faz transformação de gamut e codificação PQ.

Não inferir HDR ativo apenas de Rgba16Float. Não presumir que compositor aplicará conversão ou tone mapping desejados: WGPU entrega sinal no espaço configurado, e a aplicação codifica corretamente. Recriar/selecionar pipeline quando formato ou espaço de cor mudar.

HDR10+ dinâmico (ST 2094-40) e Dolby Vision não são declarados suportados por esse caminho genérico WGSL/WGPU. Esta proposta cobre SDR, scRGB e HDR10/PQ conforme suporte anunciado e validado.

Brilho de fósforo/scanlines deve ser parametrizado em luz linear e limitado pelo headroom informado. Não usar hdr_peak_boost fixo sem tone mapping, medição do display e teste contra clipping. Não inferir o slider de branco SDR do Windows por fallback fixo quando API não fornecer dados.

## 9. Pacing de vídeo, VRR e áudio

Taxas de quadro nominais são derivadas dos clocks e totais de linhas/ciclos do perfil de hardware, não de constantes aproximadas de 60/50 Hz. 59,922743 Hz (NTSC/PAL-M) e 49,701460 Hz (PAL) são referências de cálculo a confirmar contra cada perfil Scheduler/VDP. SG-1000, SMS e Game Gear não devem herdar automaticamente o mesmo total de linhas sem validação.

Pacing deve usar relógio monotônico e deadlines absolutos para evitar deriva acumulada. Temporizadores de alta resolução e curta fase final de espera ativa são candidatos específicos do host Windows; jitter, consumo de CPU, suspensão/alt-tab e descarte de frames precisam ser medidos. Não prometer jitter inferior a 15 µs sem benchmark repetível.

**VRR não é habilitado simplesmente por escolher Mailbox ou Immediate.** Consultar modos suportados pela superfície e manter fallback seguro (Fifo). Immediate pode permitir tearing; Mailbox pode descartar frames intermediários. Validar combinação de driver, DWM, modo de janela, monitor e indicador/OSD G-Sync/FreeSync. Não impor modo sem medição no Windows 11/Vulkan.

Sincronização de áudio não pode ser resolvida alterando silenciosamente período de vídeo enquanto Scheduler produz áudio e vídeo a partir do mesmo clock emulado. Servo PI sobre frame period pode alterar velocidade efetiva da máquina e relação entre áudio e tempo real; não é correto afirmar que preserva pitch automaticamente. Primeiro definir autoridade de pacing, medir ocupação do ring buffer e drift entre clocks, então escolher pacing coordenado, resampling assíncrono ou ajuste explicitamente limitado. Callback CPAL permanece lock-free e não executa controle complexo.

Nenhum ajuste de pacing do host pode mudar sequência de ciclos, master clock emulado ou número de amostras geradas por ciclo de hardware. Métricas de ocupação do ring buffer podem ser publicadas por atomics lock-free a partir do callback CPAL, sem Mutex; uma capacidade de 2048 frames estéreo e alvo de 50% são apenas pontos de partida para experimento, não parâmetros fixados. O callback limita-se a consumir/preencher áudio e publicar métricas; não executa servo, alocação ou espera ativa. Métricas de ocupação do ring buffer podem ser publicadas por atomics lock-free a partir do callback CPAL, sem Mutex; uma capacidade de 2048 frames estéreo e alvo de 50% são apenas pontos de partida para experimento, não parâmetros fixados. O callback limita-se a consumir/preencher áudio e publicar métricas; não executa servo, alocação ou espera ativa.

## 10. Benchmarks e aceitação

- CPU: benchmark Release no hardware alvo, 256×192 e 160×144 GG, zero alocações por frame após inicialização.
- SIMD: comparar baseline escalar e provar igualdade/erro máximo por canal.
- GPU: timestamp queries em 1080p, 1440p e 4K; registrar GPU, driver, formato, present mode e pipeline.
- Imagem: golden images para barras, gradientes, color bars, checkerboard, dithering, bordas, subpixels e movimento.
- Determinismo: mesmo input/estado inicial produz mesmo output; snapshot/restore reproduz sequência.
- Latência: medir fila, CPU, upload, GPU e apresentação separadamente.
- Robustez: resize, alt-tab, troca de região/perfil, reset, ROM, perda de frames e fallback de shader.

## 11. Etapas

| Etapa | Entrega | Critério |
|---|---|---|
| SH-0 | Arquitetura e catálogo documental | Fronteiras, hipóteses e critérios claros |
| SH-1 | POC WGSL nativo | Vulkan real, SDR, resize, lifecycle e fallback |
| SH-2 | Filtros espaciais GPU | Nearest, bilinear, sharp-bilinear, CRT e golden images |
| SH-3 | NTSC/PAL-M CPU | Modelo de fase, vetores de referência e comparação |
| SH-4 | Game Gear LCD CPU + grid GPU | Histórico por frame, reset/restore e recorte |
| SH-5 | HDR/VRR e sincronização | Negociação color space, medições e pacing |
| SH-6 | UI e persistência | Perfis por sistema, 1080p/1440p/4K e parâmetros |

Não modificar VDP, Scheduler ou runtime para facilitar pós-processamento. Exceções exigem justificativa, testes e preservação semântica/temporal demonstrada.

## Referências técnicas

- WGPU 30 HDR: https://wgpu.rs/doc/wgpu/documentation/color/hdr_surfaces/
- WGPU 30 SurfaceConfiguration: https://wgpu.rs/doc/wgpu/api/surface/type.SurfaceConfiguration.html
- librashader APIs: https://github.com/SnowflakePowered/librashader#supported-render-apis
- Windows 11 windowed optimizations: https://support.microsoft.com/en-au/windows/hardware/display-graphics/optimizations-for-windowed-games-in-windows-11
- TI TVP5147M1: https://www.ti.com/lit/ds/symlink/tvp5147m1.pdf
- Sega Game Gear Hardware Reference Manual: https://segaretro.org/images/1/16/Sega_Game_Gear_Hardware_Reference_Manual.pdf
- SMS/Game Gear VDP notes: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
