# Auditoria E4A — LD indexado com displacement no Z80

**Data:** 2026-11-03  
**Main de referência:** `5b61b918055613902a70d60a2fdf645bb543f807`  
**PR:** [#471 — E4A indexed displacement loads and stores](https://github.com/leonardo201800478/sega-emulator/pull/471)  
**Candidato de runtime validado:** `88b3c6fa7812c577a0c91693cebbe21e75b2c2c7`  
**Workflow:** [CI run 41734982519](https://github.com/leonardo201800478/sega-emulator/actions/runs/41734982519)  
**Schema do microsequenciador:** 27  
**Estado:** IMPLEMENTED / QUICK_PASS / SHORT_PASS para o escopo E4A. Não declarar LONG_PASS ou TEMPORAL_PASS.

## 1. Escopo implementado

- `LD r,(IX+d)`
- `LD r,(IY+d)`
- `LD (IX+d),r`
- `LD (IY+d),r`
- `LD (IX+d),n`
- `LD (IY+d),n`

E4A não inclui ALU `A,(IX/IY+d)` (E4B), `INC/DEC (IX/IY+d)` (E4C), DDCB/FDCB (E5) ou fallback geral para prefixos ignorados (E6).

## 2. Contrato temporal documentado

ZiLOG Z80 CPU User Manual UM008011-0816:

| Família | Página impressa | M-cycles | T-states | Sequência nominal |
|---|---:|---:|---:|---|
| `LD r,(IX+d)` | 75 | 5 | 19 | `4,4,3,5,3` |
| `LD r,(IY+d)` | 77 | 5 | 19 | `4,4,3,5,3` |
| `LD (IX+d),r` | 81 | 5 | 19 | `4,4,3,5,3` |
| `LD (IY+d),r` | 83 | 5 | 19 | `4,4,3,5,3` |
| `LD (IX+d),n` | 86 | 5 | 19 | `4,4,3,5,3` |
| `LD (IY+d),n` | 87 | 5 | 19 | `4,4,3,5,3` |

A sequência nominal foi preservada sem introduzir um executor temporal paralelo:

- Loads/stores com registrador: M1 prefixo (4T), M1 opcode (4T), displacement (3T), ciclo interno (5T), acesso à memória efetiva (3T).
- Store imediato: M1 prefixo (4T), M1 opcode (4T), displacement (3T), leitura do imediato mais 2 T-states internos no mesmo M-cycle de 5T, escrita efetiva (3T).

WAIT states são adicionais ao tempo nominal. Os testes verificam que WAIT não altera a ordem de requests nem avança prematuramente o commit da instrução.

## 3. Implementação arquitetural

Arquivo principal: `native/z80_cpu/src/sequencer.rs`.

Fases serializáveis adicionadas:

- `IndexDisplacementRead`
- `IndexDisplacementInternal`
- `IndexedImmediateReadMemoryWrite`
- `IndexedImmediateStoreInternal`

Propriedades verificadas:

- Displacement é interpretado como inteiro assinado de 8 bits.
- Effective address usa soma com wrap de 16 bits.
- PC avança em cada byte efetivamente consumido, inclusive em wrap por `0xFFFF`.
- IX/IY efetivo respeita last-prefix-wins em DD/FD repetidos ou alternados.
- Requests de memória permanecem no handshake existente do barramento.
- WAIT states são contabilizados como tempo adicional.
- Snapshot/restore preserva fase, request pendente, endereço, valor e progresso interno nos safe points testados.
- WZ recebe o effective address. Esse comportamento é mantido como modelo MEMPTR do core; o manual de instruções citado não constitui prova direta de comportamento físico de WZ em todos os clones/revisões.

### Exceção H/L

Nas formas:

- `LD H,(IX/IY+d)`
- `LD L,(IX/IY+d)`
- `LD (IX/IY+d),H`
- `LD (IX/IY+d),L`

H/L são os registradores convencionais do par HL. Não são IXH/IXL ou IYH/IYL. Os testes de matriz cobrem load e store dessa exceção.

### Snapshot

O schema foi elevado de 26 para 27 para serializar as novas fases. O codec é do microsequenciador Z80, não um save state integral da máquina SMS. A suíte cobre round-trip nos estados intermediários e com requests/WAIT pendentes.

## 4. Testes direcionados

Arquivo: `native/z80_cpu/tests/indexed_memory_e4a.rs`.

Dez testes na suíte cobrem:

1. Load via IY com displacement negativo, wrap, WAIT e snapshot.
2. `LD H,(IX+d)` com H convencional.
3. `LD (IX+d),n`, ordem displacement/imediato e snapshot.
4. `LD (IY+d),L` com L convencional e wrap.
5. Matriz H/L com displacement 0, -128, +127 e wrap de endereço.
6. WAIT em cada barramento de indexed load e snapshot.
7. PC próximo de `0xFFFF` em indexed immediate store, WAIT em cada acesso e snapshot.
8. Matriz de todos os destinos B/C/D/E/H/L/A.
9. Matriz de origens B/C/D/E/H/L/A para indexed store.
10. Prefixos DD/FD repetidos e alternados, seleção do último índice e custo de T-states.

O teste de regressão E3A1 para `LD H,(IX+d)` foi atualizado: deixou de esperar `UnsupportedOpcode` e passou a verificar o endereço efetivo IX+d e o destino H convencional.

## 5. CI do candidato de runtime

No workflow [41734982519](https://github.com/leonardo201800478/sega-emulator/actions/runs/41734982519), os quatro jobs terminaram em PASS:

| Job | Resultado |
|---|---|
| Rust quality and unit regression — fmt, Clippy, testes direcionados SMS/SG-1000 e workspace | PASS |
| Python lint and regression — pytest, Ruff lint e Ruff format | PASS |
| Scheduler deterministic stress — perfil short | PASS |
| Windows x64 — D3D12, video-runtime tests, WASAPI/XAudio2 e audio-output tests | PASS |

O run foi executado sobre o candidato de runtime `88b3c6fa`. Alterações documentais posteriores devem receber seu próprio CI antes do merge.

## 6. Limites da evidência

- Não foi executado SingleStepTests como oráculo externo nesta fatia.
- ZEXDOC e ZEXALL não foram executados.
- Não foi produzido trace independente de barramento para cada T-state.
- O Scheduler stress executado foi o perfil short; não equivale ao perfil long.
- A aprovação de CI e os testes dirigidos sustentam QUICK_PASS/SHORT_PASS para E4A, não LONG_PASS nem TEMPORAL_PASS.
- A integração sistêmica do Z80 aos barramentos SMS/SG-1000 e a validação temporal ponta a ponta continuam INTEGRATION_PENDING.

## 7. Próximas fatias

1. **E4B:** ADD, ADC, SUB, SBC, AND, XOR, OR e CP com `(IX/IY+d)`; validar flags documentadas e bits X/Y.
2. **E4C:** INC/DEC `(IX/IY+d)`; validar read-modify-write, flags, WAIT e snapshots.
3. **E5:** DDCB/FDCB, incluindo opcode final sem M1 incorreto.
4. **E6–E9:** fallback de prefixos, fechamento semântico, validação formal e integração temporal.

E4 só poderá ser declarada concluída após E4A, E4B e E4C passarem seus respectivos gates.
