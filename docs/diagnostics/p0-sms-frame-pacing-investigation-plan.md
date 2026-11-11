# P0 — Plano de testes e investigação do frame pacing SMS

**Issue:** #755 — investigação prioritária de frame pacing e gerenciamento temporal

**PR relacionada:** #759 — telemetria diagnóstica do worker SMS

**Prioridade:** P0

**Estado:** plano de investigação; nenhum resultado de execução é presumido.

## Objetivo

Reproduzir e isolar a oscilação de cadência observada no SMS (aproximadamente 50–59 FPS, média reportada próxima de 57 FPS), distinguindo:

1. progressão do tempo virtual (MachineTime, Scheduler, Z80, PSG e VDP);
2. cadência de parede do worker SMS;
3. publicação, substituição e consumo de frames;
4. submissão/renderização/apresentação física;
5. transporte PCM e consumo do endpoint de áudio, quando conectado.

A investigação deve identificar uma causa reproduzível antes de propor uma correção. O sintoma não deve ser atribuído previamente ao Scheduler, Windows timer, PSG, mailbox ou WGPU.

## Hipótese prioritária a testar

O worker calcula um orçamento de parede por frame a partir da quantidade de amostras PSG e aguarda o saldo por recv_timeout. Um comando recebido durante essa espera pode acordá-la. O fluxo atual deve ser testado para determinar se, após tratar um comando não-pausante, o worker inicia outro frame sem preservar o saldo do orçamento anterior.

Essa é uma hipótese sobre a cadência do host, não uma autorização para alterar o tempo virtual. O atraso de despertar do sistema operacional e o custo de processamento também precisam ser medidos separadamente.

## Regras arquiteturais obrigatórias

- MachineScheduler permanece como única autoridade do tempo virtual.
- Nenhum timer, sleep, deadline ou callback da GUI pode avançar, corrigir ou limitar ciclos de hardware.
- Não alterar clocks regionais, raster, Z80, VDP, PSG, barramentos ou WAIT states para alcançar 60 FPS.
- Não usar frame skipping, repetição artificial, throttling ou VSync/VRR como correção do tempo emulado.
- A saída de áudio e a apresentação gráfica não podem retroagir sobre os ticks do Scheduler.
- Preservar equivalência entre execução contínua e execução em blocos, determinismo, save states e replay.
- Instrumentação deve ser limitada, separada por domínio de relógio e avaliada quanto ao próprio overhead.

## Plano de testes

### T1 — Regressão do comportamento de wakeup do worker

Criar um teste determinístico do fluxo real de pacing, sem depender de sleeps frágeis ou de temporização física do Windows. Preferir abstração injetável de relógio/espera ou um harness que permita controlar o instante de chegada e a duração do handler.

Casos obrigatórios:

- Sem comando: o worker respeita o orçamento completo, descontado o processamento.
- Comando não-pausante no meio da espera (input/controller): verificar se o frame seguinte começa antes do deadline original.
- Comando de debug/snapshot: verificar o mesmo comportamento e o custo do handler.
- Comandos enfileirados: verificar drenagem da fila e deadline preservado.
- Pause, Stop, Reset e LoadRom: verificar que a transição de estado interrompe a execução conforme o contrato, sem reutilizar prazo de uma execução anterior.
- Fila cheia/desconectada: verificar rejeição/encerramento sem perda de invariantes.

Critério: o teste deve demonstrar o instante lógico de início do próximo frame em relação ao deadline. Não basta testar apenas que recv_timeout retorna Ok ou Timeout.

### T2 — Política de deadline absoluto (avaliação, não implementação presumida)

Depois de T1 reproduzir o comportamento, avaliar uma política de deadline absoluto monotônico:

- deadline de cada frame derivado de uma época de pacing independente do Scheduler;
- comandos não-pausantes não reiniciam nem eliminam o saldo do prazo;
- comandos de pausa/reset/load invalidam a época antiga e iniciam uma nova quando aplicável;
- se o processamento ultrapassar o prazo, registrar overrun e definir explicitamente a política de recuperação, sem deslocamento silencioso da época;
- não acumular dívida ilimitada nem descartar avanço virtual.

A correção só deve ser implementada em PR de código separada ou em escopo claramente revisado após evidência de T1/T3. Não assumir que deadline absoluto, isoladamente, resolve o sintoma global.

### T3 — Medição do worker em Release

Executar por pelo menos cinco minutos por cenário, com build Release, ROM, região, configuração e hardware registrados.

Coletar por frame:

- timestamp monotônico de início e conclusão;
- duração de step_frame_until;
- duração de publicação de vídeo;
- duração de processamento/submissão de áudio, se houver sessão;
- orçamento solicitado;
- instante de início/fim da espera;
- causa do despertar (timeout, comando, desconexão);
- tipo e duração do handler de comando;
- intervalo entre conclusões;
- ticks virtuais por frame.

Relatar p50/p95/p99, mínimo, máximo, média, desvio e histogramas. Comparar execução sem comandos com comandos de input/debug em frequências controladas.

### T4 — Temporizador e despertar do Windows

Em Windows x64, medir a resolução efetiva do temporizador e o atraso entre deadline solicitado e retorno efetivo. Comparar cenários controlados com resoluções de timer disponíveis no ambiente, sem assumir que a ausência de timeBeginPeriod prova causalidade.

Registrar versão do Windows, política de energia, estado de foco, backend gráfico e ferramentas usadas. A comparação deve correlacionar o atraso real de despertar com intervalos de frame, não apenas com FPS agregado.

### T5 — Invariantes do tempo virtual

Executar os testes existentes e adicionar cobertura direcionada quando faltar:

- Scheduler monotônico e ordem causal de eventos;
- eventos simultâneos ordenados por timestamp, prioridade e sequência de inserção;
- avanço racional/chunked equivalente;
- replay determinístico a partir do mesmo estado inicial;
- PSG idêntico entre avanço contínuo e em blocos;
- raster/frame VDP invariável à partição dos avanços;
- contagem de ticks e boundaries regionais NTSC, PAL-M e PAL;
- overshoot de step_frame_until medido e limitado pela granularidade real de conclusão de instrução.

Não declarar cycle/dot accuracy a partir de testes de chunk invariance.

### T6 — Mailbox e apresentação

Medir separadamente:

- sequência de frames publicados;
- frames substituídos no mailbox;
- frames retirados pela GUI;
- lacunas entre sequências consumidas;
- duração de conversão RGB/RGBA;
- submissão de textura;
- render/present/displayed, somente quando capturados por ferramenta externa com semântica documentada.

O contador atualmente chamado FPS VDP não deve ser tratado como FPS físico sem evidência de present/scanout. O timer GUI de 16 ms é polling, não medição de refresh físico.

### T7 — Áudio isolado

Enquanto a saída física de áudio não estiver conectada nesta build, não atribuir o sintoma ao CPAL, callback ou underrun. Validar geração digital PSG e fase separadamente. Quando existir sessão física, comparar sink ausente, sink fake imediato e sink fake lento, garantindo que ticks/estado/samples digitais permaneçam iguais para a mesma sequência de entrada.

## Matriz mínima de cenários

| Cenário | Comandos | Áudio | Região | Duração |
|---|---|---|---|---|
| A | Nenhum | Desconectado | NTSC | 5 min |
| B | Input controlado | Desconectado | NTSC | 5 min |
| C | Debug/snapshot controlado | Desconectado | NTSC | 5 min |
| D | Input + debug | Desconectado | NTSC | 5 min |
| E | Nenhum | Desconectado | PAL | 5 min |
| F | Nenhum | Desconectado | PAL-M | 5 min |
| G | Nenhum | Sessão/sink controlado, quando disponível | NTSC | 5 min |
| H | Nenhum | Sessão/sink controlado, quando disponível | PAL | 5 min |

Repetir cenários críticos com janela em foco e fora de foco. Registrar taxa de atualização do monitor e VRR como condições do host, nunca como clocks do emulador.

## Critérios de aceite

- [ ] Teste determinístico reproduz ou refuta a perda de saldo de espera após comando não-pausante.
- [ ] Pause/Stop/Reset/LoadRom possuem comportamento de deadline explicitamente testado.
- [ ] Medições separam tempo virtual, worker, mailbox, áudio e apresentação física.
- [ ] Scheduler e runtime preservam determinismo e equivalência contínua/chunked.
- [ ] NTSC, PAL-M e PAL mantêm os clocks/raster configurados, sem forçar 60/50 FPS.
- [ ] Benchmark Release de cinco minutos por cenário foi executado e anexado.
- [ ] Captura física distingue publish/take/submit/present/displayed.
- [ ] Nenhuma regressão de SMS ou SG-1000.
- [ ] Relatório final identifica causa somente quando houver reprodução e correlação mensurável.

## Dependências e bloqueios

- A PR #759 fornece instrumentação diagnóstica útil, mas deve ser validada no seu HEAD exato antes de depender de suas métricas.
- Corrigir falhas de compilação, whitespace e workflow da PR #759 é trabalho de desbloqueio da telemetria, não correção do FPS.
- Testes locais, benchmarks Release e captura Windows/Vulkan ainda precisam ser executados; nenhum resultado é marcado PASS neste plano.

## Fora de escopo

- Corrigir o Scheduler sem evidência de defeito.
- Alterar clocks, raster ou semântica de CPU/VDP/PSG.
- Implementar CPAL ou SDL3 como parte desta investigação.
- Reescrever o frontend ou mudar a arquitetura de apresentação.
- Declarar a issue #755 resolvida apenas porque o indicador passa a mostrar 60 FPS.

## Entregáveis

1. Testes determinísticos de wakeup/deadline e transições de comando.
2. Relatório Release com dados brutos e estatísticas por cenário.
3. Captura Windows/Vulkan correlacionando worker e apresentação.
4. Conclusão causal com evidências, ou lista explícita das hipóteses ainda não resolvidas.
5. Se necessário, proposta de correção mínima e separada para revisão.
