# Instruções para agentes de código — Sega Emulator

Estas regras orientam agentes e revisores. As decisões normativas vigentes são a [ADR-0016 — fidelidade observável](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md), o [roadmap mestre](docs/project-roadmap.md) e o [escopo do projeto](docs/project-scope.md).

## Prioridade e arquitetura

- Seguir a sequência P0 infraestrutura/hosts → P1 Master System → P2 SG-1000 → P3 Game Gear → P4 Mega Drive/Genesis → P5 Sega CD → P6 32X/Sega CD 32X → P7 frontend Qt/PyO3 → P8 compatibilidade/auditoria → P9 validação sistêmica.
- A prioridade imediata é consolidar o Z80 compartilhado e o Master System, preservando o SG-1000.
- Todo hardware/runtime ativo pertence a Rust. Python/PySide6 fica restrito a GUI, launcher, biblioteca, configurações, logs, diagnóstico, telemetria e tooling.
- PyO3 é coarse-grained. Não usar callbacks Python por instrução, pixel ou amostra.
- Não duplicar CPUs ou componentes compartilháveis por console.
- Não adicionar JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Fidelidade e desempenho

O objetivo é reproduzir o comportamento funcional e temporal observável de cada console com desempenho adequado. Não construir uma réplica elétrica integral.

- Implementar corretamente as instruções e operações que o software dos consoles suportados pode executar.
- Preservar clocks, ordem causal e sincronização nos pontos em que afetam o resultado de jogos e periféricos.
- Usar a menor granularidade suficiente: instrução, transação, scanline, amostra ou evento.
- M-cycles, T-states, WAIT, arbitragem, fases e sinais são requisitos somente quando houver dependência concreta da plataforma.
- Não exigir representação de todos os pinos nem simular capacitância, tensão, transistores ou fenômenos analógicos.
- Não adicionar eventos por tick/pixel/pino ou estados serializados sem necessidade demonstrada.
- Antes de detalhar um componente, identificar o console, jogo, periférico ou divergência que depende desse detalhe e escolher a menor solução capaz de reproduzir o comportamento.
- Medir desempenho no caminho crítico; otimizar sem alterar a saída funcional/temporal.

## Antes de alterar

1. Confirme repositório, branch, HEAD/SHA remoto, PRs, workflows e estado dos arquivos.
2. Leia código, testes e documentação relevantes; não presuma que um documento histórico descreve a main.
3. Defina o comportamento observável a corrigir, critérios de aceitação, riscos e testes.
4. Pesquise proporcionalmente ao risco: documentação primária, implementações de referência e testes independentes. Distinguir fato, medição, inferência e hipótese.
5. Implemente mudanças pequenas, compiláveis e revisáveis; preserve trabalho existente.
6. Use branch e PR. Não commitar diretamente na main, não fazer force-push ou reset destrutivo.
7. Não fazer merge sem autorização e sem gates aplicáveis verdes no SHA final.

## Pesquisa em três frentes

Para tarefas complexas de hardware, dividir análise sem duplicar implementação:
1. Engenharia reversa: comparar implementações abertas e identificar comportamento, sem copiar código.
2. Documentação/fidelidade: consultar manuais do fabricante e referências técnicas do console.
3. Arquitetura/validação: reconciliar evidências, identificar comportamento observável, escolher granularidade mínima e definir testes.
A pesquisa não obriga implementar detalhes sem impacto no console.

## Testes e qualidade

Criar testes de regressão para bugs quando viável. Priorizar estado arquitetural, memória/I/O, interrupções, framebuffer, áudio, comportamento de cartuchos e interações temporais relevantes. Não ajustar expectativas apenas para fazer CI passar.

Gates:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Executar os checks pertinentes e os gates exigidos pelo CI. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não é PASS. Vincular resultados ao SHA exato.

## Workflows

Inspecionar os filtros paths dos workflows e acompanhar os jobs que realmente executaram. Ler logs e artefatos quando necessário. Jobs chamados Agent 1/2/3 são automações do CI, não prova de que agentes de IA participaram.

## Relatório obrigatório

Informar objetivo, arquivos alterados, branch/PR/commits com links, testes realmente executados, resultados de CI, itens NOT RUN/BLOCKED, limitações e próximo passo. Não alegar execução, aprovação ou merge sem evidência.

Leia também [política permanente de desenvolvimento e validação](docs/development-and-validation-policy.md).
