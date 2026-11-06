# Instruções para agentes de código — Sega Emulator

Estas regras orientam agentes e revisores. As decisões normativas vigentes são a [ADR-0016 — fidelidade observável](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md), o [roadmap mestre](docs/project-roadmap.md) e o [escopo do projeto](docs/project-scope.md).

## Prioridade e arquitetura

- Seguir a sequência P0 infraestrutura/hosts → P1 Master System → P2 SG-1000 → P3 Game Gear → P4 Mega Drive/Genesis → P5 Sega CD → P6 32X/Sega CD 32X → P7 frontend Qt/PyO3 → P8 compatibilidade/auditoria → P9 validação sistêmica.
- A prioridade imediata é consolidar o Z80 compartilhado e o Master System, preservando o SG-1000.
- Arquitetura-alvo: produto integralmente Rust, incluindo GUI egui/eframe, launcher, biblioteca, mídia, configurações, controles, áudio, vídeo, telemetria, diagnósticos e serviços.
- Python, PySide6, PyO3 e Maturin são componentes transitórios do produto legado e devem ser eliminados somente após paridade funcional e validação dos equivalentes Rust.
- Durante a transição, não remover módulos Python ativos nem quebrar a ponte PyO3 antes de migrar seus consumidores. Não introduzir novos recursos de produto em Python.
- Não duplicar CPUs ou componentes compartilháveis por console.
- Não adicionar JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Fidelidade e desempenho

A correção funcional de toda instrução/operação suportada é obrigatória. A granularidade segue dois níveis:

- **Temporal prioritário para gameplay:** preservar ordem e duração de instruções/processos que interagem com barramento, mapper, VDP/raster, PSG/FM/PCM, interrupções, DMA ou periféricos quando o instante altera o jogo.
- **Atômico como padrão:** instruções internas e operações sem efeito intermediário observável executam como unidades determinísticas, com semântica, flags, estado, side effects e duração nominal correta quando conhecida.
- M-cycles, T-states, WAIT, arbitragem, fases e sinais só são exigidos quando a dependência concreta estiver demonstrada.
- Não exigir representação de todos os pinos nem simular capacitância, tensão, transistores ou fenômenos analógicos.
- Não adicionar eventos por tick/pixel/pino ou estados serializados sem necessidade demonstrada.
- Antes de detalhar um componente, identificar jogo/console/periférico, efeito observável, evidência e menor granularidade capaz de reproduzi-lo.
- Ausência de microsequenciamento não é falha funcional quando a execução atômica satisfaz o contrato observável.
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

Gates obrigatórios do produto Rust:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Validar também workspaces independentes e os targets Windows/D3D12/WASAPI/XAudio2 aplicáveis. Python/Ruff/pytest, Qt e PyO3 permanecem validações legadas opt-in por dispatch manual durante a migração; não são gates obrigatórios de PR Rust. Sua ausência em um run normal deve ser registrada como NOT RUN, nunca PASS. Não remover código legado até a paridade funcional estar comprovada. Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN e vincular resultados ao SHA exato.

## Workflows

Inspecionar os filtros paths dos workflows e acompanhar os jobs que realmente executaram. Ler logs e artefatos quando necessário. Jobs chamados Agent 1/2/3 são automações do CI, não prova de que agentes de IA participaram.

## Relatório obrigatório

Informar objetivo, arquivos alterados, branch/PR/commits com links, testes realmente executados, resultados de CI, itens NOT RUN/BLOCKED, limitações e próximo passo. Não alegar execução, aprovação ou merge sem evidência.

Leia também [política permanente de desenvolvimento e validação](docs/development-and-validation-policy.md).
