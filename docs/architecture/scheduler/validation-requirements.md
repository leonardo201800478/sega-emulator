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

**Todos os componentes**, inclusive os atômicos, devem ter testes de semântica funcional, estado, efeitos colaterais, determinismo, snapshot/restore do estado ativo e regressões de máquina proporcionais ao seu contrato.

**Somente componentes em caminhos temporais de gameplay** precisam, quando aplicável, de:
- teste de deadline/safe point;
- suspensão e retomada nas fronteiras em que a interação realmente possa interromper a operação;
- transação pendente aplicada exatamente uma vez;
- interação simultânea com outro componente;
- comparação de avanço detalhado vs agregado até a próxima fronteira observável;
- snapshot/restore no meio da operação suspensível;
- trace determinístico e primeira divergência.

Não exigir esses testes temporais de todo chip/opcode quando a execução atômica satisfaz o comportamento observável.

## 5. Testes sistêmicos

- **Z80:** semântica de instruções/prefixos/flags/registradores sempre; timing detalhado de fetch, I/O, interrupções, HALT, WAIT e ciclos internos somente nas sequências de gameplay cuja interação dependa desses instantes.
- **Bus:** read/write, mapper e side effects sempre; wait states, arbitragem e contenção somente quando existirem no perfil real e afetarem o resultado.
- **VDP:** VRAM/CRAM, IRQ, status, colisão/overflow e saída funcional sempre; raster/fetch/pipeline e writes intralinha na precisão exigida por jogos que usem essas interações.
- **PSG/FM:** escrita de registrador e saída digital funcional sempre; temporização de divisor, latch, timer e fase quando afetar áudio/jogo observável.
- **Máquina:** input, frame boundaries, reset, replay, save/restore e regressões SG-1000/SMS; comparar cronologia detalhada/agregada somente nas fronteiras de gameplay relevantes.

Não criar uma campanha universal por pino, tick ou fase elétrica.

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
- **TEMPORAL_PASS:** prova integrada de cronologia e causalidade dos caminhos temporais relevantes da plataforma avaliada.

TEMPORAL_PASS, quando declarado para uma plataforma/caminho específico, exige traces e comparação de CPU/bus/VDP/áudio somente nos caminhos temporais relevantes ao gameplay, oráculos independentes ou evidência física adequada, primeira divergência, determinismo, save/restore dos estados temporais ativos e CI verde no mesmo SHA. Não exige cobertura elétrica de todos os pinos nem microsequenciamento de operações atômicas. Não pode ser inferido de unit tests do Scheduler.

## 8. Ledger

Registrar evidência em `docs/evidence/` com:
- componente e sistema/região/revisão;
- regra validada e fonte;
- SHA de código e SHA de teste;
- comando/workflow e resultado;
- seeds/configuração;
- trace/hash e localização do artefato;
- limitações e itens UNKNOWN.
