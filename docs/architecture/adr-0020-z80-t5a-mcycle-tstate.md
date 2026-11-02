# T5a — Índices M-cycle/T-state do microsequenciador Z80

- Status: implementação em revisão; não equivale a cobertura elétrica intra-T-state.
- Base: T4 integrado em main (`62ab404e`).
- Escopo: indexar, de forma determinística e serializável, os M-cycles expostos pelo microsequenciador atual.

## Ownership e mapeamento

- `Z80MicroSequencer` continua sendo o único proprietário da continuação de instrução.
- `mcycle_index` é zero-based dentro da instrução: opcode fetch é M1 (índice 0); cada leitura/escrita de memória e cada intervalo interno explicitamente exposto avança para o próximo M-cycle.
- `tstate_index` é zero-based no início do M-cycle. O sequenciador atual expõe limites de M-cycle, não bordas individuais dentro do ciclo; por isso o valor permanece zero. Não representa amostragem elétrica, WAIT por T-state nem subfase de clock.
- `Z80BusRequest` continua sendo a identidade operacional da transação (request ID, operação, endereço, duração nominal). O contrato genérico `BusTransaction` descreve metadados e índices; não cria uma segunda continuação nem substitui o handshake.
- WAIT permanece no handshake e no Scheduler; a posição absoluta e a conversão para ticks continuam sob o Scheduler/perfil da máquina.
- Snapshot do sequenciador inclui os dois índices e incrementa o formato para versão 2.

## Cobertura temporal da fatia

A implementação cobre somente as fases que o sequenciador já expõe: M1 opcode fetch, memória read/write e um M-cycle interno de 1 T-state para INC/DEC (HL). Prefixos, I/O, stack, interrupções, HALT, refresh elétrico e bordas intra-T-state continuam fora do escopo.

## Critérios de aceitação

- [x] Contratos genéricos adicionados ao temporal-core sem duplicar estado de CPU.
- [x] Índices M/T pertencem ao estado retomável do sequenciador.
- [x] Snapshot binário versionado carrega os índices.
- [ ] Testes de sequência e snapshot no CI.
- [ ] fmt, clippy com `-D warnings`, testes do workspace e regressões Python no SHA candidato.
- [ ] Revisão arquitetural e integração por PR; não declarar `TEMPORAL_PASS`.