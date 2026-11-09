# Auditoria E7.7 — F-002: LD A,(BC)/(DE) e LD (BC)/(DE),A

**Baseline main:** `37047ca2739dcfcf1fae6d0fb7bff93dafcaed0b` (09/11/2026, após F-001).
**PR candidata:** #722 — `test(z80): audit BASE indirect accumulator transfers (F-002)`.
**HEAD de código validado:** `2eb200be183797f60721ce7db7327ec4469a4e9e`.
**CI de código:** #6093 PASS no HEAD de código; documentação e CI final da branch seguem em atualização.
**Perfil:** Zilog Z80 NMOS delimitado para semântica de software; não generalizar a CMOS, R800 ou clones.

## 1. Escopo e documentação primária

- `0A`: `LD A,(BC)`, ZiLOG UM008011-0816 p. 88.
- `1A`: `LD A,(DE)`, p. 89.
- `02`: `LD (BC),A`, p. 91.
- `12`: `LD (DE),A`, p. 92.

As quatro formas executam 2 M-cycles e 7 T-states: M1 fetch de 4T e ciclo de dados de 3T. Não modificam F. O manual não especifica WZ/MEMPTR.

## 2. Evidência independente de WZ/MEMPTR

Para loads:

`WZ = (BC/DE + 1) mod 65536`.

Para stores:

`WZ = (A << 8) | ((BC/DE + 1) & 0xFF)`.

Confronto:
- [MAME — z80.lst](https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.lst): quatro handlers BASE e atualização de WZ/MEMPTR.
- [ares — instructions.cpp](https://github.com/ares-emulator/ares/blob/0b928930af4cc59ed99e942996f0d3c815379388/ares/component/processor/z80/instructions.cpp): `instructionLD_a_irr` e `instructionLD_irr_a`.
- [superzazu/z80](https://github.com/superzazu/z80/blob/master/z80.c): terceira implementação concordante para os quatro opcodes.

Essas fontes sustentam comportamento de compatibilidade NMOS; não são prova física nem especificação do manual para WZ.

## 3. Matriz semântica

Arquivo: `native/z80_cpu/tests/base_indirect_accumulator_f002.rs`.

- 4 opcodes × 5 endereços (`00FE`, `00FF`, `1234`, `FFFE`, `FFFF`) × 256 bytes = 5.120 vetores.
- Todos os 256 padrões de F por permutação bijetiva; F preservado.
- Q inicial não zero: Q final zerado e previous-Q preservado.
- Refresh inicia em `80`; resultado `81`, preservando R7.
- Loads validam WZ=address+1; stores validam high=A e low=(address+1)&FF.
- Wrap de WZ em FFFF→0000.
- PC incrementado uma vez; BC/DE preservados.
- Memória final, acumulador e ordem de barramento verificados.
- Comparação executor atômico × microsequenciador; oracle WZ independente.
- WAIT 1T no M1 e 2T no ciclo de dados; snapshot encode/decode + restore antes/depois de cada resposta.

## 4. Integração temporal SMS

Arquivo: `native/sms_core/tests/z80_f002_temporal.rs`.

As quatro formas são executadas no SMS temporal bus:
- WAIT 1T por request.
- Snapshot enquanto a continuação M1 está pendente.
- Restore e retomada; estado completo e trace comparados à execução contínua.
- Verificação de BusCompleted, WZ, F, Q/previous-Q, refresh/R7, PC e memória.

## 5. Gates e limites

| Suíte | Estado | Evidência |
|---|---|---|
| Format Z80/SMS/SG-1000 | PASS | CI #6093 |
| Clippy focado com `-D warnings` | PASS | CI #6093 |
| Testes Z80/SMS/SG-1000 | PASS | CI #6093 |
| SingleStepTests DD/FD já existente | PASS | CI #6093; regressão adjacente, não vetor F-002 |
| Agregador Rust quality/unit | PASS | CI #6093 |
| Regressão integral `cargo test --workspace --locked` | NOT RUN | Workflow seletivo por escopo |
| Validação física SMS | NOT RUN | Sem alegação de hardware |

A implementação runtime não foi alterada. O fechamento proposto é restrito ao perfil NMOS de software e efetivo somente após squash merge da PR #722. Sem alegação de compatibilidade universal CMOS/clones. E7 permanece IN_PROGRESS; E8/ZEXDOC/ZEXALL permanece BLOCKED.
