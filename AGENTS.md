# Instruções para agentes de código — Sega Emulator

As regras normativas são a ADR 0010 (`docs/adr/0010-rust-only-sms-temporal-first.md`), a ADR-0016 (`docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md`) e o [roadmap mestre](docs/project-roadmap.md). A fotografia operacional atual fica em [docs/project-status-2026-11-07.md](docs/project-status-2026-11-07.md).

## Prioridade e arquitetura

- Produto 100% Rust. Não reintroduzir Python, PySide6/Qt, PyO3 ou Maturin em runtime, testes, tooling, scripts, CI ou distribuição.
- GUI nativa egui/eframe; apresentação e comandos coarse-grained, sem lógica de hardware.
- Único runtime ativo inicial: Master System temporal, sobre o Z80 compartilhado, SN76489 temporal, VDP temporal e MachineScheduler.
- `rust/runtime/sms-runtime` é o worker SMS temporal comum. Não duplicar worker, máquina, CPU, PSG, VDP ou Scheduler.
- O shell egui está na main; a integração vertical SMS/vídeo/áudio/input está em PRs abertas #613/#614 na fotografia de 2026-11-07. Não tratá-las como integradas antes do merge.
- SG-1000 atômico legado não é runtime alternativo nem fallback do SMS. Preservar regressões/evidências e seguir P2 somente após P1.
- A UI nunca executa hardware, avança clocks ou faz callbacks por instrução.
- Não usar JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Sequência obrigatória por tarefa

1. Auditar SHA/main, PRs, workflows, manifests e árvore relevante.
2. Pesquisar evidência independente e documentação primária; usar três frentes em tarefas complexas.
3. Definir critérios de aceitação mensuráveis.
4. Modelar estados, transições, eventos e ciclos necessários.
5. Implementar incrementos pequenos e compiláveis em branch.
6. Executar testes direcionados.
7. Executar regressão e gates aplicáveis.
8. Auditar diff, riscos e dependências.
9. Atualizar documentação e ledger de evidências.
10. Publicar por Pull Request; não fazer merge sem CI verde no HEAD final e autorização explícita.

## Fidelidade e desempenho

- MachineScheduler é a autoridade única dos clocks/eventos.
- CPU, barramento, PSG, VDP, áudio e vídeo preservam ordem causal e determinismo.
- WAIT, suspensão/retomada e snapshots devem refletir o estado real dos caminhos temporais ativos.
- Execução contínua e em blocos deve produzir estados/saídas equivalentes.
- Aplicar ADR-0016: precisão temporal detalhada nos caminhos em que ordem/duração altera comportamento observável; não impor microsequenciamento universal sem consumidor demonstrado.

## Gates obrigatórios

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Validar também targets Windows/D3D12/WASAPI/XAudio2 e workspaces standalone enquanto existirem. Testes físicos não executados são NOT RUN. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN e vincular ao SHA exato. SKIPPED nunca equivale a PASS.

## GitHub e publicação

- GitHub é a fonte da verdade.
- Toda alteração em branch e Pull Request.
- Proibido commit direto na main, force-push e reset destrutivo.
- Não fazer merge sem CI 100% verde no HEAD final e autorização explícita.
- Não declarar migração concluída por compilação isolada ou remoção de arquivos sem validação funcional.

## Relatório

Informar SHA base/final, branch/PR, arquivos alterados, testes realmente executados, CI, limitações e próximos passos. Nunca alegar execução local, teste físico, aprovação ou merge sem evidência.
