# T5a — Índices M-cycle/T-state do microsequenciador Z80

- Status: implementação em revisão; não equivale a cobertura elétrica intra-T-state.
- Base: T4 integrado em main (`62ab404e`).
- Escopo: indexar, de forma determinística e serializável, os M-cycles expostos pelo microsequenciador atual.

## Ownership e mapeamento

- `Z80MicroSequencer` continua sendo o único proprietário da continuação de instrução.
- A conclusão calcula a transição em uma cópia. Erros de protocolo/overflow preservam handshake, fase e registradores originais; `UnsupportedOpcode` é uma rejeição terminal explícita e retorna a CPU a `Ready`, preservando o contrato legado de não deixar um fetch fantasma.
- `mcycle_index` é zero-based dentro da instrução: opcode fetch é M1 (índice 0), a leitura de INC/DEC (HL) é M2 (índice 1) e a escrita é M3 (índice 2).
- A fase `MemoryWrite` preserva endereço e payload capturados; `restore_state()` exige correspondência exata com a requisição pendente para `LD (HL),A`, `LD (HL),n` e `INC/DEC (HL)`.
- O intervalo interno de 1 T-state de INC/DEC (HL) pertence ao próprio M2: sem WAIT, começa em `(mcycle_index, tstate_index) = (1, 3)`. WAITs inseridos na leitura deslocam esse offset dentro do mesmo M2; não criam outro M-cycle.
- `tstate_index` é zero-based dentro do M-cycle. O sequenciador expõe apenas o offset necessário ao intervalo interno atual, não bordas elétricas, amostragem de pinos ou subfases de clock.
- `Z80BusRequest` continua sendo a identidade operacional da transação (request ID, operação, endereço, duração nominal). O contrato genérico `BusTransaction` descreve metadados e índices; não cria uma segunda continuação nem substitui o handshake.
- WAIT permanece no handshake e no Scheduler; a posição absoluta e a conversão para ticks continuam sob o Scheduler/perfil da máquina.
- Snapshot do sequenciador inclui índices M/T e a continuação de escrita (endereço e payload esperados); o formato é versão 3.

## Cobertura temporal da fatia

A implementação cobre somente as fases que o sequenciador já expõe: M1 opcode fetch, memória read/write e um intervalo interno de 1 T-state dentro de M2 para INC/DEC (HL). Prefixos, I/O, stack, interrupções, HALT, refresh elétrico e bordas intra-T-state continuam fora do escopo.

## Critérios de aceitação

- [x] Contratos genéricos adicionados ao temporal-core sem duplicar estado de CPU.
- [x] Índices M/T pertencem ao estado retomável do sequenciador.
- [x] Snapshot binário versionado carrega índices e valida endereço/payload esperados da escrita pendente.
- [x] `SmsTemporalMachineState` versão 6 preserva `instruction_started_at`; restore reconcilia `instruction_tstates` com o tempo decorrido no Scheduler para request pendente, completion latched e ciclo interno.
- [x] Testes de sequência, WAIT e snapshot para M1/M2/M3, incluindo rejeição de payload/endereço `(HL)` adulterados com CRC válido, round-trip do codec e rejeição de progresso CPU/Scheduler inconsistente.
- [x] Teste de atomicidade da conclusão quando a alocação da próxima requisição falha.
- [x] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` no código SHA `0c178ed6c8da6aeddf8e25682a2ebc620eec786f` (run `41536695747`; Rust quality/unit regression PASS).
- [x] `python -m pytest`, `python -m ruff check .` e `python -m ruff format --check .` no mesmo SHA/run (Python lint/regression PASS).
- [x] Windows x64 D3D12/video-runtime e WASAPI/XAudio2/audio-output tests (Windows x64 D3D12 compile PASS).
- [x] Scheduler deterministic stress (PASS).
- [ ] Aprovação técnica independente do Agente 2 para o HEAD final e integração por PR; não declarar `TEMPORAL_PASS`.
## Evidência de CI

- HEAD de código auditado: `0c178ed6c8da6aeddf8e25682a2ebc620eec786f`; run `41536695747` — quatro jobs PASS, incluindo os novos testes de endereço `(HL)` e reconciliação de T-states CPU/Scheduler.
- HEAD documental predecessor: `646bd556572a1f077dc3c003407c6e64439f66f3`; run `41534269758` — quatro jobs PASS. A atualização documental corrente é validada pelo CI do SHA final, conforme body da PR.
- Execução local: NOT RUN.
