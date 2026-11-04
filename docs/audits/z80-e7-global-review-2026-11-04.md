# Auditoria global da etapa E7 do Z80 — revisão de 2026-11-04

**Repositório:** `leonardo201800478/sega-emulator`  
**Baseline de código da main:** `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`  
**HEAD documental da main após PR #501:** `a324361574a45a80f8ae3aa70868630250e55d30`  
**Branch desta reconciliação:** `docs/z80-e7-audit-e8-handoff-20261104`  
**PR documental:** [#503](https://github.com/leonardo201800478/sega-emulator/pull/503) — CI pendente no HEAD final.  
**PR E7.2E em análise:** [#502](https://github.com/leonardo201800478/sega-emulator/pull/502), HEAD `69ea4299b3858787f0f91682f3ea823371cde3b0`  
**Resultado global:** `INTEGRATION_PENDING` — E7 não está concluída.

## 1. Resumo executivo

As fatias E7.1 e E7.2A–E7.2D estão integradas na main. A PR #501, que reconcilia a evidência de E7.2D, foi integrada neste ciclo no merge SHA `a324361574a45a80f8ae3aa70868630250e55d30`. Seu CI #41817015813 passou nos jobs Rust, Python e Windows x64; Scheduler deterministic stress foi `SKIPPED`.

A PR #502 é uma candidata inicial de E7.2E, não uma validação concluída. O HEAD atual consultado não possui workflow associado. A branch é baseada no SHA de código anterior à reconciliação documental #501; a PR informa `mergeable: false`. Não houve execução local porque o ambiente desta sessão não conseguiu resolver `github.com` via DNS.

**Decisão:** não integrar #502, não declarar E7 concluída e não iniciar E8 como etapa de execução até fechar as dependências E7. A documentação pode preparar a entrada E8, mas não transformar E7 incompleta em gate aprovado.

## 2. Estado por subfase

| Subfase | Estado | Evidência / lacuna |
|---|---|---|
| E7.1 — HALT/M1 | Integrada | PR #490; HALT hold M1, PC estável durante retenção, R incrementado, WAIT e snapshot do M1 pendente. A amostragem de INT/NMI no T4 de HALT continua pendente. |
| E7.2A — NMI retomável | Integrada | PR #495; serviço NMI e fases retomáveis/serializáveis cobertas nos cenários registrados. A validação externa e temporal independente não está concluída. |
| E7.2B — INT acknowledge | Integrada | PR #496; acknowledge retomável e resposta do barramento. Não equivale ao fechamento de arbitragem de todos os cenários nem a BUSREQ. |
| E7.2C — IM0/IM1/IM2 | Integrada | PR #498; IM0 reutiliza decoder, IM1 vetor 0038h, IM2 lê vetor low/high; fases e WAIT/snapshots cobertos no escopo declarado. |
| E7.2D — EI/DI | Integrada | PR #500; EI delay, DI cancellation, repeated EI e NMI durante atraso; CI PR #41794174397 e pós-merge #41794506556. Scheduler stress SKIPPED. `EI;HALT;INT` não está aprovado. |
| E7.2E — snapshots/restore | Candidata, não integrada | PR #502; CI ausente no HEAD atual, cobertura parcial e limitações descritas na seção 3. |
| E7.3 — amostragem HALT T4 | Pendente | Amostragem real de NMI/INT no T4 e saída de HALT não demonstradas. |
| E7.4 — BUSREQ/arbitragem | Pendente | Não há evidência suficiente de BUSREQ e arbitragem com suspensão/retomada e causalidade do Scheduler. |
| E7.5 — RETI/daisy-chain | Pendente | RETI ainda precisa emitir o evento/sinal específico de retorno de serviço; não confundir com RETN. |
| E7.6 — IRQ real VDP SMS | Pendente | A linha INT precisa ser ligada à fonte real do VDP, com status/acknowledge do dispositivo. |
| E7.7 — auditoria semântica integral | Pendente | Matriz completa de famílias, flags, estado, instruções de bloco, I/O, aliases e comportamentos não documentados precisa ser fechada. |

## 3. Revisão técnica da PR #502 — E7.2E

### 3.1 Conteúdo observado

HEAD consultado: `69ea4299b3858787f0f91682f3ea823371cde3b0`, 11 commits à frente da base original `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`, 0 atrás dessa base original no momento da comparação. Após o merge documental #501, a comparação remota registra a branch #502 divergente, 11 commits à frente e 4 atrás da main atual.

Arquivos alterados:
- `docs/audits/z80-e7-2e-snapshot-restore-2026-11-04.md`
- `native/sms_core/tests/snapshot_restore_e7_2e.rs`
- `native/z80_cpu/src/snapshot.rs`
- `native/z80_cpu/src/snapshot/e7_2e_phase_codec_matrix_tests.rs`
- `native/z80_cpu/tests/snapshot_codec_invalid_phase_e7_2e.rs`
- `native/z80_cpu/tests/snapshot_codec_versioning_e7_2e.rs`

O candidato adiciona:
- Comparação entre execução contínua e execução com restore em NMI, IM0, IM1, IM2, EI delay e HALT/M1.
- WAIT de fixture nos cenários de NMI/IM e HALT; EI é exercitado sem WAIT.
- Round-trip direto de tag/payload para 70 variantes de `Z80MicroSequencerPhase`.
- Rejeição de tag desconhecida com checksum recalculado.
- Rejeição de versão de schema antiga no decoder e de DTO com versão antiga no encoder.

### 3.2 Revisão estrutural

Na baseline examinada, o enum tem 70 variantes. A inspeção estática encontrou:
- 70 variantes no `write_phase()`;
- 70 variantes no `read_phase()`;
- 70 variantes na validação de `restore_state()`;
- schema `Z80_MICRO_SEQUENCER_STATE_VERSION = 31`.

Isso é cobertura estrutural dos match arms. Não prova que todas as combinações de payload, request, M-cycle/T-state e estado arquitetural sejam válidas ou inválidas corretamente.

A matriz nova de 70 fases round-tripa valores representativos de cada variante. Ela não restaura e continua uma máquina válida em cada uma das 70 fases. A afirmação “70 fases cobertas” deve sempre ser qualificada como cobertura de codec tag/payload isolado.

### 3.3 Lacunas e riscos identificados

1. **Restore completo por fase:** ainda falta matriz de estados completos válidos por fase, com continuação até o boundary e comparação de estado final.
2. **Estados inválidos:** a PR testa tag desconhecida e versão antiga, mas não demonstra rejeição sistemática de payloads inconsistentes por fase, request ID incorreto, operação/endereço divergentes, M-cycle/T-state inválidos ou WAIT sem request.
3. **WAIT:** os cenários novos usam WAIT=1 em interrupções/HALT e WAIT=0 no EI. Falta matriz WAIT=0, WAIT=1 e WAIT múltiplo por classes de transação, incluindo fronteiras de M1, memória, I/O e ciclos internos.
4. **Trace:** `compare_runs()` compara o `Z80MicroStep` terminal, `export_state()` e `now()`. O teste não habilita/coleta `SmsTemporalTraceEvent` para comparar a sequência completa de eventos. Igualdade de estado e tempo final não substitui comparação de trace.
5. **IM0:** o teste confirma opcode aposentado e estado de PC/SP/stack, mas não fixa explicitamente o T-state terminal esperado para esse caso.
6. **Offset binário fixo:** `snapshot_codec_invalid_phase_e7_2e.rs` usa `PHASE_TAG_OFFSET_IN_BODY = 27`. Uma mudança legítima no layout pode quebrar o teste por deslocamento, não por regressão semântica. Preferir localizar o campo por encoder de teste/fixture nomeada ou encapsular a codificação.
7. **Isolamento da versão:** o teste de schema altera os bytes da versão sem recalcular CRC. Se a intenção é testar exclusivamente a precedência da validação de versão, documentar esse contrato; caso contrário, recalcular CRC para isolar a causa da rejeição.
8. **Save state SMS completo:** `SmsTemporalMachineState` é DTO em memória com `export_state()/restore_state()`. O candidato round-tripa o codec binário CPU e restaura o DTO SMS. Não existe evidência nesta PR de codec binário persistente versionado para a máquina SMS completa.
9. **Cobertura de fases de interrupção:** o teste percorre caminhos selecionados de NMI/IM0/IM1/IM2; não cobre todas as combinações de fase, vetor, WAIT, linha INT/NMI e estados de máquina.
10. **CI:** `fetch_commit_workflow_runs` não retornou workflows para o HEAD `69ea4299b3858787f0f91682f3ea823371cde3b0`. Workflows de candidatos anteriores, inclusive os que falharam em rustfmt, não validam esse HEAD.

### 3.4 Gate de aceitação da #502

Antes de aprovar:
- Rebase/atualização sobre a main atual sem descartar commits.
- CI no HEAD final exato:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `python -m pytest`
  - `python -m ruff check .`
  - `python -m ruff format --check .`
  - Windows x64 D3D12/vídeo e WASAPI/XAudio2/áudio.
  - Scheduler deterministic stress: executar quando a alteração afetar Scheduler; caso contrário, registrar SKIPPED, nunca PASS.
- Completar os casos de restore/continuação e estados inválidos listados acima.
- Documentar números absolutos, SHA, workflow IDs e limites.
- Só então classificar a subfase de acordo com a evidência. Não conceder LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS por CI convencional.

**Estado da PR #502 nesta revisão:** IN_PROGRESS / INTEGRATION_PENDING; sem aprovação de merge.

## 4. Revisão das PRs abertas

Consulta remota de 2026-11-04. O estado remoto prevalece; os PRs não relacionados a E7 não devem ser misturados à sua conclusão.

| PR | Estado remoto | Avaliação |
|---|---|---|
| #502 — E7.2E snapshot/restore | OPEN, ready for review, mergeable=false | Candidata E7.2E. Sem CI no HEAD atual. Manter aberta e corrigir/validar. |
| #501 — reconciliação E7.2D | MERGED | Integrada neste ciclo; merge SHA `a324361574a45a80f8ae3aa70868630250e55d30`. CI #41817015813: Rust/Python/Windows PASS; Scheduler SKIPPED. |
| #487 — reconciliação de gaps Z80 | OPEN, mergeable=false | Branch baseada em baseline anterior a E6. O corpo ainda descreve #486 como aberta e contém referências históricas; não integrar sem reconciliação integral com main atual. |
| #447 — antigo sequenciador DD/FD | OPEN, mergeable=false | Branch experimental antiga, 68 commits e base histórica. E2–E6 já foram implementadas por PRs sucessoras. Não integrar nem usar como base; manter apenas como referência histórica e avaliar fechamento separado. |
| #424 — SRAM SMS | OPEN, mergeable=false | Feature independente de E7, branch antiga. A própria descrição registra ausência de teste in-game e integração automática por DAT; precisa de rebase/auditoria própria antes de qualquer decisão. |
| #338 — auditoria VDP/backends gráficos | OPEN, DRAFT, mergeable=false | Trabalho separado de VDP/renderização, com preferência GUI ainda pendente e validação GPU/visual não executada. Não é dependência direta para concluir E7; a IRQ real do VDP, porém, é dependência de E7.6. |

## 5. Estado dos gates e evidências

### PR #501 — documentação E7.2D, integrada

CI #41817015813 no HEAD `9645071babe1660ee7480d633a6d226509b0679c`:
- Rust quality/unit regression: PASS (fmt, Clippy, SMS/SG-1000 e workspace).
- Python lint/regression: PASS (Ruff format, Ruff lint, pytest).
- Windows x64: PASS (D3D12/vídeo, WASAPI/XAudio2/áudio).
- Scheduler deterministic stress: SKIPPED.

Merge remoto confirmado: `a324361574a45a80f8ae3aa70868630250e55d30`.

### PR #502 — E7.2E

- Rust fmt: NOT RUN no HEAD atual.
- Rust Clippy: NOT RUN no HEAD atual.
- Rust workspace tests: NOT RUN no HEAD atual.
- Python pytest: NOT RUN no HEAD atual.
- Ruff check: NOT RUN no HEAD atual.
- Ruff format: NOT RUN no HEAD atual.
- Windows x64 D3D12/vídeo: NOT RUN no HEAD atual.
- Windows WASAPI/XAudio2/áudio: NOT RUN no HEAD atual.
- Scheduler stress: NOT RUN no HEAD atual.
- Execução local nesta sessão: BLOCKED (DNS não resolveu `github.com` para clone Git).

Nenhum status de workflow anterior foi reaproveitado como evidência do HEAD atual.

## 6. Sequência recomendada para fechar E7

1. Atualizar a PR #502 sobre a main pós-#501, preservando os commits e revendo conflitos.
2. Completar E7.2E conforme os critérios da seção 3.4.
3. Implementar e validar E7.3 (amostragem HALT T4), incluindo `EI;HALT;INT`, NMI, IFF e PC/R.
4. Implementar E7.4 (BUSREQ e arbitragem real de barramento).
5. Implementar E7.5 (RETI/daisy-chain, distinto de RETN).
6. Integrar E7.6 com a fonte IRQ real do VDP SMS, sem vetor artificial como substituto.
7. Fechar E7.7 com auditoria semântica por opcode/família, sem lacunas silenciosas em BASE/CB/ED/DD/FD/DDCB/FDCB.
8. Executar gates e regressões de cada fatia; atualizar scorecard, matriz, auditorias e evidências.
9. Somente após E7 aprovado, iniciar E8 formal/diferencial com oráculos fixados.

## 7. Decisão final da auditoria

- E7.1–E7.2D: integradas e com evidências limitadas aos cenários documentados.
- E7.2E: candidata incompleta; sem CI no HEAD atual.
- E7.3–E7.7: pendentes.
- E7 global: `INTEGRATION_PENDING`.
- E8: planejada, não liberada como próxima implementação enquanto os gates E7 estiverem pendentes.
- Não declarar o Z80 completo, LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS.

Esta auditoria é revisão documental/estática e de evidências remotas. Não substitui execução dos testes no HEAD candidato, oráculo externo ou trace temporal independente.

---

## Aditivo — continuidade E7.2E após publicação da auditoria

- A PR #503 foi integrada na main pelo merge SHA `9b606a6c785b9e7bb1999534e7e2292735254134`.
- O workflow pós-merge #41824734622 passou em Rust fmt, Clippy, testes SMS/SG-1000/workspace, Python pytest/Ruff e Windows x64 D3D12/WASAPI/XAudio2. Scheduler deterministic stress ficou SKIPPED.
- A PR #502 diverge da main em 11 commits à frente e 11 atrás. Seus commits não foram descartados.
- A continuação incremental transporta os arquivos da implementação candidata para uma branch baseada na main atual e adiciona comparação integral de `SmsTemporalTraceEvent`, cenários NMI com WAIT=0/1/3 e correções na auditoria do formato.
- A continuação será submetida em PR própria (#504). E7.2E permanece IN_PROGRESS até CI no SHA final e fechamento das lacunas de restore completo/estados inválidos. E7 global continua INTEGRATION_PENDING.

---

## Atualização operacional — PR #504 integrada e PR #505 em validação

- PR #504 integrada na main pelo merge SHA d64d86b5bb16305d6265a1522d095b68b8f4954a; CI pós-merge #41826301233 passou em Rust, Python e Windows x64. Scheduler stress SKIPPED.
- PR #502 fechada como supersedida; seus commits originais permanecem preservados no histórico Git.
- PR #505 testa restore/continuação de estados completos para as 70 variantes de fase geradas pelo corpus suportado, com comparação contínuo/retomado e cobertura NMI/IM0/IM1/IM2.
- CI #41830953731 no HEAD a21506ac4a2d8b66df94dfacdd7c749295e6bb5d: Rust fmt/Clippy/workspace PASS; Python pytest/Ruff PASS; Windows x64 D3D12/WASAPI/XAudio2 PASS; Scheduler deterministic stress SKIPPED.
- O corpus identificou e permitiu corrigir lacunas no validador de LD (HL),r, RET condicional e RST/CALL com prefixo DD/FD ignorado.
- A PR #505 ainda não foi integrada. A matriz abrangente de estados inválidos, WAIT por todas as famílias, IM0 T-states, codec binário SMS completo e demais subfases E7.3–E7.7 permanecem pendentes.
- Estado global: E7 INTEGRATION_PENDING; E8 continua bloqueada até o fechamento das dependências E7.
