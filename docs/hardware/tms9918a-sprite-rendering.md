# TMS9918A — renderização e avaliação de sprites

## Escopo desta etapa

O renderizador avalia sprites em ordem de índice e por scanline. Por padrão,
respeita o limite de quatro sprites considerados para a composição visual de
cada linha. Para diagnóstico, `TMS9918A(sprite_limit_enabled=False)` desativa
a limitação visual; essa opção não desativa o flag de overflow do status.
Sprites transparentes não desenham pixels nem geram colisão, mas continuam
contando para o limite por linha. A primeira sprite tem prioridade visual;
pixels opacos de sprites posteriores sobrepostos ativam o flag de colisão.
O quinto sprite intersectando uma linha registra overflow e seu índice nos
cinco bits inferiores do status.

Atributos de sprite são lidos da tabela apontada por R5; padrões vêm da tabela
apontada por R6. São tratados tamanho 8x8/16x16, magnificação 1x/2x, deslocamento
horizontal de -32 pixels, clipping horizontal/vertical e alinhamento e ordem dos quatro
padrões no modo 16x16 (N/N+2 na parte superior e N+1/N+3 na inferior). A leitura do status limpa os flags de status conforme o
modelo atual do chip.

## Limitações conhecidas

- A avaliação ocorre durante a renderização do quadro, não no instante exato de
  cada scanline real; efeitos de alterações de VRAM/registradores no meio do
  quadro não são reproduzidos.
- O modelo de clock avança por T-states nominais, não por ciclos individuais de
  barramento nem pelo pipeline de pixels do VDP.
- Paleta, prioridade, colisão, overflow e clipping ainda precisam de comparação
  diferencial contra uma referência independente e testes em hardware/documentação
  técnica antes de declarar fidelidade total.
- O modelo alvo é TMS9918A no SG-1000; não assumir que os resultados se aplicam
  sem ajustes a variantes posteriores de VDP Sega.

## Testes de regressão

A suíte `tests/unit/test_tms9918_sprites.py` cobre colisão entre pixels opacos,
prioridade visual, overflow e índice da quinta sprite, sprites transparentes,
deslocamento horizontal, ordem dos quadrantes 16x16, limite de sprites ativado
e desativado e limpeza do status. Os testes são
regressões do modelo implementado, não prova isolada de equivalência elétrica.
