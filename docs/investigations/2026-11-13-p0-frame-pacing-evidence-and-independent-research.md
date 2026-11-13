# P0 — Dossiê de evidências: frame pacing, apresentação e áudio

**Data de consolidação:** 2026-11-13  
**Investigação relacionada:** [Issue #755](https://github.com/leonardo201800478/sega-emulator/issues/755)  
**PR de execução determinística:** [PR #806](https://github.com/leonardo201800478/sega-emulator/pull/806)  
**Commit do build capturado:** `d093037bd5fbfef1278d4980f775efa2c549171c`  
**Captura:** `audio-paired-20261113-143303.zip`  
**ROM:** `combined.sms`, SHA-256 `434A4B34F87FA96B52D94EF4005A7D0D3D654D3B6594BFD7AA45A27FA09B306E`

> Este documento separa fatos medidos, interpretações e hipóteses. Nenhuma causa-raiz foi demonstrada. Os resultados não autorizam alterações empíricas no MachineScheduler, Z80, VDP ou PSG.

## 1. Objetivo e escopo

Investigar a cadência irregular observada no desktop SMS, distinguindo quatro domínios que não podem ser confundidos:

1. **Tempo emulado:** master clock, ticks, execução do Scheduler e conclusão de frames.
2. **Produção/publicação:** processamento do worker temporal e entrega na FrameMailbox.
3. **Consumo/renderização:** polling da GUI Slint, atualização de textura, submit/present WGPU e sincronização com o compositor.
4. **Saída física:** apresentação efetiva no monitor e consumo de áudio pelo dispositivo CPAL.

A meta funcional é apresentação uniforme e estável. Aceita-se avaliar um custo limitado de 1–2 frames de latência se necessário, desde que não haja crescimento contínuo de fila/latência, nem alteração da temporização do hardware emulado. Isso é uma preferência de projeto a validar por medição, não uma decisão de implementação já aprovada.

## 2. Evidência prévia: execução offline da máquina SMS

Os perfis P0 foram executados duas vezes cada, por 600 frames, através de `SmsTemporalMachine`:

- `cpu-light`
- `cpu-fixed`
- `vdp-sprites`
- `combined`

O runner exigiu `EXECUTED_NO_ORACLE`, 600 frames concluídos, contagem de instruções e igualdade dos registros JSONL decodificados entre repetições.

| Perfil | Instruções contabilizadas | Amostras PCM drenadas |
|---|---:|---:|
| cpu-light | 4.005.916 | 480.618 |
| cpu-fixed | 4.687.438 | 480.618 |
| vdp-sprites | 4.099.321 | 480.618 |
| combined | 4.780.488 | 480.618 |

**Interpretação permitida:** a execução full-machine dos quatro workloads foi repetível sob o runner e produziu registros idênticos nas repetições.

**Limite explícito:** `EXECUTED_NO_ORACLE` não é certificação funcional do hardware. O runner offline avança o tempo emulado tão rápido quanto possível e não exercita `SmsTemporalWorker` ao vivo, Slint, WGPU, VSync, PresentMon/ETW ou o monitor físico. Portanto, esses dados não provam frame pacing em tempo real.

## 3. Captura Windows pareada — 13/11/2026

### 3.1 Integridade e ambiente

- 3 pares de captura; cada execução durou 300 segundos.
- 6 processos do emulador e 6 CSVs PresentMon.
- PresentMon 2.6.0, iniciado oculto e direcionado ao PID exato do emulador.
- Todas as capturas terminaram com exit code 0.
- Mesma ROM e mesmo SHA-256 em todas as execuções.
- Commit: `d093037bd5fbfef1278d4980f775efa2c549171c`.
- Host: Ryzen 5 5600, RTX 5070, 32 GB RAM.
- Display: AOC 27G2G4, 1920×1080, 144 Hz.
- Windows 11 Pro, build 26300.9974; driver NVIDIA 32.0.16.2124.
- PresentMode observado: `Composed: Flip`; `SyncInterval=1`.

Condições:
- **video-only:** `SEGA_EMULATOR_VIDEO_ONLY=1`; PSG digital continua sendo gerado, mas a saída física CPAL é desativada.
- **audio-active:** variável removida; CPAL abre o dispositivo físico.

Cada par usa o mesmo workload combinado. A ordem foi video-only e depois audio-active. São três repetições pareadas, não um ensaio aleatorizado; diferenças ambientais entre execuções continuam possíveis.

### 3.2 PresentMon — eventos de apresentação

| Par | Modo | FrameTime P50 | FrameTime P95 |
|---|---|---:|---:|
| 1 | video-only | 8,62 ms | 13,02 ms |
| 1 | audio-active | 8,64 ms | 13,16 ms |
| 2 | video-only | 8,93 ms | 13,72 ms |
| 2 | audio-active | 8,90 ms | 13,13 ms |
| 3 | video-only | 8,77 ms | 13,16 ms |
| 3 | audio-active | 8,80 ms | 13,15 ms |

Observações adicionais:
- A maioria das capturas registrou aproximadamente 107 eventos/s; o par 2 video-only ficou perto de 103 eventos/s.
- DisplayLatency P50 aproximado de 20,3 ms e P95 de 26,1 ms.
- DisplayedTime P50 aproximado de 6,95 ms.
- GPUTime mediano aproximado de 5,2 ms; GPUBusy mediano aproximado de 0,56 ms.
- Todos os eventos classificados como `Composed: Flip`, `SyncInterval=1`.

**Conclusão permitida:** ativar CPAL não piorou consistentemente as métricas de eventos PresentMon nas três comparações. Os números não apontam para saturação evidente da GPU.

**Limite crítico:** eventos PresentMon não equivalem automaticamente a frames novos e únicos do emulador nem comprovam que cada evento alterou os pixels exibidos no painel. O compositor pode reapresentar conteúdo já existente. Não declarar 107 FPS reais do jogo, nem cadência física comprovada. Falta correlacionar uma identidade/sequence number do frame desde sua produção até a apresentação e, idealmente, a captura física.

### 3.3 Runtime — worker, polling e mailbox

Em cinco das seis execuções, as medianas/janelas móveis foram semelhantes:

| Métrica | Faixa típica observada |
|---|---:|
| Worker processing P50 | 12,8–13,0 ms |
| Worker processing P95 | 14,0–14,2 ms |
| Worker processing P99 | 16,4–17,0 ms |
| Worker frame interval P50 | 17,33–17,39 ms |
| Worker frame interval P95 | 18,8–18,9 ms |
| Worker frame interval P99 | 21,0–21,5 ms |
| Slint polling P50 | 17,5–17,7 ms |
| Slint polling P95 | 19,1–19,3 ms |
| Slint polling P99 | 20–21 ms |
| Consumed interval P50 | 17,6–17,7 ms |
| Consumed interval P95 | 19,4–19,5 ms |
| Consumed interval P99 | 32,9–33,1 ms |
| Mailbox residence P50 | 6,4–7,9 ms |
| Mailbox residence P95 | 16,2–16,4 ms |
| Mailbox residence P99 | 17,3–17,4 ms |
| Mailbox replaced | 488–527 por execução de 5 min |
| Pacing timeout lateness P50 | 0,6–0,8 ms |
| Pacing timeout lateness P95 | ~1,35 ms |
| Pacing timeout lateness P99 | ~1,8 ms |

O budget nominal de um frame a 60 Hz é aproximadamente 16,688 ms. Assim, o processamento típico fica abaixo do budget na mediana, mas a cauda P99 encosta ou ultrapassa o limite. O intervalo real do worker é tipicamente ~17,35 ms, portanto maior que 16,688 ms. Atraso de wake-up também participa do intervalo.

**Anomalia do par 2 video-only:** processing P50 de aproximadamente 18,58 ms, P95 de 19,80 ms e interval P50 de aproximadamente 18,60 ms. O par 2 audio-active voltou a ~12,9 ms de processing P50. A causa da anomalia não foi isolada. Não atribuir a CPAL, temperatura, energia, escalonamento do Windows ou qualquer outro fator sem nova evidência.

Master ticks por frame permaneceram próximos de 59.736 (faixa aproximada 59.725–59.748). Isso não mostra deriva temporal relevante do clock emulado nesses dados, mas não prova isoladamente toda a correção causal do Scheduler.

A mailbox teve centenas de substituições por execução. Isso é compatível com uma política latest-frame-wins quando o consumidor não acompanha todas as publicações; não demonstra por si só que a política esteja errada. A prioridade deve ser distinguir frames substituídos intencionalmente, frames nunca consumidos, frames repetidos na apresentação e atraso acumulado.

### 3.4 CPAL — saída física de áudio

CPAL abriu em todas as três execuções audio-active:
- 48.000 Hz, 2 canais, F32.
- Fallback de configuração default: false.
- Capacidade da fila: 4.096 frames.
- `output_error=None`.
- Nenhum frame descartado pelo contador (`dropped_frames=0`).

| Par | accepted_frames | consumed_frames | underrun_frames | underrun_events |
|---|---:|---:|---:|---:|
| 1 | 14.451.407 | 14.450.603 | 607.573 | 3.835 |
| 2 | 14.455.412 | 14.454.608 | 621.328 | 3.942 |
| 3 | 14.479.443 | 14.478.480 | 620.976 | 3.920 |

- Underrun events: aproximadamente 767–788 por minuto.
- Razão `underrun_frames / consumed_frames`: aproximadamente 4,20–4,30%.
- Ocupação típica da fila: ~800–963 frames; high-water ~1.443–1.444 de 4.096.
- Sem evidência de overflow; os contadores de underrun aumentaram continuamente.

**Interpretação permitida:** existe uma anomalia mensurável no caminho de áudio CPAL, com eventos de underrun reportados pelo runtime.

**Não concluir ainda:** que 4,2–4,3% do tempo audível foi silêncio, que houve dropout audível nessa proporção ou que a causa seja o callback. É obrigatório auditar a semântica exata dos contadores, a unidade de cada contador e a relação produtor/consumidor. Não houve loopback acústico; a abertura do stream e a contagem de frames não certificam qualidade perceptiva.

### 3.5 Comparação pareada

Não foi observada degradação visual consistente ao ativar CPAL. A anomalia de processamento mais clara ocorreu em video-only no par 2 e desapareceu no audio-active correspondente. Com a amostra atual, CPAL não está demonstrado como causa do problema visual.

A questão de áudio é separada e real no plano dos contadores, mas sua semântica e causa ainda precisam ser auditadas.

## 4. Estado das conclusões

### Fatos medidos
1. Os seis processos foram capturados por 300 s com PresentMon e terminaram normalmente.
2. CPAL abriu nas três execuções audio-active, sem `output_error`.
3. Os contadores CPAL registraram milhares de eventos de underrun.
4. Em cinco execuções, processing P50 ficou em ~12,8–13,0 ms; a cauda P99 aproximou-se ou passou do budget de 16,688 ms.
5. O worker apresentou interval P50 em ~17,33–17,39 ms nas execuções típicas.
6. A execução offline dos quatro perfis foi determinística segundo o critério do runner.
7. GPUTime/GPUBusy não demonstraram saturação da GPU nas capturas.
8. O PresentMode foi Composed Flip; eventos de apresentação não identificam conteúdo novo por si sós.

### Hipóteses ainda abertas
- mecanismo de espera do worker e acúmulo de lateness (deadline relativo vs. absoluto/phase-locked);
- jitter de wake-up e escalonamento do Windows;
- custo de processamento e margem insuficiente nas caudas;
- interação entre timer/polling do Slint e worker;
- comportamento de publicação/consumo da FrameMailbox;
- submissão WGPU, swapchain, compositor e sincronização de apresentação;
- semântica e funcionamento do produtor/consumidor CPAL;
- diferenças ambientais que expliquem a anomalia isolada do par 2.

Nenhuma hipótese acima deve ser promovida a causa-raiz sem teste que a discrimine.

## 5. Alternativas de solução a investigar — sem escolher antecipadamente

A investigação não deve pressupor que o caso seja apenas uma correção local de timer. Avaliar soluções por camadas, incluindo mudanças de arquitetura do host se as medições mostrarem que a arquitetura atual não consegue garantir a cadência pretendida.

### A. Corrigir a política de deadline do worker
- Comparar espera relativa (sleep após o trabalho) com deadline absoluto ancorado em QPC/clock monotônico.
- Medir erro de fase acumulado, lateness e recuperação após atraso.
- Considerar timer de alta resolução/waitable timer no Windows, se compatível com o modelo multiplataforma.
- Não adotar busy-spin permanente como solução padrão; medir custo, energia, prioridade e interferência.
- Não permitir que o worker altere ticks/ciclos para compensar atraso de host.

### B. Aumentar margem de execução
- Perfilar cada fase do worker separadamente: avanço do Scheduler, composição de frame, cópia/transferência e publicação.
- Identificar alocações, locks, cópias e trabalho não essencial no caminho crítico.
- Estabelecer orçamento P95/P99 e margem sob carga concorrente.
- Otimizar somente após localizar o custo e manter testes de equivalência temporal.

### C. Desacoplar produtor e consumidor sem fila ilimitada
- Avaliar mailbox latest-frame-wins versus buffer limitado de 1 e 2 frames.
- Medir idade do frame, atraso ponta a ponta, repetição, descarte e estabilidade.
- Aceitar no máximo uma latência limitada e explicitamente medida; proibir crescimento contínuo de fila.
- Não usar buffering para mascarar produtor permanentemente lento.

### D. Revisar a integração Slint/WGPU
- Medir separadamente callback de timer, polling, atualização da textura, submit, present e espera por VSync.
- Verificar se a GUI pode consumir uma notificação/evento de frame em vez de polling periódico, se o modelo Slint permitir sem acoplar o runtime ao ciclo da GUI.
- Testar modo janela, fullscreen exclusivo/borderless, compositor e diferentes configurações de refresh, uma variável por ensaio.
- Investigar waitable swapchain/latency controls somente se expostos pela API e se a evidência apontar para essa camada.

### E. Separar pacing de emulação e pacing de apresentação
- O Scheduler continua autoridade temporal do hardware.
- Um host pacing controller pode regular quando solicitar/consumir trabalho coarse-grained, sem comandar ciclos individuais.
- Considerar sincronização baseada em timestamps e refresh medido do display, não em FPS instantâneo isolado.
- Se necessário, comparar sincronização guiada por áudio, vídeo ou controlador de host, mantendo o tempo emulado independente.

### F. Áudio
- Auditar código de callback, fila SPSC, produtor, tamanho/unidade dos buffers e fórmula dos contadores.
- Registrar frames solicitados, disponíveis, preenchidos com silêncio, descartados e duração de underrun em unidades inequívocas.
- Verificar se o produtor entrega amostras com cadência correspondente ao master clock e à taxa de saída.
- Avaliar resampling/dynamic rate control como estratégia de sincronização de host somente após documentar a política de sincronização e preservar a frequência/semântica do PSG.
- Não bloquear o Scheduler esperando o dispositivo físico.

### G. Alternativas mais amplas
Se a correção local não resolver, comparar:
- loop de apresentação dedicado ao host versus integração direta com o event loop Slint;
- consumidor orientado a eventos versus polling;
- swapchain e política de frame latency diferentes;
- pacing dirigido pelo refresh do display ou VRR quando disponível;
- buffer temporal pequeno e limitado com política explícita;
- arquitetura de frontend alternativa apenas como experimento isolado, sem descartar Slint/WGPU antes de dados comparáveis.

Qualquer alternativa precisa manter o runtime reutilizável e não pode mover hardware/timing para a GUI.

## 6. Pesquisa externa: estratégias de outros emuladores

A documentação pública do RetroArch/Libretro oferece referências concretas, mas não uma receita automaticamente transferível:

1. **Dynamic Rate Control (DRC):** sincroniza áudio e vídeo ajustando dinamicamente a razão de resampling para evitar underrun/overrun do buffer, permitindo VSync sem bloquear o emulador no áudio. É uma estratégia de sincronização do host; a adaptação de sample rate não deve alterar a temporização interna do PSG. Referências:
   - https://docs.libretro.com/development/cores/dynamic-rate-control/
   - https://docs.libretro.com/guides/ratecontrol.pdf
2. **Calibração de refresh e VSync:** RetroArch recomenda medir/configurar a taxa real do display com precisão, usar VSync e swap interval coerente, e alerta que compositores, threaded video, overlays e latência de áudio podem afetar pacing. Isso reforça a necessidade de medir refresh efetivo e isolar uma variável por vez:
   - https://docs.libretro.com/guides/optimal-vsync/
3. **Threaded video:** a documentação descreve que pode reduzir overhead do driver em alguns cenários, mas pode piorar pacing e aumentar latência. Portanto, “colocar renderização em outra thread” não é solução universal:
   - https://docs.libretro.com/guides/troubleshooting-retroarch/
4. **Dolphin:** sua FAQ diferencia stutter de áudio causado por emulação abaixo da velocidade integral e descreve time-stretched audio como opção para preencher lacunas perceptivas, com custo de latência. É uma estratégia paliativa/perceptual, não prova de correção do relógio:
   - https://dolphin-emu.org/docs/faq/
5. **MAME:** documenta throttling para manter velocidade nativa e uma opção low-latency que antecipa desenho antes do controle de velocidade, mas alerta para possíveis problemas de sequenciamento/stutter em determinadas situações. Isso demonstra o trade-off entre latência e estabilidade:
   - https://docs.mamedev.org/commandline/commandline-all.html

**Aplicabilidade ao Sega Emulator:** usar essas referências como hipóteses experimentais e padrões comparativos. Não copiar arquitetura de frontend nem adaptar sample rate, throttle ou frame skipping sem definir invariantes e critérios. A pesquisa deve avançar para código-fonte e issues upstream de RetroArch, MAME, Dolphin, bsnes/higan e outros emuladores 8-bit, identificando: relógio mestre, entidade que dirige o pacing, fila de áudio/vídeo, política de deadline, sincronização VSync/VRR, tratamento de atraso e telemetria.

## 7. Próxima sequência de investigação

1. Preservar os artefatos originais da captura e seus hashes; manter CSVs/logs fora do Git se forem grandes, mas registrar caminhos, checksums, ambiente e commit.
2. Auditar a implementação do worker e a política de deadline; desenhar linha temporal de cada frame.
3. Adicionar identidade monotônica por frame e timestamps para: frame-ready no runtime → publish → consume → texture update → submit → present. Identificar explicitamente repetidos e descartados.
4. Repetir teste de 5 minutos com carga controlada e captura ETW/PresentMon, incluindo baseline sem overlay e sem atividade secundária; registrar condições ambientais.
5. Auditar CPAL e provar o significado dos contadores com teste unitário/instrumentado e loopback, se possível.
6. Fazer experimentos A/B de uma variável por vez: deadline absoluto, timer, polling/evento, buffers 1/2, modo de janela, refresh/VSync.
7. Só então escolher correção local, redesenho limitado do host ou alternativa de arquitetura. Cada experimento deve ter hipótese, variável única, métrica primária, critério de sucesso e rollback.
8. Revalidar execução contínua vs. em blocos e determinismo; manter o Scheduler como autoridade.

### Critérios mínimos para declarar resolvido
- Pacing de frames novos, não apenas Present events, medido por sequência de frames.
- P50/P95/P99 de produção, consumo e apresentação reportados separadamente.
- Sem crescimento de latência/fila; buffer limitado e idade máxima conhecida.
- Atrasos e frames repetidos/descartados contabilizados e explicados.
- Tempo emulado e master ticks/frame preservados.
- Áudio sem underruns reais ou com política documentada e perceptualmente validada; contadores com semântica auditada.
- Testes repetíveis em mais de uma sessão e sem regressão funcional.

## 8. Correção de contexto da Issue #755

A descrição original da Issue #755 afirmava que a saída física de áudio ainda não estava conectada/ativa e que não havia evidência para investigar CPAL. Isso era correto para a evidência disponível naquele momento, mas ficou desatualizado após a captura pareada de 13/11/2026: CPAL abriu fisicamente em três execuções audio-active e produziu métricas de consumo/underrun.

A atualização não muda o diagnóstico visual: CPAL não foi demonstrado como causa da instabilidade de vídeo. Ela cria uma trilha de investigação de áudio independente. A existência de contadores de underrun ainda não prova dropout audível nem causa-raiz.

## 9. Prompts para consulta externa independente

### 9.1 Prompt para Claude ou Manus — pesquisa independente

Copie o texto abaixo para outro assistente. Anexe este dossiê e, se possível, os CSVs/logs originais. Não forneça a ele uma solução preferida.

> Atue como engenheiro sênior independente especializado em emulação de consoles, sistemas de tempo real, Windows, renderização Vulkan/WGPU e sincronização A/V.
>
> Quero uma investigação independente de um problema de frame pacing em um emulador Sega SMS. Não assuma que a causa é o Scheduler, Slint, WGPU, Windows, áudio ou GPU. Não tente defender a arquitetura atual. Trate o dossiê e os artefatos anexados como evidência primária e diferencie rigorosamente fatos, inferências, hipóteses e dados ausentes.
>
> **Sintoma:** cadência visual irregular no desktop, historicamente perto de 50–59 FPS. O runtime emulado usa MachineScheduler como autoridade temporal; o host usa worker temporal, FrameMailbox, Slint e WGPU/Vulkan. A saída física CPAL foi ativada em uma captura pareada recente.
>
> **Dados que você deve considerar:** (1) quatro ROM workloads executados duas vezes por 600 frames através da máquina SMS offline, com registros JSONL idênticos por repetição, status EXECUTED_NO_ORACLE; (2) três pares de capturas Windows de 300 s, video-only vs audio-active; (3) PresentMon Composed: Flip, SyncInterval=1, FrameTime P50 ~8,6–8,9 ms e P95 ~13,0–13,7 ms; (4) worker processing P50 ~12,8–13,0 ms na maioria das execuções, P99 ~16,4–17,0 ms; worker interval P50 ~17,33–17,39 ms; Slint polling P50 ~17,5–17,7 ms; consumed interval P99 ~33 ms; mailbox replacements ~488–527/5 min; (5) uma anomalia isolada no par 2 video-only com processing P50 ~18,58 ms; (6) CPAL abriu em três execuções, 48 kHz stereo F32, output_error=None, mas contou ~3.835–3.942 underrun_events e ~607k–621k underrun_frames em 5 min; (7) não há correlação por frame sequence entre produção e apresentação física, e PresentMon events não provam frames novos únicos.
>
> **Tarefas:**
> 1. Faça uma análise causal independente e enumere pelo menos 5 explicações concorrentes, com evidências a favor/contra e teste que as discrimine.
> 2. Pesquise como RetroArch/Libretro, MAME, Dolphin, bsnes/higan e outros emuladores resolvem pacing, master clock, áudio/vídeo, VSync/VRR, timers, filas e jitter. Priorize documentação oficial, código-fonte, issues e discussões técnicas dos mantenedores. Cite links, versões/commits e trechos específicos; não invente detalhes.
> 3. Compare pelo menos três arquiteturas alternativas, incluindo: (a) worker com deadline absoluto e clock monotônico; (b) apresentação orientada por eventos versus polling; (c) sincronização de host baseada em áudio ou refresh do display; (d) fila limitada de 1–2 frames. Considere também se o loop de apresentação deveria ser separado do event loop GUI.
> 4. Analise os underruns CPAL: quais semânticas de contador poderiam gerar esses números? Que instrumentação e testes de loopback são necessários para provar dropout real? Não trate underrun_frames/consumed_frames como percentual de silêncio sem prova.
> 5. Proponha uma matriz de experimentos A/B, uma variável por vez, com métrica primária, duração, critério de sucesso, riscos e rollback.
> 6. Proponha caminhos de contorno se não houver correção local simples, sem exigir reescrever o runtime ou mover timing de hardware para a GUI.
> 7. Diga explicitamente o que você não consegue concluir com os dados atuais.
>
> **Invariantes:** determinismo; MachineScheduler como autoridade do tempo emulado; nenhum ajuste de ciclos por parte da GUI; nenhuma alteração especulativa em Z80/VDP/PSG; execução contínua e em blocos equivalentes; filas limitadas; sem crescimento contínuo de latência. É aceitável investigar 1–2 frames adicionais de latência se isso produzir cadência realmente uniforme.
>
> **Formato de resposta:** resumo executivo; análise causal; pesquisa comparativa com fontes; arquiteturas candidatas; matriz experimental priorizada; riscos; recomendação independente. Não faça commits nem proponha mudanças de código sem antes demonstrar a causa com medições.

### 9.2 Prompt para uma nova conversa ChatGPT — ataque independente das propostas externas

> Você é um revisor técnico independente. Sua tarefa é atacar criticamente o problema de frame pacing do Sega Emulator a partir do zero, sem viés de confirmação em favor do desenho atual e sem assumir que as propostas de outros assistentes estão corretas.
>
> Vou fornecer: (a) o dossiê de evidências P0; (b) respostas de Claude e/ou Manus; (c) fontes, links, patches ou propostas sugeridas por esses sistemas; (d) quando necessário, o estado atual do repositório.
>
> **Não comece implementando.** Primeiro reconstrua o problema a partir dos dados brutos e teste a consistência das conclusões. Separe fatos medidos, inferências, hipóteses, opinião e alegações externas não verificadas. Verifique as fontes primárias e versões atuais na web; não aceite citações ou afirmações dos outros modelos sem confirmação.
>
> **Seu trabalho é procurar uma solução, não preservar a solução existente.** Questione inclusive a escolha do worker, o uso de polling, o papel do Slint event loop, a FrameMailbox, o modelo de pacing do host, a política de VSync e a integração CPAL. Ao mesmo tempo, não proponha reescrever hardware validado sem evidência.
>
> 1. Refaça a análise causal com hipóteses concorrentes e testes discriminantes.
> 2. Faça revisão adversarial de cada proposta externa: mecanismo, evidência, premissas, pontos cegos, riscos, compatibilidade e como falsificá-la.
> 3. Procure alternativas que nenhum dos modelos tenha sugerido, inclusive arquiteturas de apresentação diferentes e estratégias usadas por emuladores maduros.
> 4. Determine quais dados faltam e qual é o experimento mínimo que mais reduz a incerteza.
> 5. Produza uma árvore de decisão: se o gargalo for worker/deadline, faça X; se for GUI/consumo, faça Y; se for WGPU/compositor, faça Z; se for áudio, siga trilha independente; se for estrutural, avalie contorno arquitetural.
> 6. Proponha plano incremental com gates, métricas, rollback e preservação do runtime.
> 7. Só recomende implementação quando a hipótese estiver sustentada. Se a evidência indicar que a arquitetura atual não atende ao objetivo, diga isso claramente e proponha alternativa tecnicamente fundamentada.
>
> **Restrições inegociáveis:** MachineScheduler continua autoridade temporal do hardware; a GUI não executa instruções nem controla ciclos; determinismo e equivalência contínua/em blocos; nenhuma fila ilimitada; nenhum frame skipping como máscara de desempenho; nenhuma alteração em Z80/VDP/PSG sem prova e testes direcionados.
>
> **Saída esperada:** (1) diagnóstico independente; (2) tabela comparativa das propostas externas; (3) propostas rejeitadas e motivo; (4) melhor solução e alternativas de contingência; (5) plano de experimentos; (6) plano de implementação condicionado aos resultados; (7) lista explícita de incertezas remanescentes.
>
> Não trate a conclusão anterior do projeto como autoridade. Use-a apenas como hipótese a ser desafiada.

## 10. Artefatos e rastreabilidade

- Captura ZIP: `audio-paired-20261113-143303.zip` (artefato externo anexado à conversa; preservar original).
- Script de captura: `captura_p0_audio_pareada.ps1`.
- PresentMon: 2.6.0.
- Commit sob teste: `d093037bd5fbfef1278d4980f775efa2c549171c`.
- ROM hash: `434A4B34F87FA96B52D94EF4005A7D0D3D654D3B6594BFD7AA45A27FA09B306E`.

Ao anexar artefatos em novas execuções, registrar SHA-256 de ZIP, CSVs, logs e executável; não sobrescrever capturas anteriores. Relatórios futuros devem incluir build SHA, configuração, ambiente, processo/PID, duração efetiva e códigos de saída.
