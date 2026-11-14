# SMS VDP — estado funcional e limites conhecidos

O Master System possui um VDP próprio, distinto do TMS9918A do SG-1000. O runtime Rust mantém estado de VRAM/CRAM, portas, registradores, modos de vídeo, scroll, sprites, status e interrupções conforme as funcionalidades implementadas.

## Arquitetura-alvo e migração

O pacote compartilhado `sega-video` reside em `native/video_core`. Ele hospeda módulos separados por ASIC/família: SMS VDP em `sega_video::sms`; o TMS9918A do SG-1000 permanece um chip distinto e não compartilha estado, registradores ou pipeline com o SMS VDP. Utilitários comuns só serão extraídos quando a equivalência estiver demonstrada.

`sega-video-runtime` continua responsável pelo transporte de frames e integração com apresentação host; não contém a lógica do chip. O contrato de vídeo não passa pela Qt/Python: o framebuffer permanece no backend Rust e a apresentação futura deve usar um host gráfico nativo. O contrato de frame carrega geometria/timestamps descritivos quando conhecidos, sem se tornar autoridade temporal.

A sequência de migração é:
1. SMS1/315-5124 como primeiro perfil de hardware, com gates T0/T0.1.
2. SMS2/315-5246 como perfil comparativo posterior.
3. Migrar o renderer funcional existente para o crate compartilhado, preservando seus testes como regressão; não manter dois engines SMS paralelos.
4. Integrar o dispositivo ao Scheduler por spans/eventos na menor granularidade observável comprovada.

Estado desta migração: os tipos de perfil e o renderer funcional foram movidos para `sega-video::sms`; `sms_core::vdp` agora é uma camada de reexportação compatível. A fundação duplicada `vdp_sms1.rs` foi arquivada em `legacy/rust/sms_vdp1_prototype.rs`; seus vetores exclusivos de máscaras R5/R6, zoom, colisão e prioridade foram migrados para os testes do engine compartilhado. O PSG temporal e o VDP SMS1 scanline-level estão conectados à `SmsTemporalMachine`: o VDP tem domínio próprio no Scheduler, as portas de dados/controle/status e H/V counter passam pelo barramento temporal, e o estado do VDP acompanha snapshot/restore em memória. Essa integração entrega frames funcionais, mas não conclui o VDP temporal: a granularidade interna continua scanline-level, e o timing legado não foi validado fisicamente.

## Comportamento implementado

- VRAM de 16 KiB e CRAM com paleta SMS.
- Portas de dados/controle, command latch, read-ahead, wrap de endereço e decodificação espelhada.
- Mode 4: tiles 4bpp, flips, paletas, prioridade, scroll, máscara da coluna esquerda, tabela de sprites, limites, overflow e colisão.
- Quirk de zoom SMS1/315-5124: com N sprites selecionados, os primeiros N−4 recebem zoom horizontal 2x; os últimos quatro recebem apenas zoom vertical. Para N≤4, nenhum recebe zoom horizontal. SMS2/GG aplicam zoom horizontal a todos os sprites permitidos.
- Conversão RGB CRAM por revisão: SMS1/315-5124 usa níveis comparativos R/G `{0,78,160,238}` e B `{0,98,160,238}`; SMS2 permanece no mapeamento anterior até sua auditoria dedicada. Os níveis SMS1 não equivalem a uma calibração física completa do caminho analógico.
- Máscaras de endereço SMS1/315-5124 em Mode 4: R2 bit 0 mascara o bit alto da linha da name table; R3 bits 7–0 mascaram os bits altos dos fetches de planes 0/1; R4 bits 2–0 mascaram os bits altos dos fetches de planes 2/3. SMS2/GG não aplicam essas máscaras.
- Modos legados compatíveis com TMS: Graphics I, Text, Multicolor e Graphics II.
- Perfis de raster NTSC/PAL e modos de altura suportados pelo runtime.
- VBlank, contador de linha/IRQ, portas H/V counter e integração atual com a CPU.
- Opção de compatibilidade para desenhar sprites além do limite, preservando o status OVERFLOW.

A lista é um resumo funcional, não uma certificação de todas as revisões de ASIC ou de todos os jogos. Consulte o código e os testes no SHA atual antes de alterar o VDP.

## Granularidade temporal

A composição continua scanline-level, com granularidade subline seletiva para efeitos observáveis: latch de R8 em HPos 21, latch de H-counter em transições TH ascendentes e commit de spans antes de escritas visíveis no perfil SMS1. Na máquina temporal, o domínio VDP avança em paralelo ao Z80/PSG e as portas são aplicadas no boundary de conclusão do barramento. Isso fornece uma sequência determinística para esses efeitos, mas não é dot-accurate, não arbitra fetches por dot e não reproduz todos os efeitos mid-scanline.

No SMS1/315-5124, o cursor de rasterização usa o modelo comparativo de 228 T-states / 342 HPos por linha e início ativo em HPos 63. Antes de uma escrita CRAM, VRAM ou dos registradores R0-R7, o renderer compõe apenas o trecho anterior ao feixe com o estado antigo; o restante é composto no próximo boundary ou no fechamento da scanline com o estado atualizado. O cursor horizontal, a colisão pendente e o overflow/index são preservados entre spans e nos snapshots temporais (bus v18 / machine v31). O Scheduler continua fornecendo T-states; o VDP subdivide somente esse avanço nos boundaries internos HPos 24, HPos 26 quantizado e HPos 319, sem criar relógio independente. A leitura do status antes de HBlank não reconhece uma colisão ainda pendente; depois do latch de HBlank, retorna e limpa a flag.

Essa estratégia é deliberadamente coarse e seletiva. Para R1-R7, o commit no boundary da escrita é um modelo de visibilidade, não um timestamp de latch físico medido por registrador. R5 não reavalia a SAT já armazenada para a linha corrente. Overflow é latched no evento HPos 24 (T16); HINT usa HPos 26 quantizado para T18/HPos 27; o início de VBlank e VINT flag usa HPos 24 (T16) da primeira linha VBlank; o limite ativo/HBlank é modelado em HPos 319 (T213), quando o restante da linha é composto e a colisão pendente é transferida ao status. O HPos 26 é quantizado à granularidade T-state e fica um HPos além do valor comparativo; HBlank HPos 319 é o boundary de fim de área ativa do modelo, não uma medição elétrica independente. Ainda não são modelados o CRAM dot relatado em escritas durante active display, o instante físico de latch da paleta, fetch-ahead de tiles/bitplanes/padrões de sprites ou timestamps individuais dos fetches SAT/X/tile/pattern. Colisão é detectada pela composição de pixels opacos e publicada no status em HBlank, não no dot exato de colisão.

A precisão temporal deve ser priorizada para efeitos de gameplay sensíveis ao instante: escrita de registrador durante a linha, mudança de scroll, H/V counter, IRQ ou comportamento de sprite que jogos realmente utilizem. Para o restante, manter a composição funcional atômica/por scanline e duração nominal correta. Se o efeito puder ser reproduzido por scanline ou evento, não criar pipeline por pixel ou evento por dot apenas por completude teórica.

No perfil SMS1/315-5124, a avaliação Mode 4 da SAT é armazenada em `Mode4SpriteLineBuffer` e a avaliação legacy/TMS em `LegacySpriteLineBuffer`, ambas preparadas para a scanline seguinte no fechamento da anterior. Mode 4 captura X, tile, linha-fonte, shift de R0 e seleção/gates de R6; legacy/TMS captura X com early-clock shift, pattern, color, linha-fonte e R6. Os bytes dos bitplanes/padrões continuam sendo lidos durante a composição dos spans. Overflow só é publicado no evento HPos 24 da linha selecionada, usando o buffer correspondente ao modo ativo nesse boundary. O caminho SMS2 permanece sem buffer persistente nesta etapa. A colisão detectada durante a composição permanece pendente até HBlank HPos 319; uma leitura de status anterior ao boundary não a limpa.

## Limites conhecidos

- R8 agora usa latch em HPos 21 (T14 no perfil de 228 T-states/342 posições) e o H-counter usa latch persistente por TH ascendente. O modelo é confrontado com MAME 315-5124 e a ROM de diagnóstico SmsVdpTest, mas ainda não foi executado em hardware SMS1/SMS2; a fase absoluta no reset permanece pendente de validação física. Outros registradores e fetches mid-scanline continuam aproximados.
- A rasterização seletiva SMS1 cobre boundaries de escrita CRAM, R7 e VRAM no intervalo comparativo HPos 63..319 (319 exclusivo). R0-R7 usam commit seletivo antes da escrita. O renderer legacy/TMS aceita spans. Overflow, HINT, VBlank e HBlank agora têm boundaries internos quantizados a T-states; HINT HPos 26 é arredondado para T18/HPos27. Isso ainda não equivale a um pipeline de dots nem a timestamps físicos medidos. R8-R10 permanecem fora do commit genérico (R8/R9 mantêm seus latches específicos e R10 seu contador de linha); fetch-ahead e latches específicos continuam aproximados. A escrita VRAM atual afeta a composição do sufixo, mas não representa o pipeline interno de fetch do ASIC.
- CRAM dots durante active display são documentados em relatos técnicos do SMS Power; o artefato de pixel transitório e o latch efetivo da paleta não são simulados nem considerados resolvidos.
- No SMS1, Mode 4 e legacy/TMS possuem buffers separados de metadados SAT por scanline. Overflow é publicado em HPos 24, HINT em HPos 26 quantizado para T18, VBlank em HPos 24 da primeira linha VBlank e collision é publicada em HBlank HPos 319. Os eventos seguem o avanço do Scheduler, mas a quantização não é dot-accurate; o dot exato da colisão e os fetches de padrão continuam não modelados.
- Diferenças específicas entre SMS1/315-5124 e SMS2/315-5246 precisam ser tratadas quando houver evidência e impacto em software.
- Modos de 224/240 linhas precisam respeitar o perfil de hardware que os suporta.
- A conversão CRAM é digital; não representa a cadeia analógica completa de vídeo.
- Light Phaser e SegaScope usam adaptações de entrada/apresentação; não simulam integralmente sensores e temporização elétrica originais.

Essas limitações devem ser priorizadas por compatibilidade observada e testes reproduzíveis, não pela busca abstrata de dot-accuracy.

## Latches horizontais R8 e H-counter

- R8 é copiado para a nova linha e pode atualizar o latch da linha corrente enquanto `HPos <= 21` (inclusive). Com 342 posições por linha e 228 Z80 T-states, o limite corresponde a T14. Escritas posteriores só afetam a próxima linha.
- O port `$7F` retorna o último H-counter capturado, não um contador livre. A implementação atual detecta saída-baixa → entrada-alta no write do I/O control `$3F` para TH1/TH2. Eventos de borda TH externos provenientes de periféricos ainda não estão ligados ao barramento.
- O Scheduler avança o VDP até o timestamp de conclusão do ciclo antes de aplicar o efeito da escrita de porta. Assim, os latches usam o instante real do boundary temporal, sem callbacks por instrução em Python.
- Esse caminho cobre apenas a borda criada por software via `$3F`. Eventos TH externos de periféricos ainda não são eventos tipados do Scheduler. O Light Phaser do VDP legado calcula um H-counter sintético a partir de X por scanline; isso não equivale à captura temporal por borda TH. Ver [auditoria de eventos TH externos](../audits/sms-external-th-events-2026-11-06.md).
- Esse caminho cobre apenas a borda criada por software via `$3F`. Eventos TH externos de periféricos ainda não são eventos tipados do Scheduler. O Light Phaser do VDP legado calcula um H-counter sintético a partir de X por scanline; isso não equivale à captura temporal por borda TH. Ver [auditoria de eventos TH externos](../audits/sms-external-th-events-2026-11-06.md).
- Evidências, derivação de HPos/T-state, vetores e limitações físicas: [auditoria R8/H-counter de 2026-11-06](../audits/sms-vdp-r8-hcounter-temporal-latches-2026-11-06.md).

## Referências

- SMS Power — VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power — Modes: https://www.smspower.org/Development/Modes
- SMS Power — Sprites: https://www.smspower.org/Development/Sprites
- SMS Power — VDP index: https://www.smspower.org/Development/VDP-Index
- SMS Power Forums — Writing to VDP and VRAM outside VBLANK (relato de CRAM dots): https://www.smspower.org/forums/18738-WritingToVDPAndVRAMOutsideVBLANK
- Sega Master System VDP documentation: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
- MAME 315_5124: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
