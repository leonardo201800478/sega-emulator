# Política permanente de desenvolvimento, fidelidade e entrega

**Status:** normativa para todo trabalho futuro no Sega Emulator.  
**Repositório:** https://github.com/leonardo201800478/sega-emulator  
**Decisão de fidelidade:** [ADR-0016 — fidelidade observável e precisão proporcional](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## 1. Objetivo e prioridade

Construir um emulador modular, determinístico, compatível e eficiente, reproduzindo o comportamento funcional e temporal observável dos consoles originais.

Seguir a sequência P0 infraestrutura/hosts → P1 Master System → P2 SG-1000 → P3 Game Gear → P4 Mega Drive/Genesis → P5 Sega CD → P6 32X/Sega CD 32X → P7 frontend Qt/PyO3 → P8 compatibilidade/auditoria → P9 validação sistêmica.

Prioridade imediata: consolidar o Z80 compartilhado e o Master System, preservando o SG-1000.

## 2. Regra de fidelidade

A emulação deve ser próxima ao console original nos resultados que jogos e periféricos conseguem observar. Não é objetivo simular todos os detalhes elétricos do chip.

- Correção funcional de instruções, registradores, flags, memória, I/O, vídeo, áudio e interrupções é essencial.
- Preservar clocks, ordem causal e sincronização nos pontos em que alteram a execução.
- Usar a menor granularidade suficiente: instrução, transação, scanline, amostra ou evento.
- M-cycles, T-states, WAIT, arbitragem, fases e sinais são modelados quando houver dependência concreta do console.
- Não exigir todos os pinos físicos, microestados internos não observáveis, capacitância, tensão ou simulação transistor-level.
- Não criar eventos por tick/pixel/pino, alocações ou campos de save state sem efeito necessário.
- Um detalhe adicional precisa de justificativa: console/revisão/jogo/periférico afetado, fonte ou teste, menor granularidade e custo de CPU/memória/manutenção.
- Quando não houver evidência suficiente, registrar UNKNOWN/hipótese; não bloquear o projeto por completude abstrata.

## 3. Estratégia por componente

1. Auditar código, testes, docs, SHA remoto, PRs e workflows.
2. Identificar lacunas funcionais reais e definir critérios de aceitação.
3. Consultar fontes primárias e referências independentes conforme risco.
4. Implementar em Rust, em fatias pequenas e compiláveis.
5. Executar testes unitários/direcionados e regressões dos consoles afetados.
6. Executar testes abrangentes e medir desempenho quando o caminho crítico for alterado.
7. Atualizar status, revisar diff e abrir PR.
8. Integrar somente com autorização e gates verdes no SHA final.

Para tarefas complexas, dividir pesquisa em três frentes: engenharia reversa, documentação de hardware e arquitetura/validação. As frentes analisam em paralelo e reconciliam evidências antes de qualquer mudança; não produzem versões concorrentes do mesmo componente.

## 4. Linguagens e arquitetura

- Rust é o runtime único de CPU, ALU, memória, barramento, mappers, VDP, áudio, interrupções, sincronização e estado.
- Python/PySide6 é GUI, launcher, biblioteca, configurações, logs, telemetria, diagnóstico e tooling.
- PyO3 é coarse-grained. Não usar callbacks Python por instrução, pixel ou amostra.
- Reutilizar CPUs e componentes quando compatíveis; não duplicar core por console.
- JIT, frame skipping, hacks temporais e otimizações especulativas são proibidos.
- Otimizações devem ser medidas e preservar o resultado funcional/temporal.

## 5. Testes e gates

Gates-base:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

- Criar teste de regressão que falhe antes da correção quando viável.
- Usar ROMs sintéticas/homebrew, vetores fixos e oráculos independentes quando disponíveis.
- Não ajustar expectativas para fazer CI passar.
- Verificar filtros paths dos workflows; acompanhar logs e artefatos do SHA exato.
- Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não é PASS.
- Testes de ciclo/barramento detalhados são necessários apenas quando a interação observável depender da ordem ou duração.

## 6. GitHub, publicação e privacidade

- Usar branch e PR; não commitar diretamente na main.
- Não usar force-push, reset destrutivo ou sobrescrever trabalho alheio.
- Não fazer merge sem autorização e gates aplicáveis aprovados no HEAD final.
- Para mudanças que afetam jogos, registrar evidência in-game reproduzível. Se depender do mantenedor, marcar BLOCKED/PENDING até receber resultado.
- Não versionar ROMs comerciais, BIOS proprietárias, firmware protegido ou dumps protegidos.
- Não alegar builds, testes, revisão ou merge sem evidência.

## 7. Relatório

Informar objetivo, arquivos alterados, branch/PR/SHA, testes executados, resultados de CI, limitações, itens NOT RUN/BLOCKED e próximo passo.

A política não transforma auditorias históricas, prompts ou propostas abertas em tarefas vigentes. Em caso de conflito, prevalece a ADR-0016, seguida do roadmap e do código integrado na main.
