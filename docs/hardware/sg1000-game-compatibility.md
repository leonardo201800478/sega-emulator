# SG-1000 — catálogo prioritário de compatibilidade

## Objetivo

Este catálogo define os jogos comerciais lançados para o SG-1000 que orientarão
a validação funcional do emulador. O projeto poderá aceitar homebrew, conversões
e outras imagens compatíveis, mas os títulos comerciais do catálogo são a
referência principal de compatibilidade.

**Importante:** estar nesta lista significa *alvo de teste*, não jogo já
suportado. O repositório já integra um caminho inicial de máquina SG-1000 com
CPU Z80, ROM/RAM, VDP TMS9918A, joystick e execução headless; entretanto, o PSG
não está integrado, a temporização não é ciclo a ciclo e a compatibilidade com
jogos reais ainda depende de validação reproduzível. Nenhum título abaixo deve
ser anunciado como funcional até que os testes correspondentes sejam executados
e registrados.

## Política de catálogo

- Manter um registro por título comercial canônico, sem contar dumps regionais
  ou revisões como jogos diferentes.
- Rastrear variantes regionais, revisões, My Cards, Golden Carts e hashes em
  inventário de ROM separado quando isso for necessário para reproduzir um caso.
- Não incluir ROMs comerciais no repositório.
- Não confundir jogos listados com dumps preservados: fontes de preservação podem
  indicar títulos conhecidos, variantes disponíveis, dumps ausentes ou dados
  ainda não verificados.
- A compatibilidade será medida por execução real e testes observáveis, não
  apenas pelo fato de a ROM carregar ou apresentar uma tela.

## Catálogo inicial de jogos comerciais

Todos os títulos começam como **não validados**. A ordem é uma sequência de
prioridade proposta para orientar a suíte; não é uma declaração de compatibilidade.

| Prioridade | Título | Observações de identificação |
|---|---|---|
| A | Borderline | Título inicial representativo |
| A | Champion Golf | Há variantes de cartucho; distinguir dumps |
| A | Champion Boxing | Há variantes de cartucho; distinguir dumps |
| A | Champion Baseball | — |
| A | N-Sub | — |
| A | Star Jacker | — |
| A | Monaco GP | Há variantes de mídia/cartucho |
| A | Sega Flipper | — |
| A | Girl's Garden | — |
| A | Flicky | — |
| A | Zaxxon | — |
| A | Lode Runner | — |
| A | Safari Hunting | — |
| A | Safari Race | — |
| A | Sindbad Mystery | — |
| A | Star Force | Há variantes regionais |
| A | Champion Soccer | — |
| A | Champion Tennis | — |
| A | Champion Pro Wrestling | — |
| A | GP World | — |
| A | Hyper Sports | — |
| A | Choplifter | Há variantes regionais |
| A | Pitfall II: The Lost Caverns | Há variantes regionais |
| A | Doki Doki Penguin Land | Também aparece como Doki Doki Penguin Land |
| A | Bank Panic | Há variantes regionais |
| A | Bomb Jack | Há variantes regionais |
| A | Chack'n Pop | Há variantes regionais |
| A | Champion Ice Hockey | — |
| A | Champion Billiards | — |
| A | Ninja Princess | — |
| A | Wonder Boy | — |
| A | Zoom 909 | — |
| B | 007 James Bond | Também catalogado como 007 James Bond |
| B | Guzzler | Publicado pela Tsukuda Original |
| B | San-nin Mahjong | Publicado pela Tsukuda Original |
| B | Okamoto Ayako no Match Play Golf | — |
| B | Space Armor | Publicado pela Tsukuda Original |
| B | Challenge Derby | Publicado pela Tsukuda Original; dumps podem variar |
| B | Mahjong | — |
| B | Home Mahjong | — |
| B | Pachinko II | — |
| B | Sega-Galaga | — |
| B | Pop Flamer | — |
| B | Pacar | — |
| B | Yamato | — |
| B | Exerion | — |
| B | Hustle Chumy | — |
| B | Orguss | — |
| B | Elevator Action | — |
| B | Rock n' Bolt | — |
| B | Sokoban | — |
| B | Championship Lode Runner | — |
| B | H.E.R.O. | — |
| B | Hang-On II | — |
| B | Dragon Wang | Revisões e variantes regionais devem ser identificadas separadamente |
| B | Drol | — |
| B | Super Tank | — |
| B | Champion Kendou | Também grafado Champion Kendo |
| B | C_So! | A grafia com sublinhado é usada em catálogos de ROM |
| B | The Castle | — |
| B | Othello | — |
| B | Space Invaders | — |
| B | Space Slalom | — |
| B | Golgo 13 | — |
| B | The Black Onyx | — |
| B | Q*bert | O inventário registra “Q-bert” na plataforma Othello Multivision; confirmar a nomenclatura canônica antes de fixar metadados de ROM |
| B | Space Mountain | Registrado para Othello Multivision |
| B | Congo Bongo | Há variantes regionais e revisões; não tratar cada dump como jogo separado |
| B | Dragon Wang | Há revisões e variantes regionais |
| B | Gulkave | O inventário também contém uma variante coreana não licenciada; manter o título comercial separado |
| B | Pachinko | Título distinto de Pachinko II |
| B | Serizawa Hachidan no Tsumeshougi | Título japonês; preservar a grafia original para pesquisa e identificação |
| B | Shinnyushain Tooru-kun | O inventário inclui uma variante Taiwan não licenciada |
| B | Soukoban | Há variantes Taiwan/Coreia não licenciadas no inventário |
| B | Space Armor | Registrado para Othello Multivision; publicado pela Tsukuda Original |

### Complemento após análise do inventário enviado

Foi analisado o arquivo `Sega - SG-1000 - SC-3000 (PC Text) (20231205-110448).zip`,
que contém um arquivo de texto com 247 linhas de registros e variantes. O conteúdo
é um inventário de software misto — SG-1000, SC-3000, SF-7000, BIOS/testes,
protótipos e títulos não licenciados/aftermarket — e não deve ser tratado como
uma lista exclusiva de jogos oficiais do SG-1000.

Os registros do pacote foram usados para encontrar lacunas no catálogo, não para
inferir compatibilidade. Os títulos abaixo foram acrescentados à prioridade B
por constarem como lançamentos de SG-1000 no inventário e ainda não estarem
representados no catálogo canônico:

| Prioridade | Título | Observações de identificação |
|---|---|---|
| B | Q*bert | O inventário registra “Q-bert” na plataforma Othello Multivision; confirmar a nomenclatura canônica antes de fixar metadados de ROM |
| B | Space Mountain | Registrado para Othello Multivision |
| B | Congo Bongo | Há variantes regionais e revisões; não tratar cada dump como jogo separado |
| B | Dragon Wang | Há revisões e variantes regionais |
| B | Gulkave | O inventário também contém uma variante coreana não licenciada; manter o título comercial separado |
| B | Pachinko | Título distinto de Pachinko II |
| B | Serizawa Hachidan no Tsumeshougi | Título japonês; preservar a grafia original para pesquisa e identificação |
| B | Shinnyushain Tooru-kun | O inventário inclui uma variante Taiwan não licenciada |
| B | Soukoban | Há variantes Taiwan/Coreia não licenciadas no inventário |
| B | Space Armor | Registrado para Othello Multivision |

#### Registros deliberadamente fora deste catálogo

O arquivo também contém itens que não devem ser incorporados à lista de jogos
comerciais prioritários do SG-1000 sem uma classificação própria:

- **SC-3000:** Home BASIC, BASIC Level II/III, programas educacionais japoneses,
  Music Editor e outros programas identificados explicitamente como SC-3000.
- **SF-7000:** Disk BASIC, Sega Logo, coleções de disco e títulos identificados
  como software SF-7000.
- **BIOS e diagnóstico:** Othello Multivision BIOS e SG-1000 M2 Test Cartridge.
- **Não licenciados/variantes de terceiros:** dumps Taiwan/Coreia marcados como
  `Unl`, protótipos e títulos aftermarket. Eles podem ser úteis numa futura
  suíte de compatibilidade ampliada, mas não comprovam lançamento comercial
  oficial nem devem ser misturados silenciosamente ao catálogo principal.
- **Periféricos/software especializado:** Terebi Oekaki e programas de
  acessórios devem ter seus requisitos de hardware documentados antes de serem
  usados como testes gerais de compatibilidade.

#### Limitações da análise

A lista enviada identifica nomes, regiões, revisões e relações entre variantes,
mas o texto não contém hashes, tamanhos ou bytes das ROMs. Portanto, não permite
validar integridade de dumps, confirmar que cada imagem funciona ou determinar
compatibilidade com o nosso código. As marcações regionais e de licença foram
preservadas conforme descritas no arquivo; qualquer classificação editorial
adicional deve ser confirmada com uma segunda fonte antes de ser considerada
definitiva.

### Prioridades

- **A — núcleo inicial:** títulos para começar a validação por diversidade de
  código, gráficos, entrada e duração de execução. A prioridade não garante
  que sejam tecnicamente simples.
- **B — expansão do catálogo:** completar depois que o caminho de inicialização,
  CPU, memória, VDP, áudio e controles tiver testes de integração estáveis.
- **C — futuro:** homebrew, conversões, protótipos, jogos não licenciados e
  software de plataformas relacionadas. Poderão ser aceitos, mas não substituem
  o catálogo comercial prioritário.

A classificação A/B é um plano de trabalho inicial e pode mudar conforme a
disponibilidade legal de imagens de teste, diagnósticos reproduzíveis e a
complexidade de cada cartucho.

## Critérios para marcar um título como validado

Cada título só poderá mudar de **não validado** para um estado de compatibilidade
após registrar, no mínimo:

1. Identificação da imagem por tamanho e hash (CRC32 e SHA-1 ou SHA-256).
2. Região, revisão e tipo de mídia conhecidos, quando aplicável.
3. Inicialização sem falha e execução observada de uma sequência reproduzível.
4. Verificação de entrada, atualização de vídeo, estado de interrupção e áudio
   conforme os recursos efetivamente usados pelo jogo.
5. Resultado de regressão reproduzível, com limitações e evidências registradas.

Estados sugeridos: **não validado**, **inicializa**, **jogável com limitações**,
**validado** e **regressão**. Não atribuir o estado apenas com base em relatos
externos ou na presença de um dump em um catálogo.

## Fontes de referência

- [Sega Retro — List of SG-1000 ROM dumps](https://segaretro.org/List_of_SG-1000_ROM_dumps):
  fonte indicada para o inventário de dumps, suas variantes e disponibilidade.
  A página não pôde ser acessada automaticamente durante a criação deste
  documento; portanto, esta lista não afirma reproduzir todos os registros ou
  estados de dump da página.
- [MAME — software list SG-1000 (sg1000.xml)](https://github.com/mamedev/mame/blob/master/hash/sg1000.xml):
  referência cruzada para nomes, seriais, regiões, variantes, tamanhos e hashes.
  O arquivo identifica sua licença como CC0-1.0; isso não altera as licenças das
  ROMs comerciais listadas.
- [Sega Retro — List of SG-1000 games](https://www.segaretro.org/List_of_SG-1000_games):
  índice complementar de jogos documentados no catálogo Sega Retro.

Este documento é um roteiro de compatibilidade do projeto, não uma distribuição
de jogos, uma autorização para baixar ROMs nem uma declaração de que os títulos
já funcionam no emulador.
