# Requisitos de validação do Scheduler e integração temporal

## 1. Regra de evidência

Todo resultado deve indicar SHA exato, workflow/run, ambiente, comando, status e limitações. Status válidos:
- `PASS`
- `FAIL`
- `BLOCKED`
- `NOT RUN`
- `SKIPPED`

`SKIPPED`, check ausente ou resultado de SHA anterior nunca é PASS.

## 2. Testes do temporal core

- razão positiva; denominador/numerador inválidos;
- fase inicial e residual nos limites;
- avanço zero, avanço monotônico e tentativa regressiva;
- chunk invariance em várias partições;
- comparação exata de `ClockEdgeTime`;
- overflow em produto/soma/conversão;
- fila ordenada por cada dimensão e sequência;
- eventos retroativos e regressão causal;
- allocator esgotado;
- clear/reset da fila.

## 3. Testes do machine scheduler

- IDs duplicados/desconhecidos;
- registro com fase inválida;
- avanço simultâneo de vários domínios em ordem canônica;
- dispatch e `MachineDispatch` completos;
- avanço idle não cruza evento;
- delta budget, evento preservado e retomada;
- preflight/atomicidade em overflow;
- snapshot/restore isolado e completo;
- codec: versões, CRC, truncamento, trailing bytes, tamanho e payload inválidos;
- replay diferencial com seed fixo e instâncias independentes.

## 4. Testes de integração por componente

Cada componente deve ter:
- teste de deadline/safe point;
- suspensão e retomada em cada tipo de fronteira relevante;
- transação pendente aplicada exatamente uma vez;
- interação simultânea com outro componente;
- comparação de avanço detalhado vs agregado;
- snapshot/restore no meio da operação;
- trace determinístico e primeira divergência;
- regressões funcionais de máquina.

## 5. Testes sistêmicos

- Z80: opcode fetch, refresh, memória, I/O, ciclos internos, prefixos, interrupções, HALT, WAIT e repetição de bloco.
- Bus: read/write, mapper, side effects, wait states, arbitragem e contenção.
- VDP: raster/fetch/pixel pipeline, VRAM/CRAM, IRQ, colisão/overflow, writes intralinha.
- PSG/FM: divisores, latches, escrita de registrador, timers, fase e saída digital.
- Máquina: input, frame boundaries, reset, replay, save/restore, SG-1000 e SMS sem regressões.

## 6. Gates do repositório

No SHA candidato:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Executar também workflows de arquitetura, headless/golden, Windows x64 e stress quando aplicáveis. Documentação-only não prova comportamento temporal e não autoriza TEMPORAL_PASS.

## 7. Estados de maturidade

- **IMPLEMENTED:** código existe.
- **QUICK_PASS:** testes focados passam.
- **SHORT_PASS:** campanha curta repetida e integrada passa.
- **LONG_PASS:** campanha longa/replay/stress passa.
- **INTEGRATION_PENDING:** falta integração de sistema.
- **TEMPORAL_PASS:** prova integrada de cronologia e causalidade do sistema completo.

TEMPORAL_PASS exige traces de CPU/bus/VDP/áudio conforme o sistema, comparação com oráculos independentes ou evidência física adequada, primeira divergência, determinismo, save/restore temporal e CI verde no mesmo SHA. Não pode ser inferido de unit tests do Scheduler.

## 8. Ledger

Registrar evidência em `docs/evidence/` com:
- componente e sistema/região/revisão;
- regra validada e fonte;
- SHA de código e SHA de teste;
- comando/workflow e resultado;
- seeds/configuração;
- trace/hash e localização do artefato;
- limitações e itens UNKNOWN.
