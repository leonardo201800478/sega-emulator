# SMS1 Mode 4 — sprite line buffer, phase 2

**ASIC alvo:** Sega 315-5124
**Escopo:** seleção de sprites SAT entre scanlines
**Classe:** implementação funcional scanline-level
**Estado:** código implementado; validação CI pendente no HEAD desta PR
**Oráculo físico:** não disponível nesta etapa

## 1. Mudança

O engine SMS1 mantém agora um `Mode4SpriteLineBuffer` persistente em `SmsVdp`. O buffer contém:

- scanline de destino;
- índices SAT selecionados, em ordem;
- linha de origem do sprite para a scanline de destino;
- primeiro índice excedente para o flag de overflow;
- configuração de altura/zoom usada na seleção.

No primeiro avanço após reset, a seleção da linha 0 é preparada. Ao concluir uma scanline visível, a seleção da próxima scanline também visível é preparada. O resultado é consumido ao renderizar a linha de destino.

A seleção preserva a ordem SAT e a capacidade do hardware de oito entradas. O modo de compatibilidade desbloqueado mantém a capacidade de desenho estendida, mas continua registrando o primeiro excedente a partir do nono sprite.

## 2. Separação de fases atual

A fase de seleção amostra Y/elegibilidade e a linha de origem. A composição posterior consulta X/tile na SAT e os dados de pattern no momento em que a scanline é renderizada.

Isso cria uma primeira separação entre seleção e composição, mas **não** representa um pipeline completo de fetch:

- não há timestamps por sprite;
- não há fetch X/tile/pattern por dot;
- não há arbitragem de VRAM ou WAIT;
- overflow e colisão continuam sendo expostos pelo caminho de composição da scanline;
- a fronteira escolhida é o fechamento da scanline anterior, não uma medição de clock do ASIC.

## 3. Perfil SMS1

A seleção persistente só é ativada para o perfil `Sega3155124`. SMS2 mantém o caminho anterior nesta etapa, aguardando validação comparativa própria.

O renderer recebe a altura efetiva do perfil. Assim, os bits de modo estendido de R1 não fazem o caminho SMS1 renderizar 224/240 linhas, mesmo que sejam escritos.

## 4. Validação adicionada

- `sms1_sprite_y_selection_is_latched_for_the_following_scanline`: uma alteração tardia do Y na SAT não remove o sprite já selecionado para a linha seguinte; a alteração passa a valer na avaliação subsequente.
- `sms1_sprite_line_buffer_is_preserved_by_vdp_clone_restore`: clonar/restaurar o VDP no boundary entre linhas preserva a seleção pendente e produz estado/frame idêntico.

Esses testes validam a semântica do modelo implementado e sua reprodutibilidade. Não são prova de fase elétrica, timestamp de avaliação nem precisão dot/cycle.

## 5. Limites e próxima etapa

A documentação funcional indica que a SAT é percorrida para selecionar sprites da linha seguinte, mas não determina aqui o instante sublinha exato. A fronteira por fechamento de scanline é uma aproximação explícita e substituível quando surgir evidência melhor.

Próximos pontos:
1. adicionar testes para SAT modificada durante a linha e para overflow com nono sprite;
2. separar o instante de atualização do status overflow/collision da composição visual;
3. caracterizar fetch de X/tile/pattern sem inventar offsets;
4. confrontar V Counter, Scroll Test e ROMs de sprites com oráculos independentes;
5. só então introduzir composição parcial para eventos raster cuja visibilidade esteja sustentada.

Não declarar o VDP SMS1 completo nem dot-accurate nesta fase.
