# Auditoria E7.7 — F-015: RLCA / RRCA / RLA / RRA

**Baseline main antes da PR:** `f726755c87fcfb61bdf9ac5e2af031afe7712e21`.
**Família:** F-015 — rotações do acumulador.
**Estado nesta branch:** IMPLEMENTED_PENDING_VALIDATION.
**Escopo de perfil:** Zilog Z80 NMOS; sem alegação de medição física ou compatibilidade universal.

## 1. Escopo

- `07` RLCA — rotate A left circular.
- `0F` RRCA — rotate A right circular.
- `17` RLA — rotate A left through carry.
- `1F` RRA — rotate A right through carry.

Instruções BASE de um byte, sem operandos de memória e sem M-cycles de dados. Cada instrução é um M1 de 4 T-states; o WAIT é associado ao próprio M1.

## 2. Referência documental — ZiLOG UM008011-0816

- RLCA: p. 205.
- RLA: p. 207.
- RRCA: p. 209.
- RRA: p. 211.
- Cada operação: 1 M-cycle / 4 T-states.
- S, Z e P/V preservadas; H e N resetadas; C recebe o bit deslocado. RLA/RRA usam o carry antigo como entrada; RLCA/RRCA fazem rotação circular.

O manual não documenta os bits X/Y. Não atribuir X/Y ao UM0080.

## 3. Comparação independente

### MAME

Fonte `src/devices/cpu/z80/z80.cpp`, commit `d763e6614a357fc529a6711d9f0c95fc9ecb051c`:
- `rlca`, `rrca`, `rla`, `rra` atualizam X/Y a partir do novo A.
- H e N são zerados; S/Z/PV são preservados; C recebe o bit deslocado.
- `QT` usa a representação interna própria do MAME.

### ares

Fonte `ares/component/processor/z80/instructions.cpp`, commit `19d051721bb14c46be92a713216421447dec91b7`:
- As quatro instruções atualizam X/Y a partir do novo A.
- H e N são zerados; C recebe o bit deslocado.
- Ares representa Q como indicador booleano de atualização de flags.

**Cuidado com Q:** o runtime deste projeto representa `q` como o último byte F escrito e `q_previous` como o byte F anterior. Portanto, para estas instruções, o resultado esperado do projeto é `q == F_novo` e `q_previous == Q_anterior`. Não comparar diretamente o byte Q do projeto com o booleano Q do ares nem com o `QT` do MAME.

X/Y têm concordância comparativa MAME/ares, mas não evidência física independente. F-023 continua responsável pela lacuna transversal de evidência independente X/Y/WZ.

Links:
- ZiLOG: https://www.zilog.com/docs/z80/um0080.pdf
- MAME: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.cpp
- ares: https://github.com/ares-emulator/ares/blob/master/ares/component/processor/z80/instructions.cpp

## 4. Auditoria do runtime existente

- `native/z80_alu/src/lib.rs`: `rotate_accumulator` já implementa as quatro fórmulas, preserva S/Z/PV, substitui X/Y pelo resultado e calcula C.
- `native/z80_cpu/src/cpu.rs`: executor atômico chama `rotate_accumulator`, grava A e `set_flags`; o tempo nominal é 4T.
- `native/z80_cpu/src/sequencer.rs`: microsequenciador executa a operação no término do M1 e chama `set_flags`; o estado Q é o F gravado.
- Cobertura anterior: somente dois vetores unitários em `z80_alu` e cobertura temporal dirigida para os quatro opcodes. Não havia matriz semântica exaustiva de A×F nem diferencial amplo de Q/previous-Q, WZ, refresh e WAIT.
- Nenhuma divergência de runtime identificada nesta auditoria; nenhuma alteração de runtime proposta.

## 5. Novas evidências nesta branch

### 5.1 Matriz semântica e temporal Z80

Arquivo: `native/z80_cpu/tests/base_accumulator_rotates_f015.rs`.

- 262.144 vetores: 4 opcodes × 256 valores de A × 256 padrões de F.
- Oracle explícito: A rotacionado, carry de entrada/saída, S/Z/PV preservadas, H/N resetadas e X/Y do resultado conforme perfil comparativo NMOS.
- Verifica PC, WZ preservado, F, Q=F novo, previous-Q, R/R7 e estado completo.
- Executor atômico: 4T; microsequenciador: 4T + WAIT de 1T.
- Um único OpcodeFetch M1; nenhuma transação de dados.
- Snapshot encode/decode + restore com WAIT de 2T, comparado à execução contínua para 1.024 casos (4 opcodes × 256 valores de A).
- Quatro casos adicionais verificam PC FFFF→0000.

### 5.2 Integração SMS / MachineScheduler

Arquivo: `native/sms_core/tests/z80_f015_temporal.rs`.

- 4 opcodes × 8 valores-limite de A × 6 padrões de F = 192 casos.
- WAIT de 1T no M1; 5T totais e 15 master ticks.
- Snapshot enquanto o M1 está pendente, restore e retomada.
- Compara estado serializado, A/F, WZ, Q/previous-Q, refresh, relógio e trace com execução contínua.
- Confirma ausência de transações de barramento de dados.

## 6. Critérios de aceite e estado

- Rustfmt: pendente no HEAD final da PR.
- Clippy com `-D warnings`: pendente.
- Testes Z80/SMS/SG-1000: pendentes.
- Documentation patch hygiene: pendente.
- Regressão integral do workspace: classificar conforme política de impacto; NOT RUN não é PASS.
- Nenhuma alteração de runtime autorizada sem falha reproduzível e divergência comprovada contra o oracle.
- F-015 permanece `IMPLEMENTED_PENDING_VALIDATION` até gates aplicáveis verdes e squash merge.
- Sem validação física; X/Y permanece comportamento de compatibilidade comparativo. E7 IN_PROGRESS; E8/ZEXDOC/ZEXALL BLOCKED.
