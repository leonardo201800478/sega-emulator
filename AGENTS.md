# Instruções para agentes de código — Sega Emulator

As regras normativas são a [ADR 0010 — produto 100% Rust e SMS temporal](docs/adr/0010-rust-only-sms-temporal-first.md), a [ADR-0016 — fidelidade observável](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md) e o [roadmap mestre](docs/project-roadmap.md).

## Prioridade e arquitetura

- Decisão definitiva: produto 100% Rust. Não manter Python, PySide6/Qt, PyO3 ou Maturin, nem como legado versionado, tooling, testes ou fallback.
- Interface nativa egui/eframe. Escopo inicial: menus operacionais mínimos; não construir biblioteca visual ou frontend rico nesta etapa.
- Único runtime ativo inicial: Master System temporal, exclusivamente sobre o Z80 compartilhado, SN76489 temporal, VDP temporal e MachineScheduler.
- Desconectar e arquivar temporariamente o SMS atômico antigo e o SG-1000 atômico antigo fora do workspace/runtime. Preservar somente código Rust necessário à auditoria durante a extração.
- Eliminar o SG-1000 antigo depois que o SMS estiver executável. Só então criar um SG-1000 novo baseado no Z80 compartilhado.
- Não criar fallback nem duplicar CPU, PSG, VDP, worker ou Scheduler.
- A UI nunca executa hardware, avança clocks ou faz callbacks por instrução.
- Não usar JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Sequência obrigatória

1. Auditar SHA/main, PRs, workflows, manifests e árvore completa.
2. Mapear dependências entre SMS temporal e módulos atualmente compartilhados com SMS atômico.
3. Extrair worker SMS temporal e serviços de host para Rust puro, sem PyO3.
4. Tornar SMS temporal o único caminho de execução e retirar os cores atômicos do workspace.
5. Integrar menus egui/eframe e comandos coarse-grained ao runtime SMS.
6. Remover todos os arquivos e dependências Python/Qt/PyO3, incluindo testes, tooling, workflows, artefatos e referências de build.
7. Validar em PRs pequenas com gates Rust e testes físicos aplicáveis.

## Fidelidade e desempenho

- MachineScheduler é a autoridade única dos clocks/eventos.
- CPU, barramento, PSG, VDP, áudio e vídeo devem preservar ordem causal e determinismo.
- WAIT states, operações suspensíveis/retomáveis e snapshots devem refletir o estado real do runtime.
- Execução contínua e em blocos deve produzir estados/saídas equivalentes.
- Desempenho só será otimizado após comprovação de correção.

## Pesquisa e modelagem

Para tarefas complexas de hardware, usar três frentes independentes: engenharia reversa, documentação primária e arquitetura/modelagem. Confrontar evidências antes de implementar. Identificar estado, transições, ciclos, eventos, critérios de aceitação e testes diferenciais/temporais.

## Gates obrigatórios

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Validar também targets Windows/D3D12/WASAPI/XAudio2 e workspaces específicos enquanto existirem. Após a consolidação, o workspace ativo e seus lockfiles devem ser Cargo-only. Testes físicos não executados são NOT RUN, nunca PASS. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN e vincular ao SHA exato.

## GitHub e publicação

- GitHub é a fonte da verdade.
- Toda alteração em branch e Pull Request.
- Proibido commit direto na main, force-push e reset destrutivo.
- Não fazer merge sem CI 100% verde no HEAD final e autorização explícita.
- Não declarar migração concluída por compilação isolada ou remoção de arquivos sem validação funcional.

## Relatório

Informar SHA base/final, branch/PR, arquivos alterados, testes realmente executados, CI, limitações e próximos passos. Nunca alegar execução local, teste físico, aprovação ou merge sem evidência.
