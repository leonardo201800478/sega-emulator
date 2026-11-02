# SMS T4 — evidência de integração Scheduler-driven

**Data:** 2026-11-02  
**PR:** [#416](https://github.com/leonardo201800478/sega-emulator/pull/416)  
**Head validado:** `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`  
**Merge commit / main:** `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`  
**CI candidato:** [#41499228458](https://github.com/leonardo201800478/sega-emulator/actions/runs/41499228458)  
**Merge ref testado:** `cea276159efd382e8289085be4b5f67dccecc753` (parents: base `bf09712659a102d6e4c02f2ff5d97a99487ac166`, head `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`).

## Resultado por gate no SHA candidato

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `python -m pytest` | PASS |
| `python -m ruff check .` | PASS |
| `python -m ruff format --check .` | PASS |
| Windows x64 D3D12/video compile + unit tests | PASS |
| Windows x64 WASAPI/XAudio2 compile + unit tests | PASS |
| Scheduler deterministic stress — short | PASS |

O workflow pós-merge [#41499594576](https://github.com/leonardo201800478/sega-emulator/actions/runs/41499594576) é distinto: Rust e Python PASS; Windows x64 vídeo/áudio deve ser lido por job no resultado final; Scheduler stress foi SKIPPED no evento push em `main`. O stress short já passou no CI candidato acima. Não converter SKIPPED em PASS.

## Regressões e testes direcionados

- `sega-sg1000-core`: 42 passed, 0 failed.
- `sega-sms-core`: 157 passed, 0 failed.
- `sega-z80-cpu`: 83 passed, 0 failed.
- Testes de diferencial contra executor atômico para NOP, LD imediato, LD (HL),n, LD A,(HL), LD (HL),A, LD A,B, INC B e INC/DEC (HL).
- INC/DEC (HL): fetch 4 + memory read 3 + internal event 1 + memory write 3 = 11 T-states; agrupamento nominal (4,4,3). O ciclo interno permanece evento separado no trace.
- Snapshot/restore de transação pendente e completion efetivada ainda sem acknowledgement.
- Overflow entre conclusão de bus e CPU: completion permanece latched; restore/retry consome a mesma resposta sem repetir o efeito de memória.
- Duração de ciclo interno divergente do 1 T-state exposto é rejeitada sem mutação.
- Trace de runtime em fronteiras reais de dispatch e replay determinístico com estados iniciais equivalentes.

## Auditorias independentes sequenciais

O runtime não disponibilizou ferramenta de criação de agentes paralelos; as três frentes foram executadas sequencialmente, sem simular agentes.

### CPU Z80
Microsequenciador retomável restrito ao subconjunto coberto. Estado versionado, handshake, WAIT checked, contador de T-states checked e codec binário com CRC32. Executor atômico preservado como referência. Não cobre ainda o estado arquitetural completo, prefixos, HALT, INT/NMI ou todas as famílias/flags.

### Scheduler, bus e SMS
Um `MachineScheduler` e uma timeline `MachineTime` com escala master-ticks/T-state explícita por parâmetro. O bus agenda deadlines e só aplica leitura/escrita no dispatch. WAIT é exclusivamente fixture. Ciclo interno usa evento próprio. VDP scanline legado não participa. Os contratos genéricos da PR #396 não são consumidos porque índices M-cycle/T-state estáveis ainda não são expostos.

### QA e CI
Rust, Python, Windows vídeo/áudio e Scheduler stress short PASS no CI candidato. A suíte do workspace cobre SG-1000 unitário. Não há nesta execução golden master de gameplay, ROM homebrew smoke, teste físico de WAIT, nem suítes externas completas Z80. Os workflows `sms-z80-platform-validation.yml`, `qt-rust-z80-core.yml` e `qt-frontend-validation.yml` são manuais e ficaram NOT RUN. Testes locais também NOT RUN.

## Histórico de PRs e ancestralidade

- #411: integrada antes, merge commit `bf09712659a102d6e4c02f2ff5d97a99487ac166`.
- #412: head `df8d571471d1b42985f5cbc9491e94033edd9a4e) é ancestral do merge T4; GitHub reconheceu merged.
- #415: head `b7b99675b69e9ed1352b5dcf8d4c98b7f3c7e2c1) é ancestral do merge T4, embora a PR tenha sido fechada como supersedida.
- #413 (head `887aa744507c49d26062c04a2fcd5ca7c14fc74b`) e #414 (head `783882e12219d1737c546c05ca8c7f58e781984b`) divergiam da branch consolidada. Foram fechadas como supersedidas; seus entregáveis funcionais de snapshot/codec e coordinator estão no diff final de #416, mas os hashes específicos de seus commits divergentes não são ancestrais de `main`. Esta é uma diferença entre preservar conteúdo funcional e preservar a linhagem exata dos commits.
- #396 permanece OPEN / Draft; T0 revisado, integração dos contratos pendente.

## Estado formal

| Componente | Estado |
|---|---|
| Estado retomável Z80 do subconjunto | IMPLEMENTED / QUICK_PASS |
| SMS temporal coordinator | IMPLEMENTED / QUICK_PASS |
| SMS temporal memory bus | IMPLEMENTED / QUICK_PASS |
| Fatia vertical SMS T4 | IMPLEMENTED / SHORT_PASS |
| Integração completa CPU/VDP/PSG/I/O/IRQ | INTEGRATION_PENDING |
| TEMPORAL_PASS | NOT DECLARED |

## Próxima tarefa

**T5a:** emitir índices M-cycle/T-state estáveis para o subconjunto atual e mapear explicitamente `Z80BusRequest`/`Z80BusOperation` para `BusTransaction`/`BusOperationResult` da PR #396. Revisar ownership de continuação, timestamp e fase sem duplicar handshake. Depois, expandir uma família de instruções por PR com diferencial e oráculo por escopo.

T4 não demonstra precisão sistêmica SMS, clock/fase regional, WAIT físico, arbitragem ou equivalência com hardware real.
