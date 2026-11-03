# Auditoria E4B — ALU A,(IX/IY+d) no Z80

**Data:** 2026-11-03  
**Main de referência após E4A:** `e9a3eff6c3733b4f11443954a647471c9d3908ff`  
**PR:** [#472 — E4B indexed-memory ALU operations](https://github.com/leonardo201800478/sega-emulator/pull/472)  
**Candidato de runtime validado:** `245d42eb0337d41295329155de8e151feceb9022`  
**Workflow:** [CI run 41738004725](https://github.com/leonardo201800478/sega-emulator/actions/runs/41738004725)  
**Schema do microsequenciador:** 27 (sem novo bump em E4B)  
**Estado:** IMPLEMENTED / QUICK_PASS / SHORT_PASS para as oito operações ALU indexadas. Não declarar LONG_PASS ou TEMPORAL_PASS.

## 1. Escopo

Implementadas as oito formas com fonte `(IX/IY+d)`:

| Opcode base | Operação |
|---|---|
| `80`–`87` | ADD A,r |
| `88`–`8F` | ADC A,r |
| `90`–`97` | SUB r |
| `98`–`9F` | SBC A,r |
| `A0`–`A7` | AND r |
| `A8`–`AF` | XOR r |
| `B0`–`B7` | OR r |
| `B8`–`BF` | CP r |

O caminho E4B seleciona o campo fonte 6. As formas de half-register (IXH/IXL/IYH/IYL) e as demais formas de ALU com registrador continuam fora desta fatia.

## 2. Contrato temporal

As oito operações indexadas usam o mesmo calendário de leitura indexada documentado para as instruções de memória IX/IY:

- 5 M-cycles e 19 T-states com um prefixo.
- Sequência nominal: `4,4,3,5,3` — prefix M1, opcode M1, displacement read, internal M-cycle e effective-address memory read.
- Prefixos DD/FD adicionais acrescentam seus M1 e T-states.
- WAIT é adicional e não pode alterar o endereço ou adiantar o commit ALU.

A implementação reutiliza a leitura efetiva já temporizada pelo sequenciador; a operação aritmética ocorre somente quando o request de memória retorna.

## 3. Implementação arquitetural

Arquivo principal: `native/z80_cpu/src/sequencer.rs`.

- Reutiliza `IndexDisplacementRead` e `IndexDisplacementInternal` de E4A.
- Encaminha o request da effective address para a fase `MemoryReadAlu` já existente.
- Reutiliza `execute_alu_operation` e os helpers ALU compartilhados. Não existe segunda implementação de soma/subtração/lógica ou de flags.
- A validação de fase permite `MemoryReadAlu` com prefixo Index somente para opcodes cujo campo fonte é 6.
- A validação de snapshot do displacement pendente foi ampliada para reconhecer a família E4B. O teste reproduz o estado com WAIT antes da conclusão da leitura de displacement.
- WZ conserva a effective address conforme o modelo MEMPTR do core. Isso não é apresentado como prova direta de comportamento de silício em todas as revisões.

Não foi adicionado novo estado serializado nem novo schema: `MemoryReadAlu` e as fases E4A já existentes são suficientes.

## 4. Flags e semântica

A suíte usa vetores fixos para os oito operadores, em IX e IY, incluindo:

- S e Z.
- Half-carry e half-borrow.
- Paridade/overflow.
- N.
- Carry e borrow.
- Carry-in para ADC e SBC.
- Bits não documentados X/Y derivados do resultado nas operações aritméticas/lógicas.
- CP preserva A e copia X/Y do operando, em vez de derivá-los do resultado da subtração.

Os vetores incluem casos de overflow assinado, carry-in, borrow-in, half-carry/borrow, resultado zero e valores com bits X/Y definidos. A ALU compartilhada permanece a única fonte de cálculo de flags.

## 5. Testes direcionados

Arquivo: `native/z80_cpu/tests/indexed_alu_e4b.rs`.

1. `indexed_alu_matrix_matches_fixed_arithmetic_and_flag_vectors_for_ix_and_iy`: 16 vetores fixos executados com IX e IY (32 execuções).
2. `indexed_alu_wait_states_are_additive_and_snapshots_resume_every_bus_phase`: WAIT no prefix M1, opcode M1, displacement e leitura efetiva; round-trip de snapshot nos safe points.
3. `indexed_alu_repeated_and_alternating_prefixes_use_last_index_register`: DD/DD, FD/FD, DD/FD, FD/DD e sequências de três prefixos.

Arquivo atualizado: `native/z80_cpu/tests/prefix_fsm.rs`.

- A regressão DD/FD alternada agora executa uma operação ALU indexada, valida a effective address escolhida pelo último prefixo e restaura snapshots no displacement e na leitura ALU pendentes.
- O teste de cadeia longa de prefixos continua verificando o limite artificial, usando `0xC6` como opcode ainda não implementado por E6, em vez do antigo `0x86` que E4B passou a suportar.

## 6. CI do candidato

No workflow [41738004725](https://github.com/leonardo201800478/sega-emulator/actions/runs/41738004725), os quatro jobs terminaram em PASS:

| Job | Resultado |
|---|---|
| Rust quality and unit regression — fmt, Clippy `-D warnings`, testes direcionados e workspace | PASS |
| Python lint and regression — pytest, Ruff lint e Ruff format | PASS |
| Scheduler deterministic stress — perfil short | PASS |
| Windows x64 — D3D12, video-runtime, WASAPI/XAudio2 e audio-output | PASS |

O candidato de runtime `245d42eb` foi validado. Alterações documentais posteriores devem receber seu próprio CI antes do merge.

## 7. Limites da evidência

- Não foi executado oráculo diferencial externo nesta fatia.
- ZEXDOC e ZEXALL não foram executados.
- Não foi produzido trace independente de barramento por T-state.
- Scheduler stress foi executado no perfil short; não equivale ao perfil long.
- Os vetores fixos e o CI sustentam QUICK_PASS/SHORT_PASS para E4B, não LONG_PASS ou TEMPORAL_PASS.
- A integração sistêmica do Z80 aos barramentos SMS/SG-1000 continua INTEGRATION_PENDING.

## 8. Próxima fatia

**E4C — INC/DEC (IX/IY+d):**

- Leitura da effective address.
- Ciclo interno de read-modify-write.
- Escrita do resultado.
- Flags S/Z/H/PV/N atualizadas e C preservado.
- WAIT e snapshot em cada safe point.

E4 só será declarada concluída após E4A, E4B e E4C passarem seus gates individuais.
