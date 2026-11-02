# T5a — Índices M-cycle/T-state do microsequenciador Z80

- Status: implementação em revisão; não equivale a cobertura elétrica intra-T-state.
- Base: T4 integrado em main (`62ab404e`).
- Escopo: indexar, de forma determinística e serializável, os M-cycles expostos pelo microsequenciador atual.

## Ownership e mapeamento

- `Z80MicroSequencer` continua sendo o único proprietário da continuação de instrução.
- A conclusão calcula a transição em uma cópia. Erros de protocolo/overflow preservam handshake, fase e registradores originais; `UnsupportedOpcode` é uma rejeição terminal explícita e retorna a CPU a `Ready`, preservando o contrato legado de não deixar um fetch fantasma.
- `mcycle_index` é zero-based dentro da instrução: opcode fetch é M1 (índice 0), a leitura de INC/DEC (HL) é M2 (índice 1) e a escrita é M3 (índice 2).
- O intervalo interno de 1 T-state de INC/DEC (HL) pertence ao próprio M2: sem WAIT, começa em `(mcycle_index, tstate_index) = (1, 3)`. WAITs inseridos na leitura deslocam esse offset dentro do mesmo M2; não criam outro M-cycle.
- `tstate_index` é zero-based dentro do M-cycle. O sequenciador expõe apenas o offset necessário ao intervalo interno atual, não bordas elétricas, amostragem de pinos ou subfases de clock.
- `Z80BusRequest` continua sendo a identidade operacional da transação (request ID, operação, endereço, duração nominal). O contrato genérico `BusTransaction` descreve metadados e índices; não cria uma segunda continuação nem substitui o handshake.
- WAIT permanece no handshake e no Scheduler; a posição absoluta e a conversão para ticks continuam sob o Scheduler/perfil da máquina.
- Snapshot do sequenciador inclui os dois índices e incrementa o formato para versão 2.

## Cobertura temporal da fatia

A implementação cobre somente as fases que o sequenciador já expõe: M1 opcode fetch, memória read/write e um intervalo interno de 1 T-state dentro de M2 para INC/DEC (HL). Prefixos, I/O, stack, interrupções, HALT, refresh elétrico e bordas intra-T-state continuam fora do escopo.

## Critérios de aceitação

- [x] Contratos genéricos adicionados ao temporal-core sem duplicar estado de CPU.
- [x] Índices M/T pertencem ao estado retomável do sequenciador.
- [x] Snapshot binário versionado carrega os índices.
- [x] Testes de sequência, WAIT e snapshot para M1/M2/M3 no CI candidato.
- [x] Teste de atomicidade da conclusão quando a alocação da próxima requisição falha.
- [x] `cargo fmt --all -- --check`, Clippy `-D warnings`, `cargo test --workspace --locked`, pytest, Ruff check e Ruff format no SHA `e782614b50db857c7333d82982de3f3a43bfdf3d` (CI #3517).
- [x] Windows x64 D3D12/video-runtime e WASAPI/XAudio2/audio-output tests; Scheduler deterministic stress short (CI #3517).
- [ ] Aprovação técnica independente do Agente 2 para o HEAD final e integração por PR; não declarar `TEMPORAL_PASS`.
## Evidência de CI

- Código validado: SHA `e782614b50db857c7333d82982de3f3a43bfdf3d`.
- GitHub Actions: [run #3516](https://github.com/leonardo201800478/sega-emulator/actions/runs/41530210481) — quatro jobs concluídos com PASS: Rust quality/unit regression, Python lint/regression, Windows x64 D3D12 + WASAPI/XAudio2 e Scheduler deterministic stress short.
- Execução local: NOT RUN. O HEAD de código e os quatro jobs do CI #3517 correspondem ao SHA acima. A atualização documental subsequente exige CI próprio; execução local: NOT RUN.
