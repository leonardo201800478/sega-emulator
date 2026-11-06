# Instruções para agentes de código — Sega Emulator

As regras normativas são a ADR 0010 (docs/adr/0010-rust-only-sms-temporal-first.md), a ADR-0016 (docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md) e o roadmap mestre (docs/project-roadmap.md).

## Prioridade e arquitetura

- Produto 100% Rust. Não reintroduzir Python, PySide6/Qt, PyO3 ou Maturin em runtime, testes, tooling, scripts, CI ou distribuição.
- GUI nativa egui/eframe. Escopo inicial: menus operacionais mínimos; não construir biblioteca visual ou frontend rico nesta etapa.
- Único runtime ativo inicial: Master System temporal, exclusivamente sobre o Z80 compartilhado, SN76489 temporal, VDP temporal e MachineScheduler.
- rust/runtime/sms-runtime é o worker SMS temporal comum. Não duplicar worker, máquina, CPU, PSG, VDP ou Scheduler.
- Desconectar e remover o SMS atômico antigo e o SG-1000 atômico antigo quando os módulos compartilhados necessários ao SMS temporal estiverem isolados e comprovados.
- Não criar fallback nem duplicar componentes por console.
- A UI nunca executa hardware, avança clocks ou faz callbacks por instrução.
- Não usar JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Sequência obrigatória

1. Auditar SHA/main, PRs, workflows, manifests e árvore completa.
2. Isolar os módulos temporais do SMS core por dependência real, preservando memória, mapper, I/O e wrappers usados pelo runtime.
3. Consolidar a árvore ativa em rust/, com um workspace e lockfile canônicos quando as dependências Windows permitirem.
4. Integrar o shell egui/eframe ao runtime SMS com comandos coarse-grained, viewport D3D12, áudio e input nativos.
5. Remover os cores atômicos SMS/SG-1000 que não sejam dependências comprovadas do SMS temporal.
6. Portar para Rust os harnesses externos essenciais que ainda não possuem equivalente; não marcar suítes removidas como PASS.
7. Validar em PRs pequenas com gates Rust e testes físicos aplicáveis.

## Fidelidade e desempenho

- MachineScheduler é a autoridade única dos clocks/eventos.
- CPU, barramento, PSG, VDP, áudio e vídeo preservam ordem causal e determinismo.
- WAIT states, operações suspensíveis/retomáveis e snapshots refletem o estado real do runtime.
- Execução contínua e em blocos deve produzir estados/saídas equivalentes.
- Durante a migração, somente a correção de desempenho com causa-raiz já identificada e comprovada está autorizada; não presumir que diagnósticos do runtime Python/SG-1000 antigo se aplicam ao SMS temporal Rust.

## Pesquisa e modelagem

Para tarefas complexas de hardware, usar três frentes independentes: engenharia reversa, documentação primária e arquitetura/modelagem. Confrontar evidências antes de implementar. Identificar estado, transições, ciclos, eventos, critérios de aceitação e testes diferenciais/temporais.

## Gates obrigatórios

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Validar também targets Windows/D3D12/WASAPI/XAudio2 e workspaces específicos enquanto existirem. Testes físicos não executados são NOT RUN, nunca PASS. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN e vincular ao SHA exato.

## GitHub e publicação

- GitHub é a fonte da verdade.
- Toda alteração em branch e Pull Request.
- Proibido commit direto na main, force-push e reset destrutivo.
- Não fazer merge sem CI 100% verde no HEAD final e autorização explícita.
- Não declarar migração concluída por compilação isolada ou remoção de arquivos sem validação funcional.

## Relatório

Informar SHA base/final, branch/PR, arquivos alterados, testes realmente executados, CI, limitações e próximos passos. Nunca alegar execução local, teste físico, aprovação ou merge sem evidência.
