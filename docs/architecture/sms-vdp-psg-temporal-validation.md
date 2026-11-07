# Matriz de validação temporal — SMS VDP 315-5124/315-5246 e PSG SN76489

**Status:** ACCEPTANCE BASELINE — gates A/V-01/02/03/04a automatizados PASS; backend físico e gameplay ainda NOT RUN.  
**ADR:** [Reconstrução Qt Temporal e fechamento vertical do Master System](ADR-sms-temporal-machine-qt-rebuild.md)  
**Base inicial:** `b0bda60e3fbfa56f820fe77d3098f8b82b5af69d`  
**Regra:** nenhuma linha desta matriz é PASS até existir teste executado no código integrado e evidência reproduzível.

## 0. Gate funcional de imagem e áudio — pré-requisito de gameplay

**Prioridade vigente em 2026-11-06:** antes de ampliar a precisão temporal ou medir desempenho de jogos, comprovar que o runtime SMS produz imagem e áudio não silenciosos e que as saídas nativas recebem esses sinais. Este gate não equivale a compatibilidade geral nem a TEMPORAL_PASS.

| ID | Fronteira | Critério de aceite | Evidência exigida |
|---|---|---|---|
| A/V-01 | VDP / core | ROM sintética programa Mode 4, VRAM/name table e CRAM; frame concluído contém pixels RGB não pretos em coordenadas esperadas | Teste Rust determinístico com expected values; dimensão do frame isoladamente não basta |
| A/V-02 | PSG / core | ROM sintética escreve registradores SN76489 e a timeline gera amostras PCM não silenciosas | Teste Rust determinístico: quantidade de amostras e amplitude não nula; estado de registrador isolado não basta |
| A/V-03 | Worker / vídeo | O worker publica o framebuffer produzido pela máquina no mailbox como RGB888, com sequência crescente e pixels preservados | Teste do worker/mailbox com conteúdo conhecido |
| A/V-04a | Worker / NativeAudioSession | O worker drena PCM e `NativeAudioSession` entrega PCM16 não silencioso ao `PcmSink` antes do flush | Teste `worker_frame_submits_non_silent_pcm_to_native_audio_session_sink` com `RecordingSink`; CI #5185 PASS |
| A/V-04b | Backend nativo | `NativeAudioBackend` WASAPI/XAudio2 aceita PCM sem erro e sem underrun sustentado | NOT RUN; sink em memória não comprova aceitação pelo backend real |
| A/V-05 | Host Windows | A janela D3D12 apresenta uma ROM de diagnóstico/jogo com imagem visível e o endpoint selecionado reproduz som audível | NOT RUN; teste manual em GPU e dispositivo de áudio reais |
| A/V-06 | Gameplay | ROM legal/homebrew ou dump local fornecido pelo usuário ultrapassa o boot e permite interação reproduzível | NOT RUN; registrar nome/região/configuração e SHA-256 apenas local; não versionar ROM comercial |

Classificar cada critério separadamente como PASS, FAIL, BLOCKED ou NOT RUN. A ausência de teste ou a execução somente de smoke test com frame sintético é NOT RUN para a geração de imagem de jogo. A geração de PCM no core não é prova de saída física; inicializar WASAPI não é prova de áudio audível.

A análise de desempenho real começa somente após A/V-01 a A/V-05 estarem comprovados. Registrar resolução, região, FPS, frametime por frame, frames publicados/apresentados, áudio underruns/dropped blocks e configuração do host. Telemetria que percorre framebuffer/VRAM/CRAM deve ser desativável ou ter seu overhead medido antes de comparar desempenho.

## 0.1 Resultados integrados — PRs #595 e #597

**Main atual:** `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`  
**PR #595:** HEAD `69ca40ab29c486f26852adb7536a04f95f9dd7ae`; [CI #5178](https://github.com/leonardo201800478/sega-emulator/actions/runs/42196938080) — SUCCESS.  
**PR #597:** HEAD `30554cc6642dd49d06adf495dca3e80f4822b081`; [CI #5185](https://github.com/leonardo201800478/sega-emulator/actions/runs/42201475412) — SUCCESS; merge SHA `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`.

| Gate | Estado | Evidência / limite |
|---|---|---|
| A/V-01 — VDP/core | PASS | ROM sintética Mode 4; RGB `[255,255,255]` exigido na linha 32 |
| A/V-02 — PSG/core | PASS | ROM sintética programa SN76489; timeline produz PCM não silencioso |
| A/V-03 — worker/mailbox | PASS | Worker publica RGB888 e preserva o pixel esperado no mailbox |
| A/V-04a — worker/NativeAudioSession/PcmSink | PASS | `RecordingSink` recebeu bloco PCM não silencioso antes do flush; CI #5185 |
| A/V-04b — NativeAudioBackend/WASAPI/XAudio2 | NOT RUN | Sink em memória não comprova aceitação pelo backend nativo real |
| A/V-05 — host físico | NOT RUN | GPU física, janela D3D12 e áudio audível em WASAPI/XAudio2 não foram testados |
| A/V-06 — gameplay | NOT RUN | Nenhuma ROM real/homebrew foi comprovada nesta etapa |

Jobs Rust, Python, Windows D3D12 e PyO3/Maturin passaram no HEAD exato. Scheduler deterministic stress, Windows Qt smoke e PowerShell tooling foram SKIPPED e permanecem NOT RUN. Nenhum SKIPPED foi contabilizado como PASS.

A correção VDP IRQ → Z80 INT da PR #595 preserva a INT externa, sincroniza após dispatch e antes de iniciar instrução, e inclui validação do vetor/linha no restore. O teste sintético VBlank/IM1/HALT exige 20 reconhecimentos em 20 frames. A PR #597 generaliza `execute_frame` sobre `PcmSink` e verifica PCM não silencioso submetido pelo worker à `NativeAudioSession` antes do flush. Isso não comprova aceitação por WASAPI/XAudio2 real, áudio audível, Line IRQ em todos os modos, gameplay real ou precisão dot-level.

## 1. Classificação de evidência

| Código | Evidência |
|---|---|
| DOC | Manual/documentação primária |
| RE | Pesquisa reversa/engenharia reversa |
| HW | Medição ou teste em hardware Master System da revisão indicada |
| DIFF | Comparação com implementação independente, identificada por versão/commit |
| MODEL | Modelo matemático ou derivação de clock |
| TEST | Teste automatizado reproduzível no repositório |

Uma implementação de outro emulador é evidência comparativa, não prova de hardware. Um teste em Z80 de outro console não é teste de VDP/PSG SMS. Quando duas fontes divergem, preservar a divergência e não escolher silenciosamente.

## 2. VDP — matriz funcional

| ID | Área | Vetor / cenário | Resultado exigido | Evidência inicial |
|---|---|---|---|---|
| VDP-F01 | Portas | Data/control ports em todos os espelhos de endereço | Decodificação correta, sem aliases inventados | DOC |
| VDP-F02 | Control word | Primeira escrita pendente, segunda escrita seleciona VRAM read/write, CRAM ou registrador | Estado parcial correto; operação aplicada somente no segundo byte | DOC + TEST |
| VDP-F03 | Read-ahead | Selecionar VRAM read, ler sequência, atravessar $3FFF | Primeiro byte vem do buffer; prefetch e wrap corretos | DOC + TEST |
| VDP-F04 | Status | Ler status com VBlank/overflow/collision/line IRQ pendentes | Retorno dos bits e clear/latches conforme perfil | DOC + TEST |
| VDP-F05 | VRAM | Escrita/leitura nos limites e espelhamentos de tabela | 16 KiB e máscaras por revisão | DOC + TEST |
| VDP-F06 | CRAM | Valores 0x00..0xFF em cada entrada | Máscara de bits e conversão de cor por perfil | DOC + TEST |
| VDP-F07 | R0 bit 5 | Background e sprites desenhados na coluna x=0..7 | Pixels da coluna esquerda mascarados com backdrop quando bit=1; x>=8 inalterado | DOC + TEST |
| VDP-F08 | R0 bit 3 | Sprite em X=0 e X=1, com bit=0/1 | Deslocamento de sprite -8 px reproduzido; independente do bit 5 | DOC + TEST |
| VDP-F09 | R0 bits 5+3 | Combinar máscara e sprite shift | Sem deslocamento duplicado, sem faixa extra; entrada/saída de sprites correta | DOC + TEST |
| VDP-F10 | R7 | Alterar backdrop e sprite palette entries | Cor da área mascarada/backdrop correta; não confundir com cor de background palette | DOC + TEST |
| VDP-F11 | R0 bit 6 | Scroll horizontal em linhas 0–15 e 16+ | Scroll fixo nas linhas superiores quando habilitado | DOC + TEST |
| VDP-F12 | R0 bit 7 | Scroll vertical nas colunas 24–31 e demais | Colunas fixas conforme regra do registrador | DOC + TEST |
| VDP-F13 | R8 | Todos os valores 0..255 | Fine scroll, coarse column, wrap e pixels de borda corretos | DOC + TEST |
| VDP-F14 | R9 | Todos os valores 0..255 e escrita durante frame | Vertical scroll, latch e momento de aplicação corretos | DOC + TEST |
| VDP-F15 | R10 | Contagem zero, underflow, reload e escrita em período ativo | Line IRQ no ponto correto e clear correto | DOC + TEST |
| VDP-F16 | Background | Tiles, duas paletas, flip H/V, prioridade e transparência | Pixel final correto por coordenada | DOC + TEST |
| VDP-F17 | Sprites | 64 entradas, terminador, 8x8/8x16, padrões, flips e prioridade | Avaliação e composição por scanline conforme hardware | DOC + TEST |
| VDP-F18 | Overflow | 9+ sprites em linha, incluindo sprites transparentes/fora da área útil | Overflow de acordo com a avaliação da tabela, não só pixels visíveis | RE + HW |
| VDP-F19 | Collision | Sprites opacos sobrepostos; pixels transparentes; x=0 mascarado | Collision flag conforme lógica de sprite, sem falsos positivos | RE + HW |
| VDP-F20 | VBlank | Última linha ativa, primeira linha de VBlank, leitura de status | Flag, IRQ e frame-ready no timestamp correto | DOC + TEST |
| VDP-F21 | Altura | 192/224/240 por revisão e standard | Apenas combinações suportadas; tabelas e endereços de name table corretos | DOC + TEST |
| VDP-F22 | SMS1 masks | Máscaras dos registradores de tabela em 315-5124 | Endereços VRAM e aliases refletem gates físicos do SMS1 | DOC + TEST |
| VDP-F23 | SMS2 masks | Comparar 315-5246 | Não aplicar máscaras exclusivas de SMS1 ao SMS2 | DOC + TEST |
| VDP-F24 | V/H counters | Cada linha e posição horizontal; wrap regional | Valor de contador e wrap conforme standard/revisão | DOC + RE + HW |
| VDP-F25 | TH/H counter | Capturar H counter em transições TH de ambos os ports | Latch no instante da transição, leitura estável até nova transição | DOC + HW |
| VDP-F26 | Raster write | Escritas de R0/R1/R7/R8/R9, VRAM e CRAM antes/depois de boundaries | Pixels e efeitos seguintes refletem o instante de commit real | RE + HW |
| VDP-F27 | Reset | Reset com preservação/limpeza de VRAM conforme tipo de reset | Registradores, latches, status e memória no estado definido para cada reset | DOC + TEST |
| VDP-F28 | Save state | Snapshot durante linha ativa, sprite evaluation e VBlank | Restore bit-a-bit e continuidade idêntica | TEST |
| VDP-F29 | DAC/paleta | Todos os 64 valores CRAM em 315-5124 e 315-5246 | Tabela de saída RGB por revisão; SMS1 deve testar nível azul não linear; SMS2 possui níveis distintos | RE + DIFF + HW |
| VDP-F30 | Raster/overscan | Capturar bordas, blanking, sync e active window por linha | Geometria/offsets e cor de cada segmento correspondem à revisão e standard | DOC + DIFF + HW |

### 2.1 Overscan, blanking e origem do viewport

A matriz deve distinguir três coordenadas, sem sobrepor semânticas:

- **Dot/raster coordinate:** posição do feixe no total da linha.
- **Active-area coordinate:** coordenada 0..255 da imagem ativa do VDP.
- **Presentation coordinate:** coordenada após crop/escala/proporção do frontend.

A documentação independente consultada descreve uma linha de 342 dots, com 256 dots de área ativa e segmentos de borda, blanking, sync e color burst. A própria fonte alerta que parte dessa descrição deriva de manual TMS9918 e medições em Genesis, podendo divergir do SMS real. Portanto:
- 342 dots é hipótese inicial de modelo a verificar por revisão;
- intervalos exatos de borda/blanking/sync e cor não podem ser considerados HW-PASS sem medição SMS ou fonte primária específica;
- a saída ativa de 256 pixels é separada dos pixels de borda;
- o raster completo pode ser exposto para diagnóstico quando a codificação de cada segmento estiver sustentada por evidência.

**Regra visual da coluna esquerda:**
- R0 bit 5 substitui os primeiros 8 pixels da área ativa pela cor de backdrop/overscan;
- R0 bit 3 desloca os sprites 8 pixels à esquerda;
- os dois bits têm efeitos independentes e podem coexistir;
- a opção de cortar esses 8 pixels é uma transformação do viewport, nunca uma alteração do framebuffer bruto, do VDP ou do save state;
- capturas de referência e testes sempre guardam a imagem bruta e metadados (active rect, overscan rect, revisão, altura, standard, R0/R7).

### 2.2 Comparação independente e limites das referências

A auditoria comparativa inicial encontrou:

- **MAME** mantém dispositivos separados para 315-5124 e 315-5246, descreve segmentos horizontais/verticais de raster e tabelas de timing por standard/altura. A implementação declara explicitamente que *mid-scanline changes* e restrições de acesso VRAM/CRAM ainda são TODO; portanto, não será tratada como referência completa para esses fenômenos.
- O MAME usa tabelas RGB diferentes por revisão: no 315-5124, níveis `{0,78,160,238}` e canal azul `{0,98,160,238}`; no 315-5246, níveis `{0,89,174,255}`. O renderer legado do projeto usa escala linear `0,85,170,255`, logo não é uma referência de saída RGB por revisão. Os valores MAME são evidência comparativa a confirmar por medições/fonte primária.
- **Gearsystem** separa overscan de *Hide Left Bar* e oferece comportamento `Auto` condicionado ao left-column masking. Isso é uma boa referência de UX: o novo Qt adotará `Não/Auto/Sempre`, com `Auto` como padrão, sem modificar o raster bruto.
- A documentação de Charles MacDonald descreve a coluna 0 mascarada com R7 e o deslocamento de sprites por R0 bit 3 como controles distintos. O teste precisa cobrir cada bit isolado e ambos juntos.

Fontes comparativas:
- MAME VDP: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Gearsystem Video: https://github.com/drhelius/Gearsystem/blob/master/src/Video.cpp
- Gearsystem VDP reference: https://github.com/drhelius/Gearsystem/blob/master/skills/gearsystem-debugging/references/msvdp.md

## 3. VDP — matriz temporal

| ID | Evento | Critério temporal |
|---|---|---|
| VDP-T01 | Clock de dot e scanline | Domínio racional e fase preservada em avanço contínuo ou fracionado |
| VDP-T02 | Linha completa | Próxima linha inicia no timestamp esperado; nenhum drift acumulado |
| VDP-T03 | Avaliação de sprites | SAT é lida/avaliada no intervalo documentado; escritas durante avaliação testadas |
| VDP-T04 | Fetch de tile/pattern/CRAM | Mudanças de VRAM/CRAM após o fetch não reescrevem pixels já emitidos |
| VDP-T05 | Scroll latches | R8/R9 são amostrados no ponto efetivo, inclusive mudanças dentro de frame |
| VDP-T06 | R0/R1 e screen enable | Mudanças afetam o raster no boundary observado; não retroagem pixels emitidos |
| VDP-T07 | Line IRQ | Underflow e assert no instante exato, com prioridade determinística |
| VDP-T08 | VBlank IRQ | Assert na fronteira de início de VBlank, sincronizada com o beam |
| VDP-T09 | Status read | Leitura limpa apenas flags especificadas e no timestamp de commit |
| VDP-T10 | H counter | Valor derivado do domínio de dots; latch TH não é sintetizado pelo frontend |
| VDP-T11 | CPU/VDP bus | Escritas e leituras de portas ordenadas por timestamp e prioridade de hardware |
| VDP-T12 | Save/restore | Preserva beam, fase de dot, fetches pendentes, avaliação de sprite e eventos agendados |
| VDP-T13 | Determinismo | Execução em blocos N, N+1 e contínua produz mesmo raster/IRQ/trace |
| VDP-T14 | Contenção/WAIT | Implementar somente quando evidência demonstrar bloqueio de barramento/CPU; WAIT de fixture não é WAIT físico |

## 4. PSG — matriz funcional

| ID | Área | Vetor / cenário | Resultado exigido | Evidência inicial |
|---|---|---|---|---|
| PSG-F01 | Latch | Todos os 8 registradores e bytes latch | Seleção correta por bits 6..4 e atualização dos bits baixos | DOC + TEST |
| PSG-F02 | Data byte | Data para cada registrador latched | Atualiza registrador latched persistente; não descartar writes | DOC + TEST |
| PSG-F03 | Tone | Três canais, períodos 0, 1, 2, 0xFE, 0x3FF | Counter, saída e frequência conforme divisor e período | DOC + TEST |
| PSG-F04 | Tone writes | Atualizar nibble baixo e depois 6 bits altos; ordem inversa | Mudança imediata do período, sem esperar palavra completa | DOC + TEST |
| PSG-F05 | Volume | 0..15 em todos os canais | Escala de atenuação e silêncio no 15 | DOC + TEST |
| PSG-F06 | Noise rate | 0x10, 0x20, 0x40 e tom 2 | Counter usa o período selecionado; canal 2 dirige a taxa quando 11 | DOC + TEST |
| PSG-F07 | White noise SMS | Seed, taps 0/3, feedback em bit 15 | Sequência de 16 bits idêntica ao perfil SMS documentado | DOC + DIFF |
| PSG-F08 | Periodic noise SMS | Feedback por bit 0 | Sequência periódica de 16 bits; escrita no noise register reinicia o LFSR sem reiniciar o divisor/fase em andamento | DOC + DIFF + TEST |
| PSG-F09 | Shift cadence | Duas expirações do contador de ruído | LFSR desloca na borda correta (uma vez a cada duas expirações) | DOC + TEST |
| PSG-F10 | Divider | Avanços que terminam antes, exatamente em e depois de 16 clocks | Fase residual correta; nenhum drift por chunking | DOC + TEST |
| PSG-F11 | Port mapping | Escritas em $40-$7F, inclusive $7E/$7F | Decode/mirror correto para SMS | DOC + TEST |
| PSG-F12 | Reset | Reset de chip e reset de console | Estado inicial SMS integrado correto e distinguido de chips discretos | DOC + TEST |
| PSG-F13 | Mixer | Canais isolados e somados | Mix determinístico; clipping/normalização documentados | TEST |
| PSG-F14 | Sampling | 44.1/48/96 kHz e clocks NTSC/PAL | Sample count/phase exatos sem comandar o chip pelo host | MODEL + TEST |
| PSG-F15 | Filter | FIR history/position e continuidade em blocos | Sem descontinuidade ao mudar o tamanho do bloco; filtro identificado como output stage, não circuito analógico físico | TEST |
| PSG-F16 | Save state | Durante tom, ruído e fase intermediária do divisor | Restore reproduz exatamente a sequência digital e amostras futuras | TEST |
| PSG-F17 | FIR espectral | NTSC/PAL em 22,05/44,1/48/96 kHz | Ganho DC unitário, simetria, ganho de banda passante ≥ 0,999 e rejeição ≥ 78 dB em toda a banda a partir de Nyquist da saída; filtro é estágio de conversão, não circuito analógico | MODEL + TEST |

## 5. PSG — integração temporal

| ID | Evento | Critério temporal |
|---|---|---|
| PSG-T01 | Clock de entrada | Mesmo oscilador/perfil da máquina; divisor /16 preservado |
| PSG-T02 | Escrita PSG | Registrador atualizado no timestamp de conclusão da transação I/O |
| PSG-T03 | Tone edge | Contadores e flip-flops avançam somente nos clocks internos devidos |
| PSG-T04 | Noise edge | Counter, fase e LFSR seguem a sequência de bordas do chip |
| PSG-T05 | Noise rate = tone 2 | A taxa segue a cadência do canal 2; relação de fase e realização interna (contador equivalente ou clock derivado do flip-flop) precisam corresponder ao hardware |
| PSG-T06 | Geração de amostras | Samples são derivados da timeline; o dispositivo host só consome blocos |
| PSG-T07 | Overflow de fila | Política de host buffer não retroage nem altera o estado digital do PSG |
| PSG-T08 | Determinismo | Diferentes tamanhos de avanço geram a mesma sequência de bits e samples |
| PSG-T09 | Restore | Snapshot inclui fase do divisor, counters, outputs, LFSR, filtro e fase de sample |
| PSG-T10 | CPU/VDP/PSG | Escritas de CPU, eventos VDP e PSG seguem ordem causal única |

### 5.1 Nota de evidência — continuidade do divisor e rate = Tone2

A documentação SMS Power afirma que a escrita no noise register reinicia o shift register, sem especificar reinício do contador/divisor. A implementação comparativa MAME atualiza o período de ruído e reinicia o RNG, mas não reinicia o contador de ruído. O RTL MiSTer mantém o contador de ruído em execução e seleciona o flip-flop do canal C quando o rate é Tone2. Essas referências sustentam separar o reset do LFSR da continuidade do divisor, mas não substituem medição de hardware SMS.

Há duas realizações comparativas para o rate Tone2:
- MAME: contador de ruído dedicado com período equivalente a duas vezes o período do canal Tone2.
- MiSTer RTL: seleciona diretamente o flip-flop interno do canal C como fonte de clock do LFSR.

Ambas podem produzir a mesma frequência média, mas podem divergir na fase após writes e na relação de fase entre os contadores. A implementação atual mantém contador dedicado; o alinhamento de fase com Tone2 permanece PENDENTE até vetor independente ou medição física. Não declarar PSG-T05 fechado apenas pela igualdade da frequência média.

## 6. Clock profile — itens que precisam de reconciliação

| Perfil | Fonte inicial | Frequência | Estado |
|---|---|---:|---|
| NTSC VDP/master | Manual de serviço SMS | 10.738635 MHz nominal | DOC; preservar precisão/racional |
| NTSC CPU/PSG input | Manual de serviço SMS | 3.579545 MHz nominal | DOC |
| PAL CPU/PSG input | Manual/guia de serviço consultado | 3.546893 MHz | DOC; investigar contra cada placa/revisão |
| PAL VDP/master | Manual SMS II/placas PAL | 10.640684 MHz nominal | DOC; confirmar placa e divisor |
| PAL-M | Perfil regional | Não assumir equivalência completa a PAL nem NTSC | BLOCKED até matriz de placas/fontes |

O código antigo em `SmsVdp::system_clock_hz()` informa `3_546_895` para PAL; o manual/guia de serviço consultado informa aproximadamente `3_546_893` e a documentação de placas MAME registra `3.54689 MHz`. O VDP PAL aparece como aproximadamente `10.6406`/`10.640684 MHz` em fontes distintas. Não classificar automaticamente a constante antiga como bug: reconciliar placa, revisão do ASIC, oscilador e divisores. O perfil deve usar razão racional/oscillator/divider quando a fonte sustentar esse nível de exatidão.

## 7. Integração e determinismo — gate sistêmico

| ID | Teste | PASS exige |
|---|---|---|
| SYS-T01 | Z80 + bus + VDP + PSG | Uma timeline e uma autoridade temporal |
| SYS-T02 | I/O no boundary | VDP/PSG observam escrita no instante de commit, sem atraso de frame |
| SYS-T03 | Run partition | Estado final e traces iguais para execução contínua e blocos |
| SYS-T04 | Save/restore | Snapshot no meio de M-cycle, raster, PSG e IRQ restaura sem divergência |
| SYS-T05 | Replay | Mesmo input e eventos temporais produzem frames/áudio idênticos |
| SYS-T06 | ROM smoke | Boot e gameplay com referências legais/homebrew e hashes registrados |
| SYS-T07 | Differential VDP | Imagens brutas + traces comparados a oráculo independente identificado |
| SYS-T08 | Differential PSG | Sequência digital, frequências e samples comparados a vetores/oráculo |
| SYS-T09 | SG-1000 regression | Perfil SG-1000 do PSG e máquina permanecem sem alteração regressiva |
| SYS-T10 | Host independence | Sem áudio/monitor/dispositivo conectado, a máquina temporal produz mesmo estado |
| SYS-T11 | Qt | Interface não avança hardware por timer/callback; apenas comandos coarse-grained |
| SYS-T12 | Cargo isolation | Runtime Legacy não aparece no build padrão nem nas dependências ativadas |
| SYS-T13 | Python packaging | Nenhum import/entrypoint instala ou inicializa Qt antigo/adaptador SMS antigo |

## 8. Apresentação Qt — validação do viewport\n\n| ID | Opção | Critério |\n|---|---|---|\n| UI-V01 | Left bar = Não | Preserva os 256 pixels ativos, inclusive a coluna mascarada |\n| UI-V02 | Left bar = Auto (padrão) | Corta 8 px somente quando R0 bit 5 estiver ativo e overscan completo não estiver selecionado |\n| UI-V03 | Left bar = Sempre | Corta 8 px independentemente de R0 bit 5, apenas na apresentação |\n| UI-V04 | Overscan completo | Mostra bordas/raster disponíveis e desabilita o crop automático |\n| UI-V05 | Raw capture | Exporta frame bruto + active rect/overscan rect e configuração de hardware, sem viewport transform |\n| UI-V06 | Proporção | Crop, escala e aspect ratio não alteram pixels, registradores ou timeline |\n\n## 9. Gate de classificação

Para cada suíte e cenário:
- **PASS:** executado no SHA declarado, resultado integral aprovado.
- **FAIL:** executado e divergente.
- **BLOCKED:** não pode ser executado por dependência/ambiente/funcionalidade ausente; registrar motivo.
- **NOT RUN:** ainda não executado.
- **SKIPPED:** nunca pode ser agregado como PASS.

Nenhuma evidência de documentação ou comparação substitui a execução do teste correspondente. Nenhum resultado em VDP/PSG funcional substitui a validação temporal. Nenhum smoke test de Qt substitui a comparação de raster/áudio.
