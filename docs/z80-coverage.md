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
| `40–75, 77–7F` — LD r,r' | Implementado — cobertura inicial | Registradores e (HL); opcode `76` HALT não está implementado. |
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
| `D3, DB` — OUT (n),A / IN A,(n) | Ausente | Requer contrato separado de portas de I/O; não deve ser simulado como memória comum. |
| `E3` — EX (SP),HL | Implementado — cobertura inicial | Troca a palavra em (SP)/(SP+1) com HL; preserva SP e flags, trata wrap de 16 bits e contabiliza 19 T-states nominais. Testes de troca, ordem dos acessos e wraparound. |
| `E9` — JP (HL) | Implementado — cobertura inicial | Copia HL para PC; 4 T-states nominais, flags preservadas. |
| `F3, FB` — DI / EI | Implementado — cobertura inicial | IFF1/IFF2, DI imediato e atraso de reconhecimento após EI; interrupções exigem validação adicional. |
| `F9` — LD SP,HL | Implementado — cobertura inicial | Copia HL para SP; 6 T-states nominais, flags preservadas. |

## Famílias prefixadas

| Prefixo | Estado | Escopo necessário |
|---|---|---|
| `CB` | Implementado — cobertura inicial | RLC, RRC, RL, RR, SLA, SRA, SRL, BIT, RES e SET em registradores/(HL); SLL incluído separadamente como não documentado. Prefixos indexados ainda pendentes. |
| `ED` | Parcial | `ADC/SBC HL,rr`, `LD I,A`, `LD R,A`, `LD A,I` e `LD A,R` implementados. `R` incrementa nos fetches de opcode M1; `LD A,I/R` atualiza S/Z/X/Y, P/V a partir do estado `IFF2`, limpa H/N e preserva C. `IFF2` existe apenas como estado mínimo (inicialmente falso), sem lógica de interrupções. Ainda faltam operações de I/O por (C), transferências/comparações de bloco, RRD/RLD, IM 0/1/2, RETI/RETN e outras instruções ED documentadas. |
| `DD` | Ausente | Substituição de HL por IX onde aplicável, instruções IX e endereçamento indexado (IX+d), incluindo prefixos redundantes conforme o modelo adotado. |
| `FD` | Ausente | Equivalente para IY. |
| `DD CB d` / `FD CB d` | Ausente | Operações de bits/rotação sobre (IX+d)/(IY+d), incluindo variantes que também copiam resultado para registrador. |

## Estado interno, interrupções e ciclo de barramento

O código atual modela registradores principais A/F/B/C/D/E/H/L, pares AF/BC/DE/HL, SP, PC e um acumulador de T-states nominais. Ainda faltam, entre outros:

- Registradores alternativos AF', BC', DE' e HL' estão modelados; I e R também estão modelados. IX/IY ainda faltam.
- IFF1/IFF2, modos 0–2, HALT e solicitações NMI/INT têm um modelo inicial; arbitragem, reconhecimento e o protocolo completo de IM 0 ainda exigem validação.
- Ciclos M1, ciclos de leitura/escrita e I/O, estados de espera, contenção e temporização de barramento.
- Flags X/Y e efeitos não documentados validados contra referências e testes específicos.
- Comportamento de prefixos repetidos, opcodes não documentados e diferenças de revisão, conforme a meta de compatibilidade.

O contador `total_cycles` atual soma os custos nominais das instruções suportadas. Não significa que o processador já execute acessos ao barramento ciclo a ciclo.

## Ordem de implementação sugerida

1. Ampliar as operações não prefixadas restantes; as oito condições de salto/chamada/retorno, EX AF,AF', EXX, EX DE,HL, EX (SP),HL e RST já estão implementados.
2. Continuar as famílias prefixadas ED e ampliar testes de flags e comportamento de barramento; ADC/SBC HL,rr, transferências absolutas ED de palavras e transferências entre A, I e R têm implementação e testes sintéticos iniciais.
3. Implementar HALT e o modelo de interrupções antes de declarar completo o fluxo de controle.
4. Criar contrato de I/O e implementar IN/OUT com testes de endereço de porta.
5. Implementar famílias CB, ED, DD, FD e DD/FD-CB de forma sistemática.
6. Adicionar testes de diagnóstico de referência e, em etapa própria, refinar os ciclos e a interação com o barramento.
7. Só então ampliar para os opcodes não documentados relevantes, com comportamento sustentado por evidências.

Cada incremento deverá atualizar esta matriz, testes, documentação de hardware e README quando o estado do marco mudar. Não declarar “Z80 completo” enquanto houver lacunas ou enquanto os critérios de validação não forem satisfeitos.
