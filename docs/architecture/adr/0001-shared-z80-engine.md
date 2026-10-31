# ADR-0001 — motor Z80 compartilhado

- **Status:** Proposto para implementação
- **Data:** 2026-10-31
- **Escopo:** SG-1000, Master System, Game Gear e futuro Mega Drive/Genesis

## Contexto

A auditoria F0 confirmou que os arquivos `native/sg1000_core/src/cpu.rs` e `native/sms_core/src/cpu.rs` têm 3.257 linhas executáveis idênticas após normalização, na mesma ordem. As ALUs locais também correspondem ao código do crate `native/z80_alu`. SMS já reexporta o contrato `sega-z80-bus`; SG-1000 ainda mantém uma cópia local do contrato e não depende dos crates Z80 compartilhados.

Manter dois executores semanticamente iguais amplia o custo de auditoria, correções, testes e evolução temporal. Copiar novamente a implementação para um terceiro lugar perpetuaria o problema.

## Decisão

1. Criar o crate de CPU independente de plataforma em `native/z80_cpu`, pacote `sega-z80-cpu`, biblioteca `sega_z80_cpu`.
2. Reutilizar `sega-z80-alu` e `sega-z80-bus`; não copiar ALU, snapshots, traces ou contratos de barramento.
3. O crate compartilhado será o único proprietário de registradores, decoder, semântica de instruções, flags, WZ/MEMPTR, Q/R, prefixos, interrupções e estado interno Z80.
4. SG-1000, SMS e Game Gear fornecerão adaptadores para memória, I/O, interrupções e perfil da máquina. O decoder não conhecerá portas, mappers, registradores VDP/PSG/FM ou layout de RAM de console.
5. O futuro Genesis reutilizará o mesmo motor como CPU auxiliar. Ownership/reset/request/grant/arbitragem do barramento Z80 pertencem ao core Genesis e ao scheduler da máquina, não ao decoder.
6. Manter uma única semântica e um único decoder com duas granularidades de condução:
   - stepping por instrução, para compatibilidade, testes e ferramentas;
   - stepping temporal retomável, por micro-operações e acessos ao barramento.
   
   O modo por instrução deve consumir a mesma sequência de operações do motor temporal. Não criar um segundo decoder.
7. Durante a extração, preservar APIs públicas por fachadas/reexports quando isso evitar migração simultânea dos consumidores. Não remover os executores locais antes de SG-1000 e SMS dependerem do crate compartilhado.

## Contrato de barramento

A primeira fatia deve reutilizar o contrato `CpuBus` existente sem introduzir mudanças semânticas. O contrato atual cobre leituras/escritas lógicas, I/O e instrumentação diagnóstica parcial. A evolução para micro-operações deve ser aditiva e explícita: fetch M1, refresh, memória, I/O, acknowledge, ciclos internos e WAIT, com instante/duração somente quando validados.

O adaptador de máquina é responsável por side effects e mapeamento. A CPU não deve receber referências diretas a memória, VDP, PSG, mapper ou UI.

## Migração incremental

1. Inventariar e congelar os testes/vetores atuais de SG-1000 e SMS, incluindo proveniência e limitações dos oráculos.
2. Extrair o executor canônico para `sega-z80-cpu` sem alterar o comportamento.
3. Substituir ALUs locais pela dependência `sega-z80-alu` e consolidar o contrato de barramento com `sega-z80-bus`.
4. Adaptar SMS e SG-1000 para o mesmo crate, mantendo os adaptadores de máquina.
5. Executar no mesmo SHA fmt, Clippy, workspace tests, testes focados de ambos os cores, vetores externos fixados e testes diferenciais.
6. Só então remover os arquivos locais de executor/ALU duplicados.

## Critérios de aceitação

- [ ] Um único crate de CPU usado por SG-1000 e SMS.
- [ ] Nenhum decoder/ALU de produção duplicado nos cores de máquina.
- [ ] Os testes e vetores anteriores continuam passando, sem atualização silenciosa de expectativas.
- [ ] O mesmo programa e estado inicial produzem snapshots, memória observável e traces determinísticos nos adaptadores equivalentes.
- [ ] O teste #314 é preservado como regressão de wraparound; seu alcance permanece limitado ao cenário coberto.
- [ ] Issues #158 e #159 continuam como gates de cobertura semântica/estado/interrupções; não são encerradas por compilar o crate.
- [ ] O crate não contém dependência de SDL, Qt, PyO3, VDP, PSG ou módulos de máquina.
- [ ] A extração não promove automaticamente `SHORT_PASS`, `LONG_PASS` ou `TEMPORAL_PASS`.

## Consequências

A consolidação inicial deve ser uma refatoração de baixo risco porque o código executável já é idêntico. O risco principal passa a ser a mudança dos contratos de barramento/dependências e a perda acidental de testes específicos de cada máquina. A evolução temporal continua sendo uma etapa separada: T-states agregados não equivalem a precisão ciclo a ciclo.

## Referências internas

- [F0 — inventário remoto e reconciliação das PRs](../migrations/f0-remote-inventory-2026-10-31.md)
- [Plano de reorganização do runtime Z80](../migrations/shared-z80-runtime-reorganization.md)
- [Issue #158 — matriz de opcodes](https://github.com/leonardo201800478/sega-emulator/issues/158)
- [Issue #159 — interrupções e temporalidade](https://github.com/leonardo201800478/sega-emulator/issues/159)
- [PR #314 — LD (nn),HL e wraparound](https://github.com/leonardo201800478/sega-emulator/pull/314)
