# Auditoria E4C — INC/DEC (IX/IY+d)

**Data:** 2026-11-03  
**Baseline:** `main` — `04ee09f9839bfe29a595abbed831d055cc8d177a`  
**PR:** [#473](https://github.com/leonardo201800478/sega-emulator/pull/473)  
**Branch:** `feat/z80-indexed-displacement-e4c-2026-11-03`  
**Candidato com cobertura de displacement ampliada:** `5bf09790d3cd0dddc0b4ed7b9a434773f3d403db`  
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
| Cobertura adicional de displacement no SHA `5bf09790d3cd0dddc0b4ed7b9a434773f3d403db` | CI pendente na criação deste relatório |
| LONG_PASS | NOT RUN |
| ZEXDOC / ZEXALL | NOT RUN |
| Comparação diferencial externa completa | NOT RUN |
| Trace independente por T-state / evidência de hardware | NOT RUN |
| TEMPORAL_PASS | NOT RUN |

O CI verde confirma gates de compilação, lint, testes do workspace, stress determinístico e compilação/testes Windows x64 de vídeo e áudio. Não constitui prova independente de precisão temporal.

## 6. Limitações remanescentes e próximos passos

- O workflow `41741995929` identificou E0689 no fixture de teste (tipos inteiros ambíguos), não no runtime; a fixture recebeu tipos explícitos no SHA `5bf09790d3cd0dddc0b4ed7b9a434773f3d403db`. Confirmar CI integral do candidato corrigido.
- Atualizar a matriz semântica DD/FD, a estratégia de resolução e os roadmaps com a integração E4C.
- E4 só pode ser fechada após E4A, E4B e E4C aprovadas individualmente.
- Prosseguir para E5: DDCB/FDCB no microsequenciador, distinguindo BIT (sem write-back) das operações RMW.
