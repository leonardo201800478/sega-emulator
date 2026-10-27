# Instruções para agentes de código — Sega Emulator

Estas regras orientam agentes de programação e revisores que trabalhem neste repositório. A política arquitetural aprovada está em [ADR 0007](docs/adr/0007-rust-first-development-policy.md); em caso de divergência, siga o ADR e reporte documentação contraditória.

## Prioridade do projeto

- Primeiro estabilizar e validar o SG-1000. Evoluir depois para Master System/Game Gear, Mega Drive/Genesis, Sega CD e 32X, sem implementar todas as plataformas ao mesmo tempo.
- Todo o hardware e runtime de emulação ativo devem ser implementados em Rust. Python de produção fica restrito à interface Qt e seus adaptadores de host, carregamento de ROMs/perfis e integração fina com PyO3. Ferramentas/testes Python são infraestrutura e nunca podem executar implementações de hardware em `legacy/`.
- Não crie nem mantenha implementações de hardware em Python no caminho ativo. Implementações antigas foram arquivadas em `legacy/python_reference/` exclusivamente para consulta e não devem ser importadas por código, testes ou workflows ativos.
- Não adicione JIT nem paralelismo entre componentes temporais por antecipação.

## Antes de alterar

1. Confirme repositório, branch, HEAD, PR associado e estado da árvore; nunca presuma que o clone local corresponde ao GitHub.
2. Leia a implementação atual, testes, documentação, dependências e histórico relevante antes de editar.
3. Descreva problema, escopo, riscos, critérios de aceitação e testes que demonstrarão a correção.
4. Prefira uma mudança pequena e completa. Preserve contratos, trabalho existente e alterações concorrentes. Não faça operações destrutivas nem mude proteções/regras de repositório sem autorização.
5. Use branches e PRs para alterações com impacto funcional/arquitetural. Não faça merge salvo solicitação explícita.

## Testes: escrever para provar comportamento

- Para cada bug, crie um teste que reproduza a falha antes de corrigir quando viável.
- Priorize testes de comportamento observável: estado da CPU, flags, PC/SP, memória, portas, interrupções, ciclos, ordem de eventos, framebuffer, áudio, hashes e determinismo conforme o subsistema.
- Inclua casos normais, limites, reset, erros e regressões. Use ROMs sintéticas/homebrew e vetores legais; não versionar ROMs comerciais, BIOS protegidas, snapshots que incorporem ROMs comerciais ou logs locais.
- Não altere expectativas só para fazer o teste passar. Distingua comportamento documentado, observado, inferido e hipotético.
- Testes Python/Rust devem comparar contratos e resultados observáveis, não detalhes internos irrelevantes. Não alegue fidelidade ciclo a ciclo sem modelo e evidências adequados.
- Evite testes frágeis dependentes de tempo de parede, áudio/dispositivo real ou estado global, salvo quando forem explicitamente testes de integração.

## Verificações locais recomendadas

Python:
```powershell
python -m pytest -q
python -m ruff check .
python -m ruff format --check .
git diff --check
```

Workspace Rust:
```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Extensão PyO3:
```powershell
cargo fmt --manifest-path native/z80_core/Cargo.toml --all -- --check
cargo clippy --manifest-path native/z80_core/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path native/z80_core/Cargo.toml
python -m pip install maturin
maturin develop --manifest-path native/z80_core/Cargo.toml --features python-extension --release
python -m pytest tests/test_sg1000_native_machine.py tests/test_vgm_machine_integration.py tests/integration/test_rust_z80_core.py -q
```

Execute apenas os comandos relevantes ao escopo, além dos checks completos exigidos pelo CI. Confirme os manifest paths e os comandos nos arquivos atuais antes de executar.

## Workflows de GitHub Actions e validação por “agentes”

- `.github/workflows/sg1000-three-agent-validation.yml`: sequência de jobs para instruções Z80, mapa de memória/determinismo e qualidade/testes completos do workspace Rust SG-1000.
- `.github/workflows/rust-sg1000-core.yml`: fmt, Clippy e testes do workspace Rust.
- `.github/workflows/qt-rust-z80-core.yml`: build Maturin, extensão PyO3, testes Rust e regressão Python/Qt de integração.
- `.github/workflows/agent-functional-validation.yml`: lint/formatação e testes da aplicação Qt/integração; não deve importar o hardware Python arquivado.
- `.github/workflows/agent-integrity.yml`: compileall, dependências, repetibilidade da suíte e relatórios.
- `.github/workflows/agent-performance-audit.yml`: durações da suíte e inventário de opcodes.

Ao concluir uma alteração:
1. Identifique os workflows que realmente cobrem os arquivos e contratos modificados; não suponha que todos disparam por qualquer mudança (verifique os filtros `paths`).
2. Abra o PR/branch correto e acompanhe os runs do commit exato.
3. Consulte os checks, jobs, logs e artefatos; registre sucesso, falha, cancelamento e checks ausentes.
4. Se um workflow necessário não disparar por causa de filtros, registre a lacuna e proponha correção explícita; não declare validação completa.
5. Os jobs “Agent 1/2/3” são tarefas automatizadas independentes no CI, não evidência de agentes de IA que escreveram ou revisaram código.

## Política de precisão e desempenho

- Não invente clocks, timings, comportamento de registradores ou efeitos analógicos. Cite fontes primárias e marque incertezas.
- Separe lógica digital, temporização, resposta analógica, reamostragem e backend de saída.
- Meça antes de otimizar. Benchmarks devem identificar commit, configuração, máquina, workload, aquecimento, backend e método.
- Não use utilização total da CPU como objetivo em si; otimize precisão, estabilidade, latência e desempenho sustentado.
- Não copie código de outros emuladores sem verificar licença, atribuição e necessidade.

## Relatório obrigatório

Informe objetivo, arquivos alterados, branch/PR e commits com links, testes realmente executados e resultados, checks CI do commit, limitações, validações não executadas e próximo passo. Nunca diga que arquivos foram alterados, testes executados ou merge realizado sem evidências.

Se o usuário disser “prossiga”, continue do último estado confirmado sem reiniciar a análise nem ampliar o escopo automaticamente.
