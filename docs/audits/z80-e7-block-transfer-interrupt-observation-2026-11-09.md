# F-022 — flags observáveis no ciclo de repetição interrompido

**Data:** 2026-11-09
**Baseline main:** `fa069162306fb330864b6d77633ceba0f976e3ca`
**Branch candidata:** `test/z80-f022-repeat-im2-observation-20261109`
**Escopo:** LDIR/LDDR/CPIR/CPDR; visibilidade de F quando INT é aceita durante o ciclo interno de repetição.

## 1. Frente documental

### ZiLOG UM0080

UM0080 documenta as operações de transferência e busca em bloco, o decremento de BC, a condição de repetição e o comportamento das flags documentadas. Não define universalmente os bits não documentados X/Y durante a interrupção que substitui a próxima iteração. O manual é usado como base para a semântica documentada; não é usado como fonte para afirmar PC.13/PC.11.

Fonte primária: [ZiLOG Z80 CPU User Manual UM0080](https://www.zilog.com/docs/z80/um0080.pdf).

### Z80 Block Flags Test v5.0

O teste público de Peter Helcmanovsky estabelece o cenário preciso: a instrução de bloco repetitiva já executou ao menos uma iteração, o contador ainda é não-zero e uma interrupção é aceita em vez de iniciar a repetição seguinte. Nesse estado, PC aponta novamente para o byte ED; YF corresponde a PC.13 e XF a PC.11 para LDxR/CPxR. O handler IM2 observa F.

O README declara resultados confirmados em hardware Z80 NMOS, mas também ressalta que a suíte é limitada e não exaustiva e que clones/CMOS podem divergir. A lista pública inclui GoldStar Z8400APS, Mostek MK3880N-4, NEC D70008AC-6, Zilog Z8400APS e SGS Z8400AB1 em diferentes versões da suíte. Isso é evidência externa de hardware para o comportamento delimitado, não validação do Master System.

Fontes:
- [README e licença MIT do teste](https://github.com/MrKWatkins/ZXSpectrumNextTests/blob/develop/Tests/ZX48_ZX128/Z80BlockInstructionFlags/README.md)
- [Código assembly v5.0](https://github.com/MrKWatkins/ZXSpectrumNextTests/blob/develop/Tests/ZX48_ZX128/Z80BlockInstructionFlags/z80_block_flags_test.asm)
- [Resultados de hardware e versões](https://github.com/redcode/Z80/wiki/Z80-Block-Flags-Test)

## 2. Frente de engenharia reversa

Foram inspecionadas implementações de referência sem reutilização de código:

| Referência | Arquivos/funções | Comportamento relevante | Limite |
|---|---|---|---|
| MAME | `src/devices/cpu/z80/z80.lst`: macros `ldir`, `lddr`, `cpir`, `cpdr` | Após a operação, quando há repetição, executa o ciclo interno de 5 T, recua PC em 2, define WZ=PC+1 e deriva X/Y do byte alto de PC | Implementação de emulador, não prova de silício |
| ares | `ares/component/processor/z80/instructions.cpp`: `instructionLDIR`, `instructionLDDR`, `instructionCPIR`, `instructionCPDR` | Mesmo recuo de PC e substituição de XF=PC.11/YF=PC.13; WZ=PC+1 | Implementação de emulador; não tratada como oracle físico |
| redcode/Z80 | `sources/Z80.c`: macros `LDXR` e `CPXR` | Repete com contador não-zero e, no caminho repetitivo, define X/Y com bits do PC | Implementação de referência; licença não usada para copiar código |

A concordância entre MAME, ares e redcode é evidência comparativa de implementação. A confirmação física relevante vem do teste v5.0, com as limitações acima.

## 3. Frente de arquitetura e modelo

A alteração candidata acrescenta em `native/z80_cpu/src/sequencer.rs` o teste:

`ed_block_transfer_repeat_interrupt_tests::block_transfer_and_compare_repeat_flags_are_observable_through_im2_and_resume`

O cenário executa LDIR, LDDR, CPIR e CPDR com contador não-zero após a primeira iteração. A linha INT é afirmada no último T-state do estado `EdBlockRepeatInternal`. A interrupção é atendida em IM2 com vetor FF e handler em 9000h.

O handler executa:

`PUSH AF; POP AF; EI; RETI`

O teste inspeciona o byte F salvo na pilha, confirma o retorno ao endereço ED, a restauração de SP/IFF1/IFF2 e a execução da iteração seguinte. O estado do microsequenciador passa por encode/decode + restore em cada fronteira de barramento e ciclo interno.

### Vetores e resultados esperados

- PC de início ED: A002h; após o recuo de repetição, PC permanece A002h.
- PC alto A0h: YF=1 (PC.13), XF=0 (PC.11), XY=20h.
- LDIR/LDDR: flags preservadas S/Z/C, PV=1, H/N=0 e XY=20h; F esperado E5h.
- CPIR/CPDR: A=10h, dado=21h, resultado EFh, half-borrow=1, N=1, PV=1, C preservado e XY=20h; F esperado B7h.
- WZ/MEMPTR=PC+1 é verificado como comportamento comparativo com MAME/ares, não como observação física independente.
- Após RETI, a instrução retoma e executa outra iteração, com BC ainda não-zero e PC/WZ coerentes.

A comparação entre executor atômico e microsequenciador é diferencial interna; não é tratada como oracle independente. O valor esperado de F decorre do contrato observado no teste v5.0 combinado com os efeitos documentados pelo UM0080.

## 4. Critérios de aceitação

1. Os quatro opcodes repetitivos de transferência/busca são exercitados.
2. INT é afirmada no último T-state do ciclo interno de repetição, não antes do ponto observável.
3. IM2 atende o vetor e permite observar F por PUSH AF.
4. RETI retorna ao endereço ED e a instrução é retomada.
5. Snapshot encode/decode + restore é aplicado em cada fronteira microsequenciada.
6. Executor atômico e microsequenciador terminam em estado arquitetural e memória equivalentes; essa comparação permanece evidência interna.
7. CI focado Z80 passa no HEAD exato da PR.
8. F-022 permanece AUDIT_PARTIAL até auditoria das flags de iteração simples, demais fronteiras, WZ/MEMPTR e efeitos temporais independentes.

## 5. Limites e classificação

- Cobertura: um cenário dirigido por opcode, não cobertura combinatória.
- X/Y durante interrupção: evidência física publicada para chips testados no ecossistema ZX Spectrum; não é uma prova de todos os lotes NMOS nem do SMS físico.
- WZ/MEMPTR: comparação com MAME/ares, não evidência física independente.
- CMOS/clones: NOT_VERIFIED.
- Hardware SMS: NOT_RUN.
- Execução integral do ROM v5.0 dentro deste projeto: NOT_RUN.
- CI da PR #712: #6045 / run `42649308679` PASS no HEAD exato `7390744d9d6dc61ef2db3786f75fc9073dabdc12`; CI pós-merge #6046 / run `42649608469` PASS no HEAD da main `5e4a703daefdebe62fb7f8d220752359df59f8e3`.

**Classificação:** a subquestão de observação de X/Y em LDxR/CPxR interrompido recebe evidência externa NMOS parcial e fixture de regressão candidato. F-022 continua AUDIT_PARTIAL; E7 continua IN_PROGRESS; E8 continua BLOCKED.


## 6. Integração e validação final

PR #712 foi squash-merged. Merge SHA e HEAD da main: `5e4a703daefdebe62fb7f8d220752359df59f8e3`. O HEAD validado da PR foi `7390744d9d6dc61ef2db3786f75fc9073dabdc12`.

CI #6045 / run `42649308679`: PASS no HEAD exato da PR. A execução focada reportou fmt, Clippy, testes Z80/SMS/SG-1000 e SingleStepTests DD/FD PASS; documentação PASS; agregador Rust quality/unit PASS. CI pós-merge #6046 / run `42649608469`: SUCCESS no HEAD exato da main, com Z80 focused e documentation patch hygiene PASS. Workspace regression e jobs não relacionados ficaram SKIPPED/NOT RUN.

A subquestão de flags X/Y no ciclo de repetição interrompido para LDIR/LDDR/CPIR/CPDR recebe fixture integrada e evidência externa NMOS delimitada. F-022 permanece AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.
