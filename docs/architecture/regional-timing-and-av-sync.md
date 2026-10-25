# Perfis regionais, temporização e sincronização audiovisual

## Objetivo

Definir como região, clocks de hardware, temporização de vídeo, áudio digital e apresentação do host se relacionam. Este documento estabelece requisitos para implementação progressiva; não afirma que todos os perfis regionais estejam implementados.

## Política de região

- Padrão da interface: **Automático**.
- Automático usa informações confiáveis do formato/metadata da ROM, cabeçalho interno quando existir, identificadores conhecidos e regras específicas do console. Uma ROM sem evidência suficiente deve produzir uma escolha reportada como incerta, não uma região inventada.
- A seleção manual pode substituir a detecção automática.
- A escolha por jogo pode expor somente regiões/modos que o jogo e o console realmente suportem; não se deve prometer que uma ROM bloqueada regionalmente iniciará sob qualquer perfil.
- Idioma/região comercial e frequência de vídeo são conceitos relacionados em alguns consoles, mas não equivalentes universais. Modelar campos separados quando o hardware os separa.
- Perfil regional faz parte do estado/configuração do núcleo de emulação, não do backend gráfico.

## Perfil de hardware por sistema

Cada perfil deve declarar, com fonte e nível de confiança:

- família e revisão de hardware, quando relevante;
- região linguística/comercial e modo de vídeo;
- clock mestre e divisores de CPU, VDP e chips de áudio;
- linhas por quadro, clocks por linha, área ativa e intervalo vertical;
- comportamento de interrupções, VBlank/HBlank e DMA pertinente;
- taxa nominal de quadros calculada a partir dos clocks, em vez de um número de FPS isolado;
- clocks/divisores de PSG/FM e relações temporais com o relógio mestre;
- restrições e incertezas conhecidas.

Não reutilizar uma única tabela PAL/NTSC para SG-1000, Master System/Game Gear e Mega Drive sem confirmar os detalhes específicos de cada máquina e revisão.

## Velocidade lógica não é refresh do monitor

A emulação deve avançar em tempo de hardware. Para perfis PAL de aproximadamente 50 quadros/s, o intervalo nominal é de 20 ms; para perfis NTSC de aproximadamente 60 quadros/s, cerca de 16,67 ms. Esses números são arredondamentos didáticos: os valores efetivos decorrem dos clocks e divisores reais do console.

Não acelerar o núcleo PAL em cerca de 20% para preencher um monitor fixo de 60 Hz. Isso mudaria velocidade do jogo, temporização das interrupções, duração das notas e pitch. Tampouco se deve desacelerar um jogo NTSC para acompanhar um monitor de 50 Hz.

## VRR/G-SYNC e apresentação

- VRR/G-SYNC pertence à camada de apresentação. O recurso permite ao monitor acompanhar a cadência de quadros dentro da faixa suportada, mas não converte a temporização PAL em NTSC e não corrige clocks errados no núcleo.
- Consultar capacidades reais do backend, driver, monitor e modo de janela; mostrar estado detectado, não apenas uma preferência habilitada.
- Usar pacing de quadros baseado no relógio de emulação e limitar a fila de quadros em voo quando suportado.
- Se a taxa de origem ficar fora da faixa VRR, usar estratégia explícita de fallback (por exemplo, apresentação em múltiplos da cadência ou VSync convencional), sem alterar o tempo lógico do console.
- Separar taxa de quadros emulados, taxa de apresentação e taxa de atualização reportada pelo monitor no diagnóstico.
- Não prometer ausência total de judder ou latência zero sem medições em hardware real.

## Sincronização de áudio

Separar três domínios:

1. **Tempo de hardware:** CPU, VDP e chips de áudio avançam de forma determinística a partir dos clocks e eventos do console.
2. **Geração digital de áudio:** o PSG/FM processa registradores, divisores, osciladores, envelopes e ruído no tempo emulado; a amostragem é derivada desse tempo, com conversão explicitamente definida.
3. **Dispositivo de áudio host:** WASAPI, XAudio2 ou SDL3 consome PCM numa frequência como 44,1 ou 48 kHz. Essa frequência não é um clock histórico do console e não pode reprogramar o PSG/FM.

Requisitos:

- O relógio do console e os eventos de áudio não podem depender da frequência do monitor.
- O número e o instante das amostras produzidas a partir de uma sequência idêntica de eventos devem ser determinísticos.
- Resampling e filtragem de saída são etapas explícitas e testáveis, separadas do modelo do chip.
- Pequenas diferenças entre relógios físicos do host podem ser tratadas no adaptador/buffer de saída; não corrigir deriva alterando a velocidade lógica do jogo ou o pitch do chip.
- Monitorar underruns, overflows e ocupação de buffer; distinguir falha de backend de erro no núcleo de áudio.
- Validar frequências de tom, envelopes, ruído, duração de notas e alinhamento audiovisual em perfis regionais diferentes.
- Não afirmar precisão de áudio por ciclo antes de testes de referência e documentação das incertezas.

## Plano incremental

1. SG-1000: confirmar perfis NTSC e PAL realmente aplicáveis à máquina; formalizar relógios, linhas e evento VBlank.
2. PSG SN76489: validar relógios/divisores e gerar amostras de forma determinística a partir do tempo de hardware.
3. Adaptador PCM e backends: manter saída configurável sem influenciar clocks do núcleo.
4. Master System/Game Gear: perfis regionais próprios, contadores, interrupções e área visível do LCD.
5. Mega Drive: separar região linguística e 50/60 Hz; modelar clocks 68000/Z80, VDP, PSG e YM2612 de acordo com a variante documentada.
6. Apresentação: validar VSync/VRR, pacing e casos fora da faixa VRR sem mexer na velocidade emulada.

## Testes e critérios de aceitação

- Mesma ROM, perfil, sequência de entradas e ciclos produzem o mesmo estado e sequência de eventos.
- Perfis regionais têm testes de fronteira para linhas/quadro e interrupções.
- Alterar frequência PCM do host não altera estado final da CPU, VDP ou PSG para o mesmo tempo emulado.
- A duração e frequência dos eventos de áudio correspondem ao clock do perfil, não ao refresh do monitor.
- O pacing não pula execução de hardware; qualquer frame repetido/descartado ocorre somente na apresentação e é diagnosticável.
- NTSC/PAL e modos de vídeo não implementados são reportados explicitamente como pendentes.

## Referências

- Charles MacDonald, *Sega Master System VDP documentation*, temporização NTSC/PAL e interrupções: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
- MAME, implementação de temporização do VDP SMS/Game Gear para comparação: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Genesis Software Manual (documentação técnica histórica): https://nemesis.hacking-cult.org/MegaDrive/Documentation/GenesisSoftwareManual.pdf
- Genesis / Mega Drive region information: https://consolemods.org/wiki/Genesis%3ARegion_Information
- Libretro, sincronização dinâmica de áudio/vídeo: https://docs.libretro.com/guides/audio-video-sync/
- Microsoft, variable refresh rate displays: https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/variable-refresh-rate-displays

## Limitações

Valores regionais exatos, revisões de ASIC e comportamento de borda devem ser validados por manuais específicos, testes de diagnóstico e, quando possível, hardware real. Esta especificação não autoriza inventar frequências nem declarar fidelidade de ciclo sem evidência.