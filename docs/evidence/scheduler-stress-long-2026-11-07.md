# MachineScheduler — campanha longa de estresse determinístico

**Data:** 2026-11-07  
**Componente:** `sega-machine-scheduler`  
**Resultado:** PASS  
**Origem do resultado:** saída de terminal fornecida pelo usuário; fontes verificadas no GitHub.

## Identificação da execução

- Sistema: Windows local.
- Shell: PowerShell.
- Repositório local: `C:\Users\leost\sega-emulator`.
- Branch: `feat/sega-app-audio-keyboard-controls-20261106`.
- HEAD executado: `f118a4d3a2547b95c679c4133b94e22d2b3a03be`.
- Tracking remoto da branch: mesmo SHA.
- Perfil: `long`.

## Comando reproduzível

```powershell
$env:SCHEDULER_STRESS_PROFILE = "long"
cargo test -p sega-machine-scheduler --test stress_replay -- --ignored --nocapture
```

## Carga executada

| Teste | Seeds | Carga por seed | Total |
|---|---:|---:|---:|
| `fixed_seed_operation_sequences_match_across_codec_restores` | 16 | 20.000 operações | 320.000 operações |
| `fixed_seed_stress_replay_survives_repeated_restores` | 16 | 250.000 dispatches | 4.000.000 dispatches |

As 16 seeds de cada campanha foram reportadas individualmente como verificadas pelo teste.

## Resultado observado

```text
running 2 tests
...
test fixed_seed_operation_sequences_match_across_codec_restores ... ok
...
test fixed_seed_stress_replay_survives_repeated_restores ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 1 filtered out; finished in 405.68s
```

**Classificação:** ambos os testes PASS; 0 falhas; duração total 405,68 segundos. O teste não ignorado restante do arquivo foi filtrado pelo comando, não executado nesta campanha.

## Comparação dos fontes com main

A main consultada estava em `3ad1781f84b7824b5aa889a583ae6d4545987ec1`. O SHA executado era uma branch diferente, portanto foram comparados os blobs dos arquivos relevantes:

| Arquivo | Blob SHA em main | Blob SHA no SHA executado | Resultado |
|---|---|---|---|
| `native/machine_scheduler/tests/stress_replay.rs` | `228b379745054ea8a8b184cda95c33191781d237` | `228b379745054ea8a8b184cda95c33191781d237` | Idêntico |
| `native/machine_scheduler/Cargo.toml` | `33f853d9fa64f7719d94d2d90462858557c3b59d` | `33f853d9fa64f7719d94d2d90462858557c3b59d` | Idêntico |
| `native/machine_scheduler/src/lib.rs` | `ba3f16500081007b05f44d9ee83997aad03a7ef4` | `ba3f16500081007b05f44d9ee83997aad03a7ef4` | Idêntico |

**Interpretação:** a implementação, o manifesto e o teste do Scheduler usados na execução são byte a byte equivalentes aos mesmos arquivos na main auditada. O resultado pode ser associado a esses arquivos da main. Não se deve descrevê-lo como execução integral da main ou de todos os crates do workspace.

## Histórico anterior — Codespaces

- Perfil curto: PASS, 4 seeds × 2.000 operações = 8.000 operações e 4 seeds × 20.000 eventos = 80.000 eventos.
- Perfil longo: foi iniciado no Codespaces, mas interrompido manualmente após duração superior a 60 segundos e consumo elevado de quota. Classificação BLOCKED/INTERRUPTED, sem resultado final.
- A campanha longa foi então executada no Windows local e concluída com PASS.
- O Codespace foi parado, não excluído.

## O que esta evidência comprova

- Reprodutibilidade das sequências de operações para as seeds e carga configuradas.
- Consistência do replay de dispatches após restaurações repetidas.
- Comportamento dos codecs/restores cobertos pelos cenários desse teste.
- Ausência de divergência detectada pelos dois testes nesta campanha.

## O que esta evidência NÃO comprova

- Precisão temporal integrada CPU + barramento + VDP + PSG.
- Correção de instruções ou flags do Z80.
- Cycle accuracy universal ou comportamento elétrico/pino a pino.
- Save States completos da máquina SMS.
- Compatibilidade de jogos, boot de ROM, imagem, som ou input físico.
- Gates `fmt`, Clippy, testes completos do workspace ou CI no SHA executado.
- Validação em GPU, WASAPI, XAudio2 ou gamepad reais.

## Referências

- [Commit executado](https://github.com/leonardo201800478/sega-emulator/commit/f118a4d3a2547b95c679c4133b94e22d2b3a03be)
- [Main auditada](https://github.com/leonardo201800478/sega-emulator/commit/3ad1781f84b7824b5aa889a583ae6d4545987ec1)
- [PR #614](https://github.com/leonardo201800478/sega-emulator/pull/614)
