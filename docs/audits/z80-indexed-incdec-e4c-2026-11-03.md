# Auditoria E4C — INC/DEC (IX/IY+d)

**Data:** 2026-11-04  
**Baseline:** `main` — `72483777dd2d7129b31fe8325d09733f15805e2e`  
**PR:** [#475](https://github.com/leonardo201800478/sega-emulator/pull/475)  
**Branch:** `feat/z80-e4c-integrated-baseline-2026-11-04`  
**SHA validado integralmente nesta PR:** `704dfd391ae1ea994fd53b8fa4a9519275746ff8`  
**Schema de snapshot:** 27 (sem alteração)

## 1. Escopo

Implementar no microsequenciador compartilhado:
- `INC (IX+d)` — DD 34 d
- `INC (IY+d)` — FD 34 d
- `DEC (IX+d)` — DD 35 d
- `DEC (IY+d)` — FD 35 d

## 2. Reutilização arquitetural

A implementação estende o caminho existente de displacement e reutiliza as fases de read-modify-write:
- `IndexDisplacementRead`
- `IndexDisplacementInternal`
- `MemoryReadIncDec`
- `MemoryIncDecInternal`
- `MemoryWrite`

Não foi criado um executor RMW paralelo nem duplicada a lógica de cálculo de flags. A operação de incremento/decremento passa por `increment_decrement_register`, já compartilhada com INC/DEC indireto.

O estado serializado mantém o schema 27. Os estados intermediários incluem request de leitura pendente, ciclo interno RMW e request de escrita pendente.

## 3. Referência ZiLOG e sequência nominal

Referência primária consultada: **ZiLOG Z80 CPU User Manual UM008011-0816**, descrição de `INC (IX+d)` nas páginas impressas 168–169 e `DEC (IX+d)/(IY+d)` nas páginas 170–171.

O manual especifica para ambas as operações:
- 6 M-cycles;
- 23 T-states;
- distribuição `4, 4, 3, 5, 4, 3`;
- S/Z/H/PV/N atualizados segundo o resultado;
- Carry não afetado.

Para INC, P/V é setado quando o operando anterior é `7Fh`; N é resetado. Para DEC, P/V é setado quando o operando anterior é `80h`; N é setado. O manual não define os bits não documentados X/Y; os testes atuais exercitam a política já adotada pelo core, sem apresentá-la como evidência elétrica independente.

## 4. Implementação e validação dirigida

Arquivo de implementação: `native/z80_cpu/src/sequencer.rs`  
Teste: `native/z80_cpu/tests/indexed_incdec_e4c.rs`

Cobertura adicionada:
- IX e IY;
- INC e DEC;
- valores `00`, `01`, `7F`, `80` e `FF`;
- Carry preservado e flags S/Z/H/PV/N;
- displacement zero, `-128` e `+127`;
- wrap do endereço efetivo e do PC;
- prefixos repetidos e alternados (last-prefix-wins);
- WAIT nos fetches M1, displacement, leitura efetiva e escrita;
- snapshot/restore nos ciclos internos, leitura pendente, intervalo RMW e escrita pendente;
- ordem read → 1T interno → write e índices M-cycle;
- 23 T-states nominais com um prefixo, acrescidos de WAIT.

## 5. Estado de evidência

| Evidência | Estado |
|---|---|
| Implementação no microsequenciador | IMPLEMENTED |
| Testes direcionados no candidato inicial `ad464672c2c3a01620a1f96b0f53395d0e905355` | QUICK_PASS / SHORT_PASS em CI |
| Workflow inicial | [41739328124](https://github.com/leonardo201800478/sega-emulator/actions/runs/41739328124), 4/4 jobs PASS |
| Execução integral dos gates no HEAD desta PR (`704dfd391ae1ea994fd53b8fa4a9519275746ff8`) | CI [41745463887](https://github.com/leonardo201800478/sega-emulator/actions/runs/41745463887): 4/4 jobs PASS |
| LONG_PASS | NOT RUN |
| ZEXDOC / ZEXALL | NOT RUN |
| Comparação diferencial externa completa | NOT RUN |
| Trace independente por T-state / evidência de hardware | NOT RUN |
| TEMPORAL_PASS | NOT RUN |

O CI verde confirma gates de compilação, lint, testes do workspace, stress determinístico e compilação/testes Windows x64 de vídeo e áudio. Não constitui prova independente de precisão temporal.

## 6. Limitações remanescentes e próximos passos

- Histórico de correções: `41741995929` identificou tipos ambíguos no fixture; `41742295721` identificou type-complexity; `41742519601` identificou formatação. As três correções foram aplicadas antes do SHA final `bac497462a3fc13a1f47892d25d435beefddd6d8`, cujo CI integral `41742682599` concluiu 4/4 jobs PASS. A validação final da PR #475 foi repetida no HEAD `704dfd391ae1ea994fd53b8fa4a9519275746ff8`, workflow `41745463887`, também com 4/4 jobs PASS.
- Esta PR atualiza somente a auditoria específica E4C. A matriz semântica, o scorecard, a estratégia de resolução e os roadmaps ainda precisam ser reconciliados em uma atualização documental própria após a integração do código.
- E4 só pode ser fechada após E4A, E4B e E4C aprovadas individualmente.
- Prosseguir para E5: DDCB/FDCB no microsequenciador, distinguindo BIT (sem write-back) das operações RMW.
