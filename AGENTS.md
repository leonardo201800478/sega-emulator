# Instruções para agentes de código — Sega Emulator

Decisões vigentes: ADR 0010 (runtime/hardware), ADR 0011 (desktop/hosts) e ADR-0016 (fidelidade temporal). O roadmap mestre define sequência e gates. GitHub é a única fonte da verdade.

## Arquitetura

- Backend/runtime 100% Rust: CPU, barramentos, memória, mappers, VDP, áudio, interrupções, Scheduler, timing e estados.
- Desktop alvo: Slint 1.18.x + WGPU 30.x + CPAL 0.18.2; Vulkan inicial preferencial no Windows x64; SDL3 após auditoria.
- egui/eframe e D3D12/WASAPI/XAudio2 são baseline integrada; preservar até paridade e aceite da nova stack.
- Preservar SmsTemporalWorker, FrameMailbox, Scheduler, SMS, SG-1000 e componentes validados.
- Não duplicar CPU, worker, máquina, PSG, VDP ou Scheduler.
- GUI exclusivamente apresentação/integração OS. Sem hardware, instruções, clocks ou callbacks por instrução.
- Comunicação coarse-grained: load_rom/start/pause/stop/reset/step_frame e snapshots/frames com ownership explícito.
- Proibidos JIT, frame skipping, hacks temporais e otimizações especulativas.

## R1–R5

1. R1: POC isolada Slint/WGPU/Vulkan; janela, device/queue compartilhados, textura externa atualizada e composição. Sem SMS real, menus, HUD, CPAL ou SDL3.
2. R2: frames reais SMS, preservando worker e Scheduler.
3. R3: CPAL 0.18.2, dispositivos, formatos, streams e underruns.
4. R4: auditar/integrar SDL3.
5. R5: frontend Slint completo e corte da baseline após paridade.

## Método

Auditar SHA/main, PRs, branches e workflows; pesquisar documentação oficial e referências primárias; verificar versões, APIs e licenças; definir critérios; modelar estados/ownership/timing; implementar incrementos pequenos; executar fmt, Clippy e testes direcionados antes do workflow; regredir; auditar diff; documentar; publicar por PR.

Para tarefas complexas, confrontar três frentes: engenharia reversa, documentação primária e arquitetura/modelagem. Não inventar resultados de agentes ou testes.

## Fidelidade e desempenho

MachineScheduler é a autoridade única dos clocks/eventos. Preservar ordem causal, determinismo, WAIT states e estado necessário à retomada. Execução contínua e em blocos deve ser equivalente. UI, renderização e áudio físico nunca controlam ciclos. Aplicativos e avaliações de desempenho somente Release.

## Gates

- cargo fmt --all -- --check
- cargo clippy --release --workspace --all-targets --locked -- -D warnings
- cargo test --release --workspace --locked

Validar workspaces/targets standalone separadamente. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca é PASS. Testes físicos não executados permanecem NOT RUN. CI verde só valida jobs executados no SHA correspondente.

## GitHub e privacidade

Toda alteração em branch e PR. Proibidos commit direto na main, force-push e reset destrutivo. Não fazer merge sem CI aplicável verde no HEAD final e autorização explícita. Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.

Ao receber "Prossiga", auditar main/PRs/CI, identificar a última tarefa concluída, resolver pendências e avançar até conclusão ou bloqueio real, sem duplicar trabalho.
