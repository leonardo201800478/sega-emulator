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
| GAP-04 | IM 0 e acknowledge de prefixos ainda exigem validação diferencial | O núcleo e os testes sintéticos já executam opcodes de acknowledge RST, NOP, `LD A,n`, `CALL` e prefixos CB/ED/DD; permanecem dúvidas de conformidade física, ciclos e variantes | Comparar estado, bytes consumidos, PC, R, pilha e ciclos com referência independente; manter a implementação estável até identificar divergência reproduzível | P0 |
| GAP-05 | Flags X/Y de operações de bloco e I/O de bloco, SCF/CCF e Q não têm confirmação independente suficiente | T sintético + R; Q ausente | Separar testes de flags documentadas dos testes de hipóteses não documentadas; investigar Q em sequências SCF/CCF | P1 |
| GAP-06 | WZ/MEMPTR ausente | R e artigos no catálogo; não é necessário para a maioria das instruções básicas | Especificar atualizações por família e só então adicionar estado e testes de salto, I/O, BIT indexado e bloco | P1 |
| GAP-07 | Temporização real do barramento não é observável | D; arquitetura atual retorna T-states agregados | Introduzir um traço determinístico de eventos/micro-operações antes de modelar contenção/WAIT | P1 arquitetural |
| GAP-08 | Não existe harness diferencial reprodutível no fluxo principal | T; catálogo lista exercitadores e testes de terceiros | Definir adaptador de estado/entrada/saída, manifesto de proveniência e comparação campo a campo; começar com pequenos testes licenciados | P1 |
| GAP-09 | Operações de 16 bits não têm varredura integral de todos os pares | T amostrado; ferramenta longa já existe | Executar a bateria prolongada no workflow estendido do GitHub Actions; preservar amostragem local curta e publicar log/artefato | P2 |
| GAP-10 | Reset explícito implementado; política de variante ainda pendente | O construtor delega a `reset()`; teste cobre registradores, interrupções pendentes, contador, preservação de barramento/RAM e retomada por NOP. Execução do teste ainda precisa ser confirmada localmente/CI | Formalizar o alvo Zilog Z80 clássico e as exceções de variante; não interpretar o estado determinístico zerado como garantia elétrica do chip | P2 parcial |
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
5. **IM 0:** casos sintéticos já caracterizam opcodes externos e prefixos CB/ED/DD, com operandos subsequentes lidos da memória. Próximo passo: comparar a origem dos bytes, PC, R, pilha e ciclos com referência independente; não alterar a semântica sem uma divergência reproduzível.

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
- [ ] IM 0 validado diferencialmente para opcodes/prefixos de acknowledge, origem dos operandos, PC, R, pilha e ciclos; ou divergências remanescentes documentadas com teste discriminante.
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


## Progresso de implementação — reset determinístico da CPU

Em 23/10/2026, foi adicionado `Z80Cpu.reset()`. O construtor agora associa os barramentos e delega a inicialização do estado interno ao método. O reset zera os registradores principais e alternativos, IX/IY, I/R, PC/SP, IFF1/IFF2, modo de interrupção, HALT, atraso de EI, solicitações de NMI/INT, opcode temporário de IM 0 e contador de T-states. O método preserva as referências aos barramentos e não limpa a memória.

Foi acrescentado `test_reset_clears_cpu_state_and_pending_interrupts_without_clearing_memory`, que verifica o contrato, inclusive a execução de um NOP após o reset. O teste foi adicionado ao repositório; **ainda não há resultado de execução nesta etapa**. A política de variante física permanece pendente.

## 4. Cruzamento com suites externas

**Fontes consultadas em 24/10/2026.** Este cruzamento separa vetores de estado final, programas exercitadores e evidência temporal. Não tratar todas as fontes como oráculos equivalentes.

| Fonte | Tipo e evidência declarada pela própria fonte | Lacunas que ajuda a cobrir | Limites / decisão para o projeto |
|---|---|---|---|
| [SingleStepTests/z80](https://github.com/SingleStepTests/z80) | JSON por sequência de instrução; cada arquivo contém 1.000 casos com estado inicial/final, RAM, portas e amostras de sinais/ciclos. Inclui PC, registradores, I/R, IFF, IM, Q e WZ. O README informa que os vetores foram gerados a partir do núcleo Z80 do ares e depois corrigidos; não os descreve como captura direta de silício. | GAP-02/03/04/05/06/08; também fornece casos para flags, registradores, RAM e I/O. Pode alimentar um harness reprodutível de estado inicial/final. | Primeira opção para um subconjunto automatizado, mas é uma referência de implementação com possíveis erros, não prova independente de hardware. Comparar primeiro estado final e RAM/portas; não exigir igualdade de traço de barramento enquanto o núcleo não modelar micro-operações (GAP-07). Não importar todo o conjunto no CI curto. |
| [raxoft/z80test](https://github.com/raxoft/z80test) | Exercitadores executáveis (z80full, z80doc, z80flags, z80docflags, z80ccf, z80memptr); o README diz que os resultados esperados foram obtidos num ZX Spectrum de 48K com Zilog Z80 real. Licença declarada: MIT. | GAP-03/05/06/08; especialmente conformidade documentada, flags X/Y, comportamento SCF/CCF associado ao Q e efeitos de MEMPTR em BIT. | Evidência independente mais forte para as áreas cobertas, mas não é um conjunto JSON unitário: é um programa para uma máquina Z80/Spectrum. Adiar execução end-to-end até haver ambiente compatível para carregar/executar o exercitador; não afirmar que os testes já rodam no núcleo isolado. Selecionar a variante Zilog como alvo e manter os testes específicos de variante separados. |
| [z80test — releases](https://github.com/raxoft/z80test/releases) | A versão 1.2a documenta correção do teste MEMPTR para instruções de bloco repetidas. O histórico também registra versões dos testes SCF/CCF para CPUs NEC e ST. | GAP-05/06/10: ajuda a evitar tratar diferenças de variante como bug universal. | Fixar uma versão/release e registrar checksum/licença quando a suíte for integrada. Não usar CRCs de variante diferente como critério de falha do alvo Zilog. |
| [jgenesis](https://github.com/jsgroth/jgenesis) e [Gearsystem](https://github.com/drhelius/Gearsystem) | Emuladores de sistemas Sega úteis como referências de arquitetura e integração; jgenesis separa o núcleo Z80 e executores de testes. Gearsystem cobre SG-1000/SMS/Game Gear. | GAP-08/12: estrutura de harness, isolamento do núcleo e integração com a máquina Sega. | Usar para comparação conceitual e de resultados observáveis, não copiar implementação sem revisão de licença. Os repositórios são GPL-3.0; não importar código GPL para este projeto sem decisão explícita de licenciamento. |

### Subconjunto recomendado para o CI curto

1. **Primeiro lote (semântica de estado final):** escolher um arquivo JSON por opcode/família já suportada e com impacto no marco atual: 00 (NOP, controle do harness), 37/3F (SCF/CCF), CB (BIT/flags), ED (família estendida), DD e FD (prefixos), além de um caso representativo de operação de bloco e um de I/O quando o runner conseguir mapear portas semânticas. Confirmar os nomes e a disponibilidade dos arquivos no repositório externo antes de fixar a lista; os nomes acima são uma seleção por opcode, não uma afirmação de que todos os casos passam hoje.
2. **Limitar a execução:** no PR, executar um número fixo pequeno de vetores por arquivo (por exemplo, 10–25 casos determinísticos por arquivo), com seed/índices fixos. Não baixar ~1,6 milhão de casos em cada execução. A suíte completa fica num workflow manual/noturno e publica um relatório por opcode.
3. **Comparar apenas campos suportados:** estado dos registradores, PC/SP, flags, memória alterada e operações de porta quando o adaptador puder observá-las. Registrar explicitamente campos ignorados, como WZ/MEMPTR se não existir no estado da CPU, em vez de removê-los silenciosamente. Os ciclos da fonte não devem ser comparados como se o núcleo atual fosse ciclo a ciclo.
4. **Classificar divergências, não mascará-las:** cada diferença vira documented mismatch, variant mismatch, unsupported model feature, reference-vector suspect ou implementation bug, com fonte e reprodução mínima. Não ajustar o esperado para a saída atual da CPU.
5. **Segundo lote (validação independente):** quando existir um caminho reprodutível para executar o exercitador, rodar primeiro z80doc/z80docflags; só então z80full/z80flags, e deixar z80ccf/z80memptr como gates separados até a política de variante e o suporte Q/WZ estarem definidos.

### Critérios de aceitação do harness

- [ ] Manifesto registra URL, versão/commit, licença, método de geração declarado pela fonte e campos comparados.
- [ ] Um teste externo falha com mensagem contendo arquivo, nome/índice do vetor, opcode e primeiro campo divergente.
- [ ] O subconjunto do PR é determinístico, curto e não depende de rede em cada execução depois que os dados aprovados estiverem versionados.
- [ ] O workflow prolongado publica relatório e continua separado do gate rápido.
- [ ] Casos não suportados são relatados explicitamente; não são convertidos em passes.
- [ ] A documentação diferencia testes sintéticos existentes, vetores derivados de outro núcleo e resultados medidos em hardware real.

**Conclusão do cruzamento:** SingleStepTests é a rota prática para criar o primeiro harness automatizado de estado final; raxoft/z80test é a validação independente posterior para documentadas, Q/SCF/CCF e MEMPTR. Nenhuma das duas substitui o trabalho futuro de traço de barramento para precisão temporal (GAP-07).

## 5. Adaptador SingleStepTests implementado

O módulo `src/sega_emulator/testing/z80_singlestep.py` implementa a primeira versão do adaptador para os vetores JSON de [SingleStepTests/z80](https://github.com/SingleStepTests/z80). Ele usa uma RAM plana de 64 KiB e um barramento de I/O de teste, sem alterar o mapa de memória físico do SG-1000.

### Executar

Depois de instalar o projeto e as dependências de desenvolvimento (`python -m pip install -e ".[dev]"`), baixe ou clone o corpus externo separadamente e execute, por exemplo:

```powershell
python -m sega_emulator.testing.z80_singlestep caminho\para\v1\00.json --limit 25 --report-json artifacts\z80-singlestep-00.json
```

`--limit 25` seleciona os primeiros 25 vetores de forma determinística; `--limit 0` executa todos os vetores do arquivo e `--start-index` permite reproduzir uma faixa. O adaptador não acessa a rede nem baixa dados automaticamente. O relatório inclui contagens e divergências por vetor.

### Contrato e limites conhecidos

- Compara os registradores representados pelo núcleo (`A/F`, registradores gerais, pares alternativos, `PC/SP`, `IX/IY`, `I/R`, `IFF1/IFF2`, modo de interrupção, atraso de `EI`, `Q` e `WZ`), além de RAM e transações de porta quando presentes no vetor.
- Usa RAM plana de 64 KiB, pois os vetores descrevem um único passo de CPU e não o mapa físico da máquina SG-1000.
- Não compara `cycles`/pinos: o núcleo ainda retorna T-states agregados e não oferece uma sequência de eventos de barramento ciclo a ciclo.
- Se o vetor exigir estado não modelado ativo (por exemplo, `P` diferente de zero) ou um opcode não implementado, ele é marcado como `unsupported`, nunca como aprovado. O processo termina com código diferente de zero para divergências/erros e, por padrão, também para vetores não suportados; `--allow-unsupported` apenas permite continuar com código zero quando só houver itens não suportados.
- O teste unitário usa vetores sintéticos mínimos para validar o adaptador e não conta como resultado de conformidade do corpus externo.

### Próximos gates

1. **Concluído:** fixar a revisão upstream e verificar SHA-256 e tamanho dos arquivos por meio do manifesto versionado.
2. Ampliar a cobertura de prefixos e opcodes com os arquivos disponíveis no corpus, mantendo separados aprovados, divergentes, não suportados e excluídos.
3. Modelar ou formalmente restringir o estado interno `p`, em vez de contar esses vetores como conformes.
4. Implementar um traço de eventos de barramento e só então comparar os arrays `cycles`.
5. Manter a bateria completa como auditoria prolongada; o subconjunto atual é diagnóstico, não gate obrigatório de conformidade.


## 6. Auditoria diferencial exploratória com corpus externo

O workflow `.github/workflows/z80-singlestep-audit.yml` baixa, durante a execução, 11 arquivos JSON selecionados de [SingleStepTests/Z80](https://github.com/SingleStepTests/z80), sempre da revisão upstream fixa `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` (commit assinado e verificado pelo GitHub). A revisão está registrada em [SingleStepTests/Z80 commit](https://github.com/SingleStepTests/z80/commit/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7). O corpus não é copiado como código-fonte para o repositório do emulador; os arquivos baixados e o relatório JSON são preservados como artefato da execução por 30 dias.

O manifesto `tools/audits/singlestep-corpus-manifest.json` registra o commit, a licença MIT com URL imutável e o SHA-256 e tamanho em bytes esperados para cada arquivo. O verificador `tools/audits/verify_singlestep_corpus.py` valida todos os arquivos baixados antes da auditoria. Se um hash, tamanho ou arquivo não corresponder ao manifesto, o workflow falha antes de executar os vetores. O relatório inclui a revisão upstream e os hashes correspondentes. Assim, alterações futuras na branch `main` do corpus não alteram silenciosamente os dados usados pelo teste.

O script `tools/audits/run_singlestep_corpus.py` exclui explicitamente vetores com o campo interno `p` diferente de zero, pois esse estado não é modelado pelo núcleo atual. Ele registra separadamente os casos executados, divergentes, não suportados e excluídos por esse estado. Divergências são resultados de auditoria e não interrompem a geração do relatório; falhas de download ou de formato interrompem o workflow.

**Limitação essencial:** o teste compara estado arquitetural representado e RAM, mas não compara o array `cycles` do corpus. Portanto, mesmo um vetor aprovado não demonstra precisão ciclo a ciclo nem equivalência de barramento. O workflow é diagnóstico e não deve ser tratado como gate de conformidade até a revisão dos primeiros resultados e das limitações do modelo.

A licença MIT foi conferida no arquivo `LICENSE` da mesma revisão fixa. O workflow guarda no artefato `SOURCE.txt` o commit utilizado e inclui uma cópia do manifesto. Os hashes são calculados sobre os bytes dos arquivos baixados, não sobre JSON reserializado.


### Resultado da primeira execução diferencial (24/10/2026)

O workflow executou 11 arquivos do corpus, com 1.000 vetores por arquivo (**11.000 vetores totais**). No estado atual do código, o relatório registrou **5.401 aprovados, 0 divergentes e 5.599 excluídos** porque o campo interno `p` estava ativo. Os vetores excluídos não são contados como aprovados.

A primeira execução com os vetores de I/O expôs divergências no registrador WZ:
- `OUT (n),A`: WZ precisava preservar o byte alto do endereço da porta e incrementar apenas o byte baixo, inclusive no wrap de `FFh` para `00h`.
- `IN r,(C)` e `OUT (C),r`: WZ precisava receber `BC + 1`, usando o BC anterior à instrução, inclusive quando a leitura altera B.

Esses comportamentos foram corrigidos em `src/sega_emulator/cpu/z80.py`, com testes de regressão para leitura de I/O derivada do traço, atualização de WZ e wrap do byte baixo. A execução seguinte eliminou as divergências nos 11 arquivos selecionados. O resultado é evidência diferencial útil para esses vetores e campos, **não** uma certificação geral do Z80: os 5.599 vetores excluídos ainda precisam de um modelo para o estado `p`, e os traços ciclo a ciclo continuam fora da comparação.

O artefato JSON e os vetores usados ficam no workflow [Z80 SingleStepTests Differential Audit](https://github.com/leonardo201800478/sega-emulator/actions/workflows/z80-singlestep-audit.yml), por 30 dias.


## Fase A2 — Expansão inicial do corpus diferencial (24/10/2026)

Após fixar a revisão upstream e validar o manifesto SHA-256, a auditoria diferencial foi ampliada de 11 para **25 arquivos de vetores**. O incremento cobre:

- **Base:** JR/JR NZ, LD rr,nn, LD (nn),HL, INC H, LD H,n, LD HL,(nn), LD (nn),A e casos de fronteira de PC/flags já representados pelos vetores escolhidos.
- **CB:** RLC (HL) e BIT 0,B, ampliando para operação sobre memória e teste de bit em registrador.
- **ED:** SBC HL,BC, NEG e RETN, além do vetor ED 40 existente, ampliando aritmética de 16 bits e retorno de interrupção.
- **DD:** LD B,(IX+d), complementando LD IX,nn com leitura indexada.

Os vetores adicionais seguem no mesmo commit upstream imutável e cada arquivo tem tamanho e SHA-256 fixados no manifesto. Esta etapa amplia a amostragem por família, mas **não equivale à cobertura integral dos 256 opcodes de cada espaço**; também não compara os traços de barramento ciclo a ciclo. Vetores com estado interno `p` ativo continuam excluídos explicitamente e devem ser reportados como excluídos, nunca como aprovados.
