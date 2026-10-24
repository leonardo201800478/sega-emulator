# Plano de fechamento das lacunas do Z80

**Data:** 23/10/2026  
**Repositório:** \`leonardo201800478/sega-emulator\`  
**Branch de referência:** \`main\`  
**Objetivo:** cruzar as lacunas da auditoria do núcleo, a matriz de opcodes, os testes já existentes e a literatura ZiLOG/redcode/Z80 para escolher correções de menor risco e critérios verificáveis.

## 1. Conclusão executiva

O maior risco atual não é ausência geral de decodificação: o núcleo já tem caminhos para instruções base, CB, ED, DD/FD e DD/FD-CB, além de testes sintéticos extensos para partes da ALU, DAA e BIT. O risco é tratar caminho de execução e ciclos nominais como se comprovassem conformidade do hardware.

A solução recomendada é **não reescrever o núcleo nem começar imediatamente uma migração total para execução ciclo a ciclo**. Primeiro, fechar as lacunas semânticas que podem ser testadas no modelo atual; em paralelo, definir contratos e testes para os comportamentos que dependem de eventos de barramento. A implementação temporal deve ser introduzida por etapas, sem quebrar a API existente nem atribuir precisão não demonstrada.

### Estado que já existe — não duplicar sem necessidade

Segundo a documentação e os arquivos presentes em \`main\`:

- \`tests/unit/test_z80_alu_exhaustive.py\`: varredura exaustiva de operandos de 8 bits para ADD/ADC/SUB/SBC/CP/AND/XOR/OR e INC/DEC, com oráculo sintético independente dos helpers da CPU.
- \`tests/unit/test_z80_daa_exhaustive.py\`: 2.048 combinações modeladas de A/N/H/C para DAA.
- \`tests/unit/test_z80_bit_flags_exhaustive.py\`: cobertura exaustiva do domínio de entrada modelado para BIT não indexado.
- \`tests/unit/test_z80_indexed_bit_flags.py\`: cobertura sintética extensa para BIT indexado, com hipótese explícita para X/Y.
- \`tests/unit/test_z80_alu_sampling.py\` e \`scripts/validate_z80_alu_16bit.py\`: amostragem determinística de operações de 16 bits; a varredura completa de todos os pares continua sendo uma tarefa longa.
- \`docs/z80-coverage.md\`: matriz de decodificação ED, testes de I/O de bloco e casos de repetição LDIR/LDDR/CPIR/CPDR.

Esses artefatos reduzem o risco funcional, mas são testes sintéticos: não comprovam por si sós equivalência com silício, variantes NMOS/CMOS nem temporização ciclo a ciclo. Os resultados históricos descritos na documentação devem ser distinguidos de qualquer nova execução.

## 2. Matriz cruzada de lacunas e soluções

Legenda de evidência: **D** = documentado por fonte primária; **R** = literatura de engenharia reversa ou implementação de referência; **T** = teste sintético existente; **H** = ainda exige teste diferencial/hardware ou decisão explícita de variante.

| ID | Lacuna atual | Evidência disponível | Solução proposta | Prioridade |
|---|---|---|---|---|
| GAP-01 | Máquina de estados de INT/NMI, EI delay, HALT e R não fechada | D + T parcial; auditoria aponta casos de borda ausentes | Criar matriz de transições e testes parametrizados de estado; corrigir somente divergências reproduzíveis | P0 |
| GAP-02 | Prefixos DD/FD repetidos/alternados e semântica H/L versus IXH/IXL | D + R; caminhos existem, caracterização incompleta | Criar testes de bytes consumidos, PC, R, flags, memória e ciclos nominais para sequências discriminantes | P0 |
| GAP-03 | Todos os 256 segundos bytes ED classificados, mas nem todas as semânticas/aliases validadas | D + R; smoke test de aceitação/rejeição existe | Gerar matriz de 256 linhas com classe, efeito esperado e fonte; testar handlers e aliases separadamente | P0 |
| GAP-04 | IM 0 e acknowledge de prefixos têm interpretações concorrentes | Manual ZiLOG e wiki redcode/Z80 são descritos como divergentes na auditoria | Isolar o problema num documento de decisão; desenhar teste que distingue origem de bytes, PC, R e sequência de acknowledge; não mudar a implementação antes da evidência | P0 |
| GAP-05 | Flags X/Y de operações de bloco e I/O de bloco, SCF/CCF e Q não têm confirmação independente suficiente | T sintético + R; Q ausente | Separar testes de flags documentadas dos testes de hipóteses não documentadas; investigar Q em sequências SCF/CCF | P1 |
| GAP-06 | WZ/MEMPTR ausente | R e artigos no catálogo; não é necessário para a maioria das instruções básicas | Especificar atualizações por família e só então adicionar estado e testes de salto, I/O, BIT indexado e bloco | P1 |
| GAP-07 | Temporização real do barramento não é observável | D; arquitetura atual retorna T-states agregados | Introduzir um traço determinístico de eventos/micro-operações antes de modelar contenção/WAIT | P1 arquitetural |
| GAP-08 | Não existe harness diferencial reprodutível no fluxo principal | T; catálogo lista exercitadores e testes de terceiros | Definir adaptador de estado/entrada/saída, manifesto de proveniência e comparação campo a campo; começar com pequenos testes licenciados | P1 |
| GAP-09 | Operações de 16 bits não têm varredura integral de todos os pares | T amostrado; ferramenta longa já existe | Executar a bateria prolongada em Colab quando necessário; preservar amostragem local curta e anexar log/artefato | P2 |
| GAP-10 | Reset explícito e política de variante não estão formalizados | D + R; estado inicial zerado é convenção do emulador | Definir \`reset()\` sem confundir estado determinístico do software com valores físicos garantidos; registrar alvo Zilog Z80 clássico e exceções conhecidas | P2 |
| GAP-11 | RETI não sinaliza controlador externo de prioridades | D; impacto depende da integração | Acrescentar evento/callback opcional no contrato de CPU/periférico quando houver daisy chain; não simular dispositivo inexistente | P2 |
| GAP-12 | Integração de CPU, I/O, VDP e PSG do SG-1000 ainda não forma máquina funcional | Arquitetura e README | Depois de estabilizar contratos de CPU/barramento, integrar portas e dispositivos com testes de integração; manter ROMs comerciais fora do repositório | Marco SG-1000 |

**Nota:** P0 significa fechar primeiro para evitar propagar erros funcionais; não significa que toda a implementação deva ser feita num único commit. GAP-07 é uma decisão arquitetural de alto impacto, mas deve começar com um protótipo mínimo de eventos, não uma migração em massa.

## 3. Sequência de implementação recomendada

### Fase A — Caracterização sem refatoração

**Objetivo:** obter evidência de comportamento sem alterar o núcleo.

1. **Interrupções:** inventariar o estado atual de \`request_int\`, \`request_nmi\`, \`_ei_delay\`, IFF1/IFF2, HALT e acknowledge. Testar NMI pendente durante HALT; INT mascarada e depois habilitada; EI seguido de DI, HALT, RETI/RETN e instrução normal; requests consecutivos.
2. **R/refresh:** verificar incrementos para fetch base, CB, ED, DD/FD, DD/FD-CB, HALT e entrada em NMI/INT. Distinguir incremento do registrador R de pulsos M1/refresh físicos, que o modelo atual ainda não representa.
3. **Prefixos:** testar DD DD, FD FD, DD FD, FD DD; prefixo antes de instrução que não usa IX/IY; DD/FD com H/L; DD/FD-CB com deslocamento; wrap de PC e deslocamento assinado.
4. **ED:** classificar cada segundo byte como instrução documentada, alias documentado/compatível, NOP/ignorado, comportamento não documentado ou explicitamente não suportado. A classificação deve apontar para uma fonte ou declarar a incerteza.
5. **IM 0:** preparar casos que discriminem as hipóteses sobre bytes de prefixo fornecidos pelo dispositivo versus bytes subsequentes lidos da memória. Guardar a questão em aberto até a evidência justificar uma decisão.

**Critério de saída:** testes reproduzíveis para cada caso, sem alterar resultados de referência apenas para fazê-los coincidir com a implementação.

### Fase B — Corrigir semântica documentada e manter regressões

Implementar correções pequenas por família, nesta ordem sugerida:

1. bugs confirmados em interrupções/R e prefixos;
2. ED documentado e aliases cujo comportamento-alvo esteja comprovado;
3. flags documentadas e bordas das operações de bloco;
4. semântica de I/O e endereçamento de portas que seja necessária para o SG-1000.

Cada correção deve incluir teste que falhava antes e passa depois. Não misturar num mesmo commit uma correção funcional, mudança de API e refatoração temporal.

### Fase C — Comportamentos não documentados

Investigar separadamente:

- **Q latch + SCF/CCF:** montar pares de sequências em que a instrução anterior altera F versus preserva F; usar pelo menos uma fonte independente da implementação-alvo. Manter a hipótese fora do comportamento obrigatório até a evidência ser suficiente.
- **WZ/MEMPTR:** desenhar tabela de atualização por família antes de criar atributo; cobrir saltos condicionais/absolutos, I/O, operações de bloco, retorno e BIT indexado conforme fontes aplicáveis.
- **X/Y:** separar os bits provenientes de operandos, resultado, byte alto do endereço e regras específicas de instruções indexadas/bloco.
- **Variantes:** documentar se o alvo é o Zilog Z80 NMOS clássico; diferenças observadas em CMOS/clones devem ser anotadas, não mescladas silenciosamente.

**Critério de saída:** cada regra não documentada tem hipótese, fonte, variante, teste discriminante, resultado e nível de confiança.

### Fase D — Modelo temporal incremental

Não adicionar atrasos arbitrários em \`step()\`. Primeiro introduzir um tipo interno de evento/micro-operação, por exemplo:

- fetch de opcode/M1;
- leitura de byte de operando;
- leitura/escrita de memória;
- leitura/escrita de I/O;
- acknowledge de interrupção;
- refresh;
- espera WAIT, quando suportada.

O evento deve carregar endereço, direção, valor quando conhecido, classe de ciclo e T-states/fase relativa. Inicialmente, gerar traços para poucas instruções representativas (NOP, LD r,n, LD (HL),A, CALL, CB BIT, ED IN/OUT, uma instrução indexada e INT/NMI). Comparar a soma dos eventos com os T-states nominais atuais. Expandir por família só depois dos testes.

**Compatibilidade:** preservar \`step() -> int\` durante a migração, implementando-o inicialmente como consumo de um conjunto de micro-operações. Não prometer precisão ciclo a ciclo até a ordem e as fases dos eventos serem validadas contra documentação e casos de referência. Contenção do VDP/WAIT só entra depois de existir um modelo de fase temporal consistente.

### Fase E — Harness diferencial e conformidade

O harness deve:
1. registrar origem, licença, versão/commit e hash do arquivo de teste;
2. inicializar PC, SP, registradores, flags, memória, I/O e interrupções de forma determinística;
3. executar um caso no núcleo Python e no oráculo selecionado;
4. comparar estado final, memória, I/O e ciclos que ambos realmente exponham;
5. parar no primeiro desvio e salvar caso mínimo reproduzível;
6. não equiparar ausência de traço temporal no oráculo a equivalência de barramento.

Começar por casos pequenos com licença/proveniência clara. ZEXDOC/ZEXALL e outras ROMs de teste devem ser obtidos e usados de acordo com as condições próprias; não incorporar binários ao Git sem revisão de licença. O redcode/Z80 pode servir de comparação, mas não copiar seu código: a licença e os avisos especiais precisam ser analisados antes de qualquer reutilização.

## 4. Critérios de aceitação mensuráveis

- [ ] Matriz de transições de INT/NMI/EI/HALT com testes para todos os estados relevantes.
- [ ] Matriz de prefixos DD/FD e DD/FD-CB com bytes consumidos, PC, R, flags, memória e ciclos nominais.
- [ ] 256 entradas ED classificadas; handlers documentados testados semanticamente, e aliases não confundidos com instruções documentadas.
- [ ] Questão de IM 0 resolvida por evidência, ou marcada explicitamente como pendente com teste discriminante registrado.
- [ ] Testes de flags de bloco dividem claramente comportamento documentado e hipóteses X/Y.
- [ ] Decisão fundamentada para Q e WZ/MEMPTR antes de adicionar estado interno.
- [ ] Protótipo de traço de barramento para um conjunto pequeno de instruções, mantendo a API atual.
- [ ] Harness diferencial com manifesto de licença/proveniência e saída reprodutível.
- [ ] pytest, Ruff e format check executados depois de cada mudança de código; testes longos separados dos testes locais curtos.
- [ ] README e matriz de cobertura atualizados com resultados reais, sem alegar execução não realizada.

## 5. Fontes cruzadas

- [Especificação ZiLOG consolidada](z80-zilog-manual-specification.md) — comportamento documentado, ciclos nominais, sinais e interrupções.
- [Auditoria técnica do núcleo](z80-code-audit-2026-10-22.md) — lacunas e riscos da implementação inspecionada.
- [Matriz de cobertura](z80-coverage.md) — estado de testes já existentes; deve ser consultada para evitar duplicação.
- [Matriz de opcodes](z80-opcode-matrix.md) — classificação estática de espaços de opcode.
- [Literatura técnica](z80-technical-literature.md) — índice de manuais, artigos, exercitadores e questões de licenciamento.
- [Pesquisa MAME](z80-mame-reference.md) e [pesquisa comparativa de emuladores](z80-undocumented-behavior-research.md) — referências secundárias; concordância entre projetos não prova, por si só, comportamento de hardware.
- [Issue #10 — investigação de IM 0](https://github.com/leonardo201800478/sega-emulator/issues/10).

## 6. Limites e estado de validação

Este documento é um plano de engenharia derivado de leitura estática dos arquivos e da documentação do repositório. Não altera o núcleo CPU, não declara nenhuma lacuna corrigida e não afirma que testes foram executados nesta etapa. Os testes de ALU/DAA/BIT existentes devem ser preservados; as próximas alterações devem mirar primeiro as lacunas abertas da matriz, em vez de repetir trabalho já coberto.
