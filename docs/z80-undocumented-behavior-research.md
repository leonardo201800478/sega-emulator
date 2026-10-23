# Pesquisa comparativa: comportamentos Z80 pouco documentados

**Data:** 22/10/2026  
**Objetivo:** usar implementações públicas de Gearsystem, MEKA, Genesis Plus GX e ares para transformar comportamentos pouco documentados do Z80 em hipóteses testáveis e tarefas de implementação. Esta pesquisa não altera o núcleo CPU nem afirma que os comportamentos estejam validados em hardware.

## Conclusão executiva

As quatro bases ajudam de formas diferentes:

- **Gearsystem** é a comparação mais diretamente alinhada ao nosso alvo SG-1000/SMS/GG. Mantém estado auxiliar `WZ`, `Q/QTemp`, implementa tratamento explícito de prefixos repetidos e anuncia suporte a comportamentos não documentados; o projeto declara que passa no ZEXALL.
- **ares** é uma referência moderna e legível para separar algoritmos aritméticos, handlers de instrução, acessos de memória e tempo. Modela `WZ`, `Q`, flags X/Y e custos por acesso. É especialmente útil para converter observações em testes unitários pequenos.
- **Genesis Plus GX** tem um núcleo Z80 maduro com histórico explícito de correções de flags X/Y, BIT, MEMPTR/WZ, DAA e ciclos. Sua própria história informa que o core deriva de trabalho do MAME, portanto não é um voto totalmente independente do MAME.
- **MEKA** contém uma lista curta e valiosa de aliases/opcodes não documentados em `UNDOC.TXT`, mas o núcleo antigo tem limitações de licença e notas de manutenção antigas. Serve como índice histórico, não como implementação para copiar.

Nenhuma divergência entre emuladores, isoladamente, prova o comportamento físico correto. Para cada caso devemos registrar: comportamento documentado, comportamento de implementações, hipótese escolhida para a variante-alvo, teste reproduzível e evidência que justifica a decisão.

## Revisões e fontes consultadas

Os links estão fixados em commits para que a pesquisa permaneça reproduzível.

### Gearsystem — alinhamento com SG-1000

- [Repositório](https://github.com/drhelius/Gearsystem)
- [`Processor.cpp`](https://github.com/drhelius/Gearsystem/blob/b1cf2c4af6fb17651f8d3a5a2e0a3ba07865be05/src/Processor.cpp)
- [`Processor_inline.h`](https://github.com/drhelius/Gearsystem/blob/b1cf2c4af6fb17651f8d3a5a2e0a3ba07865/src/Processor_inline.h)
- [`Processor.h`](https://github.com/drhelius/Gearsystem/blob/b1cf2c4af6fb17651f8d3a5a2e0a3ba07865/src/Processor.h)
- Revisão: [`b1cf2c4`](https://github.com/drhelius/Gearsystem/commit/b1cf2c4af6fb17651f8d3a5a2e0a3ba07865be05)

Observações do código: `ExecuteOPCode` percorre prefixos `DD/FD` repetidos e acrescenta estados para prefixos extras; o estado da CPU inclui `WZ`, `Q` e `QTemp`; `Processor_inline.h` atualiza `WZ` em instruções diferentes e escolhe os bits X/Y de BIT com base no endereço quando o acesso é indexado. O código também separa o incremento de R da decodificação ordinária e dos prefixos.

O README do projeto declara que passa no exercitador ZEXALL, incluindo instruções e comportamentos não documentados. Isto é uma declaração do projeto, não um teste executado por nós nesta pesquisa.

### MEKA — aliases e histórico de compatibilidade

- [`Z80.cpp`](https://github.com/ocornut/meka/blob/4c00b68ed08f2745d5ad3eeb05f5e1f258b198be/meka/srcs/z80marat/Z80.cpp)
- [`Codes.h`](https://github.com/ocornut/meka/blob/4c00b68ed08f2745d5ad3eeb05f5e1f258b198be/meka/srcs/z80marat/Codes.h)
- [`CodesED.h`](https://github.com/ocornut/meka/blob/4c00b68ed08f2745d5ad3eeb05f5e1f258b198be/meka/srcs/z80marat/CodesED.h)
- [`UNDOC.TXT`](https://github.com/ocornut/meka/blob/4c00b68ed08f2745d5ad3eeb05f5e1f258b198be/meka/srcs/z80marat/UNDOC.TXT)
- Revisão: [`4c00b68`](https://github.com/ocornut/meka/commit/4c00b68ed08f2745d5ad3eeb05f5e1f258b198be)

O arquivo `UNDOC.TXT` lista aliases ED, incluindo múltiplos códigos para NEG, RETN e IM, os opcodes ED 77/7F como NOP e ED 71 como `OUT (C),0`. Essa lista é útil para preencher a matriz de opcodes, mas os aliases e efeitos devem ser confirmados com referências independentes e com a variante de CPU definida.

A implementação trata interrupções em uma função separada, mas o histórico contém comentários de trabalho em andamento e dúvidas sobre IFF. Não usar esse comportamento antigo como árbitro único.

### Genesis Plus GX — histórico de bugs corrigidos

- [`core/z80/z80.c`](https://github.com/ekeeke/Genesis-Plus-GX/blob/49c584764893b0505ac7f768a754f97330fa4392/core/z80/z80.c)
- [`core/z80/z80.h`](https://github.com/ekeeke/Genesis-Plus-GX/blob/49c584764893b0505ac7f768a754f97330fa4392/core/z80/z80.h)
- [Histórico do repositório](https://github.com/ekeeke/Genesis-Plus-GX/blob/49c584764893b0505ac7f768a754f97330fa4392/HISTORY.txt)
- Revisão: [`49c5847`](https://github.com/ekeeke/Genesis-Plus-GX/commit/49c584764893b0505ac7f768a754f97330fa4392)

O cabeçalho do core registra correções históricas de ciclos para IXH/IXL/IYH/IYL, flags X/Y de SCF/CCF/BIT, uso de MEMPTR/WZ, BIT indexado e ciclos de interrupção IM2. Isso demonstra que essas áreas são fontes reais de bugs de compatibilidade, mesmo em um núcleo maduro. O histórico também registra que o core foi baseado em código Z80 do MAME; por isso, a concordância entre os dois não deve ser contada como duas confirmações independentes.

### ares — decomposição clara de instruções e temporização

- [`instruction.cpp`](https://github.com/ares-emulator/ares/blob/9408cb43d4948fc3ea6e152a307a34348df3fe04/ares/component/processor/z80/instruction.cpp)
- [`instructions.cpp`](https://github.com/ares-emulator/ares/blob/9408cb43d4948fc3ea6e152a307a34348df3fe04/ares/component/processor/z80/instructions.cpp)
- [`algorithms.cpp`](https://github.com/ares-emulator/ares/blob/9408cb43d4948fc3ea6e152a307a34348df3fe04/ares/component/processor/z80/algorithms.cpp)
- [`memory.cpp`](https://github.com/ares-emulator/ares/blob/9408cb43d4948fc3ea6e152a307a34348df3fe04/ares/component/processor/z80/memory.cpp)
- Revisão: [`9408cb4`](https://github.com/ares-emulator/ares/commit/9408cb43d4948fc3ea6e152a307a34348df3fe04)

O dispatcher distingue fetch de opcode (4 clocks), leitura de operando (3 clocks) e esperas explícitas. Prefixos indexados alteram a interpretação e têm caminho específico para `DD/FD CB d op`. `algorithms.cpp` centraliza as fórmulas aritméticas e flags; `instructions.cpp` mantém efeitos laterais por instrução, inclusive `WZ`, `Q`, operações de bloco e flags X/Y.

## Comportamentos para investigar e como transformá-los em código testável

### 1. Estado auxiliar WZ/MEMPTR

**Evidência comparativa:** Gearsystem e Genesis Plus GX mantêm `WZ`/MEMPTR e atualizam esse estado em diferentes classes de instruções; ares usa `WZ` explicitamente em acessos indexados, I/O, BIT e operações de bloco. O núcleo inicial deste projeto não possui um modelo equivalente validado.

**Plano independente de implementação:**
- Primeiro adicionar testes de caracterização por família sem tornar `WZ` observável pela API pública.
- Mapear atualização de endereço temporário para: saltos relativos/absolutos, CALL/RET, acesso absoluto à memória, operações de I/O, `ADD HL,rr`/aritmética indexada, operações de bloco e BIT em memória.
- Implementar estado `wz` somente após consolidar uma tabela de regras por instrução e identificar qual parte é documentada e qual é hipótese de compatibilidade.
- Verificar wraparound de 16 bits e o byte alto/baixo em acessos que cruzam `FFFF`.
- Não declarar cobertura completa de MEMPTR apenas porque existe um registrador: cada classe de instrução precisa de testes.

### 2. Q latch e flags X/Y de SCF/CCF

**Evidência comparativa:** Gearsystem mantém `Q` e `QTemp`; ares atualiza `Q` por instrução e usa estado de flags anterior em comportamentos específicos; os fontes de Genesis Plus GX documentam correções de X/Y em SCF/CCF e outras operações. MEKA não oferece a mesma evidência moderna sobre esse estado.

**Plano:**
- Adicionar estado interno separado para flags modificadas pela instrução anterior, se a variante escolhida e os testes justificarem Q.
- Escrever testes em sequência: instrução que altera flags → SCF/CCF; instrução que preserva flags → SCF/CCF; combinações com bits X/Y variados.
- Testar SCF/CCF isoladamente e em sequência, não apenas como instruções únicas.
- Não deduzir a fórmula exata por votação de fontes: comparar ZEXALL ou outra suíte independente e, quando necessário, documentação técnica/observações de hardware.

### 3. Bits X/Y não documentados

**Evidência comparativa:** os cores maduros mantêm os bits 3 e 5 em diversas operações, mas a origem varia conforme a instrução. O código Gearsystem escolhe o byte alto do endereço para BIT indexado; ares altera X/Y após BIT em memória indexada com base no byte alto de WZ. Genesis Plus GX registra correções em BIT, SCF/CCF e aritmética de 16 bits.

**Plano:**
- Criar casos separados para ALU de 8 bits, CP, aritmética de 16 bits, rotações, SCF/CCF, `BIT (HL)`, `BIT (IX+d)` e `BIT (IY+d)`.
- Para `BIT (IX/IY+d)`, testar endereços cuja parte alta tenha padrões distintos nos bits 3 e 5, sem depender do valor testado.
- Distinguir bits derivados do resultado, do operando e do endereço efetivo.
- Não espalhar regras X/Y em cada handler; usar helpers específicos por classe, com nomes explícitos sobre a origem dos bits.

### 4. Alias de ED e opcodes não documentados

A lista histórica do MEKA é um índice útil para ampliar a matriz: aliases de `NEG`, `RETN`, modos IM, `NOP` e `OUT (C),0`. Gearsystem e ares têm tabelas explícitas de ED; porém, as fontes diferem na classificação de certos aliases RETN/RETI e em detalhes de saída de I/O.

**Plano:**
- Criar tabela para todos os 256 valores de segundo byte de ED, classificando: instrução documentada, alias, NOP/ignorado, instrução não documentada com efeito, ou comportamento específico de variante.
- Incluir teste para cada classe de alias, incluindo flags, PC, SP, IFFs, valor emitido no barramento de I/O e ciclos.
- Não tratar todo opcode ED desconhecido como NOP sem evidência: isso mascara lacunas.
- Para `OUT (C),0`, manter o valor de saída configurável por variante/modelo até confirmar o comportamento do Z80/NEC alvo.

### 5. Prefixos DD/FD repetidos, ignorados e combinados

**Evidência comparativa:** Gearsystem percorre prefixos DD/FD repetidos, contabiliza custo adicional e atualiza o prefixo corrente. ares usa um loop de prefixos antes de despachar a instrução; possui caminho separado para `DD/FD CB d op` e comenta o fetch peculiar do opcode final. Isso é mais sutil do que apenas substituir HL por IX/IY.

**Plano:**
- Matriz de testes para: DD op; FD op; DD DD op; DD FD op; FD DD op; prefixo DD/FD antes de instrução que não usa HL; DD/FD CB d op.
- Verificar qual prefixo prevalece quando repetido/alternado, PC consumido, R incrementado e ciclos.
- Testar substituições de H/L por IXH/IXL e IYH/IYL, inclusive instruções de transferência entre registradores e casos que usam memória indexada.
- Tratar prefixos redundantes como comportamento do decodificador, não duplicar handlers de instrução.

### 6. Registrador R, fetch M1 e HALT

**Evidência comparativa:** MEKA incrementa R ao buscar os bytes de opcode nos caminhos base e prefixados. ares incrementa os sete bits baixos de R nos fetches de opcode e destaca um caminho especial DD/FD-CB no qual o byte final é lido como operando/pseudo-fetch e não incrementa R da mesma forma. MAME também separa fetch M1 e refresh. Esses detalhes não podem ser reproduzidos por um incremento indiscriminado por instrução.

**Plano:**
- Testar R após NOP, instruções com operandos imediatos, CB, ED, DD/FD, prefixos repetidos, DD/FD-CB e ciclos de HALT.
- Verificar os sete bits baixos e a preservação do bit 7.
- Separar explicitamente fetch M1, leitura de operando e leitura do opcode final indexado.
- Depois modelar evento de refresh; não afirmar precisão de refresh enquanto o barramento não expuser esse evento.

### 7. Flags de operações de bloco e repetição

**Evidência comparativa:** ares dá handlers separados a LDI/LDD/LDIR/LDDR, CPI/CPD/CPIR/CPDR e às operações de bloco de I/O; cada repetição adiciona estados e altera `WZ`/flags. Genesis Plus GX registra correções históricas nos flags de INI/IND/OUTI/OUTD e variantes repetitivas. O MAME também modela repetição e interrupção entre iterações.

**Plano:**
- Testar cada instrução base e variante repetitiva com contagem zero, um elemento, múltiplos elementos, wraparound e condição de parada antecipada.
- Verificar BC/B decrementado, HL/DE, PC em repetição, flags H/PV/X/Y e carry.
- Incluir I/O com valores de porta e dados controlados por fake bus.
- Validar interrupções no ponto de repetição antes de alegar fidelidade temporal.

### 8. Interrupções: IM 0, EI delay e LD A,I/R

**Evidência comparativa:** MEKA usa estado de IFF e possui comentários históricos de manutenção; o MAME documenta a limitação de IM 0 para instruções arbitrárias; ares modela IFF, modo de interrupção e estado interno. O núcleo Genesis Plus GX registra correções de ciclos IM2 e mudanças no momento de reconhecimento das interrupções.

**Plano:**
- Testar IM 0 com byte de acknowledge que representa RST e, em seguida, com instruções não-RST que o hardware alvo deve aceitar.
- Verificar o atraso de EI, DI, NMI durante HALT, IFF1/IFF2 e RETN/RETI.
- Criar casos para interrupção imediatamente antes/depois de `LD A,I` e `LD A,R`, verificando P/V conforme a variante física escolhida.
- Separar aceitação lógica de interrupção do ciclo de acknowledge/barramento.

### 9. DAA e aritmética

**Evidência comparativa:** ares implementa DAA por algoritmo e centraliza as operações aritméticas em helpers; Genesis Plus GX registra que substituiu uma tabela por algoritmo e corrigiu flags. MEKA usa tabela pré-calculada `DAATable`. A existência de abordagens diferentes reforça que devemos testar o resultado e os flags, não copiar uma forma de implementação.

**Plano:**
- Gerar uma matriz de entradas de A × N × H × C, cobrindo combinações válidas e inválidas, e comparar A/F resultantes.
- Testar overflow, half-carry, paridade e preservação/alteração de carry.
- Usar algoritmo legível em Python inicialmente; otimização ou tabela só depois de equivalência comprovada.
- Manter testes de referência separados dos testes unitários de cada helper.

## Estratégia de validação recomendada

1. Fazer inventário estático dos handlers atuais e cruzar com a matriz base/CB/ED/DD/FD/DD-CB/FD-CB.
2. Adicionar testes unitários de comportamento para cada item P0, sem introduzir todas as extensões no mesmo commit.
3. Usar testes públicos de exercitador Z80, como ZEXALL, somente após verificar licença, proveniência e condições de distribuição. Não incluir ROMs de teste no repositório sem revisão de licença.
4. Executar testes contra mais de uma implementação de referência, mas registrar dependências entre elas: Genesis Plus GX declara origem no core do MAME; logo, sua concordância não é uma confirmação totalmente independente.
5. Usar discrepâncias como casos de investigação. Quando os cores divergem, não escolher maioria automaticamente: registrar o alvo físico, a hipótese e a evidência.
6. Após a bateria unitária, fazer comparação diferencial automatizada de estado da CPU, memória, I/O e T-states nominais; ciclos de barramento só podem ser comparados quando ambos os modelos os expuserem.

## Licenciamento e reutilização

Esta pesquisa é uma análise de comportamento e arquitetura; **não copia código-fonte desses projetos para o Sega Emulator**.

- Gearsystem está sob GPL-3.0. Copiar código pode impor obrigações de copyleft e distribuição compatível.
- Os arquivos históricos de CPU do MEKA incluem avisos específicos de Marat Fayzullin que proíbem distribuição comercial; não reutilizar esse código sem autorização/licença inequívoca.
- Genesis Plus GX declara restrições, incluindo proibição de venda/uso comercial e exigências de disponibilização de fontes para modificações.
- ares usa licença permissiva no arquivo principal, mas a árvore inclui componentes de terceiros com avisos próprios; qualquer reutilização deve ser verificada por arquivo.

Para este projeto, a opção segura é **reimplementar a partir de especificações de comportamento e testes independentes**, preservar atribuições de referências documentais e realizar revisão de licença antes de portar qualquer trecho de código.

## Limites desta análise

- Foi feita leitura estática de fontes em revisões fixadas; não foram compilados nem executados esses emuladores.
- Não foram executados ZEXALL, testes de hardware ou comparação diferencial.
- Os detalhes de Q, WZ/MEMPTR, bits X/Y, aliases ED e diferenças NMOS/CMOS continuam pendentes de confirmação por suíte independente e definição da variante-alvo.
- Esta documentação propõe trabalho futuro; não afirma que a CPU do nosso repositório tenha sido alterada.


---

## Atualização experimental — 23/10/2026

A pesquisa de comportamentos não documentados passou a orientar uma auditoria diferencial concreta. O lote anterior de `SingleStepTests/z80 v1` reportou divergências concentradas em `BIT b,(HL)`, `SCF/CCF` e `DAA`; os dados e a revisão exata estão registrados em [Auditoria diferencial Z80 — 23/10/2026](z80-differential-audit-2026-10-23.md).

### Hipóteses convertidas em mudanças testáveis

- **WZ/MEMPTR e BIT (HL):** o código de trabalho usa o byte alto de WZ para F3/F5 em BIT sobre memória. A atualização geral de WZ por instrução continua incompleta.
- **Q e SCF/CCF:** o código de trabalho condiciona F3/F5 ao Q anterior e atualiza Q em alguns caminhos. Ainda falta demonstrar o ciclo de vida completo de Q, em especial instruções prefixadas DD/FD e instruções que preservam flags.
- **DAA:** foi corrigida a seleção das correções para incluir o caso de subtração reproduzido pelo corpus; o resultado esperado foi transformado em teste de regressão.

Essas mudanças estão na branch `fix/z80-single-step-audit`, no [PR #14](https://github.com/leonardo201800478/sega-emulator/pull/14). Elas são **hipóteses implementadas para validação**, não evidência de comportamento integralmente correto.

### Evidência e cautelas

O caso DAA reportado com A=`F3h`, F=`4Eh` espera A=`93h`, F=`87h`. A família BIT (HL) sugere dependência de estado auxiliar WZ/MEMPTR. Os casos SCF/CCF sugerem que os bits F3/F5 não podem ser tratados apenas como uma cópia direta dos bits do acumulador em todos os contextos.

Antes de encerrar a investigação, comparar os resultados corrigidos com o corpus, conferir a representação dos estados iniciais no harness e revisar as diferenças por variante de Z80. Não extrapolar um caso single-step para atualização completa de estado entre instruções nem para precisão ciclo a ciclo.
