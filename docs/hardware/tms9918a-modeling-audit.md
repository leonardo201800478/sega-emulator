# TMS9918A — auditoria de modelagem e plano de fidelidade

## Objetivo e regra de evidência

Este documento acompanha a implementação de `TMS9918A` em
`src/sega_emulator/video/tms9918.py`. O alvo imediato é o VDP do SG-1000, não
uma implementação genérica de todos os VDPs da família.

Usamos três rótulos distintos:

- **Implementado:** existe código e ao menos um teste que exercita o caminho.
- **Parcial / precisa de validação:** existe código, mas faltam casos de referência,
  comparação diferencial ou detalhes do hardware.
- **Ausente:** ainda não há modelo suficiente para afirmar suporte.

Passar nos testes unitários não prova fidelidade total, e uma imagem renderizada
plausível não prova temporização correta.

## Referência primária

A referência principal é o manual *Texas Instruments TMS9918A/TMS9928A/TMS9929A
Video Display Processors Data Manual* (novembro de 1982), em especial:

- seção 2.1: interface CPU, comandos, leituras/escritas e interrupção;
- seção 2.2: registradores 0–7;
- seção 2.3: registrador de status, colisão e quinto sprite;
- seção 2.4: modos gráficos e sprites;
- seção 3.2: derivação de endereços de VRAM;
- seção 3.6: clocks e geração de sincronismo.

Cópia online consultada: https://www.bitsavers.org/components/ti/TMS9900/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf

Referência complementar, útil para observações de comportamento e quirks, mas
não substitui o manual nem deve ser generalizada automaticamente para todas as
plataformas: https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt

## Inventário da implementação atual

| Área | Estado atual | Lacuna importante |
|---|---|---|
| VRAM | 16 KiB, acesso por endereço de 14 bits | Não há arbitragem temporal nem acesso por dot/ciclo |
| Portas | Decodificação por máscara `port & 0xC1` | Quirks e efeitos de portas precisam de testes de referência |
| Comando de controle | Latch de dois bytes, escrita em registrador e seleção de endereço | Validar sequências interrompidas, mudanças de modo e comportamento exato do buffer |
| Buffer de leitura | Prefetch ao configurar leitura e incremento automático | Casos de escrita após leitura e reset precisam de comparação com referência |
| Registradores | Oito registradores graváveis | Máscaras de bits reservados, valores após reset e efeitos colaterais precisam ser formalizados |
| Status | VBlank, colisão e quinto sprite; leitura limpa status | Flags de sprite agora são avaliadas no fechamento de cada linha; resolução por dot e arbitragem ainda ausentes |
| Graphics I | Renderização de tabela de nomes, padrões e cores | Máscaras de endereços e casos extremos ainda precisam de testes exaustivos |
| Graphics II | Renderização de padrões/colorização por seção vertical | Máscaras programáveis de R3/R4 e todos os limites de endereço precisam de auditoria |
| Multicolor | Renderização por blocos coloridos | Confirmar endereçamento vertical e repetição de padrões contra o manual |
| Text | 40 colunas por 24 linhas, área central de 240×192 | A geometria, cores e regiões laterais precisam de comparação de imagem |
| Sprites | 8×8/16×16, magnificação, early clock, terminador e prioridade | Flags temporais e bordas devem ser comparadas com ROMs de diagnóstico |
| Interrupção | `interrupt_pending` deriva de status VBlank e R1 bit 5 | O momento exato da linha INT e a integração CPU/VDP ainda são aproximados |
| Temporização | Contador nominal de T-states por linha e quadro NTSC | Não é temporização ciclo a ciclo nem uma configuração PAL validada |
| Saída | Framebuffer RGB24 de 256×192 | Não modela sinal composto, burst, artefatos analógicos nem saída externa |

## Regras que devem permanecer explícitas

### VRAM e portas

- A VRAM do TMS9918A tem 16 KiB no modelo do SG-1000 e endereço interno de
  14 bits.
- A CPU não acessa VRAM diretamente no mapa de memória; usa as portas do VDP.
- O acesso de leitura tem prefetch: programar endereço de leitura carrega o
  buffer e incrementa o endereço; leituras posteriores retornam o buffer e
  buscam o próximo byte.
- O endereço faz wrap de `0x3FFF` para `0x0000`.
- A leitura de status limpa as flags que o hardware define como clear-on-read
  e cancela o primeiro byte de um comando de controle pendente, conforme
  confirmado por testes e documentação.

### Modos de tela

O modo é selecionado por M1/M2/M3 nos registradores R1/R0. Os quatro modos
oficiais a validar são:

1. **Graphics I:** 32×24 células de 8×8; tabela de cores por grupo de padrões.
2. **Graphics II:** 32×24 células de 8×8; até 768 padrões com cores por linha;
   máscaras e sub-blocos dependem de R3/R4.
3. **Multicolor:** 64×48 blocos lógicos de 4×4, cada bloco com uma cor.
4. **Text:** 40×24 caracteres de 6×8 dentro da área de 256×192; sprites desativados.

Combinações não documentadas dos bits de modo não devem ser classificadas como
modos oficiais. Só implementar combinações adicionais com fonte e regressão.

### Sprites

- Até 32 entradas na tabela de atributos; o valor Y `0xD0` encerra a lista.
- Máximo de quatro sprites avaliados por linha visível; o quinto não é exibido e
  o status guarda seu índice.
- O bit early-clock desloca o sprite 32 pixels para a esquerda.
- Sprite 16×16 usa grupos de quatro padrões; magnificação dobra a escala.
- A colisão depende da sobreposição de pixels ativos dos padrões, não da cor
  visível. Portanto, a cor zero não deve ser usada como atalho para pular a
  análise de colisão. Pixels fora da tela e sprites não exibidos por excesso
  exigem testes contra a descrição do fabricante.
- A ordem da varredura importa para determinar qual quinto sprite é reportado.

## Mudanças desta etapa

A rotina de composição de sprites foi reorganizada por linha de varredura e
ordem de entrada da tabela. Isso corrige dois problemas no caminho de renderização:

1. o índice do quinto sprite deve refletir a primeira linha de varredura que
   excede o limite, não simplesmente a primeira entrada que ultrapassa quatro
   sprites em qualquer linha;
2. sprites de cor zero não pintam pixels, mas seus pixels ativos continuam sendo
   considerados na colisão; sprites posteriores visíveis ainda podem ser
   desenhados por transparência.

A avaliação das flags de colisão e quinto sprite ocorre agora ao fechar cada
linha visível e a leitura de status limpa as flags, que podem ser novamente
levantadas por linhas seguintes. A resolução temporal ainda é por linha, não por
dot; a composição RGB continua sendo refeita como um quadro completo e não
reproduz mudanças de VRAM/registradores ocorridas no meio de uma linha.

## Plano de validação

### P0 — regressões determinísticas

- [x] VRAM de 16 KiB, wrap de endereço e leitura bufferizada.
- [x] Escrita de registrador pela sequência de dois bytes.
- [x] Clear-on-read de status e cancelamento de comando parcial.
- [x] Modo Graphics I básico e Graphics II básico.
- [x] Colisão de sprites e prioridade de pintura.
- [x] Índice de quinto sprite pela ordem raster.
- [x] Flags de colisão/overflow atualizadas por linha e colisão considerada fora da área visível.
- [ ] Matriz completa de endereços-base/máscaras de R2–R6.
- [ ] Sprite 8×8 e 16×16 em cada quadrante, clipping, magnificação e early-clock.
- [ ] Colisão com cor zero, pixels sobrepostos em bordas, sprites parcialmente
  fora da tela e terminador da tabela.
- [ ] Matriz completa de modos, combinações de bits e valores reservados.

### P1 — testes diferenciais

Usar ROMs de diagnóstico/homebrew legalmente obtidas e uma versão de referência
identificada por nome, versão/commit e configuração. Comparar:
- framebuffer por linha/quadro;
- valor e instante de mudança do status;
- valor do buffer e endereço interno após sequências de I/O;
- interrupção observada pela CPU;
- estado após reset.

Guardar hashes dos artefatos, resultados e instruções de reprodução. Não usar
screenshots sem correspondência exata de jogo, ROM, região, estado e frame.

### P2 — precisão temporal

A avaliação de status já ocorre por linha, mas a CPU ainda avança o VDP somente
após cada instrução Z80. Substituir esse avanço por eventos determinísticos de
linha e, quando necessário, de dot, com scheduler comum e sem threads independentes
ou alteração dos clocks lógicos. O framebuffer ainda é composto ao final do
quadro; mudanças de VRAM/registradores durante a varredura não afetam retroativamente
os pixels já emitidos. Só então medir se acessos concorrentes CPU/VDP precisam de
arbitragem ou contenção explícita.

### P3 — cor e saída analógica

Separar a paleta digital lógica da conversão RGB e documentar diferenças entre
TMS9918A NTSC e variantes relacionadas. Sinal composto, níveis elétricos e
artefatos de vídeo são outra camada; não os confundir com a geração digital dos
pixels.

## Critérios para declarar o modelo completo

Não declarar o TMS9918A “completo” até que:

- os oito registradores, seus bits documentados e efeitos colaterais estejam
  descritos e cobertos por testes;
- todos os quatro modos oficiais, tabelas e máscaras de endereço tenham testes
  de limite;
- sprites cubram tamanho, magnificação, prioridade, early-clock, colisão,
  overflow, clipping e terminador;
- status, VBlank e interrupção tenham sequência temporal reproduzível;
- testes diferenciais de diagnóstico sejam executados e arquivados;
- os limites de precisão temporal e de saída analógica estejam explicitamente
  documentados.

## Fontes

- Texas Instruments, *TMS9918A/TMS9928A/TMS9929A Video Display Processors Data
  Manual*, novembro de 1982:
  https://www.bitsavers.org/components/ti/TMS9900/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf
- Sean Young, *Texas Instruments TMS9918A VDP — Almost complete description
  including undocumented features*, versão 0.4.2 (2002):
  https://github.com/cbmeeks/TMS9918/blob/master/tms9918a.txt
- ChipDB, resumo de interface e registradores:
  https://www.msarnoff.org/chipdb/TMS9918A
