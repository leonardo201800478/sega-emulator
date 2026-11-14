# P0 — Git forensics da regressão de frame pacing

- Data: 2026-11-13
- Base atual: `2942abfe0d4754dd857be0bc103d594abfd9b865`
- Issue: #755
- PR investigação: #795

## Timeline verificada

| Data UTC | Commit/PR | Evento | Relevância |
|---|---|---|---|
| 01/11 20:57 | `355779a6dc1b8da207e2872fc3f4c978eb9ab406` | Implementação inicial de MachineScheduler | Introduz timeline determinística genérica |
| 02/11 17:46 | `51a507909ca6b6dcefcb04ca07b18160bc81c473` | Integra Z80 resumível ao Scheduler e barramento SMS | Primeiro marco da integração temporal da máquina |
| 05/11 23:34 | PR #571 / `76f2a4d6f18c87bca2728f8dc524af12b7474a7d` | Worker Rust contínuo e integração SMS temporal | **Primeira candidata de pacing** |
| 06/11 | `76c032cc272828d446cef429d883e99656de789c` | Extração do worker para sms-runtime | Continuidade do caminho de execução |
| 11/11 13:53 | Issue #755 | Sintoma reportado; áudio físico ainda não ativo | Problema antecede CPAL |
| 11/11 20:05 | PR #754 | Preferências e histórico Slint | Merge posterior à abertura da issue; não explica o início |
| 12/11 00:16 | PR #763 / `2fc3055fc2cb87c40cb86dc18b5112bcdafd349d` | Telemetria e FramePacingDeadline | Instrumenta/refatora; não introduz o pacing relativo original |
| 12/11 13:19 | PR #766 | Medições QPC, wake lateness e callback Slint | Instrumentação, sem alteração de política |
| 12/11 16:56 | PR #770 | Integração CPAL | Posterior ao início da issue |
| 12/11 20:21 | PR #774 | Fonte PCM explícita de 48 kHz | Muda taxa de fonte PCM; não altera clock da máquina |
| 12/11 21:24 | PR #785 | Medição de handlers pós-deadline | Fecha lacuna de telemetria |
| 12/11 22:32 | PR #790 | Percentis de frame pacing | Instrumentação |
| 12/11 22:57 | PR #791 | Captura pareada vídeo-only/CPAL | Instrumentação e procedimento |
| 13/11 01:33 | PR #792 | Controles de áudio e DSP experimental | Preservar; avaliar somente como variável secundária |
| 13/11 03:26 | PR #793 / `2942abfe0d4754dd857be0bc103d594abfd9b865` | Renderer H2 experimental integrado | HEAD atual |

## Candidato histórico principal: PR #571

No worker original `native/z80_core/src/sms_temporal_worker.rs`, no merge da PR #571, a duração de frame era calculada a partir da contagem de amostras PSG e da taxa de amostragem. O tempo já consumido era subtraído e o restante passado a `recv_timeout(frame_wait)`.

O código atual mantém a mesma ideia temporal: `frame_started + frame_duration`, com `recv_timeout` até o deadline.

Isso torna a PR #571 a primeira fronteira histórica que deve ser comparada contra o runtime anterior. Não prova que o merge introduziu o déficit.

## O que não é candidato ao início

- PR #754 foi integrada depois da abertura da issue #755.
- PR #763 foi integrada depois da abertura da issue e não alterou MachineScheduler, clocks ou WAIT states.
- PR #770 integrou CPAL depois da abertura da issue; o corpo original da issue afirma que o caminho físico de áudio ainda não estava ativo.
- PRs #774, #785, #790 e #791 são alterações de taxa PCM/telemetria/captura, não evidência de origem do sintoma.
- PR #793 é posterior à issue e deve ser mantida como variável secundária, sem reversão preventiva.

## Última revisão comprovadamente estável

Ainda não identificada por evidência objetiva. O relato de estabilidade anterior não está associado a SHA, ROM, região, build Release e captura QPC pareada.

O commit anterior à transição temporal deve ser usado como candidato de comparação, mas não rotulado como last-known-good até repetir a medição.

## Próxima comparação Git

1. Baseline legado imediatamente anterior à integração temporal da PR #571.
2. Merge da PR #571.
3. Primeira extração do worker em `76c032...`.
4. HEAD atual `2942ab...`.

Manter ROM, região, configuração, duração, máquina física e modo de diagnóstico constantes. Não fazer bisect baseado apenas em FPS do HUD.
