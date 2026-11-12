# Continuidade — Modern Hi-Fi PSG / H2

Data: 2026-11-12

## Estado Git confirmado

- Repositório: `leonardo201800478/sega-emulator`.
- Main: `b375d4bb85149678b82ab39a497177d1a1cd2319` (merge da PR #770, CPAL 0.18.2).
- Branch: `feat/modern-hifi-audio-foundation-20261112`.
- PR #771: fechada sem merge (`merged=false`), sem commits/arquivos alterados segundo a API. Não tratar como integrada nem tentar fazer merge dela.
- O HEAD reportado da PR #771 era igual ao SHA da main. Confirmar branch e HEAD reais antes de qualquer alteração.

## Evidências e conclusões preservadas

1. O SN76489 é compartilhado por SMS e SG-1000. Preservar os perfis, o estado digital, os clocks e o MachineScheduler. A GUI e o host de áudio não podem corrigir/avançar tempo de hardware.
2. H1 possui comparação A/B histórica registrada na PR #771: baseline `f47f71830344b85f72329376505d400bfbfaae69`, workflow `43260895841`, artefato `13962279974`. Valores reportados: 214.552 amostras por perfil a 48 kHz; NRMSE 3,9032% SMS e 3,8742% SG-1000; correlação 0,999235 e 0,999247; PCM contínuo e segmentado em blocos de 997 clocks idêntico. Revalidar artefatos e SHA antes de usá-los como aceite atual.
3. A interpolação linear atual tem atenuação espectral crescente nas bandas altas; diferença reportada até 4 kHz praticamente nula. Lagrange4 foi apenas experimental/test-only e não foi promovido à produção.
4. H2 investiga reconstrução event-based: capturar deltas de amplitude com timestamp no clock PSG e reconstruir por resposta de passo band-limited com fase subamostral. A hipótese usa kernel sinc/Kaiser integrado numericamente. Manter isolada de produção.
5. Resultado preliminar apresentado nesta sessão: NRMSE próximo de 3,07% SMS e 3,04% SG-1000 e correlação próxima de 0,99953. É dado exploratório, ainda não reproduzido/validado independentemente. Não alegar superioridade até repetir a experiência com CI e revisão do alinhamento temporal.
6. Ruído por perfil a preservar/validar:
   - SMS: LFSR 16-bit, taps brancos 0 e 3, período branco reportado 57.337, periódico 16.
   - SG-1000: LFSR 15-bit, taps brancos 0 e 1, período branco 32.767, periódico 15.
   Não substituir por LFSR maximal genérico sem evidência de hardware.

## Próximas ações obrigatórias

1. Inspecionar main/branch/PR no GitHub, confirmar HEADs e conteúdo; #771 está fechada sem merge.
2. Auditar implementação real do PSG: API, campos, divisor, fase, mixagem, drenagem e convenções temporais. Não presumir nomes de campos/métodos.
3. Consultar documentação técnica do projeto e SMS Power para cada afirmação de hardware.
4. Implementar o protótipo H2 em módulo de teste isolado, sem modificar a síntese de produção.
5. Revisar matematicamente timestamp racional, sample grid, atraso de grupo, bordas, deltas simultâneos, overflow e continuidade.
6. Testar períodos 2–1023 e casos 0/1; perfis SMS/SG-1000; tons, volumes, writes durante execução, ruído periódico/branco, reset e mudanças de controle.
7. Provar invariância contínua/segmentada em blocos 1, 7, 997, 16.384 e fronteiras de amostra.
8. Gerar WAV baseline/current/event-based e relatório JSON/Markdown em CI: NRMSE, correlação, FFT por bandas, fundamental/harmônicos, THD, aliasing, ringing, overshoot, undershoot e settling.
9. Reproduzir o NRMSE preliminar; semelhança com baseline não equivale automaticamente a fidelidade de hardware nem preferência auditiva.
10. Medir CPU, memória, latência e adequação a streaming.
11. Atualizar roadmap H0–H6 com evidências e gates objetivos.
12. Só após validação, criar PR nova baseada na main atual, executar CI, revisar diff e integrar. Não fazer cherry-pick/merge da PR #771 fechada sem inspeção.

## Limites

- Não alterar CPAL/host, GUI, Slint/WGPU, Scheduler ou runtime de vídeo nesta etapa.
- Não promover o protótipo event-based para produção sem aprovação dos gates.
- Não iniciar H3–H6 antes do fechamento demonstrável de H2.
- Compilar ou passar um teste sintético isolado não é critério suficiente para merge.
