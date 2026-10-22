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
| `07, 0F, 17, 1F` — rotações do acumulador | Ausente | RLCA, RRCA, RLA, RRA. |
| `08` — EX AF,AF' | Ausente | Requer banco alternativo AF'. |
| `09, 19, 29, 39` — ADD HL,rr | Implementado — cobertura inicial | Inclui SP; validar flags X/Y e todos os pares/casos-limite. |
| `0A, 1A` — LD A,(BC/DE) | Implementado — cobertura inicial | Lê A do endereço apontado por BC ou DE; 7 T-states nominais e flags preservadas. |
| `10` — DJNZ e | Ausente | Desvio condicional pelo contador B. |
| `18` — JR e | Implementado — cobertura inicial | Deslocamento relativo com sinal e wrap do PC já têm testes selecionados. |
| `20, 28, 30, 38` — JR cc,e | Implementado — cobertura inicial | NZ, Z, NC, C; as condições relativas disponíveis na base são estas quatro. |
| `22, 2A` — LD (nn),HL / LD HL,(nn) | Ausente | Acesso absoluto de 16 bits. |
| `27` — DAA | Ausente | Ajuste decimal; exige testes de flags e combinações de entrada. |
| `2F` — CPL | Ausente | Complemento do acumulador. |
| `32, 3A` — LD (nn),A / LD A,(nn) | Ausente | Acesso absoluto de 8 bits. |
| `37, 3F` — SCF / CCF | Ausente | Operações de carry e flags X/Y. |
| `40–75, 77–7F` — LD r,r' | Implementado — cobertura inicial | Registradores e (HL); opcode `76` HALT não está implementado. |
| `76` — HALT | Ausente | Precisa de estado suspenso, comportamento de PC e interação com interrupções. |
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
| `C2, CA, D2, DA` — JP cc,nn | Parcial | Só NZ, Z, NC e C; faltam PO, PE, P e M. |
| `C5, D5, E5, F5` — PUSH qq | Implementado — cobertura inicial | BC, DE, HL e AF; verificar ordem de acesso e tempos de barramento em fase futura. |
| `C1, D1, E1, F1` — POP qq | Implementado — cobertura inicial | BC, DE, HL e AF; verificar ordem de acesso e efeitos de AF. |
| `C7, CF, D7, DF, E7, EF, F7, FF` — RST | Ausente | Chamadas a vetores fixos e comportamento de pilha. |
| `C4, CC, D4, DC` — CALL cc,nn | Parcial | Só NZ, Z, NC e C; faltam PO, PE, P e M. |
| `CD` — CALL nn | Implementado — cobertura inicial | Sub-rotina absoluta. |
| `C0, C8, D0, D8` — RET cc | Parcial | Só NZ, Z, NC e C; faltam PO, PE, P e M. |
| `C9` — RET | Implementado — cobertura inicial | Retorno incondicional. |
| `D3, DB` — OUT (n),A / IN A,(n) | Ausente | Requer contrato separado de portas de I/O; não deve ser simulado como memória comum. |
| `D9` — EXX | Ausente | Requer bancos alternativos BC', DE' e HL'. |
| `E3` — EX (SP),HL | Ausente | Troca entre pilha e HL. |
| `E9` — JP (HL) | Ausente | Salto indireto. |
| `EB` — EX DE,HL | Ausente | Troca de pares. |
| `F3, FB` — DI / EI | Ausente | Requer estado de habilitação de interrupções e regra de atraso do EI. |
| `F9` — LD SP,HL | Ausente | Transferência de par para SP. |

## Famílias prefixadas

| Prefixo | Estado | Escopo necessário |
|---|---|---|
| `CB` | Ausente | RLC, RRC, RL, RR, SLA, SRA, SLL (não documentado), SRL, BIT, RES e SET para registradores e (HL). |
| `ED` | Ausente | ADC/SBC HL,rr; operações de I/O por (C); transferências e comparações de bloco; LD I/R; RRD/RLD; IM 0/1/2; RETI/RETN e demais instruções documentadas. |
| `DD` | Ausente | Substituição de HL por IX onde aplicável, instruções IX e endereçamento indexado (IX+d), incluindo prefixos redundantes conforme o modelo adotado. |
| `FD` | Ausente | Equivalente para IY. |
| `DD CB d` / `FD CB d` | Ausente | Operações de bits/rotação sobre (IX+d)/(IY+d), incluindo variantes que também copiam resultado para registrador. |

## Estado interno, interrupções e ciclo de barramento

O código atual modela registradores principais A/F/B/C/D/E/H/L, pares AF/BC/DE/HL, SP, PC e um acumulador de T-states nominais. Ainda faltam, entre outros:

- Bancos alternativos AF', BC', DE', HL', registradores IX/IY, I e R.
- IFF1/IFF2, modo de interrupção e comportamento de EI/DI, HALT, NMI e INT.
- Ciclos M1, ciclos de leitura/escrita e I/O, estados de espera, contenção e temporização de barramento.
- Flags X/Y e efeitos não documentados validados contra referências e testes específicos.
- Comportamento de prefixos repetidos, opcodes não documentados e diferenças de revisão, conforme a meta de compatibilidade.

O contador `total_cycles` atual soma os custos nominais das instruções suportadas. Não significa que o processador já execute acessos ao barramento ciclo a ciclo.

## Ordem de implementação sugerida

1. Completar operações básicas não prefixadas e condições: acessos indiretos, rotações, DAA, CPL, SCF/CCF, DJNZ, JP (HL), EX, LD SP,HL e RST.
2. Completar instruções aritméticas de 16 bits, incluindo `ADC HL,rr` e `SBC HL,rr`, com flags e tempos testados.
3. Implementar HALT e o modelo de interrupções antes de declarar completo o fluxo de controle.
4. Criar contrato de I/O e implementar IN/OUT com testes de endereço de porta.
5. Implementar famílias CB, ED, DD, FD e DD/FD-CB de forma sistemática.
6. Adicionar testes de diagnóstico de referência e, em etapa própria, refinar os ciclos e a interação com o barramento.
7. Só então ampliar para os opcodes não documentados relevantes, com comportamento sustentado por evidências.

Cada incremento deverá atualizar esta matriz, testes, documentação de hardware e README quando o estado do marco mudar. Não declarar “Z80 completo” enquanto houver lacunas ou enquanto os critérios de validação não forem satisfeitos.
