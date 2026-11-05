# Matriz de validação temporal — SMS VDP 315-5124/315-5246 e PSG SN76489

**Status:** ACCEPTANCE BASELINE — testes propostos, ainda não executados.  
**ADR:** [Reconstrução Qt Temporal e fechamento vertical do Master System](ADR-sms-temporal-machine-qt-rebuild.md)  
**Base inicial:** `b0bda60e3fbfa56f820fe77d3098f8b82b5af69d`  
**Regra:** nenhuma linha desta matriz é PASS até existir teste executado no código integrado e evidência reproduzível.

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
| VDP-F31 | Sprite evaluation | SAT alterada antes/durante/depois da janela de avaliação | Seleção para linha seguinte e fetch de atributos/padrões refletem o instante real de leitura | RE + DIFF + HW |
| VDP-F32 | Collision + left mask | Dois sprites opacos sobrepostos em x=0..7 com R0 bit 5 em 0/1 | Colisão suprimida na coluna mascarada conforme revisão; fora dela permanece | RE + DIFF + HW |
| VDP-F33 | SMS1/SMS2 sprite address | Variar R5/R6 e SAT em 315-5124/315-5246 | Extra offset e seleção de tile obedecem máscaras específicas de cada ASIC | RE + DIFF + HW |
| VDP-F34 | Sprite zoom | 8x8/8x16 com zoom e 9+ sprites na mesma linha | Limites de avaliação/zoom e overflow seguem perfil do ASIC | RE + DIFF + HW |
| VDP-F35 | Fine-scroll left edge | R8 com valores 1..7 e padrões específicos na coluna anterior | Primeiros pixels reproduzem o fetch/quirk observado, sem preencher cegamente com backdrop | RE + DIFF + HW |
| VDP-F36 | Sprite limit option | HardwareLimit e UnlockedCompatibility | HardwareLimit é default; modo sem limite só altera compatibilidade explicitamente | TEST |
| VDP-F37 | Frame geometry | 192/224/240 e NTSC/PAL com overscan | Frame transporta active_rect, visible_rect, aspecto e timestamp; viewport não altera raster | TEST |

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

- **MAME** mantém dispositivos separados para 315-5124 e 315-5246, descreve segmentos horizontais/verticais de raster e tabelas de timing por standard/altura. A implementação declara explicitamente que *mid-scanline changes* e restrições de acesso VRAM/CRAM ainda são TODO; portanto, não será tratada como referência completa para esses fenômenos. O código comparativo também seleciona sprites para a linha seguinte, lê atributos/padrões em momentos distintos e documenta que a máscara da coluna zero suprime colisões nessa região.
- O MAME usa tabelas RGB diferentes por revisão: no 315-5124, níveis `{0,78,160,238}` e canal azul `{0,98,160,238}`; no 315-5246, níveis `{0,89,174,255}`. O renderer legado do projeto usa escala linear `0,85,170,255`, logo não é uma referência de saída RGB por revisão. Os valores MAME são evidência comparativa a confirmar por medições/fonte primária.
- **Gearsystem** separa overscan de *Hide Left Bar* e oferece comportamento `Auto` condicionado ao left-column masking. Isso é uma boa referência de UX: o novo Qt adotará `Não/Auto/Sempre`, com `Auto` como padrão, sem modificar o raster bruto.
- A documentação de Charles MacDonald descreve a coluna 0 mascarada com R7 e o deslocamento de sprites por R0 bit 3 como controles distintos. O teste precisa cobrir cada bit isolado e ambos juntos. O MAME diferencia ainda a máscara do extra offset da SAT e da seleção de tile entre 315-5124 e 315-5246. Seu modelo de fine-scroll nos primeiros pixels usa dados específicos do padrão/tile anterior; isso é hipótese diferencial a confirmar, não verdade de silício sem teste SMS.

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
| VDP-T15 | Master clock | Avanços de master ticks fracionados em relação a CPU T-state | VDP preserva fase interna do divisor de pixel/H-counter e não arredonda por scanline | MODEL + TEST |
| VDP-T16 | Sprite evaluation pipeline | Avaliação da linha N para exibição em N+1 e fetches posteriores | Escritas concorrentes em SAT/VRAM produzem efeitos no boundary correto | RE + DIFF + HW |
| VDP-T17 | Mid-dot I/O write | Escrita CPU em posições alternadas de fase VDP | Pixels já emitidos não mudam; fetches futuros observam o commit conforme pipeline | RE + HW |
| VDP-T18 | IRQ/beam ordering | HINT/VINT no mesmo timestamp de acesso I/O | Ordem de eventos e prioridade são estáveis e reproduzíveis | MODEL + TEST |

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
| PSG-F08 | Periodic noise SMS | Feedback por bit 0 | Sequência periódica de 16 bits e reset ao escrever noise register | DOC + TEST |
| PSG-F09 | Shift cadence | Duas expirações do contador de ruído | LFSR desloca na borda correta (uma vez a cada duas expirações) | DOC + TEST |
| PSG-F10 | Divider | Avanços que terminam antes, exatamente em e depois de 16 clocks | Fase residual correta; nenhum drift por chunking | DOC + TEST |
| PSG-F11 | Port mapping | Escritas em $40-$7F, inclusive $7E/$7F | Decode/mirror correto para SMS | DOC + TEST |
| PSG-F12 | Reset | Reset de chip e reset de console | Estado inicial SMS integrado correto e distinguido de chips discretos | DOC + TEST |
| PSG-F13 | Mixer | Canais isolados e somados | Mix determinístico; clipping/normalização documentados | TEST |
| PSG-F14 | Sampling | 44.1/48/96 kHz e clocks NTSC/PAL | Sample count/phase exatos sem comandar o chip pelo host | MODEL + TEST |
| PSG-F15 | Filter | FIR history/position e continuidade em blocos | Sem descontinuidade ao mudar o tamanho do bloco; filtro identificado como output stage, não circuito analógico físico | TEST |
| PSG-F16 | Save state | Durante tom, ruído e fase intermediária do divisor | Restore reproduz exatamente a sequência digital e amostras futuras | TEST |

## 5. PSG — integração temporal

| ID | Evento | Critério temporal |
|---|---|---|
| PSG-T01 | Clock de entrada | Mesmo oscilador/perfil da máquina; divisor /16 preservado |
| PSG-T02 | Escrita PSG | Registrador atualizado no timestamp de conclusão da transação I/O |
| PSG-T03 | Tone edge | Contadores e flip-flops avançam somente nos clocks internos devidos |
| PSG-T04 | Noise edge | Counter, fase e LFSR seguem a sequência de bordas do chip |
| PSG-T05 | Noise rate = tone 2 | O ruído usa a saída/cadência do canal 2, sem clock independente |
| PSG-T06 | Geração de amostras | Samples são derivados da timeline; o dispositivo host só consome blocos |
| PSG-T07 | Overflow de fila | Política de host buffer não retroage nem altera o estado digital do PSG |
| PSG-T08 | Determinismo | Diferentes tamanhos de avanço geram a mesma sequência de bits e samples |
| PSG-T09 | Restore | Snapshot inclui fase do divisor, counters, outputs, LFSR, filtro e fase de sample |
| PSG-T10 | CPU/VDP/PSG | Escritas de CPU, eventos VDP e PSG seguem ordem causal única |
| PSG-T11 | Shared clock | Clock Z80/PSG derivado do mesmo perfil SMS | Domínios avançam com razão racional e snapshots validam fase/ciclos iguais | MODEL + TEST |

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
| SYS-T14 | FM variant | `FmHardware::Ym2413` habilitado | Não declarar suporte completo sem core YM2413, mixer e timeline próprios | BLOCKED até implementação |

## 8. Apresentação Qt — validação do viewport

O engine compartilhado `sega-video` produz pixels e geometria de hardware; `sega-video-runtime` transporta frames e não implementa o VDP. O frame deve carregar active/visible rect, proporção, raster geometry e timestamp/timebase. A Qt só escolhe o viewport.

| ID | Opção | Critério |
|---|---|---|
| UI-V01 | Left bar = Não | Preserva os 256 pixels ativos, inclusive a coluna mascarada |
| UI-V02 | Left bar = Auto (padrão) | Corta 8 px somente se todos os scanlines ativos tiverem R0 bit 5 aplicado e overscan completo não estiver selecionado |
| UI-V03 | Left bar = Sempre | Corta 8 px independentemente de R0 bit 5, apenas na apresentação |
| UI-V04 | Overscan = Desativado | Mostra apenas a área ativa |
| UI-V05 | Overscan = Superior/Inferior | Mostra bordas verticais e preserva largura ativa |
| UI-V06 | Overscan = Completo 284 | Mostra área ativa + bordas laterais visíveis conforme geometria do perfil |
| UI-V07 | Overscan = Completo 320 | Preset de compatibilidade, offsets provenientes dos metadados do frame |
| UI-V08 | Raw capture | Exporta pixels brutos + active/visible rect, revisão, standard, R0/R7 e timestamp |
| UI-V09 | Proporção | Crop, escala e aspect ratio não alteram pixels, registradores ou timeline |

## 9. Gate de classificação

Para cada suíte e cenário:
- **PASS:** executado no SHA declarado, resultado integral aprovado.
- **FAIL:** executado e divergente.
- **BLOCKED:** não pode ser executado por dependência/ambiente/funcionalidade ausente; registrar motivo.
- **NOT RUN:** ainda não executado.
- **SKIPPED:** nunca pode ser agregado como PASS.

Nenhuma evidência de documentação ou comparação substitui a execução do teste correspondente. Nenhum resultado em VDP/PSG funcional substitui a validação temporal. Nenhum smoke test de Qt substitui a comparação de raster/áudio.
