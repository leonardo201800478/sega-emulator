# Especificação de fidelidade do VDP: sprites e VBlank

## Estado e escopo

Este documento registra requisitos para a emulação dos VDPs Sega. É uma especificação de arquitetura e validação, não uma declaração de que os comportamentos já estejam integralmente implementados. O primeiro alvo continua sendo SG-1000/TMS9918A; Master System/Game Gear e Mega Drive exigem perfis próprios.

## Princípios invariantes

- O comportamento do VDP emulado é independente do backend gráfico (D3D11, D3D12, Vulkan ou renderização por CPU).
- A apresentação pode repetir ou descartar a apresentação de um quadro conforme política explícita, mas nunca pode alterar clocks, estado de CPU, interrupções ou eventos do VDP.
- Todos os acessos ao framebuffer são limitados por dimensões verificadas; um modo de compatibilidade nunca pode permitir escrita fora dos buffers.
- Separar fatos documentados, comportamento observado em hardware/emuladores, inferências e hipóteses.

## Política opcional para limite de sprites

### Modo Hardware (padrão)

Respeita o limite de sprites por linha e quaisquer limites adicionais documentados para a variante de VDP ativa. Deve preservar a ordem de avaliação, prioridade, transparência, colisões, clipping e os bits/índices de status observáveis pelo jogo.

### Modo Sem limite (opção de compatibilidade)

- Permite desenhar sprites que excederiam o limite de exibição do hardware, para reduzir cortes visuais em jogos que se beneficiem disso.
- Não altera clocks, rasterização de fundo, coordenadas, tamanho, rotação, escala, prioridade, colisões ou conteúdo de VRAM.
- Mantém limites de memória e clipping de pixels rigorosos.
- Deve ser claramente rotulado como comportamento não original do hardware.
- Os registradores e interrupções relacionados a overflow permanecem com comportamento de hardware, a menos que uma opção futura, separada e explicitamente documentada, diga o contrário.
- O modo não pode ser implementado como um aumento global arbitrário do limite. Cada família/VDP precisa de estratégia específica.

É possível que o modo sem limite mostre sprites adicionais enquanto o jogo ainda observa overflow no estado do VDP. Essa divergência é deliberada e deve constar no diagnóstico.

### Diagnóstico e testes de sprites

Registrar em ferramentas de diagnóstico, sem perturbar o caminho crítico: sprites avaliados por linha, sprites efetivamente desenhados, limite aplicado, overflow e modo de compatibilidade ativo.

Cobertura mínima:

- sprites transparentes e sua participação na contagem/colisão;
- ordem e prioridade de sprites sobrepostos;
- colisões dentro e fora da área visível;
- sprites parcialmente fora da tela e deslocamentos à esquerda;
- sprites de tamanho ampliado e tabelas de atributos/padrões;
- mudança de atributos durante a varredura, quando a temporização correspondente estiver implementada;
- cenas de alta densidade e movimento, incluindo rotação/escala simuladas por sprites;
- determinismo e nenhuma escrita fora do framebuffer;
- imagens de referência comparando modo Hardware com modo Sem limite.

## VBlank é um evento de hardware, não um evento de apresentação

VBlank deve ser modelado no núcleo determinístico do VDP. A apresentação gráfica apenas consome quadros já produzidos. O fim da área ativa, o início do intervalo vertical, o indicador de VBlank, a ocorrência de interrupção vertical, a saída IRQ e a leitura/limpeza de status são eventos relacionados, mas não devem ser colapsados em uma única chamada genérica de fim de quadro.

Para cada VDP, documentar e testar separadamente:

- clock mestre e divisores relevantes;
- clocks por linha e número de linhas por quadro para NTSC/PAL, quando suportados;
- área ativa e linhas de blanking/sync;
- linha/fase de ativação de VBlank e interrupção vertical;
- habilitação e reconhecimento de IRQ;
- semântica de leitura dos registradores de estado e bits que são limpos ou persistem;
- interrupções horizontais/linha, quando disponíveis;
- janelas e restrições de acesso a VRAM/CRAM, DMA e efeitos de contenção, conforme o nível de precisão implementado;
- modos de 192/224/240 linhas quando suportados pelo hardware, sem presumir que todo modo funcione em toda região;
- variantes de hardware e incertezas conhecidas.

## Estado observado no SG-1000

No modelo atual em `src/sega_emulator/video/tms9918.py`, os valores declarados incluem `TSTATES_PER_LINE = 228`, `LINES_PER_FRAME = 262` e `VBLANK_LINE = 192`. O VBlank é sinalizado quando o contador de linha chega a 192, o estado recebe bit 7 e o framebuffer é publicado. O próprio módulo declara não simular acessos à VRAM por pixel, contenção ou temporização elétrica.

Esses valores constituem um perfil inicial de NTSC, não prova de precisão de ciclo nem suporte PAL completo. Antes de ampliar o modelo, confirmar se os T-states usados correspondem ao clock e à temporização reais do console, e separar a geração do evento de hardware da publicação do framebuffer.

## Critérios de aceitação

1. Avanços de tempo que terminem antes, exatamente na fronteira ou depois da fronteira de linha produzem estado coerente.
2. Uma chamada de avanço que atravesse múltiplas linhas/quadros produz os mesmos eventos e estado final que avanços particionados equivalentes.
3. Leituras de status, habilitação/desabilitação de interrupções e reconhecimento da CPU são cobertos por testes.
4. Perfis NTSC/PAL não compartilham constantes de temporização sem evidência de equivalência.
5. Renderizadores diferentes recebem o mesmo framebuffer de origem para a mesma sequência determinística de eventos.
6. O modo sem limite altera somente a política de desenho dos sprites; não corrompe memória nem muda o restante do estado do console.
7. Testes de integração/diferenciais com ROMs de diagnóstico e referências conhecidas documentam divergências em vez de mascará-las.

## Referências técnicas

- Charles MacDonald, *Sega Master System VDP documentation*: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
- MAME, implementação do VDP SMS/Game Gear (fonte de comparação, não especificação normativa): https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Sean Young, documentação detalhada TMS9918A (alerta do autor: parte das observações é baseada em MSX e pode não descrever todas as variantes): https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt
- Texas Instruments, datasheet TMS9918A/TMS9928A/TMS9929A: https://www.ti.com/lit/ds/symlink/tms9918a.pdf

## Fora de escopo desta especificação

Não declara compatibilidade completa com todos os jogos, não define timings sem comprovação e não implementa ainda o VDP de Master System/Game Gear ou Mega Drive. As implementações futuras devem adicionar dados e testes específicos de cada família.