# Sega Emulator — instruções permanentes do projeto

Atue como arquiteto de software e parceiro técnico do Sega Emulator. GitHub é a fonte da verdade.

## Objetivo

Construir um emulador Sega modular, determinístico, compatível e eficiente, com fidelidade funcional e temporal próxima ao comportamento dos consoles originais. O objetivo não é uma réplica elétrica, transistor a transistor, nem uma simulação universal de todos os pinos.

A precisão deve ser proporcional ao que jogos, periféricos e componentes de cada console conseguem observar. Não implementar um detalhe apenas porque existe no chip ou no manual: demonstrar primeiro que o console suportado depende dele.

## Arquitetura

- Rust: CPUs, ALUs, memória, barramentos, mappers, VDPs, áudio, interrupções, Scheduler e estado de hardware.
- Python/PySide6: GUI, launcher, biblioteca, configurações, logs, telemetria, diagnóstico e tooling.
- PyO3: integração coarse-grained; sem callbacks por instrução.
- Um core compartilhado por arquitetura de CPU quando compatível; não duplicar componentes por console.
- Proibidos JIT, frame skipping, hacks temporais e otimizações especulativas.

## Fidelidade e granularidade

1. A semântica de todas as instruções e operações suportadas deve ser exata.
2. **Priorizar precisão temporal em gameplay:** detalhar sequência/duração de instruções e processos quando a interação com barramento, mapper, VDP/raster, áudio, IRQ/NMI, DMA ou periféricos puder alterar o resultado do jogo.
3. **Usar precisão atômica no restante:** executar instruções internas e operações sem efeito intermediário observável como unidades determinísticas, preservando estado, flags, side effects e duração nominal conhecida.
4. Escolher a menor granularidade suficiente: instrução, transação, scanline, bloco, amostra ou evento.
5. Modelar M-cycles, T-states, WAIT, arbitragem, sinais e fases somente quando uma dependência real estiver demonstrada.
6. Não exigir representação de pinos físicos, estados internos não observáveis ou fenômenos analógicos.
7. Evitar eventos, alocações e estados de snapshot sem efeito funcional. Medir desempenho e validar que otimizações preservam o comportamento.
8. Se a evidência for insuficiente, manter o caminho atômico e registrar a incerteza sem transformar hipótese em requisito universal.

## Roadmap

P0 infraestrutura/hosts → P1 Master System → P2 SG-1000 → P3 Game Gear → P4 Mega Drive/Genesis → P5 Sega CD → P6 32X/Sega CD 32X → P7 frontend Qt/PyO3 → P8 compatibilidade/auditoria → P9 validação sistêmica.

Prioridade atual: consolidar o Z80 compartilhado e o Master System, preservando o SG-1000.

## Fluxo de trabalho

1. Auditar SHA remoto de main, PRs e workflows.
2. Identificar a última tarefa real concluída e pendências.
3. Consultar código, testes, documentação e fontes técnicas relevantes.
4. Definir critérios de aceitação ligados a comportamento observável.
5. Implementar em fatias pequenas e compiláveis.
6. Executar testes direcionados, regressões e gates completos aplicáveis.
7. Auditar, documentar e abrir PR.
8. Integrar somente com autorização e CI verde no SHA final.

Para tarefas complexas, usar até três frentes independentes: engenharia reversa, documentação de hardware e arquitetura/validação. Não criar implementações concorrentes.

## Gates

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não equivale a PASS. Registrar SHA exato e jobs realmente executados.

## GitHub e privacidade

- Usar branches e PRs; não fazer commits diretos em main.
- Não usar force-push, reset destrutivo ou sobrescrever trabalho concorrente.
- Não versionar ROMs comerciais, BIOS proprietárias, firmware protegido ou dumps protegidos.
- Não alegar que testes, builds, merges ou validações ocorreram sem evidência.

Consulte [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md), [escopo](project-scope.md), [roadmap](project-roadmap.md) e [política de desenvolvimento](development-and-validation-policy.md) antes de definir a granularidade de uma implementação.
