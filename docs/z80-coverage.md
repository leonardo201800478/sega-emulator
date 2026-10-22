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
| `C6, CE, D6, DE, E6, EE, F6, FE` — ALU imediata | Implementado — cobertura inicial | ADD, ADC, SUB, SBC, AND, XOR, OR e CP com 7 T-states nominais. |
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
| `DD` | Implementado — cobertura inicial | IX, IXH/IXL, (IX+d), pilha, ALU e DD-CB implementados; testes de fumaça percorrem os opcodes do segundo byte, mas validação de flags/ciclos ainda é parcial. |
| `FD` | Implementado — cobertura inicial | Equivalente para IY; validação diferencial e de casos especiais ainda pendente. |
| `DD CB d` / `FD CB d` | Implementado — cobertura inicial | Operações sobre (IX+d)/(IY+d), incluindo cópia opcional para registrador; validação de flags e temporização ainda pendente. |

## Estado interno, interrupções e ciclo de barramento

O código atual modela registradores principais e alternativos, I/R, IX/IY, SP, PC, flags e um acumulador de T-states nominais. Ainda faltam, entre outros:

- Os registradores alternativos AF', BC', DE' e HL', I/R e IX/IY estão modelados.
- IFF1/IFF2, modos 0–2, HALT e solicitações NMI/INT têm um modelo inicial; IM 0 atualmente aceita apenas instruções RST no barramento de dados; outros opcodes IM 0 falham explicitamente em vez de serem simulados incorretamente.
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
5. **Flags e estado auxiliar:** ampliar testes de DAA, operações de bloco, BIT em memória/indexado e preservação de flags; pesquisar WZ/MEMPTR, Q e X/Y como trabalho de compatibilidade, sem presumir que o MAME represente toda variante física.
6. **Interrupções:** validar NMI por borda, IFF1/IFF2, atraso de EI, HALT e acknowledge. Fechar a limitação conhecida de IM 0 para o modelo de CPU escolhido, em vez de aceitar apenas RST por conveniência.
7. **Reset e variante:** distinguir estado inicial do objeto e reset de hardware; definir a variante-alvo compatível com SG-1000 antes de assumir peculiaridades NMOS/CMOS/NEC.
8. **Integração de plataforma:** futuramente conectar WAIT/READY e I/O do SG-1000 por interfaces de barramento, sem incorporar o mapa do console diretamente ao núcleo Z80.

**Cuidado com a referência:** o código do MAME contém limitações e TODOs explícitos, inclusive sobre IM 0 e diferenças entre variantes. Usá-lo como comparador independente, junto ao manual Zilog e a testes dedicados; não como oráculo único. A pesquisa consultou o código-fonte, mas não executou testes diferenciais contra o MAME.

## Verificação local registrada em 22/10/2026

Na branch `main`, commit `d6228f24b4f562d5b366296c0494f00c23def8c9`, o usuário executou os testes no Windows com Python 3.14.7 e pytest 9.1.1:

- `python -m pytest -v`: **262 passed in 0.60s**.
- `ruff check .`: aprovado.
- `ruff format --check .`: 19 arquivos já formatados.
- `git diff --check`: sem erros reportados.

Essa execução confirma os testes presentes no repositório nesse commit, não valida instruções ainda ausentes, não substitui comparação diferencial com uma suíte independente e não comprova precisão ciclo a ciclo. A cobertura indicada nesta matriz continua sendo “caminho implementado” até validação específica por família.
