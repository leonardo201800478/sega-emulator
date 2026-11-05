# SMS VDP — estado funcional e limites conhecidos

O Master System possui um VDP próprio, distinto do TMS9918A do SG-1000. O runtime Rust mantém estado de VRAM/CRAM, portas, registradores, modos de vídeo, scroll, sprites, status e interrupções conforme as funcionalidades implementadas.

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

A granularidade deve ser refinada quando um efeito observável exigir: por exemplo, uma escrita de registrador durante a linha, uma mudança de scroll, um contador/IRQ ou um efeito de sprite que jogos realmente utilizem. Se o efeito puder ser reproduzido corretamente por scanline ou por evento, não criar um pipeline por pixel ou evento por dot apenas por completude teórica.

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
