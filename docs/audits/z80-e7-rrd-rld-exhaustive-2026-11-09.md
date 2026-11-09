# F-023 — RRD/RLD: auditoria de nibbles, flags e ciclos

**Data:** 2026-11-09
**Baseline main:** \`397b01e32af41746d00b016858c4cc316147ba6b\`
**Branch candidata:** \`test/z80-f023-exhaustive-nibble-oracle-20261109\`

## 1. Frente documental — fabricante

### ZiLOG UM0080 (UM008011-0816)

As descrições oficiais RLD (p. 238) e RRD (p. 240) especificam a permutação entre os nibbles do acumulador e da memória apontada por HL. Ambas são 5 M-cycles e 18 T-states, com sequência nominal 4,4,3,4,3.

As flags explicitamente documentadas são:
- S e Z conforme o novo A;
- H=0;
- P/V conforme paridade par do novo A;
- N=0;
- C não afetado.

O UM0080 não especifica explicitamente os bits X/Y no texto de flags de RRD/RLD, nem WZ/MEMPTR. Portanto, esses itens não são promovidos a especificação pelo projeto.

Fonte: [ZiLOG UM0080 — RLD/RRD](https://www.zilog.com/docs/z80/um0080.pdf).

O ZiLOG Z80 CPU Technical Manual de 1977 também lista RLD/RRD com 5 M-cycles e 18 T-states e os efeitos de flags documentados. A matriz não é usada para inferir X/Y ou WZ.

Fonte histórica: [ZiLOG Z80 CPU Technical Manual (1977)](https://www.cpcwiki.eu/imgs/1/1b/Z80_CPU_Technical_Manual_1977.pdf).

## 2. Frente de engenharia reversa

Foram comparadas implementações sem reutilização de código:

| Referência | Arquivo/funções | Observação | Limite |
|---|---|---|---|
| MAME | \`src/devices/cpu/z80/z80.lst\`, macros \`rrd\` e \`rld\`; commit \`df58101a63890084c249119566c97bd849ca28ac\` | Define WZ=HL+1; X/Y são derivados de A; mantém C e atualiza S/Z/PV; H/N zerados | Emulador de referência, não prova de silício |
| ares | \`ares/component/processor/z80/instructions.cpp\`, \`instructionRLD\` e \`instructionRRD\`; commit \`19d051721bb14c46be92a713216421447dec91b7\` | Mesma transformação, WZ=HL+1 e flags X/Y de A | Emulador de referência, não prova de silício |

A concordância MAME/ares sustenta uma hipótese de compatibilidade observada. Não há evidência experimental independente nesta etapa para X/Y ou WZ.

## 3. Frente de arquitetura — oracle independente

A candidata adiciona em \`native/z80_cpu/src/cpu.rs\`:

\`tests::ed_rrd_rld_exhaustive_nibble_transform_and_flag_contract\`

Cobertura:
- RRD e RLD;
- 256 valores de A;
- 256 valores de (HL);
- Carry inicial 0 e 1;
- Total: 2 × 256 × 256 × 2 = 262.144 vetores;
- HL=FFFF incluído em vetor de borda para verificar o wrap de WZ.

O oracle calcula diretamente:
- RRD: \`A'=(A&F0)|(M&0F)\`; \`M'=((A&0F)<<4)|(M>>4)\`.
- RLD: \`A'=(A&F0)|(M>>4)\`; \`M'=(M<<4)|(A&0F)\`.
- Flags documentadas: C preservado; S/Z conforme A'; P/V conforme paridade par de A'; H/N zerados.

O teste também confere A, memória, HL, PC, R, Q, duração de 18T e WZ=HL+1. A verificação X/Y deriva bits 5/3 de A segundo a concordância MAME/ares; essa parte não é apresentada como oracle de fabricante ou evidência física.

## 4. Microsequenciador, WAIT e snapshot

A candidata adiciona em \`native/z80_cpu/src/sequencer.rs\`:

\`ed_return_nibble_rotate_differential_tests::rrd_rld_boundary_vectors_preserve_wait_snapshot_and_bus_order\`

São 16 casos dirigidos (dois opcodes × oito vetores), incluindo zeros, nibbles alternados, extremos, ambos os estados de Carry e HL=FFFF. Cada pedido de barramento recebe um WAIT T-state e o helper aplica encode/decode + restore após cada ciclo de barramento e ciclo interno.

Verifica:
- 18T nominais e 22T com quatro WAITs (um por cada pedido);
- sequência de barramento: fetch ED, fetch opcode terminal, read (HL), write (HL);
- estado arquitetural e memória finais iguais ao executor atômico;
- PC=1002, R=80 a partir de FE, WZ=HL+1 e ausência de request pendente.

A comparação atômico↔microsequenciador é diferencial interna, não oracle independente. O oracle independente é a fórmula direta das permutações e das flags documentadas.

## 5. Critérios de aceitação

1. 262.144 vetores atômicos passam para RRD/RLD.
2. S/Z/PV/H/N/C correspondem à fórmula derivada do UM0080.
3. X/Y é rotulado como hipótese de referência, não especificação.
4. 16 vetores microsequenciados passam com WAIT e snapshot/restore.
5. Ordem de barramento e tempos 18T/22T conferidos.
6. fmt, Clippy e testes Z80/SMS/SG-1000 passam no HEAD exato da PR.
7. F-023 permanece AUDIT_PARTIAL até resolver X/Y/WZ com evidência independente e ampliar a validação temporal.

## 6. Limites e estado

- WZ=HL+1: concordância MAME/ares, não verificação física.
- X/Y derivados de A: concordância MAME/ares, não especificação UM0080.
- ROM externo dedicado: NOT RUN.
- Hardware SMS: NOT RUN.
- CMOS/clones: NOT_VERIFIED.
- A alteração é de testes e documentação; nenhum runtime é alterado.

**Classificação:** a semântica documentada de transformação e flags ganha cobertura exaustiva no executor atômico; a trilha temporal ganha vetores dirigidos com WAIT/snapshot. F-023 permanece AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.
