# Pesquisa técnica: núcleo Z80 do MAME

**Data da pesquisa:** 22/10/2026  
**Projeto analisado:** [mamedev/mame](https://github.com/mamedev/mame)  
**Revisão consultada:** [`1ee501dbef32ba9a4597d9f5c27bed4a0164e499`](https://github.com/mamedev/mame/commit/1ee501dbef32ba9a4597d9f5c27bed4a0164e499)  
**Objetivo:** extrair padrões de arquitetura, pontos de compatibilidade e tarefas úteis para a CPU Z80 deste projeto. Esta pesquisa não afirma equivalência entre os dois núcleos e não importa código do MAME.

## Resumo executivo

O MAME é uma referência útil não por ser infalível, mas porque explicita separações que ainda precisamos desenvolver: busca de opcode versus busca de operandos, acessos de memória versus I/O, ciclos de barramento, refresh, sinais externos, estado interno e diferenças entre variantes físicas do Z80. O próprio código do MAME documenta limitações e comportamentos dependentes da variante; portanto, deve ser tratado como implementação de referência comparativa, não como especificação definitiva.

**Prioridade imediata:** terminar o inventário dos opcodes e validar funcionalidade/flags; em seguida, evoluir do contador de T-states por instrução para um modelo de eventos de barramento. Não copiar a arquitetura completa do MAME nem implementar recursos de hardware que a etapa SG-1000 ainda não exige.

## Fontes primárias consultadas

Todas as referências abaixo apontam para a revisão MAME registrada acima:

- [`z80.cpp` — helpers de flags, estado, reset, execução e interrupções](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80/z80.cpp)
- [`z80.h` — interface do dispositivo, espaços de memória, callbacks e estado](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80/z80.h)
- [`z80.inc` — aliases de registradores e constantes internas](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80/z80.inc)
- [`z80.lst` — descrição declarativa dos opcodes, macros de acesso e temporização](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80/z80.lst)
- [`z80make.py` — gerador do despachante de execução a partir da lista de opcodes](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80/z80make.py)
- [`z80daisy.cpp` — arbitragem de interrupções em cadeia e sinalização RETI](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/machine/z80daisy.cpp)
- [`sg1000.cpp` — configuração MAME da plataforma SG-1000](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/mame/sega/sg1000.cpp)

O núcleo Z80 consultado contém avisos de licença BSD-3-Clause e créditos próprios. Esta documentação registra ideias e observações, sem copiar implementação. Se no futuro decidirmos reutilizar trechos de código, será necessária uma revisão explícita da licença, dos avisos de copyright e das obrigações de redistribuição.

## Descobertas e implicações para o nosso núcleo

### 1. A tabela de opcodes é uma fonte de auditoria, não apenas um decodificador

O MAME descreve instruções em `z80.lst` e gera o código de execução com `z80make.py`. A lista identifica famílias e prefixos, enquanto o gerador verifica inconsistências como definições de opcode não encontradas.

**Aplicação no projeto:**
- Criar um inventário independente e verificável por byte de opcode e por sequência de prefixos: base, `CB`, `ED`, `DD`, `FD`, `DD CB d` e `FD CB d`.
- Classificar cada combinação como documentada, alias documentado, não documentada, prefixo que altera a interpretação, prefixo ignorado ou combinação não aplicável.
- Ligar cada linha a handler, teste e estado de validação. Não inferir cobertura completa apenas porque um loop de testes percorre os 256 valores do segundo byte.
- Fazer a auditoria estática da matriz primeiro; não substituir o decodificador atual por um gerador antes de demonstrar que isso simplifica a manutenção.

### 2. Buscar opcode não é o mesmo que ler dados ou operandos

O núcleo separa o espaço de leitura de opcode (`AS_OPCODES`, quando configurado), a leitura de argumentos e o espaço de programa/dados. Também separa I/O do espaço de memória. Isso permite que sistemas anfitriões imponham comportamentos diferentes a fetches e acessos de dados, inclusive mapas de opcode especiais.

**Aplicação no projeto:**
- Definir operações explícitas no contrato de barramento: `opcode_fetch`, `operand_read`, `memory_read`, `memory_write`, `io_read` e `io_write`.
- Preservar o contrato atual até existirem testes que cubram a transição; evitar uma refatoração grande antes do inventário de opcodes.
- Em SG-1000, não presumir que endereços de I/O são apenas endereços de RAM. O mapa de portas é uma camada da plataforma.

### 3. T-states por instrução não bastam para precisão temporal

Em `z80.lst`, operações de fetch M1, leitura/escrita de memória e I/O passam por macros com custos próprios (`m_m1_cycles`, `m_mreq_cycles`, `m_iorq_cycles`). O núcleo também expõe sinais de refresh e de WAIT, e tem suporte a BUSREQ/BUSACK. O custo dos acessos e o estado externo podem afetar a execução.

**Aplicação no projeto:**
- Manter `step()` e `total_cycles` como camada de compatibilidade enquanto introduzimos um modelo interno de eventos de barramento.
- Modelar cada acesso com tipo, endereço, direção, valor e duração; contabilizar fetches M1, leituras/escritas e I/O separadamente.
- Permitir que a plataforma introduza estados de espera sem alterar arbitrariamente os clocks lógicos da CPU.
- Não declarar precisão de ciclo até que a sequência e o tempo dos acessos, inclusive instruções prefixadas, tenham testes próprios.
- WAIT/BUSREQ/BUSACK e refresh devem ser fases posteriores, implementadas quando os requisitos e evidências de hardware justificarem o custo.

### 4. O registrador R está ligado aos fetches M1 e ao refresh

O código MAME mantém os sete bits inferiores de R separados do bit alto, que preserva um comportamento próprio, e forma o endereço de refresh a partir de I e R. O arquivo de opcode contém ainda uma observação explícita sobre o momento do incremento de R em relação ao refresh em análises de contenção.

**Aplicação no projeto:**
- Criar testes para incremento de R em instruções de um e vários bytes, prefixos repetidos, interrupções e HALT.
- Verificar separadamente o bit 7 de R e o comportamento de refresh; não tratar R como um contador de oito bits que simplesmente incrementa em toda instrução.
- Documentar que o modelo atual não reproduz ainda um sinal de refresh de barramento ciclo a ciclo.

### 5. Flags e estado interno podem depender de detalhes que não aparecem no resultado aritmético

O MAME usa helpers específicos para ALU, `DAA`, `BIT` em `(HL)` e em `(IX/IY+d)`, operações de bloco e flags de I/O de bloco. Mantém `WZ` (comportamento conhecido como MEMPTR em outras referências) e estado `Q/QT), relacionado a determinados comportamentos não documentados de flags. Por exemplo, os bits X/Y de `BIT` em memória indexada não são calculados simplesmente como os bits do valor lido.

**Aplicação no projeto:**
- Priorizar comparação diferencial de todos os flags documentados: S, Z, H, P/V, N e C, incluindo preservação de flags.
- Testar `DAA` com uma matriz ampla de valores de A e combinações válidas/limítrofes de H, N e C; os poucos casos unitários atuais não bastam.
- Testar flags de LDI/LDD/LDIR/LDDR, CPI/CPD/CPIR/CPDR e IN/OUT de bloco, incluindo condição de repetição e saída antecipada.
- Investigar X/Y, `WZ/MEMPTR` e `Q` como estado explícito apenas na etapa de compatibilidade não documentada, após definir a variante de CPU e as fontes de evidência.
- Não copiar fórmulas do MAME sem entender o estado que elas assumem; comparar com o manual Zilog, testes independentes e, quando disponível, hardware.

### 6. Interrupções: há mais que saltar para um endereço

O MAME representa IFF1/IFF2, os modos de interrupção, NMI pendente na borda, HALT, reconhecimento de IRQ e atraso após EI. A interface também admite um encadeamento daisy-chain, que informa dispositivos sobre acknowledge e RETI.

O próprio comentário TODO do MAME declara que IM 0 não executa opcodes arbitrários fornecidos pelo barramento. Também documenta uma particularidade de `LD A,I/R` interrompida e diferenças entre variantes CMOS e NMOS. Logo, mesmo esse núcleo não deve ser tratado como autoridade perfeita para todos os modelos.

**Aplicação no projeto:**
- Cobrir NMI por borda, prioridade de NMI/INT, IFF1/IFF2, atraso de EI, DI, HALT e retorno de interrupção com testes de sequência.
- Implementar o comportamento IM 0 definido para o alvo, em vez de deixar uma limitação incidental: a CPU deve interpretar o byte fornecido pelo dispositivo de interrupção conforme o modelo adotado.
- Modelar o acknowledge de interrupção como interação com a plataforma, não como constante universal.
- Não implementar daisy-chain como abstração genérica sem um periférico SG-1000 que a exija; preservar uma interface simples de interrupção até surgir necessidade real.

### 7. Escolher a variante física faz parte da especificação

O cabeçalho de `z80.cpp` lista diferenças conhecidas entre NMOS/CMOS e fabricantes em `LD A,I/R`, `OUT (C),0` e flags X/Y de SCF/CCF. O núcleo MAME consultado assume um modelo Zilog NMOS e deixa algumas particularidades configuráveis ou desativadas.

O driver MAME de SG-1000 documenta CPUs vistas na plataforma como NEC D780C-1 ou Zilog Z8400A (revisão A), com clock nominal de 3,579545 MHz no modelo base.

**Aplicação no projeto:**
- Definir a revisão/variante-alvo antes de implementar comportamentos não documentados.
- Manter um registro separado de comportamento documentado, observado, inferido e ainda hipotético.
- Criar testes de variante somente quando a evidência de hardware/documentação sustentar a diferença.
- Não adicionar diferenças do Z180, R800 ou outros derivados ao Z80 clássico por conveniência.

### 8. Reset e estado serializável merecem testes explícitos

O MAME trata a inicialização do dispositivo e o reset como operações distintas. O código inicializa vários registradores e, no reset, limpa sinais e estados específicos; por exemplo, IX/IY recebem um valor inicial no start, enquanto o reset altera outros campos. Isso é uma decisão concreta do core MAME, não uma regra automaticamente aplicável a todas as revisões físicas.

**Aplicação no projeto:**
- Separar estado inicial determinístico do objeto de emulação de um reset de hardware definido pelo alvo.
- Criar testes explícitos para reset, preservação/limpeza de registradores, IFFs, HALT, flags e contadores.
- Planejar snapshot/restore determinístico do estado completo, útil para testes de regressão e depuração, sem introduzir serialização prematuramente.

## Particularidades da plataforma SG-1000 observadas no MAME

O driver `sg1000.cpp` é útil para a futura integração, sem substituir documentação de hardware:

- CPU base a 10,738635 MHz / 3 = 3,579545 MHz.
- Mapa de programa: cartucho em `0000–BFFF`; RAM de `C000–C3FF`, espelhada pela região superior conforme o mapa.
- O PSG SN76489A recebe escrita na porta `40`, espelhada nas portas de I/O correspondentes.
- O sinal READY do PSG está ligado à entrada WAIT da CPU (com inversão no callback), demonstrando que o estado do periférico pode suspender a CPU.

Esses detalhes devem virar testes de integração de plataforma quando barramento, PSG e agendamento temporal forem implementados. Não devem ser inseridos como lógica específica dentro do núcleo Z80.

## Lista priorizada de implementação

### P0 — antes de expandir a cobertura

1. **Matriz completa de opcodes:** uma linha por opcode/combinação de prefixos, classificação, handler, teste e estado de validação.
2. **Inventário de lacunas real:** listar explicitamente opcodes oficiais sem handler; separar aliases e combinações não documentadas.
3. **Testes diferenciais independentes:** comparar estado pós-instrução (registradores, flags, memória, I/O, PC/SP) e ciclos nominais com uma referência independente. MAME pode ser um dos comparadores, nunca o único oráculo.
4. **Definição do alvo:** Z80 clássico e variante compatível com SG-1000; registrar quais diferenças de silício são relevantes e quais permanecem desconhecidas.

### P1 — correção funcional e estado da CPU

5. Testes exaustivos ou gerados para flags de ALU, INC/DEC, rotações, BIT, DAA e condições de salto.
6. Validar flags e condições de repetição das operações de bloco ED.
7. Validar prefixos repetidos, prefixos ignorados, IXH/IXL/IYH/IYL, DD/FD-CB e wraparound de endereços.
8. Validar interrupções, atraso de EI, HALT, R, I e estado de reset; fechar a limitação de IM 0 conforme o alvo.

### P2 — temporalidade e integração

9. Introduzir eventos explícitos de fetch M1, operandos, memória e I/O sem quebrar a API atual de `step()`.
10. Integrar WAIT/estados de espera e refresh com evidências de hardware; só depois avaliar BUSREQ/BUSACK.
11. Criar testes de barramento para acesso, ordem, duração, espera e efeitos colaterais por instrução.
12. Integrar CPU, mapa de memória, VDP, PSG e entrada do SG-1000 como dispositivos separados e sincronizados por um modelo temporal explícito.

### P3 — compatibilidade não documentada

13. Pesquisar e testar `WZ/MEMPTR`, Q latch, flags X/Y e opcodes não documentados com fontes independentes.
14. Só então decidir quais peculiaridades de variantes NMOS/CMOS/NEC entram no modelo suportado.

## Limites da pesquisa

- Esta foi uma leitura estática do código-fonte MAME na revisão indicada, não uma execução do MAME contra o nosso emulador.
- Não foi realizada comparação binária automática nem benchmark.
- A implementação MAME contém TODOs explícitos e assume uma variante específica em alguns pontos.
- A lista de tarefas acima é uma proposta de trabalho; não significa que qualquer item tenha sido implementado no nosso código.
