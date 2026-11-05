# SMS VDP — estado funcional e limites conhecidos

O Master System possui um VDP próprio, distinto do TMS9918A do SG-1000. O runtime Rust mantém estado de VRAM/CRAM, portas, registradores, modos de vídeo, scroll, sprites, status e interrupções conforme as funcionalidades implementadas.

## Arquitetura-alvo e migração

O pacote compartilhado `sega-video` reside em `native/video_core`. Ele hospeda módulos separados por ASIC/família: SMS VDP em `sega_video::sms`; o TMS9918A do SG-1000 permanece um chip distinto e não compartilha estado, registradores ou pipeline com o SMS VDP. Utilitários comuns só serão extraídos quando a equivalência estiver demonstrada.

`sega-video-runtime` continua responsável pelo transporte de frames e integração com apresentação host; não contém a lógica do chip. O contrato de frame carrega geometria/timestamps descritivos quando conhecidos, sem se tornar autoridade temporal.

A sequência de migração é:
1. SMS1/315-5124 como primeiro perfil de hardware, com gates T0/T0.1.
2. SMS2/315-5246 como perfil comparativo posterior.
3. Migrar o renderer funcional existente para o crate compartilhado, preservando seus testes como regressão; não manter dois engines SMS paralelos.
4. Integrar o dispositivo ao Scheduler por spans/eventos na menor granularidade observável comprovada.

Estado desta migração: os tipos de perfil SMS estão sendo extraídos para `sega-video`; o renderer funcional ainda permanece em `sms_core/src/vdp.rs` e a fundação parcial em `vdp_sms1.rs` ainda não foi consolidada. O PSG temporal já foi integrado, mas o VDP ainda não está conectado à `SmsTemporalMachine`.

## Comportamento implementado

- VRAM de 16 KiB e CRAM com paleta SMS.
- Portas de dados/controle, command latch, read-ahead, wrap de endereço e decodificação espelhada.
- Mode 4: tiles 4bpp, flips, paletas, prioridade, scroll, máscara da coluna esquerda, tabela de sprites, limites, overflow e colisão.
- Modos legados compatíveis com TMS: Graphics I, Text, Multicolor e Graphics II.
- Perfis de raster NTSC/PAL e modos de altura suportados pelo runtime.
- VBlank, contador de linha/IRQ, portas H/V counter e integração atual com a CPU.
- Opção de compatibilidade para desenhar sprites além do limite, preservando o status OVERFLOW.

A lista é um resumo funcional, não uma certificação de todas as revisões de ASIC ou de todos os jogos. Consulte o código e os testes no SHA atual antes de alterar o VDP.

## Granularidade temporal

A implementação atual usa o modelo de raster e os pontos de integração que estão presentes no runtime. Isso não é dot-accurate nem pretende ser uma simulação elétrica do ASIC.

A precisão temporal deve ser priorizada para efeitos de gameplay sensíveis ao instante: escrita de registrador durante a linha, mudança de scroll, H/V counter, IRQ ou comportamento de sprite que jogos realmente utilizem. Para o restante, manter a composição funcional atômica/por scanline e duração nominal correta. Se o efeito puder ser reproduzido por scanline ou evento, não criar pipeline por pixel ou evento por dot apenas por completude teórica.

## Limites conhecidos

- Efeitos de registrador no meio da scanline e certas fases do H-counter ainda podem ser aproximados.
- Diferenças específicas entre SMS1/315-5124 e SMS2/315-5246 precisam ser tratadas quando houver evidência e impacto em software.
- Modos de 224/240 linhas precisam respeitar o perfil de hardware que os suporta.
- A conversão CRAM é digital; não representa a cadeia analógica completa de vídeo.
- Light Phaser e SegaScope usam adaptações de entrada/apresentação; não simulam integralmente sensores e temporização elétrica originais.

Essas limitações devem ser priorizadas por compatibilidade observada e testes reproduzíveis, não pela busca abstrata de dot-accuracy.

## Referências

- SMS Power — VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power — Modes: https://www.smspower.org/Development/Modes
- SMS Power — Sprites: https://www.smspower.org/Development/Sprites
- SMS Power — VDP index: https://www.smspower.org/Development/VDP-Index
- Sega Master System VDP documentation: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
- MAME 315_5124: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
