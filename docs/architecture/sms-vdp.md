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
- Modos legados compatíveis com TMS: Graphics I, Text, Multicolor e Graphics II.
- Perfis de raster NTSC/PAL e modos de altura suportados pelo runtime.
- VBlank, contador de linha/IRQ, portas H/V counter e integração atual com a CPU.
- Opção de compatibilidade para desenhar sprites além do limite, preservando o status OVERFLOW.

A lista é um resumo funcional, não uma certificação de todas as revisões de ASIC ou de todos os jogos. Consulte o código e os testes no SHA atual antes de alterar o VDP.

## Granularidade temporal

A composição continua scanline-level, mas o caminho temporal agora modela dois boundaries subline observáveis: o latch de R8 em HPos 21 e o latch de H-counter em transições TH ascendentes. Na máquina temporal, o domínio VDP avança em paralelo ao Z80/PSG e as portas são aplicadas no boundary de conclusão do barramento. Isso fornece uma sequência determinística para esses efeitos, mas não é dot-accurate, não arbitra fetches por dot e não reproduz todos os efeitos mid-scanline.

A precisão temporal deve ser priorizada para efeitos de gameplay sensíveis ao instante: escrita de registrador durante a linha, mudança de scroll, H/V counter, IRQ ou comportamento de sprite que jogos realmente utilizem. Para o restante, manter a composição funcional atômica/por scanline e duração nominal correta. Se o efeito puder ser reproduzido por scanline ou evento, não criar pipeline por pixel ou evento por dot apenas por completude teórica.

## Limites conhecidos

- R8 agora usa latch em HPos 21 (T14 no perfil de 228 T-states/342 posições) e o H-counter usa latch persistente por TH ascendente. O modelo é confrontado com MAME 315-5124 e a ROM de diagnóstico SmsVdpTest, mas ainda não foi executado em hardware SMS1/SMS2; a fase absoluta no reset permanece pendente de validação física. Outros registradores e fetches mid-scanline continuam aproximados.
- Diferenças específicas entre SMS1/315-5124 e SMS2/315-5246 precisam ser tratadas quando houver evidência e impacto em software.
- Modos de 224/240 linhas precisam respeitar o perfil de hardware que os suporta.
- A conversão CRAM é digital; não representa a cadeia analógica completa de vídeo.
- Light Phaser e SegaScope usam adaptações de entrada/apresentação; não simulam integralmente sensores e temporização elétrica originais.

Essas limitações devem ser priorizadas por compatibilidade observada e testes reproduzíveis, não pela busca abstrata de dot-accuracy.

## Latches horizontais R8 e H-counter

- R8 é copiado para a nova linha e pode atualizar o latch da linha corrente enquanto `HPos <= 21` (inclusive). Com 342 posições por linha e 228 Z80 T-states, o limite corresponde a T14. Escritas posteriores só afetam a próxima linha.
- O port `$7F` retorna o último H-counter capturado, não um contador livre. A implementação atual detecta saída-baixa → entrada-alta no write do I/O control `$3F` para TH1/TH2. Eventos de borda TH externos provenientes de periféricos ainda não estão ligados ao barramento.
- O Scheduler avança o VDP até o timestamp de conclusão do ciclo antes de aplicar o efeito da escrita de porta. Assim, os latches usam o instante real do boundary temporal, sem callbacks por instrução em Python.
- Evidências, derivação de HPos/T-state, vetores e limitações físicas: [auditoria R8/H-counter de 2026-11-06](../audits/sms-vdp-r8-hcounter-temporal-latches-2026-11-06.md).

## Referências

- SMS Power — VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power — Modes: https://www.smspower.org/Development/Modes
- SMS Power — Sprites: https://www.smspower.org/Development/Sprites
- SMS Power — VDP index: https://www.smspower.org/Development/VDP-Index
- Sega Master System VDP documentation: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
- MAME 315_5124: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
