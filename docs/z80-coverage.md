# Auditoria de cobertura da CPU Z80

## Finalidade e método

Este documento registra a cobertura observável em `src/sega_emulator/cpu/z80.py` na revisão auditada e orienta a implementação incremental. “Implementado” significa que há um caminho de execução no código; não significa que todos os flags, tempos de barramento, variantes não documentadas ou comportamentos de interrupção tenham sido validados contra hardware.

Fontes de referência para o inventário e os ciclos:
- [Zilog — Z80 CPU User Manual (UM008011)](https://www.zilog.com/docs/z80/um0080.pdf): referência primária para instruções documentadas, operações, flags e tempos nominais.
- [Z80.info — The Undocumented Z80 Documented](https://z80.info/zip/z80-documented.pdf): pesquisa secundária sobre opcodes não documentados e seus efeitos.
- [Grauw — Z80 / R800 instruction set](https://map.grauw.nl/resources/z80instr.php): tabela de instruções e tempos, útil como verificação cruzada.

O projeto deve escolher explicitamente o alvo inicial como Zilog Z80 clássico compatível com a plataforma SG-1000. Não misturar extensões de Z180, R800 ou outros derivados. Comportamentos não documentados serão uma meta posterior, após cobrir e testar o conjunto documentado.

## Legenda

- **Implementado — cobertura inicial:** existe decodificação, mas os testes atuais cobrem casos selecionados, não o espaço completo de entradas.
- **Parcial:** parte da família está implementada, mas faltam condições, modos ou variantes.
- **Ausente:** ainda não há execução correspondente.
- **Fora da etapa atual:** prefixos e variantes especiais serão auditados e implementados depois da base não prefixada.

## Base não prefixada: inventário por família

| Família / opcodes | Estado atual | Lacunas e observações |
|---|---|---|
| `00` — NOP | Implementado — cobertura inicial | Testado quanto a PC e 4 T-states nominais. |
| `01, 11, 21, 31` — LD rr,nn | Implementado — cobertura inicial | BC, DE, HL e SP; testes de endianess, preservação de flags e avanço do PC. |
| `02, 12` — LD (BC/DE),A | Implementado — cobertura inicial | Escreve A no endereço apontado por BC ou DE; 7 T-states nominais e flags preservadas. |
| `03, 13, 23, 33` — INC rr | Implementado — cobertura inicial | Inclui SP; testar todos os pares, wrap e preservação de flags. |
| `04, 0C, ... 3C` — INC r | Implementado — cobertura inicial | Inclui (HL); ampliar testes de todas as flags e operandos. |
| `05, 0D, ... 3D` — DEC r | Implementado — cobertura inicial | Inclui (HL); ampliar testes de todas as flags e operandos. |
| `06, 0E, ... 3E` — LD r,n | Implementado — cobertura inicial | Inclui `LD (HL),n`; ampliar casos de limites e efeitos no barramento. |
| `07, 0F, 17, 1F` — rotações do acumulador | Implementado — cobertura inicial | RLCA, RRCA, RLA e RRA; atualizam C e XY, limpam H/N e preservam S/Z/PV; 4 T-states nominais. Testes sintéticos por opcode. |
| `08` — EX AF,AF' | Implementado — cobertura inicial | Troca AF com o banco alternativo AF'; 4 T-states nominais. |
| `D9` — EXX | Implementado — cobertura inicial | Troca simultaneamente BC/DE/HL com BC'/DE'/HL'; AF/AF' não são alterados; 4 T-states nominais. |
| `EB` — EX DE,HL | Implementado — cobertura inicial | Troca DE e HL sem alterar flags; 4 T-states nominais. |
| `C7, CF, D7, DF, E7, EF, F7, FF` — RST p | Implementado — cobertura inicial | Empilha o endereço de retorno e salta para os vetores 00h–38h em passos de 8; 11 T-states nominais. Testes dos oito vetores e wrap da pilha. |
| `09, 19, 29, 39` — ADD HL,rr | Implementado — cobertura inicial | Inclui SP; validar flags X/Y e todos os pares/casos-limite. |
| `ED 4A, 5A, 6A, 7A` — ADC HL,rr | Implementado — cobertura inicial | Inclui carry de entrada, S/Z/H/PV/N/C e X/Y; 15 T-states nominais; testes de limites. |
| `ED 42, 52, 62, 72` — SBC HL,rr | Implementado — cobertura inicial | Inclui borrow de entrada, S/Z/H/PV/N/C e X/Y; 15 T-states nominais; testes de limites. |
| `ED 43, 53, 63, 73` — LD (nn),rr | Implementado — cobertura inicial | Escreve BC, DE, HL ou SP em little-endian; 20 T-states nominais, flags preservadas e wrap de 16 bits no segundo acesso. |
| `ED 4B, 5B, 6B, 7B` — LD rr,(nn) | Implementado — cobertura inicial | Lê uma palavra little-endian para BC, DE, HL ou SP; 20 T-states nominais, flags preservadas e wrap de 16 bits no segundo acesso. |
| `0A, 1A` — LD A,(BC/DE) | Implementado — cobertura inicial | Lê A do endereço apontado por BC ou DE; 7 T-states nominais e flags preservadas. |
| `10` — DJNZ e | Implementado — cobertura inicial | Decrementa B; desvia com deslocamento relativo assinado se B não for zero; preserva flags. Testes de ramo tomado/não tomado, wrap de B e wrap do PC; 13/8 T-states nominais. |
| `18` — JR e | Implementado — cobertura inicial | Deslocamento relativo com sinal e wrap do PC já têm testes selecionados. |
| `20, 28, 30, 38` — JR cc,e | Implementado — cobertura inicial | NZ, Z, NC, C; as condições relativas disponíveis na base são estas quatro. |
| `22` — LD (nn),HL | Implementado — cobertura inicial | Escreve L no endereço e H no endereço seguinte, com wrap de 16 bits; 16 T-states nominais e flags preservadas. |
| `2A` — LD HL,(nn) | Implementado — cobertura inicial | Lê dois bytes em little-endian, com wrap de 16 bits; 16 T-states nominais e flags preservadas. |
| `27` — DAA | Implementado — cobertura inicial | Ajuste decimal baseado em H/N/C anteriores; atualiza S/Z/PV/H/C e preserva N; testes sintéticos para soma, subtração e limites. |
| `2F` — CPL | Implementado — cobertura inicial | Complementa A; atualiza H/N, copia X/Y do resultado e preserva S/Z/PV/C; 4 T-states nominais. |
| `32` — LD (nn),A | Implementado — cobertura inicial | Endereço absoluto little-endian; 13 T-states nominais, flags preservadas. |
| `3A` — LD A,(nn) | Implementado — cobertura inicial | Lê de endereço absoluto little-endian; 13 T-states nominais, flags preservadas. |
| `37` — SCF | Implementado — cobertura inicial | Define C, limpa H/N, preserva S/Z/PV e copia X/Y de A; 4 T-states nominais. |
| `3F` — CCF | Implementado — cobertura inicial | Inverte C, copia C anterior para H, limpa N, preserva S/Z/PV e copia X/Y de A; 4 T-states nominais. |
| `40–75, 77–7F` — LD r,r' | Implementado — cobertura inicial | Registradores e (HL); `76` é documentado separadamente como HALT. |
| `76` — HALT | Implementado — cobertura inicial | Estado de suspensão e ciclos repetidos; validar detalhes de fetch durante HALT. |
| `80–87` — ADD A,r | Implementado — cobertura inicial | Registros e (HL); expandir testes de flags. |
| `88–8F` — ADC A,r | Implementado — cobertura inicial | Carry de entrada implementado; ampliar testes exaustivos de carry e overflow. |
| `90–97` — SUB r | Implementado — cobertura inicial | Registros e (HL); ampliar testes de flags. |
| `98–9F` — SBC A,r | Implementado — cobertura inicial | Borrow de entrada implementado; ampliar testes de limites. |
| `A0–A7` — AND r | Implementado — cobertura inicial | Registros e (HL); verificar flags em casos variados. |
| `A8–AF` — XOR r | Implementado — cobertura inicial | Registros e (HL); verificar flags em casos variados. |
| `B0–B7` — OR r | Implementado — cobertura inicial | Registros e (HL); verificar flags em casos variados. |
| `B8–BF` — CP r | Implementado — cobertura inicial | Não altera A; bits X/Y são tratados a partir do operando. |
| `C6, CE, D6, DE, E6, EE, F6, FE` — ALU imediata | Implementado — validado exaustivamente por testes sintéticos | ADD, ADC, SUB, SBC, AND, XOR, OR e CP com 7 T-states nominais. `tests/unit/test_z80_alu_immediate.py` percorre todos os pares A/n e ambos os estados de carry para ADC/SBC, verificando resultado, flags, PC, R e ciclos nominais. Isso não substitui comparação com hardware ou exercitador independente. |
| `C3` — JP nn | Implementado — cobertura inicial | Salto absoluto incondicional. |
| `C2, CA, D2, DA, E2, EA, F2, FA` — JP cc,nn | Implementado — cobertura inicial | Oito condições NZ, Z, NC, C, PO, PE, P e M; testes de ramos tomados/não tomados. |
| `C5, D5, E5, F5` — PUSH qq | Implementado — cobertura inicial | BC, DE, HL e AF; verificar ordem de acesso e tempos de barramento em fase futura. |
| `C1, D1, E1, F1` — POP qq | Implementado — cobertura inicial | BC, DE, HL e AF; verificar ordem de acesso e efeitos de AF. |
| `C4, CC, D4, DC, E4, EC, F4, FC` — CALL cc,nn | Implementado — cobertura inicial | Oito condições; testes de ramos tomados/não tomados, endereço de retorno, pilha e T-states nominais. |
| `CD` — CALL nn | Implementado — cobertura inicial | Sub-rotina absoluta. |
| `C0, C8, D0, D8, E0, E8, F0, F8` — RET cc | Implementado — cobertura inicial | Oito condições; testes de ramos tomados/não tomados, pilha e T-states nominais. |
| `C9` — RET | Implementado — cobertura inicial | Retorno incondicional. |
| `D3, DB` — OUT (n),A / IN A,(n) | Implementado — cobertura inicial | Usa IOBus separado e endereço de porta formado por A antes da operação no byte alto e n no byte baixo; flags preservadas. |
| `E3` — EX (SP),HL | Implementado — cobertura inicial | Troca a palavra em (SP)/(SP+1) com HL; preserva SP e flags, trata wrap de 16 bits e contabiliza 19 T-states nominais. Testes de troca, ordem dos acessos e wraparound. |
| `E9` — JP (HL) | Implementado — cobertura inicial | Copia HL para PC; 4 T-states nominais, flags preservadas. |
| `F3, FB` — DI / EI | Implementado — cobertura inicial | IFF1/IFF2, DI imediato e atraso de reconhecimento após EI; interrupções exigem validação adicional. |
| `F9` — LD SP,HL | Implementado — cobertura inicial | Copia HL para SP; 6 T-states nominais, flags preservadas. |

## Famílias prefixadas

| Prefixo | Estado | Escopo necessário |
|---|---|---|
| `CB` | Implementado — cobertura inicial | RLC, RRC, RL, RR, SLA, SRA, SRL, BIT, RES e SET em registradores/(HL); SLL incluído separadamente como não documentado; DD/FD-CB também está implementado. |
| `ED` | Implementado — cobertura inicial, sujeito à validação | Inclui `ADC/SBC HL,rr`, transferências de palavras, I/O por (C), operações de bloco, RRD/RLD, IM 0/1/2, RETN/RETI e transferências entre A, I e R. `R` incrementa nos fetches M1 modelados; `LD A,I/R` usa IFF2. As flags de operações de bloco e a arbitragem de interrupções ainda precisam de validação diferencial. |
| `DD` | Implementado — cobertura inicial | IX, IXH/IXL, (IX+d), pilha e ALU; testes sintéticos cobrem transferências entre IXH/IXL, seleção de IX/IY por prefixos mistos, wrap do endereço efetivo nos dois limites (`0000h - 1 = FFFFh` e `FFFFh + 1 = 0000h`) e a exceção de `LD H/L,(IX+d)` e `LD (IX+d),H/L`, que usam H/L comuns, não IXH/IXL. Validação diferencial e temporal ainda pendente. |
| `FD` | Implementado — cobertura inicial | Equivalente para IY; testes sintéticos cobrem transferências entre IYH/IYL, seleção de IX/IY por prefixos mistos, wrap do endereço efetivo nos dois limites (`0000h - 1 = FFFFh` e `FFFFh + 1 = 0000h`) e a mesma exceção H/L para ambos os prefixos. Validação diferencial e temporal ainda pendente. |
| `DD CB d` / `FD CB d` | Implementado — cobertura inicial | Operações sobre (IX+d)/(IY+d), incluindo cópia opcional para registrador; validação de flags e temporização ainda pendente. |

## Estado interno, interrupções e ciclo de barramento

O código atual modela registradores principais e alternativos, I/R, IX/IY, SP, PC, flags e um acumulador de T-states nominais. Ainda faltam, entre outros:

- Os registradores alternativos AF', BC', DE' e HL', I/R e IX/IY estão modelados.
- IFF1/IFF2, modos 0–2, HALT e solicitações NMI/INT têm um modelo inicial. A limitação conhecida de IM 0 (aceitar apenas RST no byte de acknowledge) permanece pendente até validação e implementação genérica; confirme o comportamento no código atual antes de ampliar essa afirmação.
- Ciclos M1, ciclos de leitura/escrita e I/O, estados de espera, contenção e temporização de barramento.
- Flags X/Y e efeitos não documentados validados contra referências e testes específicos.
- Comportamento de prefixos repetidos, opcodes não documentados e diferenças de revisão, conforme a meta de compatibilidade.

O contador `total_cycles` atual soma os custos nominais das instruções suportadas. Não significa que o processador já execute acessos ao barramento ciclo a ciclo.

## Próximos passos de validação

1. Executar comparação diferencial com uma suíte de referência Z80 confiável para flags, resultados, PC, SP e ciclos de cada família.
2. Refinar o reconhecimento de INT/NMI, em especial IM 0 para opcodes fornecidos pelo barramento além de RST.
3. Validar os efeitos de I/O e as flags de LDI/LDD, CPI/CPD e instruções de bloco de I/O contra referências primárias e testes independentes.
4. Implementar um modelo de acesso a barramento/ciclo para M1, leitura, escrita, I/O, estados de espera e contenção quando a arquitetura do SG-1000 exigir isso.
5. Auditar prefixos repetidos, opcodes não documentados e diferenças de revisão de hardware; não misturar extensões de outros derivados do Z80.

O conjunto documentado tem caminhos de execução nas famílias base, CB, ED, DD/FD e DD/FD-CB. Isso não equivale a validação exaustiva de cada combinação de entradas nem à precisão ciclo a ciclo.

Cada incremento deverá atualizar esta matriz, testes, documentação de hardware e README quando o estado do marco mudar. Não declarar “Z80 completo” enquanto houver lacunas ou enquanto os critérios de validação não forem satisfeitos.


## Backlog ampliado após análise do núcleo Z80 do MAME

Pesquisa detalhada e referências fixadas em revisão específica do MAME: [docs/z80-mame-reference.md](z80-mame-reference.md).

A análise acrescenta estes itens à lista de implementação, sem afirmar que já estejam implementados:

1. **Matriz de opcode auditável:** classificar cada opcode e combinação de prefixos, com handler, documentação, testes e status de validação.
2. **Separar tipos de acesso:** planejar contratos distintos para fetch M1, leitura de operandos, acesso a dados e I/O; manter a API atual até haver testes de regressão.
3. **Temporização por acesso:** substituir gradualmente a suposição de que T-states nominais por instrução bastam; modelar eventos de barramento e estados de espera antes de alegar precisão de ciclo.
4. **Refresh e registrador R:** testar incrementos por fetch M1, prefixos, HALT/interrupções e preservação do bit 7; o sinal de refresh ainda não é um evento de barramento modelado.
5. **Flags e estado auxiliar:** DAA, BIT e SCF/CCF estão em correção diferencial. Q tem atualização centralizada em avaliação; WZ/MEMPTR recebeu atualizações parciais por família, ainda pendentes de auditoria completa e validação externa.
6. **Interrupções:** validar NMI por borda, IFF1/IFF2, atraso de EI, HALT e acknowledge. Fechar a limitação conhecida de IM 0 para o modelo de CPU escolhido, em vez de aceitar apenas RST por conveniência.
7. **Reset e variante:** distinguir estado inicial do objeto e reset de hardware; definir a variante-alvo compatível com SG-1000 antes de assumir peculiaridades NMOS/CMOS/NEC.
8. **Integração de plataforma:** futuramente conectar WAIT/READY e I/O do SG-1000 por interfaces de barramento, sem incorporar o mapa do console diretamente ao núcleo Z80.

**Cuidado com a referência:** o código do MAME contém limitações e TODOs explícitos, inclusive sobre IM 0 e diferenças entre variantes. Usá-lo como comparador independente, junto ao manual Zilog e a testes dedicados; não como oráculo único. A pesquisa consultou o código-fonte, mas não executou testes diferenciais contra o MAME.

## Backlog após comparação Gearsystem, MEKA, Genesis Plus GX e ares

Pesquisa detalhada com revisões fixadas e análise de licenças: [docs/z80-undocumented-behavior-research.md](z80-undocumented-behavior-research.md).

Os itens abaixo complementam a pesquisa do MAME e continuam **pendentes**, salvo se testes futuros comprovarem o contrário:

1. **Matriz dos 256 opcodes ED:** os handlers conhecidos/aliases são decodificados e os bytes sem operação atribuída agora usam ED-NOP (8 T-states); testes classificam os 256 segundos bytes. Ainda é necessário validar semântica, flags, IFFs, PC/SP, I/O, WZ e ciclos de cada família.
2. **WZ/MEMPTR:** o registrador está no núcleo e já recebe atualizações em operações de memória, 16 bits, fluxo de controle, I/O, acessos indexados e parte das instruções ED/bloco. Revisar a tabela de cada opcode e validar os estados finais.
3. **Q latch e flags X/Y:** SCF/CCF consultam o Q anterior, e as escritas internas de F atualizam Q. Ainda é necessário validar a sequência completa de instruções, prefixos, interrupções e diferenças de variante.
4. **Prefixos DD/FD:** validar prefixos repetidos/alternados, prefixos ignorados, IXH/IXL/IYH/IYL, DD/FD-CB, PC, R e custos temporais.
5. **R/M1/HALT:** separar fetch de opcode, operandos e opcode final de DD/FD-CB; testar o contador R e só depois modelar refresh.
6. **Operações de bloco ED:** validar flags e condições de repetição/saída antecipada de LDI/LDD/LDIR/LDDR, CPI/CPD/CPIR/CPDR e operações de I/O de bloco.
7. **DAA e aritmética:** gerar matriz de casos para A/N/H/C e validar A/F com referência independente.
8. **Interrupções e variantes:** testar IM 0 com bytes de acknowledge além de RST, atraso de EI, NMI/HALT e o comportamento de LD A,I/R interrompida após escolher a variante-alvo.
9. **Testes externos:** avaliar ZEXALL e outros exercitadores quanto a proveniência/licença; não incorporar imagens de teste ao repositório sem revisão.
10. **Licenças das referências:** não copiar código de MEKA ou Genesis Plus GX sem autorização/licença compatível; Gearsystem é GPL-3.0. Usar a análise para desenvolver implementação independente e testes próprios.

A concordância entre fontes não é automaticamente independente: o Genesis Plus GX documenta origem no núcleo Z80 do MAME. Gearsystem e ares oferecem perspectivas adicionais, mas discrepâncias devem ser investigadas em vez de resolvidas por votação. Esta pesquisa é estática; nenhum dos quatro emuladores foi compilado/executado nesta etapa.

## Verificação local registrada em 22/10/2026

Na branch `main`, commit `d6228f24b4f562d5b366296c0494f00c23def8c9`, o usuário executou os testes no Windows com Python 3.14.7 e pytest 9.1.1:

- `python -m pytest -v`: **262 passed in 0.60s**.
- `ruff check .`: aprovado.
- `ruff format --check .`: 19 arquivos já formatados.
- `git diff --check`: sem erros reportados.

Essa execução confirma os testes presentes no repositório nesse commit, não valida instruções ainda ausentes, não substitui comparação diferencial com uma suíte independente e não comprova precisão ciclo a ciclo. A cobertura indicada nesta matriz continua sendo “caminho implementado” até validação específica por família.


## Auditoria estática do código — 22/10/2026

A leitura direta de `src/sega_emulator/cpu/z80.py` e `tests/unit/test_z80.py` identificou lacunas de arquitetura e validação que devem ser tratadas separadamente de “opcode implementado”. Ver [auditoria técnica completa](z80-code-audit-2026-10-22.md).

Prioridades confirmadas:
1. **Temporização:** os ciclos ainda são contabilizados por instrução; os acessos ao barramento não são agendados em T-states.
2. **Interrupções:** a cobertura atual inclui testes para incremento de R em NMI/INT, modos IM 0/1/2, atraso de EI e saída de HALT por INT. Ainda faltam validação diferencial abrangente do acknowledge, dos estados IFF e das sequências de borda; a limitação de IM 0 para bytes RST deve ser confirmada no código atual.
3. **Comportamentos auxiliares:** WZ/MEMPTR e Q latch têm implementação parcial na branch de correção; a cobertura por opcode e a validação diferencial pós-correção permanecem pendentes.
4. **ED:** handlers importantes existem, mas falta classificar e testar todos os 256 segundos bytes, incluindo aliases e NOPs não documentados.
5. **Validação semântica:** os testes “todos os opcodes têm caminho” não demonstram correção de resultados, flags, ciclos ou ordem de barramento. DAA, flags X/Y, prefixos e operações de bloco precisam de comparação diferencial.

Esta auditoria foi estática; não foram executados testes nesta etapa e nenhum código da CPU foi alterado. O relatório separa lacunas confirmadas de áreas que precisam de testes antes de qualquer correção.


## Progresso validado: interrupções e registrador R

A CPU incrementa os sete bits inferiores de `R` ao aceitar NMI e INT, preservando o bit 7, conforme os testes registrados nesta revisão. Os testes acrescentados cobrem IM 0/1/2, overflow de `R`, atraso de `EI`, saída de `HALT` por INT e estado de `IFF1/IFF2` em NMI.

A [CI do commit `6d82d45`](https://github.com/leonardo201800478/sega-emulator/actions/runs/39787665564) foi registrada como aprovada, com Ruff aprovado e **267 testes passando** naquele commit. Esse resultado é histórico e não deve ser apresentado como execução da revisão atual. Ele valida os casos existentes naquele ponto, mas não comprova precisão ciclo a ciclo nem conformidade integral com todas as variantes do Z80.


## Retomada documental — 22/10/2026

Esta matriz é um inventário de implementação e validação, não uma certificação de conformidade. Ao retomar o trabalho, distinguir os resultados históricos de CI/testes locais do estado atual da branch. Antes de ampliar o núcleo, verificar no `main` o código e os testes vigentes, então executar uma comparação diferencial reproduzível para flags, operações ED de bloco, prefixos DD/FD e interrupções. Não tratar a existência de um handler como prova de semântica correta. A temporização por instrução continua sendo nominal: acessos M1, refresh, leitura/escrita e I/O ainda não são eventos agendados individualmente por T-state.

## Validação exaustiva ALU de 8 bits

`tests/unit/test_z80_alu_exhaustive.py` percorre todos os pares de operandos de 8 bits para ADD, ADC, SUB, SBC, CP, AND, XOR e OR, incluindo os dois estados de carry para ADC/SBC. Também percorre todos os valores de byte e carry inicial para INC/DEC, verificando resultado, flags e preservação de C. Para 16 bits, `ADD HL,BC`, `ADC HL,BC` e `SBC HL,BC` são verificados em combinações de valores-limite (incluindo fronteiras de byte, nibble, sinal e wraparound) e 256 pares determinísticos adicionais. O teste compara resultados e flags com fórmulas de referência independentes dos helpers da CPU e verifica os T-states nominais.

A cobertura de 8 bits é exaustiva para as famílias citadas; a cobertura de 16 bits é direcionada e amostrada, não exaustiva. Os testes sintéticos não equivalem a uma comparação com silício real nem comprovam precisão temporal global. O CP verifica também que o acumulador não é alterado e que X/Y derivam do operando.

## Matriz de decodificação ED

`test_all_ed_second_bytes_are_supported_or_explicitly_rejected` percorre os 256 valores do segundo byte após `ED`. Cada valor é classificado como codificação aceita pelo núcleo atual ou opcode explicitamente rejeitado. Para os rejeitados, verifica PC após os dois bytes do prefixo e ausência de T-states contabilizados.

Esta é uma verificação de cobertura de decodificação, não de correção semântica. Aliases aceitos pelo código não são, por esse fato, considerados documentados. Resultados, flags, efeitos de I/O e ciclos de cada família ainda exigem validação própria.



## Validação ALU de 16 bits com progresso e paralelismo

Para investigações demoradas, existe o utilitário opcional `scripts/validate_z80_alu_16bit.py`. Ele valida a execução real da CPU para `ADD HL,BC`, `ADC HL,BC` e `SBC HL,BC`, comparando resultado, flags e T-states nominais com fórmulas independentes. O processo principal exibe percentual, valores HL concluídos, tempo decorrido e estimativa restante; os casos são distribuídos entre processos para usar vários núcleos sem depender do GIL do CPython.

Exemplos no PowerShell, a partir da raiz do repositório:

```powershell
# Amostra mínima, útil para verificar o arranque do utilitário
.\\.venv\\Scripts\\python.exe scripts/validate_z80_alu_16bit.py --left-count 1 --chunk-size 1

# Amostragem distribuída por todo o domínio de HL, incluindo fronteiras
.\\.venv\\Scripts\\python.exe scripts/validate_z80_alu_16bit.py --left-count 1024 --sampling distributed --chunk-size 1

# Modo sequencial legado: os primeiros N valores de HL
.\\.venv\\Scripts\\python.exe scripts/validate_z80_alu_16bit.py --left-count 16 --sampling sequential --chunk-size 1
```

`--sampling sequential` mantém o comportamento legado e testa HL de 0 até N-1. `--sampling distributed` seleciona valores determinísticos espalhados por todo o domínio de 16 bits; para amostras de pelo menos 64 valores, também inclui fronteiras importantes para carry, half-carry, sinal e wraparound. Com `--left-count 1024 --sampling distributed`, cada um dos 1.024 valores HL é combinado com todos os 65.536 valores BC, para até **335.544.320 verificações de instrução**. Essa quantidade levou cerca de 3m29s no ambiente Windows registrado pelo usuário, mas o tempo varia por máquina; reduza `--left-count` se a estimativa local ultrapassar cinco minutos.

`--full` percorre todos os 65.536 valores HL e, portanto, todos os pares de 16 bits. Equivale a cerca de **21,47 bilhões de verificações de instrução** e não deve ser executado como parte da rotina limitada a cinco minutos. O utilitário é separado do pytest padrão. O paralelismo é por processos, não por threads, porque o teste é CPU-bound e threads Python normalmente não aceleram esse trabalho sob o GIL do CPython.

Os testes unitários de `tests/unit/test_z80_alu_sampling.py` verificam a seleção, a distribuição, as fronteiras e o determinismo sem executar a varredura aritmética prolongada.

## Validação exaustiva de DAA

`tests/unit/test_z80_daa_exhaustive.py` percorre todas as **2.048 combinações** do acumulador de 8 bits e dos flags de entrada N, H e C. Um oráculo separado calcula o ajuste decimal e os flags S/Z/Y/H/X/PV/N/C esperados, verificando também o PC, o incremento e o wrap do registrador R, e os 4 T-states nominais. Os flags de entrada que não participam da regra DAA são semeados intencionalmente para verificar que o resultado os recalcula.

Este teste é uma validação sintética exaustiva do domínio de entrada modelado, não uma comparação com silício real nem prova de precisão ciclo a ciclo. Qualquer divergência deve ser investigada contra o manual Zilog e uma referência independente antes de alterar a implementação.

## Validação exaustiva de BIT no prefixo CB

O arquivo `tests/unit/test_z80_bit_flags_exhaustive.py` verifica as flags de `BIT b,r` e `BIT b,(HL)` com um oráculo sintético para as flags documentadas e uma hipótese explícita para os bits X/Y não documentados:

- `BIT b,B`: 2.048 estados (256 valores × 8 bits × 2 estados de carry).
- `BIT b,(HL)`: 8.192 estados (256 valores × 8 bits × 2 estados de carry × 4 padrões dos bits X/Y do byte alto do endereço).
- Verifica H=1, N=0, Z/PV quando o bit testado é zero, S apenas para `BIT 7` com o bit definido e preservação de C. Para X/Y, registra a hipótese atual do núcleo: vêm do operando em `BIT b,B` e do byte alto do endereço em `BIT b,(HL)`; isso não é apresentado como comportamento documentado nem confirmado por hardware.
- Também verifica que o operando não é alterado, PC avança dois bytes, R é incrementado duas vezes no modelo atual e o custo nominal é 8 T-states para registrador e 12 para `(HL)`.

**Limites:** são testes sintéticos de regras e do modelo atual, não uma comparação contra silício ou um núcleo de referência executado. Os quatro padrões de X/Y no endereço são amostras direcionadas, não uma varredura de todos os endereços. A semântica e a temporização de `BIT b,(IX+d)`/`BIT b,(IY+d)` continuam pendentes e devem ser tratadas em uma etapa separada.

## Validação de BIT indexado (DDCB/FDCB)

O arquivo `tests/unit/test_z80_indexed_bit_flags.py` acrescenta:

- 32.768 estados sintéticos no total para `BIT b,(IX+d)` e `BIT b,(IY+d)` (16.384 por prefixo), cobrindo os 256 valores, oito posições de bit, ambos os estados de carry e quatro padrões do byte alto do endereço efetivo.
- Cobertura dos 256 valores de deslocamento (0x00–0xFF) para IX e IY em quatro padrões do byte alto do endereço efetivo, além de casos direcionados de wrap de 16 bits; PC, R e 20 T-states nominais são verificados.
- Verificação de que o operando de memória permanece inalterado e que X/Y acompanham os bits 5 e 3 do byte alto do endereço efetivo no modelo testado.

**Limites:** os testes exercitam a hipótese do núcleo para X/Y e o custo nominal; não são validação em hardware nem prova de temporização por ciclo. Os 256 valores de deslocamento são testados em quatro padrões do endereço efetivo, mas não em todas as combinações possíveis de base IX/IY e deslocamento.



## Cobertura inicial das operações de I/O de bloco ED — 22/10/2026

Foram adicionados testes parametrizados para `INI`, `IND`, `INIR`, `INDR`, `OUTI`, `OUTD`, `OTIR` e `OTDR`. Os casos verificam a direção da atualização de HL, transferência entre porta e memória, endereço de porta observado pelo barramento de I/O, decremento de B, retorno do PC e ciclos nominais de repetição (21 T-states) versus término (16 T-states). Testes separados cobrem o término das quatro variantes repetidas quando B chega a zero.

Referência de arquitetura: [Zilog — Z80 CPU User Manual, Input/Output Group](https://www.zilog.com/docs/z80/um0080.pdf). O código ainda usa operações lógicas de I/O, sem modelar fases de barramento, WAIT states ou interrupções reconhecidas entre iterações. As flags não documentadas e os detalhes elétricos do endereço de porta exigem validação diferencial adicional; estes testes não demonstram precisão ciclo a ciclo.

## Verificação das flags de I/O de bloco em limites de carry — 22/10/2026

Foram adicionados quatro casos dirigidos para `INI`, `IND`, `OUTI` e `OUTD`. Os operandos fazem a soma auxiliar `k` chegar a `0x101`, exercitando H/C, e deixam B em `0x28`, exercitando os bits X/Y modelados pelo núcleo. Os testes verificam o valor integral de F (`0x39`), além do endereço de porta pré-decremento e da direção da transferência.

A fórmula de H/C e P/V para essas instruções inclui efeitos tradicionalmente tratados como não documentados; foi usada como hipótese de implementação para criar os testes, não como prova de comportamento universal em todas as revisões do silício. O manual da Zilog descreve parte das flags de I/O de bloco como dependentes do resultado ou indeterminadas; validação diferencial com uma referência independente continua necessária.

## Validação adicional de BC inicial zero nas instruções de bloco ED — 22/10/2026

Foram acrescentados testes para os casos em que `BC=0000h` antes de `LDIR` e `CPIR`. O Z80 não trata esse estado como uma operação vazia: a primeira iteração decrementa BC para `FFFFh` e a instrução continua repetindo. Os testes verificam a primeira iteração, o retorno do PC ao opcode, os registradores atualizados, os flags pertinentes e os 21 T-states nominais da iteração de repetição.

A validação cobre apenas o primeiro passo desse percurso de 65.536 iterações; não executa a sequência completa nem constitui validação diferencial contra hardware.

## Validação adicional de repetição das instruções ED — 22/10/2026

Foram acrescentados casos direcionados para `LDDR` e `CPDR`, complementando os testes existentes de `LDIR` e `CPIR`:

- `LDDR`: verifica cópia em ordem decrescente, atualização de HL/DE/BC, retorno do PC para repetir enquanto BC não for zero, 21 T-states na repetição e 16 T-states na iteração final.
- `CPDR`: verifica comparação em ordem decrescente, repetição quando o byte não corresponde, encerramento antecipado quando A coincide com `(HL)`, preservação de C, N/Z e P/V, atualização de HL/BC e custos nominais de 21/16 T-states.

Esses casos são testes sintéticos de fluxo de controle e estado observável. Não constituem validação diferencial contra hardware ou prova de temporização ciclo a ciclo; permanecem pendentes a cobertura sistemática de todas as condições de saída, wrap de endereços e comparação diferencial independente das flags.


---

## Atualização da validação diferencial — 23/10/2026

O primeiro lote reportado do corpus `SingleStepTests/z80 v1` executou 1.000.000 de casos no commit `4a6eac4468645843c2cab22aa653413db4478221`: 992.309 aprovados e 7.691 divergências. O agrupamento indicou 6.001 casos em `BIT b,(HL)`, 1.342 em `SCF/CCF` e 348 em `DAA`. Consulte [o relatório detalhado](z80-differential-audit-2026-10-23.md).

Na branch `fix/z80-single-step-audit`, o PR #14 propõe correções e testes direcionados para essas famílias. A presença dos handlers e dos testes não deve ser confundida com aprovação: os números acima pertencem à revisão antiga e ainda não há, neste registro, resultado de uma auditoria repetida após as alterações.

### Como interpretar a cobertura a partir daqui

- **Caminho implementado:** há código que executa a instrução.
- **Teste unitário presente:** há um caso automatizado no repositório, mas seu status depende de execução real.
- **Validação diferencial anterior:** há um resultado registrado para uma revisão específica; ele não valida revisões posteriores.
- **Validação diferencial pós-correção:** só pode ser marcada após executar o corpus com o SHA corrigido e revisar os artefatos.
- **Precisão temporal:** permanece limitada a T-states nominais por instrução. A bateria single-step não demonstra temporização de acesso ao barramento ciclo a ciclo.

### Lacunas prioritárias

1. Confirmar pytest/Ruff no commit corrigido.
2. Corrigir e testar o ciclo de vida de Q para todas as instruções que modificam ou preservam F, incluindo prefixos DD/FD antes de SCF/CCF.
3. Definir e testar as atualizações de WZ/MEMPTR para as famílias de instruções relevantes; não assumir que inicializar WZ é suficiente.
4. Reexecutar a auditoria com o SHA explícito da branch corrigida.
5. Classificar as divergências remanescentes por opcode, campo de estado e possível limitação do harness.
