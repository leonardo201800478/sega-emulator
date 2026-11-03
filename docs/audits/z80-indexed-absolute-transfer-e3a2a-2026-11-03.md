# E3A2a — Transferências absolutas com IX/IY

**Baseline:** ee9d39fc017b53493cc48d1343a7ca4b77581d3b  
**Escopo:** LD (nn),IX/IY e LD IX/IY,(nn).

## Implementação

- Reutiliza `BaseAbsoluteTransfer`; não cria uma segunda sequência para IX/IY.
- O par 2 é resolvido pelo latch E2, portanto o mesmo fluxo serve a HL, IX ou IY.
- O contador M-cycle conserva todos os M1 DD/FD, inclusive prefixos repetidos/alternados.
- Snapshot/restore aceita o latch indexado nos estágios de endereço e de leitura/escrita de dados.
- WAIT continua associado ao request ativo e estende a duração sem alterar sua identidade.

## Semântica e testes

- Escrita little-endian: byte baixo em nn, byte alto em nn+1.
- Leitura little-endian: byte baixo de nn, byte alto de nn+1.
- WZ recebe nn+1.
- Vetores cobrem IX store, IY load, snapshot nos dois ciclos de dados, WAIT e DD/FD last-prefix-wins.

**Limites:** PUSH/POP IX/IY e EX (SP),IX/IY permanecem E3A2b/E3A2c; não há LONG_PASS ou TEMPORAL_PASS.
