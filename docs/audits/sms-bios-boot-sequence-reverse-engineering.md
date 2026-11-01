# Engenharia reversa: sequência de boot da BIOS do Sega Master System

**Estado:** pesquisa documental ampliada; runtime não alterado.  
**Escopo primário:** BIOS US/Europeia v1.3 (1986); comparação com a variante incomum v2.0 (1987).  
**Baseline da PR:** \`767d89b227b328b8d17b677944a832353240a74e\`.  
**Evidência binária local:** NÃO ANALISADA nesta alteração; hashes e procedência dos dumps ainda não foram fornecidos.  
**Objetivo:** registrar o comportamento sustentado por documentação/disassembly público e manter explícitas as lacunas que dependem de análise dos dumps ou de hardware.

## 1. Resumo executivo

A BIOS do Master System é código Z80 executado no próprio barramento da máquina. Ela apresenta a abertura, sonda os slots e, quando encontra mídia aceita pela rotina da revisão em uso, deixa o slot selecionado visível para a CPU. A BIOS não deve ser substituída por um atalho de boot no emulador: a CPU precisa executar o firmware real e as alterações de mapeamento devem ocorrer pelo hardware modelado.

A BIOS US/Europeia v1.3 procura cabeçalho \`TMR SEGA\` em três offsets, exige código de região \`4\` e calcula checksum de 16 bits sobre faixas definidas pelo nibble inferior do byte \`0x7FFF\`. A v2.0 não faz essa validação: testa variação entre os primeiros 513 bytes. A BIOS japonesa v2.1 usa outro teste, baseado na variação dos bits médios dos primeiros 256 bytes. Essas rotinas não são intercambiáveis.

**Conclusão arquitetural:** BIOS é firmware executável e específico por revisão/modelo. Parser de cabeçalho de ROM, identificação de mídia, mapper e política de detecção da BIOS são responsabilidades distintas.

## 2. Classificação de evidência

- **DOCUMENTADO:** descrito em referência técnica especializada e/ou acompanhado de trecho de disassembly público.
- **RELATADO, NÃO REPRODUZIDO:** informação apresentada a partir de dumps ou observações, mas ainda sem hash, disassembly e trace reproduzíveis neste projeto.
- **LACUNA — ANALISAR:** exige inspeção de dump identificado, execução instrumentada ou captura em hardware.
- **NÃO IMPLEMENTAR COMO FATO:** afirmação contradita pelas referências consultadas ou generalização indevida.

Esta atualização consultou a documentação técnica pública do SMS Power, incluindo o resumo da BIOS v1.3/v2.0/v2.1, o formato do cabeçalho e o port \`0x3E\`. Isso não equivale a uma auditoria independente dos arquivos binários de BIOS.

## 3. Reset, estado inicial e configuração de hardware

### 3.1 Reset da CPU

**DOCUMENTADO — CPU Z80:** após reset, o Z80 inicia a busca de instruções em \`PC = 0x0000\`. Isso descreve o estado arquitetural da CPU, não todo o estado elétrico do console.

**LACUNA — ANALISAR:** a sequência física de power-on/reset do SMS, duração do sinal, estado de todos os latches, estado inicial exato do port \`0x3E\`, habilitação inicial de BIOS/RAM/slots e eventuais diferenças entre revisões de placa ainda precisam ser documentados com fonte de hardware ou medição.

Não assumir que o valor inicial do port \`0x3E\` seja um valor arbitrário escolhido pelo emulador. O estado deve ser derivado da documentação do circuito e confirmado por dump/trace ou hardware.

### 3.2 Port \`0x3E\` e arbitragem dos slots

**DOCUMENTADO:** BIOS, Card, Cartridge e Expansion compartilham o espaço de endereços \`0x0000–0xBFFF\`. O port de I/O \`0x3E\` controla as linhas de enable que arbitram qual dispositivo responde.

| Bit | Função | Semântica documentada |
|---:|---|---|
| 7 | Expansion slot | ativo em nível baixo |
| 6 | Cartridge slot | ativo em nível baixo |
| 5 | Card slot | ativo em nível baixo |
| 4 | Work RAM | ativo em nível baixo |
| 3 | BIOS ROM | ativo em nível baixo |
| 2 | I/O | ativo em nível baixo |
| 1–0 | desconhecidos | não atribuir função sem evidência |

Escrever \`0\` habilita a função correspondente; escrever \`1\` a desabilita. Os bits 7, 6, 5 e 2 têm exceções/semânticas de ausência de efeito em alguns modelos, conforme a documentação do port.

A BIOS executa código copiado para RAM para desabilitar a ROM da BIOS e habilitar os slots um a um durante a detecção. A prioridade documentada para a v1.3 é Card → Cartridge → Expansion.

**LACUNA — ANALISAR:** obter a sequência completa de valores escritos em \`0x3E\`, incluindo valor inicial, cada transição, leituras do latch e valor final por caminho (mídia aceita, mídia inválida, nenhum slot). O documento técnico descreve a função geral, mas a implementação do emulador precisa de valores e ordem exatos extraídos do disassembly da revisão.

### 3.3 Primeira instrução

**DOCUMENTADO:** o PC inicia em \`0x0000\`; a primeira instrução vem do dispositivo que o mapa inicial deixa visível nesse endereço. Para um perfil com BIOS habilitada, a BIOS fornece esse conteúdo.

**LACUNA — ANALISAR:** confirmar para cada modelo-alvo a combinação inicial de BIOS, slots, RAM e I/O. Não modelar o reset apenas como \`PC = 0\` sem estabelecer o estado inicial do barramento.

## 4. Inicialização da BIOS v1.3

A referência da BIOS v1.3 documenta a animação SEGA/Master System e o jingle de dois tons antes da sondagem dos slots. A sequência exata de instruções e escritas de registradores precisa ser confirmada no disassembly completo.

| Afirmação | Estado | Observação / trabalho necessário |
|---|---|---|
| Executa \`DI\` no início | LACUNA — ANALISAR | Localizar opcode e posição no fluxo; não inferir apenas pela necessidade de setup. |
| Define \`SP = 0xDFFF\` | LACUNA — ANALISAR | Confirmar valor e momento exatos no disassembly. |
| Limpa \`0xC000–0xDFFF\` com zero | LACUNA — ANALISAR | A RAM física é 8 KiB e é espelhada em \`0xC000–0xFFFF\`; identificar os endereços efetivamente escritos e o padrão. |
| Envia \`0x9F, 0xBF, 0xDF, 0xFF\` ao SN76489 | LACUNA — ANALISAR | Confirmar sequência, temporização, ordem dos canais e se há outras escritas. |
| Desliga display antes de limpar VRAM | LACUNA — ANALISAR | Extrair escritas de registradores do VDP e a ordem. |
| Limpa toda a VRAM | LACUNA — ANALISAR | Determinar faixa, valor, mecanismo (CPU/VDP) e tempo consumido. |
| Carrega paleta e fonte/tile data | PARCIALMENTE DOCUMENTADO | Identificar tabelas/rotinas e offsets no dump da BIOS alvo. |
| Apresenta animação e jingle antes de sondar slots | DOCUMENTADO | A referência da v1.3 descreve o bumper animado e o jingle de dois tons. |

**Importante sobre RAM:** o SMS tem 8 KiB de Work RAM, visíveis com espelhamento em \`0xC000–0xFFFF\`. A frase “limpa \`0xC000–0xDFFF\`” não está confirmada como rotina exata da BIOS. O emulador não deve transformar esse intervalo relatado em requisito sem verificar o código.

**Importante sobre áudio/vídeo:** a presença de animação e jingle demonstra que a BIOS usa VDP e PSG, mas não comprova, por si só, a sequência de inicialização descrita no material recebido. A ordem de disable display, VRAM clear, palette, tile/font upload e writes ao PSG fica pendente de disassembly.

## 5. Detecção de mídia — BIOS US/Europeia v1.3

### 5.1 Ordem e busca

**DOCUMENTADO:** a BIOS v1.3 sonda os slots nesta ordem:

1. Card;
2. Cartridge;
3. Expansion.

Para cada slot, copia 16 bytes para RAM e procura um cabeçalho nos offsets \`0x7FF0\`, \`0x3FF0\` e \`0x1FF0\`, nessa ordem. Se não encontrar \`TMR SEGA\` em nenhum dos três locais, considera o slot vazio para essa rotina.

A BIOS não inicializa registradores de paginação do mapper antes dessa busca. Portanto, o mapper/cartucho precisa expor os dados esperados no estado inicial. Para o cabeçalho usual em \`0x7FF0\`, a referência destaca a necessidade de o slot 1 apresentar o banco inicial apropriado quando bancos/janelas forem de 16 KiB.

### 5.2 Região: correção importante

**DOCUMENTADO:** a BIOS US/Europeia v1.3 exige que o nibble alto do byte \`0x7FFF\` seja exatamente \`0x4\` (SMS Export). Se o valor for diferente, rejeita a mídia com \`SOFTWARE ERROR\`.

**NÃO IMPLEMENTAR COMO FATO:** a afirmação de que a BIOS v1.3 aceita códigos \`3\` e \`4\` está incorreta segundo a descrição da rotina v1.3 consultada. O código \`3\` identifica SMS Japan no formato do cabeçalho, mas a BIOS de exportação descrita verifica igualdade com \`4\`, não uma lista \`{3,4}\`.

Os códigos de região/sistema do cabeçalho são:

| Nibble alto | Identificação |
|---:|---|
| \`3\` | SMS Japan |
| \`4\` | SMS Export |
| \`5\` | Game Gear Japan |
| \`6\` | Game Gear Export |
| \`7\` | Game Gear International |

A BIOS japonesa v2.1 usa uma estratégia diferente de detecção, pois software japonês legado não tinha a mesma exigência de cabeçalho. Não generalizar a rotina da BIOS de exportação para consoles japoneses.

### 5.3 Checksum: algoritmo e faixas

**DOCUMENTADO:** a rotina pública da BIOS v1.3 soma bytes em um acumulador de 16 bits \`DE\`. Para cada byte em \`(HL)\`, soma o valor ao byte baixo \`E\`, propaga o carry para \`D\`, avança \`HL\` e decrementa o contador \`BC\`. O overflow além de 16 bits é descartado (aritmética módulo \`65536\`).

A rotina soma as faixas definidas pelo nibble inferior do byte \`0x7FFF\`. O cabeçalho final de 16 bytes não é incluído na faixa inferior: por exemplo, código \`0xC\` cobre \`0x0000–0x7FEF\`, não \`0x0000–0x7FFF\`. Para ROMs maiores, uma segunda faixa é somada a partir de \`0x8000\`, assumindo o mapper Sega padrão.

| Nibble baixo | Faixa inferior | Faixa superior | Total declarado |
|---:|---|---|---:|
| \`A\` | \`0x0000–0x1FEF\` | — | 8 KiB |
| \`B\` | \`0x0000–0x3FEF\` | — | 16 KiB |
| \`C\` | \`0x0000–0x7FEF\` | — | 32 KiB |
| \`D\` | \`0x0000–0xBFEF\` | — | 48 KiB |
| \`E\` | \`0x0000–0x7FEF\` | \`0x8000–0xFFFF\` | 64 KiB |
| \`F\` | \`0x0000–0x7FEF\` | \`0x8000–0x1FFFF\` | 128 KiB |
| \`0\` | \`0x0000–0x7FEF\` | \`0x8000–0x3FFFF\` | 256 KiB |
| \`1\` | \`0x0000–0x7FEF\` | \`0x8000–0x7FFFF\` | 512 KiB |
| \`2\` | \`0x0000–0x7FEF\` | \`0x8000–0xFFFFF\` | 1 MiB |
| \`3–9\` | não suportado | não suportado | — |

Os valores \`D\` e \`2\` são apontados como problemáticos/buggy em implementações da BIOS; não assumir que a tabela descreve um checksum confiável para todos os valores. Também não inferir o tamanho real do arquivo a partir do nibble: a documentação registra jogos cujo campo indica faixa menor que o tamanho efetivo da ROM.

O valor armazenado no cabeçalho em \`0x7FFA–0x7FFB\` é descrito na página ROM Header do SMS Power como palavra little-endian. O parser futuro deve confirmar essa interpretação contra os bytes de dumps conhecidos e o disassembly da BIOS; há documentação histórica de tags/formatos que descreve a ordem de bytes de outra forma, então a decisão deve ficar vinculada à fonte e ao teste de referência.

**Correção do material recebido:** o nibble \`0xC\` significa 32 KiB, não 48 KiB; \`0xD\` significa 48 KiB. Os códigos \`A–B\` são 8/16 KiB, \`E–F\` são 64/128 KiB, \`0\` é 256 KiB, \`1\` é 512 KiB e \`2\` é 1 MiB.

### 5.4 Resultado da validação

- Cabeçalho ausente em todos os offsets testados: slot tratado como vazio.
- Cabeçalho presente, mas região diferente de \`4\`: rejeição com \`SOFTWARE ERROR\`.
- Região aceita, mas checksum divergente: rejeição com \`SOFTWARE ERROR\`.
- Região e checksum aceitos: BIOS inicia a mídia do slot selecionado.

A referência indica que o último valor escrito no port \`0x3E\` também é salvo em \`0xC000\), permitindo que software reative o slot de origem quando necessário.

**LACUNA — ANALISAR:** mapear a rotina de checksum completa para cada nibble suportado, incluindo preparação de \`HL/BC/DE\`, acesso à faixa superior através do mapper, leitura do checksum armazenado e branch exato em sucesso/falha. A tabela documenta o comportamento esperado, mas os testes do projeto devem reproduzir o disassembly.

## 6. Handoff: transferência para o cartucho

**DOCUMENTADO em nível funcional:** o port \`0x3E\` controla a visibilidade da BIOS e dos slots; a documentação descreve a execução de código em RAM para desabilitar a BIOS e habilitar slots durante a sondagem. Após selecionar mídia válida, a BIOS executa o conteúdo do slot.

**LACUNA — ANALISAR:** o relato recebido especifica uma rotina copiada para \`0xC000\`, um salto para essa rotina, escrita final em \`0x3E\` e salto explícito para \`0x0000\`. A referência consultada não é suficiente para confirmar todos esses detalhes de sequência. Precisamos obter do disassembly v1.3:
- endereço e bytes da rotina temporária copiada para RAM;
- origem, comprimento e destino exatos da cópia;
- valor escrito no port \`0x3E\` no handoff;
- instrução de salto e endereço efetivo de destino;
- estado de \`SP\`, \`IFF1/IFF2\`, registradores e latch do port no instante da transferência.

Não descrever o evento como “desligamento físico do chip” no modelo lógico. O emulador deve reproduzir a alteração de mapeamento/enable e a resolução do barramento. Só especificar o salto exato depois de confirmar o disassembly.

## 7. Caminho sem mídia, erro e Snail Maze

**DOCUMENTADO para v1.3:** se nenhum slot for reconhecido, a BIOS mostra uma tela de instruções. Durante essa tela, a combinação Up + 1 + 2 no controle 1 aciona o jogo interno Snail Maze; a referência informa que outros botões também podem estar pressionados.

**DOCUMENTADO:** falha de região ou checksum resulta em \`SOFTWARE ERROR\`.

**LACUNA — ANALISAR:** a descrição recebida afirma que a BIOS fica em polling contínuo das portas de controle e abandona um loop ao detectar a combinação. Confirmar portas exatas, máscaras, polaridade, cadência de polling, debounce (se houver), endereço de entrada do Snail Maze e se há diferença entre o caminho “nenhuma mídia” e “mídia inválida”.

Não generalizar a tela/fluxo de erro para outras BIOSes: a BIOS protótipo v1.0, v2.0, v2.1 e revisões posteriores apresentam diferenças documentadas.

## 8. Diferenças entre revisões

| Comportamento | US/Europeia v1.3 | US/Europeia v2.0 | Japonesa v2.1 |
|---|---|---|---|
| Detecção principal | cabeçalho + região + checksum | variação nos primeiros 513 bytes | variação nos bits médios dos primeiros 256 bytes |
| Exige \`TMR SEGA\` | sim | não | não |
| Valida região/checksum | sim | não | não pela rotina descrita |
| Mídia japonesa antiga sem header | rejeitada pela rotina de cabeçalho | pode ser aceita pelo teste de variação | compatível com o método de detecção |
| Conteúdo/tela interna | Snail Maze | Snail Maze | instruções japonesas; música diferente |

Na v2.0, o código copia 513 bytes para RAM e compara o primeiro byte com os 512 seguintes. Se qualquer byte divergir, considera que existe dado de ROM; se todos forem iguais, falha na detecção. A cópia temporária sobrepõe parcialmente dados usados pelo tilemap do Snail Maze e provoca glitches nos cantos superiores.

A BIOS japonesa v2.1 não usa o mesmo algoritmo da v2.0: examina 256 bytes e compara os bits mascarados por \`0x3C\` com os do primeiro byte. São algoritmos diferentes e devem ter perfis de teste separados.

## 9. Formato do cabeçalho Sega — parser não é validação da BIOS

O cabeçalho possui 16 bytes e pode estar nos offsets \`0x1FF0\`, \`0x3FF0\` ou \`0x7FF0\`. \`0x7FF0\` é a posição usual em jogos conhecidos.

| Offset absoluto usual | Campo |
|---|---|
| \`0x7FF0–0x7FF7\` | ASCII \`TMR SEGA\` |
| \`0x7FF8–0x7FF9\` | reservado |
| \`0x7FFA–0x7FFB\` | checksum 16-bit |
| \`0x7FFC–0x7FFD\` | código do produto, BCD |
| \`0x7FFE\`, nibble alto | parte adicional do código do produto |
| \`0x7FFE\`, nibble baixo | revisão do software |
| \`0x7FFF\`, nibble alto | região/sistema |
| \`0x7FFF\`, nibble baixo | código de faixa/tamanho para checksum |

O código do produto ocupa 2,5 bytes, não os três bytes completos \`0x7FFC–0x7FFE\`. O nibble baixo de \`0x7FFE\` é a revisão do software e não pode ser incluído no código do produto. O checksum é little-endian segundo a página ROM Header do SMS Power.

**Limites para um futuro parser Rust:**
- validar cada posição candidata sem assumir que toda ROM tenha cabeçalho;
- distinguir parse estrutural de validação de BIOS v1.3;
- não exigir que uma ROM japonesa antiga tenha \`TMR SEGA\`;
- não tratar nibble de tamanho como tamanho físico definitivo do arquivo;
- representar códigos desconhecidos/buggy sem fallback silencioso para 32 KiB;
- não calcular checksum sobre o arquivo inteiro: aplicar as faixas específicas e o mapper assumido pela rotina;
- não misturar a validação de BIOS com identidade No-Intro, perfil do console ou seleção do mapper.

A implementação Rust enviada como proposta no material de entrada **não foi incorporada**. Ela tem um fallback arbitrário para 32 KiB e um modelo de tamanho que precisa ser revisado. A criação de parser deve ser uma tarefa separada, com vetores de ROM sintética e ROMs de referência legalmente utilizáveis.

## 10. Afirmações do material recebido: decisão de auditoria

| Afirmação recebida | Decisão nesta revisão |
|---|---|
| Reset leva PC a \`0x0000\` | Confirmado como estado arquitetural Z80; estado elétrico/mapeamento inicial SMS permanece pendente. |
| Port \`0x3E\` inicia com valor específico que ativa BIOS e desativa slots | Não afirmar valor; pendente de fonte de hardware/dump. |
| BIOS começa com \`DI\` | Pendente de disassembly. |
| \`SP = 0xDFFF\` | Pendente de disassembly. |
| Limpa \`0xC000–0xDFFF\` | Pendente; RAM é 8 KiB espelhada. |
| PSG recebe exatamente \`9F BF DF FF\` | Pendente de disassembly/trace. |
| VDP desliga display, limpa VRAM e carrega paleta/fonte em sequência fixa | Pendente de disassembly/trace. |
| Busca cabeçalho em \`7FF0/3FF0/1FF0\` e slots Card → Cartridge → Expansion | Confirmado para BIOS US/Europeia v1.3. |
| BIOS v1.3 aceita regiões \`3\` e \`4\` | Corrigido: a rotina descrita exige nibble alto exatamente \`4\`. |
| Checksum é soma de 16 bits com overflow descartado | Confirmado. |
| Checksum soma tudo menos o cabeçalho | Simplificação incompleta; faixas dependem do nibble e podem incluir faixa superior via mapper. |
| Checksum armazenado em little-endian | Documentado pela página ROM Header; confirmar nos dumps/disassembly usados como vetores. |
| Copia rotina de handoff para \`0xC000\` e faz \`JP 0x0000\` | Pendente de confirmação no disassembly completo. |
| Nenhum cartucho/erro sempre resulta na mesma tela e loop | Diferenciar slot vazio, região inválida, checksum inválido e revisão de BIOS. |
| Adaptador 44→50 pinos permite boot de qualquer jogo japonês | Não afirmar; depende de conectividade, cabeçalho, hardware e revisão de BIOS. Não é consequência automática do código de região. |
| Nibble de região deve selecionar FM e framerate do emulador | Não implementar: região do cabeçalho é metadado da mídia, não autoridade para configurar hardware físico. FM e padrão de vídeo pertencem ao MachineProfile explícito. |

## 11. Consequências para a arquitetura Sega Emulator

- **Z80:** executa instruções e transações; não conhece BIOS, validade de cartucho ou handoff.
- **SMS bus/memory:** resolve BIOS, Card, Cartridge, Expansion e RAM conforme os latches/enable reais, incluindo prioridades e estados iniciais.
- **Port I/O \`0x3E\`:** implementar bits documentados e manter bits desconhecidos sem semântica inventada.
- **Mapper/cartucho:** expor o estado inicial real; não inicializar bancos para “ajudar” a BIOS a encontrar o cabeçalho.
- **BIOS:** imagem opcional por perfil/modelo/revisão; executar o firmware real, sem HLE disfarçado de BIOS original.
- **ROM parser:** metadados e validação estrutural não substituem a execução da rotina de validação da BIOS.
- **MachineProfile:** região física, clock, VDP e FM são propriedades do console/perfil, não inferências automáticas do nibble da ROM.
- **Determinismo:** mesma BIOS, mídia, perfil, estado inicial e entradas devem produzir os mesmos traces de barramento e resultado.

Instrumentação recomendada: log determinístico de reset, valor inicial e escritas em \`0x3E\`, slot selecionado, bytes lidos nos offsets de header, faixas de checksum, resultado das comparações e endereço/estado no handoff. Evitar logar ou versionar dumps protegidos.

## 12. Lacunas priorizadas para análise

| ID | Prioridade | Lacuna | Evidência necessária | Critério de encerramento |
|---|---|---|---|---|
| BIOS-01 | P0 | Identidade dos dumps v1.3/v2.0/v2.1 | tamanho, SHA-256, CRC32, procedência e revisão | manifesto local reproduzível sem incluir BIOS no Git |
| BIOS-02 | P0 | Reset e estado inicial de \`0x3E\` | documentação de circuito + disassembly/trace ou hardware | estado por modelo/revisão definido e testado |
| BIOS-03 | P0 | Sequência completa de writes \`0x3E\` | disassembly anotado e trace | valores, ordem e caminho de execução testados |
| BIOS-04 | P0 | Checksum v1.3 | disassembly completo + ROMs de referência | faixas, contador, endianess e branches reproduzidos por testes |
| BIOS-05 | P1 | Rotina de handoff | disassembly de v1.3 | cópia RAM, destino, valor \`0x3E\`, salto e estado da CPU documentados |
| BIOS-06 | P1 | Inicialização CPU/RAM | disassembly e trace de writes | \`DI\`, \`SP\`, faixa/padrão RAM confirmados |
| BIOS-07 | P1 | Inicialização PSG/VDP | disassembly e trace de I/O/memória | ordem, valores e duração confirmados |
| BIOS-08 | P1 | Polling de controles/Snail Maze | disassembly + trace de input | portas, máscaras, polaridade, cadência e vetor de entrada confirmados |
| BIOS-09 | P1 | Diferenças de tela/erro por revisão | disassemblies e dumps identificados | matriz v1.0/v1.3/v2.0/v2.1 sem generalizações |
| BIOS-10 | P2 | Compatibilidade física de cartuchos japoneses/adaptadores | pinout, esquemático e testes com cartuchos | comportamento descrito por combinação real de hardware/mídia |

### Próxima sequência de trabalho

1. Obter localmente os dumps de BIOS que o usuário está analisando e gerar apenas manifesto de metadados (nome local, tamanho, SHA-256, CRC32, procedência); não copiar os binários ao repositório.
2. Identificar revisão com comparação contra referências públicas e registrar ambiguidades.
3. Obter/estudar os disassemblies comentados da BIOS v1.3 e v2.0 referenciados pelo SMS Power.
4. Criar anotações por endereço: reset, setup, PSG/VDP, sondagem, checksum, handoff, tela de erro e Snail Maze.
5. Produzir vetores sintéticos para cabeçalho, região, cada faixa de checksum, mapper inicial e casos de slot vazio/constante/variável.
6. Implementar somente depois de especificação testável; preservar SG-1000 e executar a matriz de CI do SHA candidato.

## 13. Critérios de aceitação da pesquisa

- [ ] Hashes e identificação das BIOS examinadas.
- [ ] Disassembly referenciado com endereços e referências cruzadas.
- [ ] Estado inicial e sequência de escritas em \`0x3E\` confirmados.
- [ ] Rotina de checksum v1.3 reproduzida byte a byte, incluindo faixas e endianess.
- [ ] Handoff e estado da CPU no destino confirmados.
- [ ] Rotinas de RAM, PSG e VDP confirmadas sem inferência a partir de strings.
- [ ] Polling de input e Snail Maze confirmados.
- [ ] Testes diferenciais separados para v1.3, v2.0 e v2.1.
- [ ] Regressões SG-1000/SMS e CI executadas no SHA candidato.

**Estado atual:** pesquisa documental ampliada; várias lacunas explícitas permanecem abertas. Nenhum runtime foi alterado. Nenhum teste de código ou temporal foi executado por esta alteração. Não declarar \`IMPLEMENTED\`, \`QUICK_PASS\`, \`SHORT_PASS\`, \`LONG_PASS\`, \`INTEGRATION_PENDING\` ou \`TEMPORAL_PASS\` para o comportamento da BIOS com base somente neste documento.

## 14. Referências técnicas

1. SMS Power!, [BIOSes — Development](https://www.smspower.org/Development/BIOSes). Fluxo da v1.3, código de checksum, faixas por nibble, diferenças v2.0/v2.1 e disassemblies comentados.
2. SMS Power!, [ROM Header — Development](https://www.smspower.org/Development/ROMHeader). Campos do cabeçalho, offsets, checksum little-endian, produto/revisão, região e tamanhos.
3. SMS Power!, [Port $3E — Development](https://www.smspower.org/Development/Port3E). Compartilhamento do espaço \`0x0000–0xBFFF\`, bits ativos baixos e uso pela BIOS.
4. SMS Power!, [RAM — Development](https://www.smspower.org/Development/RAM). Capacidade e espelhamento da Work RAM.
5. ZiLOG, [Z80 CPU User Manual, UM008011-0816](https://www.zilog.com/docs/z80/um0080.pdf). Semântica da CPU; não substitui a documentação específica do circuito SMS.
6. SMS Power!, [SDSC ROM Tag Specification](https://www.smspower.org/Development/SDSCROMTagSpecification). Referência histórica adicional; há diferenças de convenção/descrição de endianess em documentos relacionados, a confirmar contra os bytes de ROM e a rotina BIOS efetivamente usada.

