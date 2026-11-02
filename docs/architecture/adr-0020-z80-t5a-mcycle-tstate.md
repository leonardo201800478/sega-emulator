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
- [x] Testes de sequência, WAIT e snapshot para M1/M2/M3, incluindo rejeição de payload de escrita adulterado e round-trip do codec.
- [x] Teste de atomicidade da conclusão quando a alocação da próxima requisição falha.
- [x] `cargo fmt --all -- --check`, Clippy `-D warnings`, `cargo test --workspace --locked`, pytest, Ruff check e Ruff format no SHA `9a92386c1bb923d0e5bcf8486f7cb88eeec5a6cf` (CI #3524; quatro jobs PASS).
- [x] Windows x64 D3D12/video-runtime e WASAPI/XAudio2/audio-output tests; Scheduler deterministic stress (CI #3524).
- [ ] Aprovação técnica independente do Agente 2 para o HEAD final e integração por PR; não declarar `TEMPORAL_PASS`.
## Evidência de CI

- HEAD de código auditado: `9a92386c1bb923d0e5bcf8486f7cb88eeec5a6cf`.
- GitHub Actions: [CI #3524 / run `41532813264`](https://github.com/leonardo201800478/sega-emulator/actions/runs/41532813264) — quatro jobs concluídos com PASS: Rust quality/unit regression, Python lint/regression, Windows x64 D3D12 compile e Scheduler deterministic stress.
- Follow-up de regressão: `codec_rejects_pending_write_payload_mismatch_with_valid_checksum` altera o payload binário mantendo CRC32 válido e exige `InvalidState`, provando a rejeição semântica. O HEAD candidato que contém esse teste e a documentação reconciliada está aguardando CI; execução local: NOT RUN.
