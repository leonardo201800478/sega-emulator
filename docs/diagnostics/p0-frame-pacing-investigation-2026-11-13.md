# P0 — Frame pacing: investigação da regressão após MachineScheduler

**Issue principal:** #755  
**Baseline de investigação:** main em `744ebe8e55cbebe731e6393f992a2970d79a0aa1`  
**Data:** 2026-11-13  
**Estado:** INVESTIGAÇÃO BLOQUEADORA — nenhuma expansão funcional até a causa raiz ser corrigida e validada.

## Mandato

Investigar a perda de velocidade/cadência observada no SMS após a introdução do `MachineScheduler`. Outros emuladores funcionam normalmente no mesmo equipamento e o Sega Emulator já executou anteriormente em velocidade estável. O usuário observou aproximadamente 55–59 FPS no HUD, com fotografias de 57,7, 58,1 e 59,6 FPS VDP.

A investigação deve localizar a primeira mudança causal no histórico e separar rigorosamente tempo virtual, processamento do worker, espera/pacing, mailbox/UI e apresentação física.

## Política de congelamento

Até a issue #755 ser encerrada com evidência:

- Não iniciar novas funcionalidades, migrações ou expansões de sistemas.
- Não alterar CPU/Z80, VDP, PSG, barramentos, clocks regionais, WAIT states ou semântica do MachineScheduler para compensar atraso de host.
- Não aplicar sleeps, throttling, busy-wait, VSync feedback ou temporização na GUI como correção empírica.
- Não fazer otimizações especulativas no Scheduler.
- Mudanças permitidas limitam-se à investigação, instrumentação diagnóstica não intrusiva, testes causais e correção mínima após identificação da causa.

## Evidência de captura disponível

A captura pareada de aproximadamente cinco minutos contém os cenários CPAL-active e video-only. O modo video-only desabilita somente a sessão física CPAL; a geração digital do PSG permanece ativa.

A telemetria registra, entre outros:

- Clock de máquina: 3.579.545 Hz.
- Orçamento de frame reportado: aproximadamente 16.687 µs.
- Ticks por frame na faixa aproximada 59.717–59.755; mediana próxima de 59.735–59.736.
- Na janela móvel final de 512 observações, o intervalo do worker p50 foi aproximadamente 17.324 µs no cenário áudio ativo e 17.415 µs no vídeo-only.
- Na mesma janela, o processamento completo de frame ficou aproximadamente em 11.140 µs (áudio ativo) e 13.664 µs (vídeo-only), p50.
- A espera observada e a lateness de timeout devem ser analisadas como distribuições separadas; não assumir que o atraso de `recv_timeout()` sozinho explica todo o déficit.
- Mailbox replacement foi observado nos dois cenários.
- O host Slint é dono do caminho surface/present. A instrumentação declara `physical_present_timestamp=unavailable_slint_owned`; portanto não há timestamp físico de apresentação disponível nesta coleta.

**Interpretação limitada:** os ticks/frame observados são estáveis e compatíveis com o perfil NTSC configurado, enquanto o intervalo de parede do worker excede o orçamento. O cenário vídeo-only não recupera a cadência. Isso direciona a investigação ao caminho de pacing/worker/host, mas não prova a causa raiz nem inocenta cada componente do Scheduler.

As estatísticas acima são da janela móvel final de 512 observações, não devem ser apresentadas como médias globais dos cinco minutos. A análise final deve consultar e preservar os logs brutos do ZIP original.

## Hipóteses que precisam ser testadas — não assumir

1. Deadline relativo/absoluto criado ou atualizado incorretamente; deriva acumulada.
2. Uso de `recv_timeout()` e atraso de despertar/retomada no Windows.
3. Custo variável de `step_frame_until()` ou dispatch do Scheduler.
4. Relação entre master ticks, quantidade de amostras PSG e orçamento `sample_count / sample_rate`.
5. Trabalho de command handlers antes do frame seguinte, inclusive polling após deadline expirado.
6. Backpressure, coalescing e consumo da FrameMailbox.
7. Cadência de polling/callback Slint e caminho de apresentação WGPU/PresentMon.
8. Overhead da instrumentação/logging.
9. Diferenças introduzidas em commits específicos durante a integração do Scheduler.

## Plano de investigação obrigatório

### A. Reconstrução histórica

- Encontrar o último commit em que a execução SMS foi comprovadamente estável.
- Identificar commits que introduziram MachineScheduler no caminho SMS e commits posteriores que alteraram worker, deadlines, PSG sample budgeting, mailbox ou host.
- Comparar a semântica anterior e atual, por commit e por função.
- Usar bisect/reversão experimental somente em branches isoladas; nunca reescrever main.
- Identificar o primeiro commit que reproduz a regressão, se o histórico e fixtures permitirem.

### B. Auditoria estática Rust

Auditar ownership/threading, sincronização, monotonicidade de deadlines, overflow/conversões inteiras, duração de amostras, tratamento de comando, estados pause/resume/reset/load, encerramento de worker, mailbox e fronteiras host/runtime. Para cada achado registrar arquivo, linha, função, caminho causal, severidade e evidência.

### C. Instrumentação e experimentos causais

Correlacionar timestamps monotônicos/QPC de:

- início/fim de step do Scheduler;
- ticks virtuais antes/depois de cada frame;
- amostras PSG por frame e orçamento derivado;
- início/fim de processamento;
- deadline planejado, entrada/retorno da espera e wake lateness;
- comando e duração do handler;
- publicação, replacement, retirada/consumo;
- callback Slint, conversão e upload;
- PresentMon/QPC, quando suportado.

Executar matrizes A/B mantendo ROM, região, estado inicial, duração, display e build idênticos. Isolar cada variável; não combinar foco, resize, V-Sync ou G-SYNC em uma mesma comparação causal.

### D. Correção e validação

Só propor patch após hipótese ser confirmada por experimento reproduzível. O patch deve ser mínimo, reversível e acompanhado por:

- teste de regressão que falhava antes e passa depois;
- equivalência de execução contínua versus blocos;
- igualdade de ticks/estado virtual;
- ausência de deriva acumulada após overrun;
- testes pause/resume/reset/load e comandos durante espera;
- regressões SMS e SG-1000 pertinentes;
- fmt, Clippy, testes workspace e builds Windows x64 aplicáveis;
- captura física pareada antes/depois, com logs brutos preservados.

## Critérios de saída do bloqueio

- Cadência de worker compatível com a frequência regional nominal em regime estável.
- Ausência de déficit sistemático próximo de 4%.
- Tempo virtual e determinismo preservados.
- Diferença entre produção, consumo e apresentação explicada por métricas.
- Nenhum underrun persistente inexplicado no cenário de referência; problemas de áudio devem ser reportados separadamente.
- Testes automatizados e CI aprovados no SHA final.
- Relatório antes/depois com commit causal, patch, logs e limites de medição.

## Entregáveis

1. Linha do tempo Git e identificação do primeiro commit suspeito.
2. Mapa de fluxo temporal do SMS, do Scheduler ao present.
3. Matriz de hipóteses com evidência que confirma/refuta cada uma.
4. Relatório de auditoria Rust com achados classificados.
5. Patch de correção mínimo, somente após confirmação.
6. Relatório de validação e decisão explícita de liberar ou manter o bloqueio.

**Regra de conclusão:** compilar, abrir o emulador ou observar 60 FPS por alguns segundos não fecha P0. A correção precisa sustentar a cadência e preservar o determinismo.
