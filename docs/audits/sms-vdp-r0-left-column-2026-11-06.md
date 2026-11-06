# Auditoria SMS VDP — R0 bit 3, R0 bit 5 e colisão na coluna esquerda

**Data:** 2026-11-06  
**Baseline:** `e312ddd3b3d1e97a576f40449bac2bf52688f3ee`  
**Branch:** `test/sms-vdp-r0-left-column-matrix-20261106`  
**Escopo:** composição visual Mode 4 e divergência documental sobre colisão na coluna mascarada.

## 1. Contrato documental confirmado

O Software Reference Manual do Sega Mark III descreve:

- R0 bit 3 (`Sprite Shift`): desloca os padrões de sprite oito pixels para a esquerda.
- R0 bit 5 (`Left Column Blank`): mascara a primeira coluna de oito pixels.
- A combinação bit 3=0 e bit 5=1 mantém os sprites sem deslocamento e mascara a coluna esquerda.

Referência primária: [SMS Power — SMS Official Docs, seção SPRITES e Register 0](https://www.smspower.org/Development/SMSOfficialDocs&num=5).

## 2. Auditoria do código atual

Arquivo: `native/video_core/src/sms/vdp.rs`.

- O cálculo da coordenada X aplica `sx -= 8` quando R0 bit 3 está ativo.
- A composição do background substitui os primeiros oito pixels pelo backdrop quando R0 bit 5 está ativo.
- A composição dos sprites também oculta pixels na faixa X=0..7 quando R0 bit 5 está ativo.
- O limite de oito sprites por scanline permanece habilitado no caminho testado.

A implementação visual corresponde ao contrato documental consultado. A lacuna estava na matriz de regressão: o teste existente cobria máscara esquerda sobre sprites, mas não exercitava explicitamente as quatro combinações dos dois bits nem as bordas X=0, X=1, coordenadas negativas e sprites parcialmente mascarados.

## 3. Alteração de teste

Foi adicionado:

`mode4_r0_sprite_shift_and_left_column_mask_cover_all_combinations_for_sms1_and_sms2`

A matriz executa as oito posições/casos selecionados para os dois perfis:

- SMS1 / Sega 315-5124;
- SMS2 / Sega 315-5246.

As combinações de R0 cobertas são:

| Bit 3 — Sprite Shift | Bit 5 — Left Column Blank | Cobertura |
|---:|---:|---|
| 0 | 0 | Sprite sem deslocamento e coluna visível |
| 1 | 0 | Deslocamento de -8 px, inclusive X negativo e X=1 |
| 0 | 1 | Máscara da coluna esquerda, incluindo sprite que cruza X=8 |
| 1 | 1 | Deslocamento e máscara simultâneos |

O teste constrói background verde, sprite vermelho e backdrop preto. Verifica cada pixel da scanline, tornando observável o resultado dos dois bits sem depender de screenshot ou do frontend.

**O teste não altera nem declara validada a semântica do flag de colisão.**

## 4. Divergência sobre colisão na coluna mascarada

As fontes consultadas não são concordantes:

1. [SMS Power — Collision Detection](https://www.smspower.org/Development/CollisionDetection) descreve o flag como resultado da sobreposição de pixels opacos de sprites, sem especificar o efeito de R0 bit 5.
2. [SMS Power — fórum, Sprite Overflow Test Program](https://www.smspower.org/forums/1568-SpriteOverflowTestProgram), posts de Charles MacDonald em 2001: relata que a colisão ocorria quando um sprite tocava os oito pixels esquerdos mascarados por R0 bit 5. O próprio post esclarece que se refere ao blanking da coluna esquerda, não ao desligamento total do display. A mensagem não identifica inequivocamente a revisão do VDP usada nessa observação.
3. [MAME — 315_5124.cpp](https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp): a rotina `sprite_collision` retorna sem marcar colisão quando R0 bit 5 está ativo e X da colisão é menor que 8.
4. [SmsVdpTest](https://github.com/FluBBaOfWard/SmsVdpTest) contém testes de colisão e de pixels fora da tela, mas a lista pública de testes não demonstra uma prova específica para a coluna mascarada.

O código atual segue a regra implementada pelo MAME: colisões em X<8 são suprimidas quando R0 bit 5 está ativo. Diante do relato conflitante do SMS Power, não há base suficiente para alterar essa regra neste incremento.

### Estado da questão de colisão

- Implementação atual: conhecida e localizada.
- Relato histórico de hardware: existe, mas não identifica a revisão de forma inequívoca.
- Confirmação independente em SMS1 e SMS2: **NOT RUN**.
- ROM de teste específica para essa hipótese: ainda não adicionada.
- Alteração de comportamento: **não realizada**.

## 5. Critérios de aceitação e validação

| Critério | Estado |
|---|---|
| Quatro combinações R0 bit 3/bit 5 | Teste adicionado; CI pendente |
| X=0, X=1 e coordenadas negativas | Teste adicionado; CI pendente |
| Sprite parcialmente mascarado em X=8 | Teste adicionado; CI pendente |
| SMS1 e SMS2 | Teste adicionado; CI pendente |
| Colisão na coluna mascarada | BLOCKED — evidências conflitantes; hardware não testado |
| Cargo fmt / Clippy / testes Rust | NOT RUN localmente; aguardando CI remoto |
| Validação física em console SMS1/SMS2 | NOT RUN |

## 6. Decisão

Este incremento é exclusivamente de teste e documentação. Não modifica o renderer, o cálculo de colisão, o Scheduler, o clock do VDP nem a política de limite físico de sprites.

A próxima decisão sobre colisão deverá ser tomada somente após um teste reproduzível em hardware, identificando revisão do VDP e cobrindo separadamente:
- um sprite tocando a coluna mascarada;
- dois sprites sobrepostos somente na coluna mascarada;
- dois sprites parcialmente sobrepostos entre X=0..7 e X=8..15;
- as quatro combinações de R0 bit 3/bit 5.

Nenhum resultado visual ou de colisão obtido apenas por MAME será tratado como prova física.
