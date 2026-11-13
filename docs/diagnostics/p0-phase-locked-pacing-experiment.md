# P0 — Experimento isolado de deadline phase-locked

- Branch: `p0/experiment-phase-locked-deadline-20261113`
- Base: `2942abfe0d4754dd857be0bc103d594abfd9b865`
- Estado: experimental, não integrado
- Padrão: comportamento atual preservado

## Objetivo

Testar se a reancoragem do deadline em cada início real de frame faz a lateness de despertar do Windows deslocar permanentemente a cadência dos frames seguintes.

O experimento compara duas políticas usando o mesmo `recv_timeout`, a mesma ROM e o mesmo runtime virtual. Não altera Scheduler, clocks, Z80, VDP, PSG, WAIT states, produção de áudio ou publicação de vídeo.

## Modificação isolada

O worker reconhece a variável de ambiente:

```powershell
$env:SEGA_P0_PHASE_LOCKED_PACING = "1"
```

A variável é lida uma vez quando o worker é criado.

- **Variável ausente:** política baseline, deadline do próximo frame = `frame_started + frame_duration`.
- **Variável igual a 1:** política experimental, deadline seguinte = `previous_deadline + frame_duration`.

No modo experimental, o deadline expirado é preservado como âncora de fase. Ele continua sendo limpo em pause, stop, reset, troca de ROM e transições equivalentes. Os comandos continuam interrompendo a espera pelo mesmo canal; nenhum `sleep` substitui o recebimento de comandos.

O log `SMS worker pacing mode` registra o modo ativo. A política experimental não é o padrão e não deve ser integrada sem evidência física.

## Testes adicionados

Os testes determinísticos verificam:

1. A política phase-locked avança o alvo anterior pela duração do frame, mesmo quando o início efetivo do frame seguinte está atrasado.
2. A política baseline continua reancorando o alvo ao início efetivo do frame.

Esses testes validam a aritmética da política, não a precisão do timer do Windows.

## Procedimento A/B no Windows x64

Executar em build Release, com a mesma ROM, região, estado inicial, renderer, configuração gráfica e duração.

### A — Baseline

1. Remover a variável de ambiente.
2. Reiniciar o emulador para garantir que o worker leia o ambiente limpo.
3. Capturar pelo menos cinco minutos.
4. Preservar log bruto, metadados QPC e CSV/trace PresentMon.

```powershell
Remove-Item Env:SEGA_P0_PHASE_LOCKED_PACING -ErrorAction SilentlyContinue
```

### B — Phase-locked

1. Fechar completamente o emulador.
2. Definir a variável.
3. Iniciar o mesmo build e repetir a captura por pelo menos cinco minutos.
4. Confirmar no log `phase_locked_pacing=true`.

```powershell
$env:SEGA_P0_PHASE_LOCKED_PACING = "1"
```

### Métricas a comparar

- `worker_frame_interval_p50/p95/p99`.
- `worker_step_p50/p95/p99`.
- `frame_processing_p50/p95/p99`.
- `pacing_wait_elapsed_p50/p95/p99`.
- `pacing_timeout_lateness_p50/p95/p99`.
- `machine_ticks_per_frame_min/p50/max`.
- `psg_samples_p50/p95/p99`.
- `mailbox_replaced_total`.
- underruns/dropped frames do áudio quando CPAL estiver ativo.
- PresentMon: cadência, dropped presents e duração dos presents.

Percentis independentes não devem ser somados para inferir uma observação individual. A comparação deve usar janelas equivalentes e preservar os logs brutos.

## Interpretação

A hipótese ganha suporte se a política phase-locked reduzir o intervalo de parede para próximo do orçamento regional, sem alterar ticks virtuais por frame, estado final, sequência de frames ou determinismo.

A hipótese perde suporte se o intervalo continuar acima do orçamento, se o custo de execução exceder sistematicamente a duração do frame ou se a melhoria depender de alteração de clocks virtuais.

Se o worker ficar atrasado por mais de um frame, a política phase-locked poderá executar frames consecutivos sem espera até recuperar a fase. Isso não pula frames nem acelera o relógio virtual, mas exige observar CPU, p95/p99 e estabilidade visual.

## Limites

- Este branch não altera o padrão de produção.
- Este branch não implementa timer de alta resolução.
- O experimento isola apenas a política de deadline entre frames.
- A execução física precisa ocorrer no Windows x64 alvo.
- Não fazer merge antes de CI, revisão independente e captura A/B aprovada.
