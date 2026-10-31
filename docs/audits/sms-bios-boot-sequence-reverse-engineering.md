# Engenharia reversa: sequência de boot da BIOS do Sega Master System

**Estado:** pesquisa documental inicial; não altera o runtime.  
**Escopo:** BIOS US/Europeia v1.3 e variante incomum v2.0.  
**Baseline documental:** `main` em `767d89b227b328b8d17b677944a832353240a74e` (2026-10-31).  
**Objetivo:** registrar comportamento útil para o core SMS sem transformar hipóteses de leitura de dump em requisitos de hardware.

## 1. Resumo executivo

A BIOS do Master System é código Z80 executado pela CPU, acompanhado de tabelas e dados gráficos/textuais. O dump binário, portanto, mistura opcodes, operandos, dados de inicialização, fontes/tile data, tabelas e strings legíveis. Encontrar uma string ASCII no arquivo não demonstra, por si só, quando ou como ela é exibida: é necessário localizar referências no disassembly e acompanhar o fluxo de execução.

A documentação pública do SMS Power descreve a BIOS US/Europeia v1.3 como uma rotina de apresentação inicial seguida por sondagem dos slots de mídia. A v1.3 verifica cabeçalho, região e checksum. A variante v2.0, embora visualmente semelhante e com texto de versão atualizado, altera significativamente a detecção: não executa a mesma validação de cabeçalho/checksum; verifica se os primeiros 513 bytes do slot não são todos iguais.

**Conclusão importante:** v1.3 e v2.0 não podem ser modeladas como uma única rotina genérica de validação de cartucho. A revisão de BIOS é um perfil de comportamento observável.

## 2. Evidência e níveis de confiança

Este registro separa três classes:

- **Documentado por disassembly/referência técnica:** comportamento descrito em fonte especializada e acompanhado de código ou mapa de bits.
- **Relatado no material de entrada:** strings e sequência descritas a partir de um dump, ainda sem identificação binária reproduzível nesta tarefa.
- **Pendente de confirmação:** detalhe que exige dump identificado, hash, disassembly e/ou captura de hardware.

Não foram fornecidos nesta tarefa os arquivos binários das BIOS v1.3/v2.0, seus tamanhos, SHA-256, procedência ou dumps de execução. Assim, este documento não declara ter realizado uma análise byte a byte dos arquivos.

## 3. Sequência de boot — visão funcional

### 3.1 Reset e vetor inicial

O Z80 inicia a execução após reset com o PC no endereço `0x0000`. No Master System, a BIOS pode ocupar o espaço de memória inferior compartilhado com os slots de Card, Cartridge e Expansion. A seleção de qual dispositivo responde nessa região é controlada pelo hardware de mapeamento, acessível pelo port de I/O `0x3E`.

Para o emulador, não basta inicializar o PC em zero: o estado inicial do registrador de controle de memória e a visibilidade da BIOS precisam corresponder ao perfil da máquina/revisão.

**Atenção:** não assumir que toda máquina SMS possui a mesma BIOS, tamanho de ROM, conteúdo de inicialização ou jogo interno.

### 3.2 Apresentação inicial da BIOS v1.3

A BIOS US/Europeia v1.3 apresenta uma animação de abertura SEGA/Master System e reproduz o jingle de dois tons antes da sondagem dos slots. A referência consultada descreve uma espera fixa de aproximadamente 1,67 s na v1.3.

A tela de instruções, quando nenhum slot contém mídia reconhecida, inclui a combinação secreta para iniciar Snail Maze. A combinação é **Up + botão 1 + botão 2 no controle do jogador 1**; a referência informa que outros botões podem também estar pressionados sem impedir o acionamento.

### 3.3 Sondagem de slots na v1.3

A ordem documentada é:

1. Card;
2. Cartridge;
3. Expansion.

A BIOS usa o port `0x3E` para selecionar/mapear cada slot para a região de memória acessível pela CPU. Para cada slot, copia 16 bytes para RAM e procura um cabeçalho válido nos offsets `0x7FF0`, `0x3FF0` e `0x1FF0` do espaço do slot.

Quando encontra a assinatura ASCII `TMR SEGA`, executa as validações seguintes:

- **Região:** os quatro bits superiores do último byte do cabeçalho devem ser `0x4` para a BIOS US/Europeia v1.3; caso contrário, apresenta `SOFTWARE ERROR`.
- **Checksum:** soma as regiões indicadas pelo nibble inferior do byte final do cabeçalho e compara o resultado com o checksum armazenado. Divergência também resulta em `SOFTWARE ERROR`.
- **Mapeamento inicial:** a rotina não inicializa os registradores de paginação do mapper antes da busca. Para o cabeçalho usual em `0x7FF0`, a configuração inicial do mapper precisa expor os dados esperados no slot/banco correspondente.

Se nenhum cabeçalho for encontrado nos três slots, a BIOS considera os slots vazios para essa rotina e mostra a tela de instruções.

### 3.4 Transferência de controle

A seleção de BIOS, slots e RAM é controlada pelo port `0x3E`. Os bits relevantes são ativos em nível baixo:

| Bit | Função |
|---:|---|
| 7 | Expansion slot |
| 6 | Cartridge slot |
| 5 | Card slot |
| 4 | Work RAM |
| 3 | BIOS ROM |
| 2 | I/O |

Escrever 0 habilita a função correspondente; escrever 1 a desabilita. A referência do SMS Power descreve que a BIOS executa uma rotina copiada para RAM, desabilita a ROM da BIOS e habilita os slots um a um para sondagem.

Ao iniciar a mídia reconhecida, a BIOS precisa deixar o mapeamento do slot correto ativo. Não modelar isso como “desligar fisicamente o chip” nem como um simples salto abstrato sem reproduzir o estado de `0x3E`, a prioridade dos slots e o endereço/banco efetivamente visível. O último valor escrito em `0x3E` é também guardado em `0xC000` pela v1.3, conforme a referência consultada.

## 4. Diferença crítica: BIOS v1.3 versus v2.0

A variante US/Europeia v2.0 é descrita como incomum e derivada da v1.3, mas a rotina de detecção de cartucho é diferente:

| Característica | v1.3 | v2.0 |
|---|---|---|
| Busca por `TMR SEGA` | Sim | Não, na rotina de detecção descrita |
| Validação de região | Sim | Não |
| Cálculo/comparação de checksum | Sim | Não |
| Teste de presença | Cabeçalho e validações subsequentes | Compara os primeiros 513 bytes |
| Tela sem mídia | Instruções | Instruções, com texto `V2.0` |
| Snail Maze | Presente | Presente |

Na v2.0, os primeiros 513 bytes são copiados para RAM. A rotina compara o primeiro byte com os 512 seguintes; se encontrar qualquer diferença, considera que há dados de ROM e aceita o slot para execução. Se todos forem iguais, a rotina considera que não encontrou mídia. Isso é um teste heurístico de presença, não validação de integridade ou autenticidade.

A fonte também documenta um efeito colateral: a área temporária usada para a cópia dos 513 bytes sobrepõe-se parcialmente à cópia em RAM do tilemap de Snail Maze, causando pequenos glitches nos cantos superiores do labirinto em hardware real.

**Implicação para o emulador:** implementar validação de cabeçalho/checksum como comportamento obrigatório de toda BIOS SMS seria incorreto. A rotina precisa ser selecionada por revisão de BIOS.

## 5. Formato do cabeçalho de cartucho

A referência de desenvolvimento descreve um cabeçalho de 16 bytes, que pode estar nos offsets `0x1FF0`, `0x3FF0` ou `0x7FF0`. A posição `0x7FF0` é a usual nos jogos comerciais conhecidos.

| Offset relativo ao cabeçalho | Campo |
|---|---|
| +0x00..+0x07 | ASCII `TMR SEGA` |
| +0x08..+0x09 | Reservado |
| +0x0A..+0x0B | Checksum de 16 bits little-endian |
| +0x0C..+0x0E | Código do produto |
| +0x0E low nibble | Revisão do software |
| +0x0F high nibble | Região/sistema |
| +0x0F low nibble | Código de tamanho da ROM |

O byte final contém dois nibbles com significados distintos. O código de região `0x4` representa SMS Export; `0x3` representa SMS Japan. A documentação observa que somente a BIOS SMS de exportação efetivamente verifica esse campo.

O checksum não deve ser confundido com um hash criptográfico: é uma soma de 16 bits usada pela rotina de validação da BIOS. A extensão e a seleção das regiões somadas dependem do código de tamanho. A própria documentação registra valores de tamanho pouco usados ou problemáticos; não inferir o tamanho real do arquivo somente desse nibble.

## 6. Afirmações do material inicial que exigem cautela

As seguintes descrições foram apresentadas no material de entrada, mas não devem ser tratadas como fatos universais sem confirmar a revisão e o disassembly:

1. **“A BIOS limpa RAM de `0xC000` a `0xDFFF`.”** A RAM física do SMS é 8 KiB e aparece espelhada no intervalo `0xC000`–`0xFFFF`. A faixa efetivamente inicializada e o padrão escrito precisam ser confirmados na rotina da BIOS analisada.
2. **“A BIOS sempre inicializa VDP, VRAM, paleta e PSG antes de verificar mídia.”** A v1.3 executa animação e áudio antes da detecção, mas a sequência exata de cada escrita de registrador/porta deve ser extraída do disassembly. Não presumir que todas as revisões executam a mesma sequência.
3. **“Toda BIOS valida cabeçalho, região e checksum.”** Falso para a v2.0 descrita acima e para outras famílias/revisões japonesas que usam testes diferentes.
4. **“O comando final desativa fisicamente a BIOS e salta para `0x0000`.”** A abstração correta é uma alteração do mapeamento via `0x3E`, seguida da execução no espaço de endereços agora fornecido pelo slot. A sequência de instruções e o valor escrito devem ser obtidos no disassembly da revisão específica.
5. **“Os textos encontrados no dump são necessariamente exibidos.”** Strings como `WELCOME TO THE SEGA MASTER SYSTEM`, `SOFTWARE ERROR`, `V1.3` e instruções de inserção foram relatadas no material de entrada. A presença, offset, codificação, revisão e uso de cada string ainda devem ser conferidos nos arquivos binários identificados.

## 7. Consequências para a arquitetura do Sega Emulator

Este estudo deve orientar os contratos do SMS, sem colocar a lógica de BIOS dentro da CPU Z80:

- **Z80:** executa instruções e transações de barramento; não conhece “BIOS”, “cartucho válido” ou “handoff”.
- **SMS memory/bus:** resolve a região `0x0000`–`0xBFFF` conforme o estado do port `0x3E`, com prioridade e visibilidade dos slots.
- **I/O:** implementa o latch do port `0x3E` e seus efeitos de habilitação/desabilitação, respeitando bits ativos baixos.
- **Cartridge/mapper:** expõe o estado de paginação inicial; a BIOS v1.3 não deve ser usada como justificativa para inicializar automaticamente bancos que o hardware não inicializa.
- **BIOS image:** é firmware opcional/configurável por perfil/revisão. O conteúdo da BIOS é executado como ROM pela CPU; não substituir sua lógica por um atalho de boot.
- **Determinismo:** mesma revisão de BIOS, mídia, estado inicial e entradas devem produzir o mesmo trace de bus e o mesmo resultado.

A futura implementação deve registrar o valor inicial e cada escrita em `0x3E`, o slot selecionado, os bytes lidos durante a sondagem, o resultado das verificações e o ponto de handoff. Esses dados permitem comparar o emulador com disassemblies e traces externos.

## 8. Plano de confirmação por dump

Para transformar esta pesquisa documental em uma auditoria reproduzível:

1. Identificar cada arquivo local de BIOS por tamanho, SHA-256, CRC32 e procedência; não versionar os bytes no Git.
2. Associar cada hash a uma revisão/território usando catálogos e comparação de conteúdo, não apenas o nome do arquivo.
3. Extrair strings ASCII e seus offsets, mas tratar isso somente como inventário de dados.
4. Desassemblar o código Z80 com endereços de ROM e mapear referências cruzadas para strings, tabelas e rotinas.
5. Marcar basic blocks de inicialização, sondagem de slots, checksum/região, renderização da tela e Snail Maze.
6. Criar testes com ROMs sintéticas: cabeçalho válido, cabeçalho ausente, região incompatível, checksum inválido, slot vazio, slot com bytes uniformes e slot com variação.
7. Comparar v1.3 e v2.0 em testes separados; incluir Card/Cartridge/Expansion e estados de mapeamento de `0x3E`.
8. Validar traces e resultados em emulador de referência e, quando disponível, hardware real.

Não adicionar dumps de BIOS comerciais, ROMs, screenshots protegidos ou dados binários proprietários ao repositório. Registrar hashes, offsets, resultados e instruções de reprodução.

## 9. Critérios de aceitação para promover os achados

- [ ] SHA-256 e identificação da revisão para cada BIOS examinada.
- [ ] Disassembly com offsets e referências cruzadas arquivado ou referenciado por fonte/licença.
- [ ] Sequência de escritas no port `0x3E` extraída e testada.
- [ ] Ordem Card → Cartridge → Expansion confirmada para v1.3.
- [ ] Validação de cabeçalho, região e checksum reproduzida para v1.3.
- [ ] Teste dos 513 bytes reproduzido para v2.0.
- [ ] Diferenças de RAM/VDP/PSG confirmadas sem inferência a partir de strings.
- [ ] Testes determinísticos e regressões SG-1000/SMS aprovados no SHA candidato.

**Estado atual deste documento:** pesquisa documental inicial. Não representa implementação, não altera o estado de compatibilidade da BIOS e não declara PASS de hardware ou temporal.

## 10. Referências

1. SMS Power!, [BIOSes — Development](https://www.smspower.org/Development/BIOSes). Fonte principal para diferenças entre BIOSes, fluxo da v1.3, validação de slots e algoritmo da v2.0. Inclui referência a disassemblies comentados.
2. SMS Power!, [ROM Header — Development](https://www.smspower.org/Development/ROMHeader). Formato do cabeçalho, offsets, campos, região e códigos de tamanho.
3. SMS Power!, [Port $3E — Development](https://www.smspower.org/Development/Port3E). Mapeamento de BIOS/slots, bits ativos baixos e uso do port durante a detecção.
4. SMS Power!, [RAM — Development](https://www.smspower.org/Development/RAM). Mapeamento e espelhamento da RAM de 8 KiB no espaço `0xC000`–`0xFFFF`.
5. ZiLOG, [Z80 CPU User Manual, UM008011-0816](https://www.zilog.com/docs/z80/um0080.pdf). Referência primária para semântica da CPU Z80; não substitui documentação específica do hardware SMS.
