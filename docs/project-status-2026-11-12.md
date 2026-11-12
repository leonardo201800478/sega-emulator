# Auditoria geral e plano de continuidade — 2026-11-12

**Repositório:** https://github.com/leonardo201800478/sega-emulator  
**Branch de documentação:** `docs/audit-project-status-2026-11-12`  
**Main auditada:** `f5a0b691824322ce56c1a921329d3aa0957c4963`  
**Escopo:** PRs abertas, issues abertas, workflows associados aos HEADs conhecidos, documentação normativa e estado das trilhas de áudio, Z80 e migração desktop.

> Esta é uma fotografia remota do GitHub. CI PASS significa apenas que os jobs executados passaram no SHA indicado. SKIPPED/queued/NOT RUN não são PASS. Validação física só é considerada realizada quando há evidência de execução no hardware/ambiente correspondente.

## 1. Resumo executivo

A main está atualizada até o merge da PR #774 (H3 — PCM SMS/SG-1000 com taxa interna explícita de 48 kHz). O backend Rust, o MachineScheduler e o runtime SMS/SG-1000 devem continuar preservados.

**Entregas integradas relevantes:**
- PR #747: POC Slint 1.18 + WGPU 30 + Vulkan (R1).
- PR #752: integração de frames reais SMS no desktop (R2).
- PR #754: preferências persistentes do desktop.
- PR #770: saída física CPAL 0.18.2.
- PR #774: fonte PCM interna SMS/SG-1000 em 48 kHz, taxa de entrada/saída explícita e prova de que a taxa PCM não altera a progressão do Scheduler.

**Conclusão principal:** a base de runtime e as entregas incrementais estão integradas, mas R2 ainda tem aceite manual incompleto; a estabilidade de frame pacing é P0; CPAL está integrado em software, mas validação física prolongada, hotplug e formatos Hi-Fi adaptativos ainda não estão demonstrados; SDL3 e R5 permanecem pendentes.

A nova política de trabalho é integração contínua: código validado deve ser incorporado à main em incrementos curtos. Não manter branches antigas abertas por conveniência. Para branches divergentes, portar apenas alterações úteis para uma branch atual, validar no HEAD atual e integrar; nunca fazer merge cego de histórico antigo.

## 2. Baseline e entregas integradas

| Área | Estado | Evidência / limite |
|---|---|---|
| Main | ATUAL | `f5a0b691824322ce56c1a921329d3aa0957c4963` |
| R1 — POC Slint/WGPU/Vulkan | MERGED; execução física reportada | PR #747; RTX 5070/Vulkan, device/queue compartilhados, textura animada e composição reportados. Recriação in-process/teardown instrumentado ainda exige evidência adicional. |
| R2 — frames SMS no desktop | MERGED; aceite parcial | PR #752. Preservar SmsTemporalWorker, FrameMailbox e Scheduler. Issue #756 ainda aberta. |
| R2 — preferências | MERGED | PR #754. |
| CPAL 0.18.2 | MERGED | PR #770; integração de saída física. Não equivale a campanha completa de hotplug/execução prolongada. |
| H3 — PCM 48 kHz | MERGED | PR #774; taxa-fonte explícita e resampler com taxas explícitas; teste de invariância do Scheduler. |
| H2 — pesquisa de reconstrução | CONCLUÍDA COMO INVESTIGAÇÃO | H2 não aprovou DSP alternativo para produção. Renderer event-based e variantes permanecem test-only. |
| A1 — semântica PSG SMS | PARCIAL | Pendências de oráculo para períodos 0/1, fase absoluta do ruído, Noise Rate=Tone 2 e validação física. |
| Z80 E7 | NÃO DECLARADO FECHADO | Scorecard/matriz têm fotografias históricas e precisam ser reconciliados com a main atual antes de qualquer avanço. E8 permanece bloqueado. |

### 2.1 Áudio — estado consolidado

- H2: o protótipo event-based de 65 taps/256 fases apresentou ringing/overshoot excessivos; windowed-sinc 8 pontos e Lagrange3/4 também não foram aprovados para produção. Não substituir a reconstrução atual sem A/B reproduzível e aprovação objetiva/auditiva.
- PR #772: modelagem matemática/event reconstruction adicional, HEAD `2ec2d54a80de0d142a8868552c7c82abb74a903f`; os workflows H2, CI e SMS Debugger Runtime terminaram SUCCESS. Porém a branch está 29 commits à frente e 70 atrás da main, `mergeable=false`. A PR declara explicitamente que não altera síntese de produção. Não fazer merge direto; portar testes/artefatos úteis para branch atual se a revisão confirmar valor.
- PR #767: A1, HEAD `670809184f76f63646c238f76c27b18ee867b44e`; CI e SMS Debugger Runtime SUCCESS no HEAD consultado. Branch 40 à frente/128 atrás, `mergeable=false`, Draft. Pendências: oráculo para períodos 0/1; fase inicial e pós-escrita do LFSR; alinhamento elétrico absoluto Noise Rate=Tone 2; validação física SMS.
- CPAL #770 e H3 #774 estão integrados. H4 ainda não possui implementação de produção/PR integrada. A política aprovada pelo usuário é suportar PCM16 e formatos de maior precisão quando realmente anunciados pelo endpoint (I24/F32 e outros formatos suportados), preservar precisão interna e negociar taxa/formato em conjunto. Fonte SMS/SG-1000 permanece 48 kHz; conversão para endpoint 88,2/96 kHz não cria informação adicional.
- Decisão futura multissistema aprovada: núcleo digital SN76489 compartilhado com perfis de placa/clock/roteamento/saída específicos para SG-1000, SMS, Game Gear (estéreo próprio) e Mega Drive (PSG integrado ao VDP e mixado com YM2612). Implementação adiada até fechamento das frentes atuais.

## 3. PRs abertas — inventário e decisão

A consulta identificou as seguintes PRs abertas. A contagem de commits/behind é comparativa com a main consultada; não significa que todos os commits sejam úteis ou devam ser preservados como uma única unidade.

| PR | Situação observada | Diagnóstico e ação |
|---|---|---|
| #772 — H2 mathematical/event model | CI SUCCESS; 29 ahead / 70 behind; diverged; não mergeable | Revisar patch e portar somente testes/relatórios úteis para branch atual. Manter DSP experimental fora da produção. |
| #767 — PSG A1 | CI SUCCESS; Draft; 40 ahead / 128 behind; diverged | Resolver oráculos e validação física. Não declarar A1 fechado nem fazer merge da branch antiga. |
| #769 — isolamento D3D12 opt-in | Draft; 16 ahead / 126 behind; CI FAIL | Falha de rustfmt em `native/video_runtime/src/d3d12.rs`; falha de Clippy Windows porque `windows v0.62.2` aparece com aliases/names diferentes; SMS runtime também falha no fmt. Reimplementar em incremento curto sobre main e validar as duas combinações (desktop Slint sem D3D12; shell legado com feature). |
| #758 — preservar SpriteLimitMode no reset | 1 commit; CI estava QUEUED; compare diverged por base antiga | Patch pequeno e localizado em `worker.rs`, com teste de regressão. Integrar assim que CI do HEAD atual ficar verde; não aceitar apenas `mergeable=true`. |
| #757 — aspect ratio viewport | 1 commit; CI estava QUEUED; compare diverged por base antiga | Patch altera `image-fit: fill` para `contain`. CI e validação Windows/Vulkan do resize/modos 4:3, 1:1 e 16:9 ainda necessários. |
| #749 — documentação/lockfile R1 | Draft stacked sobre PR #747 já merged | Não integrar como PR independente. Recuperar algum conteúdo ainda útil, se houver, e encerrar sem merge. |
| #746 — FrameMailbox portátil | CI SUCCESS no HEAD antigo; 2 commits; branch muito atrás | Alteração pequena de API worker portátil. Conferir se o contrato ainda falta na main/R2; se necessário, portar os poucos hunks para branch nova e executar CI atual. Não mergear histórico antigo. |
| #744 — FPS fallback no shell egui | Draft; 103 ahead / 222 behind; CI FAIL | Clippy aponta método `dimensions` morto e `if_not_else`. Além disso, destino egui é legado; só portar a telemetria se houver requisito equivalente no Slint e fonte de FPS válida. |
| #645 — scheduler + cartridge SRAM | 123 ahead / 387 behind; diverged | Grande integração de scheduler, mídia/save e SRAM. Sobrepõe #643 e toca caminho temporal. Não integrar em bloco; separar feature SRAM, auditar mapeamento e dependências e validar contra issue #642. |
| #644 — diretórios configuráveis | 24 ahead / 387 behind; diverged | Implementação no shell antigo. Portar somente política/serviço reutilizável se ainda necessário no Slint; não trazer UI egui antiga. |
| #643 — cartridge SRAM | 116 ahead / 387 behind; diverged | Sobrepõe #645. Auditar implementação de mapper/SRAM e portar em mudanças pequenas; preservar evidências e evitar dupla implementação. Issue #642 permanece aberta. |
| #639 — otimização Scheduler | 6 ahead / 387 behind; diverged | Alteração temporal sensível. Não portar antes do diagnóstico P0 #755; medir equivalência e overflow/stale-preview no HEAD atual. |
| #619 — auditoria documental de cartucho | 6 ahead / 203 behind; diverged | Documentação. Reaproveitar apenas achados ainda válidos após revisão; atualizar em branch atual ou fechar como redundante. |
| #533 — contrato permanente de conclusão | 1 ahead / 869 behind; diverged | Documento isolado e muito antigo. Comparar com instruções/ADRs atuais; não integrar sem reconciliação editorial. |

**Regra para todas as PRs divergentes:** não fazer merge direto, rebase destrutivo ou force-push. Preservar branch/commits históricos, extrair as mudanças úteis e recriá-las sobre a main atual com CI próprio. Fechar PRs antigas somente depois de confirmar que nenhum trabalho único foi perdido ou já integrado.

## 4. Issues abertas — triagem

### 4.1 Prioridade imediata

**#755 — frame pacing / MachineScheduler (P0).** Sintoma relatado em Windows x64/Vulkan: 50–59 FPS, média perto de 57 e cadência visual irregular. A issue corretamente não atribui o problema ao áudio, pois o caminho físico não estava ativo no build observado. Agora que CPAL está integrado, a causa ainda precisa ser isolada: medir Scheduler, worker, publicação/consumo FrameMailbox e submit/present WGPU separadamente. Instrumentar timestamps monotônicos e p50/p95/p99 por pelo menos 5 minutos. Não adicionar sleeps, throttling ou clocks na GUI.

**#756 — aceite R2.** Itens reportados: proporções/viewport, ROM ZIP com validação segura, sprite-limit, layout de menus, clique para retomar, F11/fullscreen. PR #757 endereça apenas fit/aspect; PR #758 endereça preservação da política de sprite após reset. O texto da issue contém afirmações antigas sobre ausência de API pública de sprite-limit; revalidar contra a main atual antes de manter essa subpendência literalmente. Fechar cada item somente com teste automatizado aplicável e validação manual Windows/Vulkan.

### 4.2 Bugs/funcionalidades de runtime que permanecem no backlog

- #642 — mapper SRAM/battery saves; correlacionar com #643/#645 e validar capacidade/perfil, persistência e reset.
- #641 — corrupção de tiles/sprites no menu de save do Phantasy Star; investigar VDP/memória e capturar reprodução determinística.
- #545 — visibilidade de escrita CRAM intrasscanline; revalidar se já foi resolvida por mudanças posteriores.
- #335 — input/perfis de periférico; reavaliar em conjunto com a futura auditoria SDL3.
- #158/#159 — auditorias Z80 SMS; comparar com scorecard/matriz atuais, sem duplicar entregas integradas.
- #312 — rework de tracing de boot BIOS; proposta histórica empilhada em base antiga.
- #309/#310/#311 — PAUSE, catálogo de hardware e versionamento; reavaliar escopo contra arquitetura atual.
- #374/#375/#376 — propostas históricas de VDP temporal/host D3D12; não presumir compatibilidade com Slint/WGPU nem reabrir runtime validado.
- #122 — migração Clean Architecture/Rust SG-1000; conferir se ainda representa trabalho ativo ou histórico.

### 4.3 Issues de áudio com backends substituídos

- #640 (underruns WASAPI) e parte de #659 (WASAPI/XAudio2) referem-se aos backends substituídos pela integração CPAL. Preservar como evidência histórica; reproduzir novamente no CPAL antes de fechar ou transportar correções.
- Não classificar o problema como resolvido apenas porque o backend antigo foi removido: sintomas semelhantes precisam ser testados no caminho atual CPAL.

## 5. Migração desktop — estado e sequência

| Fase | Estado real |
|---|---|
| R1 | POC integrada (#747) e execução física inicial reportada; complementar evidências de teardown/recriação e manter o escopo mínimo. |
| R2 | Frames reais SMS e preferências integrados (#752/#754); aceite funcional ainda incompleto (#756), com #757/#758 aguardando CI no HEAD observado e #746 exigindo conferência/portabilidade. |
| R3 | CPAL 0.18.2 integrado (#770) e fonte PCM 48 kHz consolidada (#774); completar validação física prolongada, formatos, device removal/reconnect, underruns e telemetria. |
| R4 | PENDENTE — auditoria e integração SDL3 para teclado/gamepads/hotplug. |
| R5 | PENDENTE — consolidar launcher, biblioteca, configurações, HUD/logs/telemetria em Slint; remover hosts anteriores somente após inventário de consumidores e aceite. |

A GUI continua sem autoridade temporal. O runtime, MachineScheduler, SmsTemporalWorker e contratos de frame/áudio permanecem donos da execução e dos estados.

## 6. Z80 e fidelidade temporal

Os documentos `docs/z80-completion-scorecard.md` e `docs/z80-dd-fd-semantic-matrix.md` contêm baselines históricas que não correspondem ao SHA atual da main. Antes de qualquer nova mudança, reconciliar seus critérios com o código/testes/workflows atuais e registrar o SHA exato.

- Não declarar E7 fechado sem demonstração de todas as pendências semânticas relevantes.
- Não iniciar E8/ZEXDOC/ZEXALL antes do fechamento demonstrável de E7.
- Não exigir precisão elétrica/M-cycle para caminhos sem consumidor observável; priorizar I/O, IRQ/NMI, HALT, VDP/PSG e interações de gameplay.
- Não reabrir E4C/E5/E6 já validadas sem defeito ou regressão concreta.

## 7. Política de integração contínua aprovada

1. Integrar na main em incrementos pequenos assim que o código estiver validado.
2. Manter branches curtas e atualizadas; evitar acumular features independentes numa branch.
3. Para branch antiga/divergente, preservar o histórico original e portar apenas os hunks/commits úteis para branch nova baseada na main atual.
4. Exigir fmt, Clippy, testes direcionados, regressões pertinentes e CI no HEAD exato.
5. PASS é específico ao SHA e ao ambiente; queued, skipped, cancelled ou not run não são PASS.
6. Corrigir bugs após integração quando necessário, com teste de regressão e correção direcionada; não sacrificar invariantes para acelerar merge.
7. Não promover protótipos/test-only para produção sem critérios próprios e evidência independente.
8. Atualizar documentação na mesma sequência de integração, não no final de uma longa fase.

## 8. Próximas etapas — ordem recomendada

1. **P0 temporal (#755):** instrumentar e reproduzir frame pacing; separar tempo de Scheduler, worker, mailbox e apresentação WGPU. Resultado esperado: relatório objetivo, sem correção especulativa.
2. **R2 quick wins:** revalidar #758 e #757 no HEAD atual; corrigir qualquer conflito; integrar assim que CI verde. Conferir #746 e portar somente API portátil ainda ausente. Avançar os itens restantes de #756 em PRs pequenas (ZIP seguro, menus, resume, F11).
3. **A1 PSG:** resolver conflitos de oráculo e fase de ruído; validar Noise Rate=Tone 2 e hardware físico quando disponível. Manter PR #767 como fonte de evidência, não como branch para merge integral.
4. **H2 follow-up:** revisar #772 e decidir quais testes/artefatos acrescentam valor além da investigação H2 já registrada; portar seletivamente e integrar com CI atual. Não alterar DSP de produção.
5. **R3/H4 áudio:** após caracterizar a estabilidade temporal, completar formatos adaptativos (PCM16 sempre suportado como fallback; I24/F32 quando suportados), taxas, conversão 48→endpoint, lifecycle/hotplug e campanha física; callback sem locks/alocação e sem impacto no Scheduler.
6. **R4 SDL3:** auditoria de APIs/versão/licença e integração isolada de input/gamepads.
7. **R5:** paridade funcional Slint, telemetria, biblioteca/launcher/configurações e retirada dos hosts anteriores com evidência.
8. **Z80 E7:** atualizar scorecard/matriz com o HEAD atual e fechar apenas gaps comprovados; E8 permanece bloqueado.
9. **PSG multissistema:** somente depois das frentes anteriores; core SN76489 comum, perfis de placa próprios e integração GG/MD sem apagar características de cada console.

## 9. Limitações desta auditoria

- Não houve build local/Windows executado nesta conversa.
- A CI de #758 e #757 estava QUEUED no momento da consulta.
- As validações físicas adicionais de R1/R2/R3 não foram executadas nesta auditoria.
- Issues antigas foram triadas por metadados e descrição; não se declara que cada bug foi reproduzido no HEAD atual.
- Branches antigas não foram apagadas, rebased ou integradas.
