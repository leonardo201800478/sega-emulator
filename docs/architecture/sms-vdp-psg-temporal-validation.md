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

## 2. VDP — matriz funcional SMS1/315-5124

A primeira matriz executável é exclusivamente para SMS1 com ASIC 315-5124. SMS2/315-5246 terá uma matriz comparativa posterior. As expectativas funcionais abaixo não fixam timestamps, fase, fetch slot ou prioridade sublinha.

| ID | Área | Vetor / cenário | Resultado funcional exigido | Evidência / limite |
|---|---|---|---|---|
| VDP-F01 | Portas | Data/control ports nos espelhos documentados | Decodificação correta dos espelhos, sem aliases inventados | DOC |
| VDP-F02 | Control word | Primeiro byte pendente; segundo byte em separado | Palavra de controle decodifica code/address; ação ocorre no segundo byte | DOC + TEST |
| VDP-F03 | Read-ahead | Programar VRAM read, ler sequência e atravessar $3FFF | Buffer retorna byte anterior, prefetch e wrap corretos | DOC + TEST |
| VDP-F04 | Status | Flags VBlank/overflow/collision pendentes; ler $BF duas vezes | Primeira leitura retorna e limpa flags definidas; segunda não repete flags reconhecidas | PRIMARY_DOC + TEST; collision bit 5/overflow bit 6 é baseline documental, hardware oracle pendente |
| VDP-F05 | VRAM | Acessar limites e endereços com máscaras R2–R6 | 16 KiB e gates de endereço documentados do 315-5124 | DOC + PROPERTY |
| VDP-F06 | CRAM | Escrever valores 0x00..0xFF em índices conhecidos | 32 bytes; apenas seis bits digitais de cor são armazenados; alias de endereço correto | DOC + TEST; não valida DAC analógico |
| VDP-F07 | R0 bit 5 | Padrões de background/sprite nos primeiros 8 pixels ativos | Coluna esquerda é substituída por backdrop quando habilitada | DOC + TEST |
| VDP-F08 | R0 bit 3 | Sprite em X=0/1 com bit 3 em 0/1 | Deslocamento horizontal de sprites de -8 pixels; independente da máscara bit 5 | DOC + TEST |
| VDP-F09 | R0 bits 5+3 | Combinar máscara e sprite shift | Efeitos permanecem independentes; sem deslocamento duplicado | TEST |
| VDP-F10 | R7 | Alterar backdrop e sprite palette entries | Backdrop/área mascarada usa a cor definida por R7 conforme o modo | DOC + TEST |
| VDP-F11 | R0 bit 6 | Scroll horizontal nas primeiras 16 linhas e restante | Horizontal scroll lock nas linhas superiores conforme o registrador | DOC + TEST |
| VDP-F12 | R0 bit 7 | Scroll vertical nas colunas 24–31 e demais | Vertical scroll lock nas colunas da direita conforme o registrador | DOC + TEST |
| VDP-F13 | R8 | Valores 0..255 em testes funcionais por linha | Fine/coarse scroll, wrap e coordenadas funcionais corretas | DOC + TEST; aplicação intralinha UNKNOWN |
| VDP-F14 | R9 | Valores 0..255 e alterações antes/durante área efetiva | Semântica de scroll vertical e condição funcional descrita no manual | PRIMARY_DOC + TEST; fronteira exata UNKNOWN |
| VDP-F15 | R10 | Down-counter com valores conhecidos | Contagem/reload funcional do line counter e habilitação H-Line IRQ | PRIMARY_DOC + TEST; fase de assert UNKNOWN |
| VDP-F16 | Background | Tiles, paletas, flips, prioridade, transparência e scroll | Pixel funcional correto para estado estático de linha | DOC + TEST |
| VDP-F17 | Sprites | 64 SAT entries, buffer de até 8 por linha, 8x8/8x16, prioridade | Seleção/composição funcional segundo elegibilidade SAT | DOC + TEST |
| VDP-F18 | Overflow | 7/8/9+ sprites elegíveis; incluir transparentes e X fora da área útil | Overflow reflete sprites adicionais elegíveis, não só pixels desenhados | COMMUNITY_DOC + TEST; fase UNKNOWN |
| VDP-F19 | Collision | Pixels opacos sobrepostos; repetir com transparência | Collision somente quando pixels opacos de sprites se sobrepõem | DOC + TEST; primeira posição/clock UNKNOWN |
| VDP-F20 | VBlank | Status antes/depois da transição funcional para VBlank | Flag e clear por leitura corretos; não fixar timestamp de linha sem oracle | DOC + STATE_TRANSITION |
| VDP-F21 | Altura | Perfil 315-5124 | Somente modo de 192 linhas no primeiro perfil; 224/240 não habilitados | DOC + TEST |
| VDP-F22 | SMS1 masks | Variar individualmente R2–R6 | Geração de endereços segue gates AND documentados do 315-5124 | COMMUNITY_DOC + TEST; efeitos secundários não testados permanecem fora |
| VDP-F23 | SMS2 | 315-5246 | Fora do primeiro gate; implementar depois em perfil/testes independentes | BLOCKED / FUTURE |
| VDP-F24 | V-counter | Sequências de 192 linhas NTSC/PAL | Valores funcionais apenas quando sustentados por tabela SMS1 específica | COMMUNITY_DOC; PARTIAL |
| VDP-F25 | H-counter/TH | Ler $7E/$7F e variar TH | Contrato/offset/latch SMS1 ainda UNKNOWN; não fixar valores | BLOCKED até T0 hardware/oracle |
| VDP-F26 | Raster writes | R0/R1/R7/R8/R9, VRAM e CRAM durante active display | Sem expectativa de pixel/timestamp até evidência independente | BLOCKED |
| VDP-F27 | Reset | Power-on/reset funcional com memória/latches conhecidos | Estado inicial definido por perfil e teste; não assumir fase de clock | DOC + TEST; fase UNKNOWN |
| VDP-F28 | Save state | Snapshot em estado funcional estável e depois em linha ativa quando suportado | Restore reproduz exatamente o estado serializado e a execução subsequente | TEST; não prova fidelidade temporal |
| VDP-F29 | CRAM/DAC | Todos os 64 códigos de cor digitais | CRAM digital correto; conversão RGB comparativa não é hardware PASS | DOC + TEST; colorimetria física UNKNOWN |
| VDP-F30 | Overscan/raster geometry | Capturas e retângulos por perfil | Não publicar offsets/áreas de borda como hardware sem fonte/medição SMS1 | BLOCKED até evidência |
| VDP-F31 | Sprite evaluation | 7/8/9/10 sprites elegíveis, SAT de 64 entradas | Avaliação durante uma linha seleciona sprites da linha seguinte; overflow conta elegíveis | COMMUNITY_DOC + TEST; fase UNKNOWN |
| VDP-F32 | Collision + left mask | Colisão opaca em x=0..7 com R0 bit 5 em 0/1 | Hipótese MAME de supressão de colisão na coluna mascarada permanece investigativa | BLOCKED até hardware/oracle |
| VDP-F33 | SMS1 zoom | Grupos de 2/4/6/8 sprites com padrões assimétricos | Zoom vertical aplica-se a todos; zoom horizontal não se aplica aos últimos quatro sprites encontrados | COMMUNITY_DOC + TEST |
| VDP-F34 | Fine-scroll left edge | R8=1..7 e padrões de borda | Comportamento de primeiros pixels é hipótese diferencial; não fixar sem vetor SMS1 independente | BLOCKED / RESEARCH |
| VDP-F35 | Sprite limit option | HardwareLimit e UnlockedCompatibility | HardwareLimit é padrão; modo sem limite altera apenas política de desenho, não overflow/status | TEST |
| VDP-F36 | Frame metadata | Frame com active/visible rect, aspect ratio, timestamp e masked regions | Metadados são validados e não alteram pixels/estado de hardware | TEST; geometria SMS específica só com perfil validado |

### 2.1 Overscan, blanking e origem do viewport

A matriz distingue:
- **Active-area coordinate:** coordenada 0..255 da imagem ativa do VDP.
- **Raster coordinate:** posição do feixe/contador, somente quando o perfil tiver evidência.
- **Presentation coordinate:** coordenada após crop, escala e proporção do frontend.

Para o SMS1/315-5124, a geometria completa de overscan, offsets de borda e número de posições horizontais ainda não está fixada pela evidência T0. Não importar 342 dots/171 H-counts/228 CPU clocks da documentação Game Gear. Também não promover 228 T-states/linha do renderer legado a fato físico.

**Regra visual da coluna esquerda:**
- R0 bit 5 substitui os primeiros 8 pixels da área ativa pela cor de backdrop;
- R0 bit 3 desloca sprites 8 pixels à esquerda;
- os dois bits são independentes;
- o crop é uma transformação do viewport, nunca alteração do framebuffer bruto, VDP ou save state;
- Auto só corta quando a região está mascarada em toda a altura ativa;
- Full 284/Full 320 permanecem presets candidatos; só podem ser habilitados quando o perfil fornecer retângulos de viewport sustentados por fonte/medição SMS1.

### 2.2 Comparação independente e limites das referências

- **MAME** é EMULATOR_SOURCE, não oráculo físico. A implementação separa 315-5124/315-5246 e serve para gerar hipóteses sobre sprite evaluation, máscaras, DAC e raster. Não promove offsets/fases desconhecidos a fatos.
- O código MAME sugere supressão de colisões na coluna zero mascarada e descreve comportamento especial dos primeiros pixels em fine-scroll. Ambos permanecem hipóteses diferenciais até teste SMS1 independente.
- As tabelas RGB MAME (315-5124 e 315-5246) são comparação de software; a colorimetria física do DAC SMS1 permanece UNKNOWN.
- **Gearsystem** separa overscan de Hide Left Bar e oferece Auto condicionado ao left-column masking. É referência de UX, não fonte de geometria/timing SMS1.
- SMS Power VDP Registers, Sprites, Control Port e SMS Official Docs são fontes documentais com classes/proveniência registradas no T0/T0.1. A discrepância editorial dos bits de collision deve continuar isolada.

Fontes comparativas:
- MAME VDP: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Gearsystem Video: https://github.com/drhelius/Gearsystem/blob/master/src/Video.cpp
- SMS Power VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power Sprites: https://www.smspower.org/Development/Sprites
- SMS Official Docs: https://www.smspower.org/Development/SMSOfficialDocs

## 3. VDP — matriz temporal e bloqueios T0

A regra T0.1 é obrigatória: todo teste que dependa de posição horizontal, fase, fetch slot, WAIT ou desempate simultâneo permanece BLOCKED sem evidência apropriada. Determinismo não comprova fidelidade temporal.

| ID | Evento | Estado / critério |
|---|---|---|
| VDP-T01 | Clock nominal | SMS1 service manual: 10.738635 MHz VDP e 3.579545 MHz CPU, razão nominal 3:1; fase inicial/duty/offset UNKNOWN |
| VDP-T02 | Linha/frame | Contagem e fronteiras NTSC/PAL SMS1 ainda PARTIAL; não usar 228 T-states/linha do legado como oracle |
| VDP-T03 | V-counter 192 | Sequências regionais SMS1 precisam de vetor independente; manter PARTIAL até confirmação |
| VDP-T04 | H-counter/TH latch | UNKNOWN; offsets, wrap e instante de latch BLOCKED |
| VDP-T05 | HBlank/active display | UNKNOWN em fase precisa; não inferir de descrição aproximada |
| VDP-T06 | R8 intralinha | BLOCKED até captura/oracle que determine a primeira posição afetada |
| VDP-T07 | R9 latch | Semântica funcional documentada; fronteira exata UNKNOWN |
| VDP-T08 | R10/HINT | Down-counter e função documentados; fase de assert/reload e amostragem Z80 UNKNOWN |
| VDP-T09 | Sprite evaluation | Regra funcional de seleção para linha seguinte documentada; offsets de SAT read/buffer close UNKNOWN |
| VDP-T10 | Pattern/VRAM/CRAM fetch | Fetch slots e prioridade de write simultâneo UNKNOWN |
| VDP-T11 | VRAM access cadence | 16 T-states em VBlank/29 em active video é cadência documentada de software; não prova WAIT automático |
| VDP-T12 | IRQ/CPU sample | Fase de INT e amostragem Z80 UNKNOWN; tie-break simultâneo BLOCKED |
| VDP-T13 | Save/restore | Preservar estado temporal que esteja efetivamente implementado; não serializar fases hipotéticas |
| VDP-T14 | Determinismo | Avanços contínuos/particionados equivalentes produzem mesmo estado/eventos; não prova hardware timing |
| VDP-T15 | Dot/pixel pipeline | Não é requisito universal; só implementar se uma dependência observável e evidência justificarem a granularidade |
| VDP-T16 | WAIT/contention | BLOCKED até trace de barramento provar qual transação recebe stall |
| VDP-T17 | Sprite collision first pixel | BLOCKED; condição funcional documentada, instante de assert UNKNOWN |
| VDP-T18 | Midline writes | BLOCKED até teste SMS1 que determine primeira posição/pixel afetado |

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
| PSG-F13 | Digital mixer | Canais isolados e somados | Mix digital determinístico e escala explícita; não confundir com circuito de saída da placa | TEST |
| PSG-F14 | Sampling | 44.1/48/96 kHz e clocks NTSC/PAL | Sample count/phase exatos sem comandar o chip pelo host | MODEL + TEST |
| PSG-F15 | Filter | FIR history/position e continuidade em blocos | Sem descontinuidade ao mudar o tamanho do bloco; filtro identificado como output stage, não circuito analógico físico | TEST |
| PSG-F16 | Save state | Durante tom, ruído e fase intermediária do divisor | Restore reproduz exatamente a sequência digital e amostras futuras | TEST |
| PSG-F17 | Board output circuit | Medir os 16 níveis de volume e combinações de canais em placas SMS1/SMS2 identificadas | Perfil de saída é separado do SN76489 digital; variante de resistor só é habilitada com curva medida; não inferir apenas pelo ASIC | BLOCKED até medição |
| PSG-F18 | PCM/DC response | Reproduzir padrões PCM por volume writes e capturar saída de placa | Qualquer decay/RC ou resposta não linear precisa de parâmetros medidos e teste independente; não adicionar circuito analógico genérico | BLOCKED até medição |

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
| PSG-T11 | Shared clock | Domínios avançam com razão racional; relação nominal 3:1 VDP:CPU/PSG no SMS1 NTSC não fixa fase inicial |
| PSG-T12 | Board output stage | Perfil de circuito processa samples já gerados pelo chip; estado/curva é determinístico e serializável, sem retroagir no clock digital do PSG. BLOCKED até perfil medido |

## 6. Clock profile — itens que precisam de reconciliação

| Perfil | Fonte inicial | Frequência | Estado |
|---|---|---:|---|
| SMS1 NTSC VDP/IC5 pino 30 | Manual de serviço SMS | 10.738635 MHz ±100 Hz | PRIMARY_DOC nominal; fase UNKNOWN |
| SMS1 NTSC CPU/PSG/IC5 pino 34 | Manual de serviço SMS | 3.579545 MHz ±30 Hz | PRIMARY_DOC nominal |
| Relação VDP:CPU/PSG | Valores nominais do manual | 3:1 aritmética | DOC; não determina fase |
| SMS1 PAL CPU/PSG | Fontes de serviço/comunitárias | 3.546893 MHz reportado | PARTIAL; reconciliar placa/revisão |
| SMS1 PAL VDP | Fontes de serviço/comunitárias | 10.640684 MHz reportado | PARTIAL; confirmar placa/divisor |
| PAL-M | Perfil regional | Não assumir equivalência completa a PAL nem NTSC | BLOCKED até matriz de placas/fontes |

A constante legada 3_546_895 Hz e o valor reportado 3_546_893 Hz diferem em 2 Hz; não classificar automaticamente como bug. Reconciliar placa, revisão, oscilador e divisor. Não importar clocks/contadores de Game Gear. Nenhum clock nominal fixa fase inicial, H-counter offset, fetch slot ou prioridade de evento.

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

O engine compartilhado sega-video produz pixels e metadados; sega-video-runtime transporta frames e não implementa o VDP. O frame pode transportar active_rect, visible_rect, proporção, timestamp/timebase, regiões mascaradas e raster metadata quando conhecido. Metadados de raster desconhecido devem permanecer ausentes, não preenchidos por constantes de outro console.

| ID | Opção | Critério |
|---|---|---|
| UI-V01 | Left bar = Não | Preserva os pixels ativos, inclusive a coluna mascarada |
| UI-V02 | Left bar = Auto (padrão) | Corta 8 px somente se todos os scanlines ativos tiverem R0 bit 5 aplicado e o viewport não estiver exibindo overscan completo |
| UI-V03 | Left bar = Sempre | Corta 8 px independentemente de R0 bit 5, apenas na apresentação |
| UI-V04 | Overscan = Desativado | Mostra apenas active_rect |
| UI-V05 | Overscan = Superior/Inferior | Só habilitar quando retângulo vertical do perfil estiver validado |
| UI-V06 | Overscan = Full 284 | Preset candidato; habilitar somente quando retângulo 284 for sustentado para o ASIC/standard |
| UI-V07 | Overscan = Full 320 | Preset candidato; habilitar somente quando retângulo 320 for sustentado para o ASIC/standard |
| UI-V08 | Raw capture | Exporta pixels brutos + active/visible rect, perfil ASIC/standard e timestamp |
| UI-V09 | Proporção | Crop, escala e aspect ratio não alteram pixels, registradores ou timeline |

## 9. Gate de classificação

Para cada suíte e cenário:
- **PASS:** executado no SHA declarado, resultado integral aprovado.
- **FAIL:** executado e divergente.
- **BLOCKED:** não pode ser executado por dependência/ambiente/funcionalidade ausente; registrar motivo.
- **NOT RUN:** ainda não executado.
- **SKIPPED:** nunca pode ser agregado como PASS.

Nenhuma evidência de documentação ou comparação substitui a execução do teste correspondente. Nenhum resultado em VDP/PSG funcional substitui a validação temporal. Nenhum smoke test de Qt substitui a comparação de raster/áudio.
