# P0 — Plano de ataque às fontes de jitter e inconsistência de frames

- Data: 2026-11-13
- Estado: **PLANEJAMENTO — investigação aberta**
- Issue: #755
- PR de documentação: #795
- HEAD de referência: `2942abfe0d4754dd857be0bc103d594abfd9b865`
- Runtime: Rust / SMS temporal worker / MachineScheduler
- Host de validação: Windows x64
- Política: **não alterar a semântica temporal do hardware durante a investigação**

## 1. Reenquadramento do sintoma

O relato operacional é uma cadência que oscila aproximadamente entre **55 e 59,7 FPS**, em vez de permanecer próxima de 60 FPS.

Se esses valores representarem FPS instantâneo calculado sobre intervalos curtos, equivalem a:

| Cadência | Intervalo equivalente |
|---:|---:|
| 59,7 FPS | 16,750 ms |
| 59 FPS | 16,949 ms |
| 58 FPS | 17,241 ms |
| 57 FPS | 17,544 ms |
| 56 FPS | 17,857 ms |
| 55 FPS | 18,182 ms |

O orçamento de referência de 60 Hz é 16,667 ms. A diferença entre 59,7 e 55 FPS corresponde a aproximadamente 1,43 ms por frame.

**Ressalva de medição:** se 55–59,7 for uma leitura suavizada de HUD/overlay, ela não representa necessariamente o intervalo individual entre frames. A investigação deve usar timestamps por frame e distribuição de deltas, não apenas o número de FPS exibido.

O padrão relatado é compatível com jitter e/ou atrasos esporádicos. Não demonstra, por si só, uma fonte aleatória: variações podem ser periódicas, correlacionadas com carga, quantização de amostras, escalonamento do SO ou filas de apresentação.

## 2. Objetivo e perguntas que precisam ser respondidas

Separar quatro fenômenos que hoje podem estar misturados:

1. **Produção virtual:** quanto tempo o runtime leva para executar um frame e quantos ticks/amostras virtuais ele consome.
2. **Pacing do worker:** quanto tempo é solicitado em espera, quanto tempo o worker realmente bloqueia e quanto tarde ele retorna.
3. **Entrega ao host:** quanto tempo se gasta publicando vídeo e submetendo áudio, e se há contenção entre threads.
4. **Apresentação física:** quando o frame é consumido pelo renderer, submetido à GPU e efetivamente apresentado.

Perguntas decisivas:

- O intervalo irregular já existe entre o início/fim de frames no worker?
- O worker conclui em cadência correta, mas o renderer/desktop apresenta irregularmente?
- Os atrasos coincidem com timeout tardio, preempção, DPC/ISR ou variação de custo do Scheduler?
- Há uma sequência determinística de frames de 801/802 amostras que está sendo confundida com jitter?
- A telemetria atual altera a carga o suficiente para afetar o resultado?
- O phase-lock recupera fase ou cria rajadas de recuperação que degradam a distribuição?

## 3. Evidências de partida

A investigação anterior registrou, na captura pareada final de 512 observações:

| Métrica | CPAL ativo | Vídeo-only |
|---|---:|---:|
| Worker interval p50 | 17.324 µs | 17.415 µs |
| Worker interval p95 | 19.600 µs | 20.404 µs |
| Worker interval p99 | 21.179 µs | 22.621 µs |
| Worker step p50 | 11.108 µs | 13.644 µs |
| Frame processing p50 | 11.140 µs | 13.664 µs |
| Pacing wait budget p50 | 5.542 µs | 3.022 µs |
| Pacing wait elapsed p50 | 6.130 µs | 3.664 µs |
| Timeout lateness p50 | 449 µs | 373 µs |
| Mailbox replaced total | 1.313 | 2.440 |

Ticks virtuais por frame permaneceram na faixa informada de 59.717–59.755. Uma captura complementar curta de 13/11 apresentou worker interval p50 de 17.332 µs, p99 de 20.751 µs, timeout lateness p50 de 525 µs e PSG samples p50 de 801.

Interpretação limitada:

- O problema não desapareceu no modo vídeo-only; CPAL físico não é a causa primária demonstrada.
- A lateness de timeout é uma candidata relevante, mas seus percentis isolados não provam que explique toda a diferença.
- O processamento mediano informado está abaixo do orçamento nominal, mas p50 não prova que todos os frames estejam abaixo do orçamento. É necessário correlacionar cada observação.
- `mailbox_replaced_total` sugere perda de frames intermediários na entrega ao consumidor, mas não localiza a origem do atraso.
- Não há timestamp físico de scan-out disponível na telemetria Slint atual.

Percentis de colunas diferentes não devem ser somados/subtraídos como se fossem amostras pareadas.

## 4. Priorização das fontes suspeitas

A ordem abaixo prioriza poder explicativo e custo de teste, não declara culpabilidade.

| Prioridade | Fonte suspeita | Por que investigar | Como confirmar/refutar |
|---|---|---|---|
| P0-A | Wake lateness / escalonamento Windows | A espera pode retornar centenas de microssegundos ou milissegundos depois do deadline; o atraso pode variar frame a frame | QPC no instante exato antes/depois do wait, ETW context switches e DPC/ISR |
| P0-B | Política de deadline e recuperação de fase | O deadline atual é reancorado por frame; atrasos podem deslocar a cadência ou produzir recuperação irregular | A/B relativo vs phase-locked com mesmo mecanismo de espera |
| P0-C | Variação do custo de execução do frame | p50 abaixo do orçamento não exclui caudas, preempção ou frames caros | Medidas por frame de step, Scheduler, domínios, instruções e CPU time |
| P0-D | Renderer, mailbox e contenção de threads | Substituições elevadas indicam consumidor atrás do produtor; GPU/GUI podem competir por CPU | Correlacionar sequência produzida, consumida, submit e PresentMon |
| P0-E | Instrumentação e telemetria | Locks, timestamps, coleta de trace e publicação de métricas podem alterar o comportamento | Diagnósticos ligados/desligados, medição externa e buffers pré-alocados |
| P1-A | Quantização PSG / amostras por frame | Alternância 801/802 altera o orçamento em aproximadamente 20,833 µs por amostra a 48 kHz | Registrar sequência exata, fase acumulada e relação com ticks virtuais |
| P1-B | Scheduler / alocações | `calculate_advances` cria `Vec<ClockAdvance>`; pode acrescentar custo variável ou pressão de allocator | Contar despachos/alocações e perfilar Release antes de otimizar |
| P1-C | CPAL callback / contenção de áudio | Underflow foi observado; pode ser consequência do worker lento ou fonte adicional de carga | A/B áudio ativo/vídeo-only com mesmo build e carga, correlacionando callback |
| P1-D | Frequência/estados de energia da CPU | Mudanças de clock, thermal/power limits e migração entre cores podem alterar caudas | ETW/PerfMon: effective clock, uso por core, temperatura/limites quando disponíveis |
| P1-E | Interrupções e drivers | DPC/ISR longos podem atrasar a thread sem elevar seu tempo de CPU | WPR/WPA/ETW com CPU Usage, DPC/ISR e context switches |
| P1-F | Configuração de apresentação / V-Sync / compositor | O FPS exibido pode medir apresentação, não produção do worker | PresentMon e comparação worker-produced vs displayed |
| P2 | Alocação de memória, page faults e cache | Possíveis picos esporádicos em execução real | ETW heap/VirtualAlloc/page faults e profiling direcionado |

### P0-A — Wake lateness do Windows

Não inferir que `recv_timeout` é impreciso apenas porque o intervalo total está alto. Instrumentar a chamada real:

- QPC imediatamente antes de chamar `recv_timeout`.
- Duração efetiva solicitada.
- Deadline absoluto e tempo restante calculado.
- QPC imediatamente após retornar.
- Resultado: timeout, comando ou desconexão.
- Tempo entre retorno e início do próximo frame.
- Identificador de thread e sequência do frame.

Separar explicitamente:
`requested_wait`, `actual_block_time`, `wake_lateness`, `post_wake_work`.

### P0-B — Política de deadline

Comparar três políticas, sem mudar o relógio virtual:

1. **Baseline:** deadline ancorado no início real do frame atual.
2. **Phase-locked:** deadline anterior + duração do frame.
3. **Absolute epoch:** alvo calculado a partir de uma época monotônica e do índice acumulado de frames/durações virtuais, sem somar erro de arredondamento repetidamente.

A política phase-locked pode executar frames consecutivos sem espera quando estiver atrasada. Isso pode recuperar fase, mas também causar rajadas, aumento de CPU e piora de p95/p99. Avaliar distribuição e estabilidade, não apenas FPS médio.

Não aceitar phase-lock como correção antes de demonstrar que não altera ticks, estado, sequência virtual, áudio e comandos.

### P0-C — Custo do frame e MachineScheduler

Por frame, coletar:

- duração total de `step_frame_until`/execução equivalente;
- tempo de CPU da thread, separado do tempo de parede;
- número de instruções/steps e eventos de suspensão;
- despachos do Scheduler por domínio (Z80, PSG, VDP);
- tempo de `dispatch_next` e `calculate_advances`;
- contagem de alocações e bytes alocados;
- WAIT states e operações de barramento relevantes.

O perfil deve ser Release e sem logs síncronos no hot path. Não alterar Scheduler, Z80, VDP, PSG ou WAIT states por antecipação.

### P0-D — Pipeline de vídeo

Adicionar timestamps e contadores independentes para:

- frame produzido pelo runtime;
- publicação no mailbox;
- substituição de frame antigo;
- frame consumido pelo renderer;
- início/fim de upload ou atualização de textura;
- submit à GPU;
- apresentação reportada por PresentMon.

A sequência deve carregar um frame sequence ID. Assim será possível detectar se a irregularidade está no produtor, no consumidor ou no compositor. Não usar callback Slint como substituto de timestamp de apresentação física.

### P0-E — Instrumentação

A coleta por frame não pode fazer I/O síncrono, alocar memória ou adquirir mutex global no caminho crítico.

Preferir ring buffer pré-alocado, por thread, com gravação compacta de eventos e flush fora do worker. Fazer ensaio externo com diagnóstico ligado e desligado.

## 5. Plano de execução — ordem obrigatória

### Fase 0 — Congelar baseline reproduzível

- Fixar SHA, build Release, ROM/hash, região, estado inicial, renderer e resolução.
- Fixar modo de áudio, V-Sync, VRR e limite de FPS.
- Registrar CPU/GPU, driver, plano de energia, temperatura e processos de fundo.
- Fazer warm-up e três capturas de pelo menos cinco minutos por condição.
- Preservar logs brutos, CSV PresentMon, QPC frequency e metadados.
- Não comparar capturas de commits, ROMs ou configurações diferentes como se fossem A/B.

**Saída:** baseline reproduzível e distribuição por frame, não apenas FPS médio.

### Fase 1 — Instrumentar o caminho temporal

Instrumentar os pontos exatos de worker, Scheduler, wait, mailbox, áudio e renderer. Todos os eventos devem usar a mesma base monotônica/QPC ou conter conversão verificável.

Gerar uma linha lógica por frame, com sequence ID e timestamps. Se usar buffers separados por thread, correlacionar por sequence ID.

**Saída:** timeline capaz de atribuir cada frame fora do prazo a um intervalo específico.

### Fase 2 — Separar execução virtual de pacing

Executar a mesma ROM e estado com pacing de parede desabilitado, usando a execução em blocos já existente e sem mudar a semântica da máquina.

Medir throughput, ticks, estado final, instruções, amostras PSG e equivalência determinística. Comparar com execução contínua.

**Saída:** provar se o runtime consegue executar acima do throughput necessário quando não espera pelo relógio de parede.

### Fase 3 — Isolar espera e wake-up

Comparar o timeout solicitado com o bloqueio real, wake lateness e trabalho pós-wake. Fazer captura ETW/WPA com context switches, DPC/ISR e ready time da thread.

Só então comparar o mecanismo atual com waitable timer de alta resolução no Windows, mantendo espera interrompível por comandos. Não introduzir busy-spin como solução permanente.

**Saída:** determinar se o atraso nasce no mecanismo de espera, no escalonador do SO ou depois do despertar.

### Fase 4 — A/B de deadline

Executar baseline, phase-locked e absolute epoch em branches experimentais separadas ou em configuração explicitamente opt-in.

Avaliar:
- FPS médio e distribuição de intervalos;
- p50/p90/p95/p99/max de lateness;
- número e duração de frames de recuperação;
- frames produzidos/consumidos/substituídos;
- CPU e energia;
- ticks e estado virtual.

**Saída:** decidir política temporal com evidência, sem integrar automaticamente o experimento atual.

### Fase 5 — Isolar vídeo e áudio

Matriz mínima:
- worker sem renderer/consumer;
- renderer com consumer determinístico;
- Slint + WGPU/Vulkan;
- vídeo-only;
- CPAL ativo.

Manter todas as outras variáveis constantes. Medir áudio produzido/consumido, callback duration, underruns, mailbox e PresentMon.

**Saída:** localizar contenção ou desacoplamento entre produção, áudio e apresentação.

### Fase 6 — Perfil do Scheduler e do host

Executar profiling Release de CPU e allocator; correlacionar com ETW, effective clock, page faults e DPC/ISR. Testar afinidade/prioridade apenas como diagnóstico controlado, nunca como correção presumida.

**Saída:** comprovar ou descartar custo variável de runtime e interferência do host.

### Fase 7 — Correção mínima e validação

Só implementar a correção que corresponda à fonte comprovada. Preservar o runtime validado e limitar a mudança ao ponto responsável.

Executar regressões de:
- determinismo e equivalência contínua/em blocos;
- ticks e estado final;
- pause/resume/reset/load e comandos durante wait;
- overrun e recuperação;
- contagem de amostras PSG e áudio;
- SMS e SG-1000;
- Z80, WAIT states, VDP e barramentos;
- CI aplicável, Windows x64 Release e captura A/B física.

**Saída:** PR pequena, com causa, evidência, testes e comparação antes/depois.

## 6. Esquema mínimo de telemetria por frame

Cada registro deve conter, no mínimo:

- `run_id`, `build_sha`, `frame_seq`, `region`, `audio_mode`;
- `qpc_frame_start`, `qpc_step_start`, `qpc_step_end`;
- `qpc_wait_start`, `qpc_wait_end`, `qpc_next_frame_start`;
- `deadline_qpc`, `requested_wait_us`, `actual_wait_us`, `wake_lateness_us`;
- `step_wall_us`, `step_thread_cpu_us`, `scheduler_dispatch_count`;
- `machine_ticks`, `psg_samples`, `audio_samples_submitted`;
- `mailbox_publish_qpc`, `mailbox_replace_count`, `renderer_consume_qpc`;
- `gpu_submit_qpc`, quando disponível, e referência ao evento PresentMon;
- `wake_reason`, `command_count`, `diagnostics_enabled`.

Usar inteiro QPC bruto nos artefatos. Converter para microssegundos na análise com a frequência capturada no início. Não registrar apenas percentis agregados: preservar amostras pareadas e ordem temporal.

## 7. Análise estatística obrigatória

Para cada condição, produzir:

- histograma e CDF de `worker_frame_interval`;
- p50/p90/p95/p99/max e desvio padrão;
- sequência temporal de intervalos por frame;
- autocorrelação e periodicidade;
- correlação entre intervalos longos, wake lateness, step cost, DPC/ISR e mailbox replacement;
- contagem de frames acima de 16,667 ms, 17,0 ms e 18,0 ms;
- distribuição por motivo de wake;
- comparação entre FPS do overlay e FPS calculado de timestamps de apresentação.

Não chamar o atraso de aleatório sem testar periodicidade e correlação. Distinguir jitter de baixa frequência, picos esporádicos e drift acumulado.

## 8. Referências técnicas para o caminho Windows/Rust

- Rust `Instant`: no Windows, a documentação atual descreve QueryPerformanceCounter como base subjacente de `Instant::now()`. Isso torna apropriado manter timestamps monotônicos e capturar a frequência QPC para análise. https://doc.rust-lang.org/stable/std/time/struct.Instant.html
- Rust `std::sync::mpsc::Receiver::recv_timeout`: retorna quando chega uma mensagem ou quando o timeout expira; a documentação não oferece garantia de deadline rígido ou latência máxima de despertar. https://doc.rust-lang.org/stable/std/sync/mpsc/struct.Receiver.html
- Windows `QueryPerformanceCounter`: usar QPC/QPF para intervalos de alta resolução e capturar a frequência em cada execução. https://learn.microsoft.com/windows/win32/sysinfo/acquiring-high-resolution-time-stamps
- Windows `CreateWaitableTimerExW`: oferece `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` desde Windows 10 versão 1803. É uma alternativa experimental a avaliar, não garantia de ausência de preempção nem justificativa para mudar o runtime sem A/B. https://learn.microsoft.com/windows/win32/api/synchapi/nf-synchapi-createwaitabletimerexw
- Microsoft recomenda cautela com timers de alta resolução por impacto potencial no consumo de energia. https://learn.microsoft.com/windows-hardware/drivers/kernel/high-resolution-timers

## 9. Critérios de conclusão

A investigação só termina quando:

1. A fonte do jitter estiver vinculada a eventos medidos e código/commit específico.
2. A correção reduzir a cauda de latência e a variação dos intervalos, não apenas elevar FPS médio.
3. O comportamento virtual continuar idêntico: ticks, estados, amostras e sequência determinística.
4. A apresentação física estiver medida independentemente do callback Slint.
5. O áudio não apresentar regressão ou underflow atribuível à mudança.
6. Execução contínua e em blocos permanecerem equivalentes.
7. SMS e SG-1000 passarem as regressões aplicáveis.
8. A captura A/B Windows x64 durar pelo menos cinco minutos por condição, com artefatos brutos preservados.

## 10. Decisões proibidas antes das evidências

- Não reescrever MachineScheduler.
- Não alterar clocks, T-states, M-cycles, WAIT states ou temporização do Z80.
- Não acelerar o hardware virtual para compensar atraso do host.
- Não descartar frames virtuais para esconder backlog.
- Não introduzir busy-spin, prioridade elevada ou afinidade fixa como solução de produção sem prova e análise de impacto.
- Não adotar timer de alta resolução como correção automática.
- Não atribuir o problema ao CPAL, Slint, WGPU, Vulkan ou Windows sem correlação temporal.
- Não fazer merge da PR #796 apenas porque seus testes unitários passaram.

## 11. Próxima ação imediata

A primeira execução deve ser **Fase 0 + Fase 1**, antes de qualquer otimização.

A observação mais importante não é o FPS agregado: é o frame individual que excedeu 16,667 ms, com seus timestamps de execução, espera, wake-up, publicação e apresentação. A partir dele será possível apontar qual subsistema consumiu o orçamento e qual evento introduziu a variação.
