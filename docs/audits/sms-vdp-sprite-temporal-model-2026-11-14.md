# SMS VDP — modelo temporal de sprites, auditoria da fase 2

**Base:** `e2ac354949c027b95407412c2f32cefb1e012910`
**Branch:** `audit/sms-vdp-sprite-temporal-model-20261114`
**Status:** auditoria técnica; nenhuma alteração de comportamento do VDP nesta etapa.

## Objetivo

Definir o modelo temporal correto antes de introduzir estado persistente no renderer. A meta é eliminar divergências de sprites sem assumir que SMS1 (315-5124) e SMS2 (315-5246) possuem exatamente o mesmo mecanismo interno.

## Estado observado no código

O caminho ativo está em `native/video_core/src/sms/vdp.rs`. `SmsVdp::complete_scanline()` renderiza a linha atual quando ela termina. `render_scanline_for_revision()` chama `evaluate_mode4_sprites()` para a própria linha renderizada. O helper percorre a SAT e retorna tuplas que já contêm índice, X, tile e linha interna; o renderer então lê os bitplanes e compõe os pixels.

Consequência: a extração da fase 1 é estrutural, não temporal. A seleção não é persistida entre scanlines e os atributos X/tile são amostrados junto com a composição.

## Evidência técnica e ressalvas por revisão

### Comportamento documentado comum

SMS Power descreve a SAT sendo percorrida em uma scanline para selecionar sprites da próxima linha, em ordem de tabela, com buffer interno de oito entradas. O limite considera sprites transparentes e fora da tela; o nono candidato causa overflow. O terminador `$D0` encerra a tabela somente no modo de 192 linhas.

Fonte: https://www.smspower.org/Development/Sprites

### Diferença que impede uma implementação genérica precipitada

O código MAME separa `select_sprites()` de `draw_sprites_mode4()`. Na implementação base 315-5124, a seleção calcula e mantém arrays de X, tile selecionado e linha do padrão. A implementação derivada 315-5313 contém um comentário atribuído a Charles MacDonald dizendo que, no SMS, apenas os oito números de sprite são bufferizados e que os dados restantes são lidos em tempo real.

Fonte comparativa: https://sources.debian.org/src/mame/0.289%2Bdfsg1-1/src/devices/video/315_5124.cpp (funções `select_sprites`, `draw_sprites_mode4` e comentário da implementação 315-5313).

Isso exige distinguir o ASIC/revisão e confirmar a aplicabilidade exata do comentário à revisão SMS2 modelada como `Sega3155246`. Não se deve aplicar automaticamente o comportamento 315-5313 ao 315-5246 nem presumir que a organização interna do MAME corresponda diretamente ao hardware.

### SMS1

A documentação SMS Power descreve a particularidade de zoom horizontal do SMS1: com zoom habilitado, somente os primeiros N-4 sprites selecionados recebem zoom horizontal; os quatro últimos recebem apenas zoom vertical. O código atual tenta reproduzir isso com base na posição/rank da lista ativa.

Fonte: https://www.smspower.org/Development/Sprites

A seleção temporal SMS1 deve preservar essa regra e o mapeamento de atributos/padrões já existente no projeto. A mudança não pode deslocar a regra de rank inadvertidamente.

## Decisões para a implementação seguinte

1. Modelar a seleção para a linha seguinte como estado explícito do VDP; não recalcular a lista da linha atual durante a composição.
2. Separar o resultado da seleção (índices e informação de overflow) do fetch de atributos/padrões.
3. Não fixar ainda uma representação única de buffer para SMS1 e SMS2. A forma persistida deve refletir a evidência por revisão e os testes que a sustentam.
4. Preservar a ordem SAT e interromper a seleção após oito entradas; detectar o primeiro candidato excedente para overflow sem incluir o nono na lista exibida.
5. Definir o comportamento de linha pré-renderizada/frame wrap para a primeira scanline visível.
6. Definir quando overflow e colisão tornam-se observáveis via status port; não inferir que ambos ocorrem no mesmo boundary.
7. Incluir qualquer estado temporal persistente em reset, clone/equality e no formato de estado da máquina quando save states forem implementados. A documentação atual do crate SMS ainda declara save states persistentes serializados como um gate separado; não inventar um formato local para o VDP.
8. Não alterar `MachineScheduler`, Z80, PSG, GUI ou frame pacing.

## Matriz mínima de regressão

- 0, 1, 8 e 9 candidatos na mesma linha.
- Sprites transparentes e completamente fora da tela contando para o limite.
- Ordem SAT, prioridade visual e colisão entre sprites.
- Terminador `$D0` em 192 linhas e ausência de terminador especial em 224/240 linhas.
- Y wrap/top clipping, 8x8, 8x16, zoom e sprite shift.
- Diferenças SMS1/315-5124 e SMS2/315-5246.
- Alterações de SAT entre avaliação e exibição, separando Y de X/tile.
- Alterações de pattern data antes do fetch da linha.
- Avaliação da última linha não visível e preparação da linha 0.
- Equivalência de `advance(n)` contínuo e fragmentado.

## Critério de aceite

A fase temporal só pode ser considerada concluída quando testes determinísticos demonstrarem que a linha N seleciona os candidatos da linha N+1, que os dados bufferizados correspondem à revisão do VDP, que overflow/collision são observáveis no boundary correto e que o comportamento anterior não regrediu nos modos legados.

Este documento não declara correção de jogos, validação física do hardware, execução de ROM diagnóstica nem conclusão da fase 2.
