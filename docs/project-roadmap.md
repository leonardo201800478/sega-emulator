# Roadmap mestre — Sega Emulator

**Status:** única rota normativa de implementação.  
**Regra de atualização:** este documento define a sequência normativa, não uma fotografia de branches/PRs. Antes de cada tarefa, auditar o SHA corrente da `main`, código, PRs e workflows.  
**Prioridade imediata:** Master System e consolidação do Z80 compartilhado, preservando o SG-1000.  
**Política de fidelidade:** [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## Diretriz vigente — 2026-11-05

A prioridade imediata de P1 é fechar uma máquina Master System temporal completa antes de declarar gameplay operacional pelo novo caminho. A decisão aceita em [ADR SMS Temporal + Qt](architecture/ADR-sms-temporal-machine-qt-rebuild.md) substitui a reconstrução incremental da Qt.

Ordem vertical atual:
1. PSG SN76489 SMS temporal, integrado ao scheduler e ao instante de commit das escritas I/O.
2. VDP SMS por revisão (315-5124/315-5246), incluindo raster, modos, sprites, IRQ, counters, overscan e coluna esquerda.
3. Integração Z80 + bus + memória/mapper + VDP + PSG + input + outputs numa única timeline.
4. Fronteira PyO3 coarse-grained e janela Qt mínima de emulador (viewport, abrir/fechar ROM, executar/pausar, reset), com metadados de frame para active area, overscan, aspecto e timestamp. Biblioteca, capas, perfis, diagnóstico visual e configurações avançadas ficam fora do escopo inicial.
5. Retirada controlada do runtime SMS antigo somente após a nova máquina temporal, PyO3 e smoke test Qt passarem; preservar componentes Qt compartilhados e não regredir SG-1000.

A matriz de aceite e evidências está em [sms-vdp-psg-temporal-validation.md](architecture/sms-vdp-psg-temporal-validation.md). A implementação só avança de gate com testes executados e classificados individualmente. A interrupção temporária de gameplay pelo runtime SMS antigo foi aceita; isso não reduz o critério final de gameplay temporal funcional.

O objetivo é uma emulação funcionalmente correta e temporalmente fiel nos caminhos de execução que influenciam jogos e periféricos. O detalhamento temporal é prioritário para sequências de instruções e processos de gameplay sensíveis a ordem/duração; o restante usa operações atômicas determinísticas e semanticamente exatas. Não reproduzir pinos, fases elétricas ou estados internos sem efeito observável.

Não manter roadmaps ou planos de sprint paralelos. ADRs, auditorias e referências de hardware não criam fases ou prioridades concorrentes.

## 1. Princípios de execução

1. Rust é o único runtime de hardware; Python/PySide6 é frontend, launcher, configuração, diagnóstico e tooling.
2. PyO3 oferece operações coarse-grained; sem callbacks por instrução.
3. Reutilizar componentes compartilhados quando representarem hardware compatível; manter diferenças reais por plataforma.
4. Corrigir primeiro comportamento funcional e compatibilidade; adicionar precisão temporal onde uma interação real exigir.
5. Um Scheduler por máquina composta coordena os eventos relevantes, sem impor atualização de cada pino/tick de cada componente.
6. Não implementar detalhes de barramento, WAIT, arbitragem, M-cycle/T-state ou pinos sem identificar o console e o comportamento que dependem deles.
7. Desempenho deve ser medido; não usar JIT, frame skipping, hacks de timing ou otimizações especulativas.
8. Mudanças não triviais seguem branch + PR. Sem commit direto na main, force-push ou merge sem gates verdes no HEAD final.
9. ROMs comerciais, BIOS proprietárias e dumps protegidos não entram no Git.

## Regra normativa de granularidade — prioridade temporal e execução atômica

O projeto adota dois níveis complementares, sem reduzir a correção funcional:

1. **Precisão temporal prioritária para gameplay:** instruções, sequências e processos cuja ordem ou duração possa alterar a execução de jogos ou a interação entre CPU, memória/barramento, mapper, VDP/raster, PSG/FM/PCM, interrupções, DMA ou periféricos. Nesses caminhos, preservar clocks, ordem causal, pontos de interação e a menor resolução temporal que reproduza o efeito observado. Usar M-cycles, T-states, WAIT, suspensão/retomada ou fases de transação somente quando testes, documentação ou divergência reproduzível demonstrarem essa necessidade.
2. **Precisão funcional atômica como padrão para o restante:** executar a operação em uma unidade atômica definida pela interface (instrução, acesso, escrita de registrador, avanço de scanline/bloco ou amostra), com resultado, flags, memória, efeitos colaterais, estado e duração nominal corretos quando conhecidos. Não criar micro-operações, fases internas, safe points ou eventos por tick apenas para espelhar a estrutura elétrica do chip.

A semântica funcional de toda instrução e operação suportada continua obrigatoriamente correta. “Atômica” não significa aproximada, nondeterminística ou incompleta; significa que estados intermediários não observáveis não são modelados sem necessidade. O detalhamento temporal deve concentrar-se nos caminhos de gameplay sensíveis ao tempo. **Não existe requisito de precisão por pino, por borda elétrica ou por transistor.**

Antes de elevar uma operação do nível atômico ao temporal, registrar: jogo/console/revisão/periférico afetado, efeito observável, evidência, menor granularidade suficiente, teste de regressão e custo. Na ausência dessa evidência, manter execução atômica e registrar a incerteza sem bloquear a plataforma.

## 2. Fases de plataforma

### P0 — Infraestrutura e hosts

Consolidar workspace, ferramentas, CI, runtime Rust, integração coarse-grained e interfaces de host necessárias às plataformas ativas.

**Saída:** builds reproduzíveis, gates confiáveis, fronteira Rust/Python clara e runtime de hardware independente da GUI.

### P1 — Master System

Prioridade principal. Consolidar a máquina SMS em Rust:
- Z80 compartilhado e semanticamente correto para o software suportado;
- memória, mappers, I/O e periféricos;
- VDP Mode 4, registradores, sprites, raster e interrupções relevantes;
- PSG e áudio;
- sincronização CPU/vídeo/áudio nos pontos de interação que afetem jogos;
- compatibilidade com ROMs de teste e jogos representativos.

Priorizar temporização de instruções e processos que afetem gameplay: escritas/leituras de I/O e registradores de VDP/PSG, efeitos de mapper, interrupções, raster/H-counter, DMA, contenção e WAIT quando presentes no console. Instruções puramente internas de CPU e operações sem interação observável permanecem atômicas, preservando semântica e duração nominal conhecida. Não exigir execução por T-state de todas as instruções nem representação integral dos pinos.

### P2 — SG-1000

Preservar e consolidar o runtime SG-1000, reutilizando o Z80 e componentes comuns quando compatíveis. Manter VDP TMS9918A, mapa de memória, cartuchos, PSG e particularidades do sistema. Não regredir jogos já validados.

### P3 — Game Gear

Reutilizar a base SMS onde houver equivalência. Implementar diferenças reais de LCD, viewport, paleta, áudio estéreo, I/O e perfil de hardware.

### P4 — Mega Drive / Genesis

Adicionar o Motorola 68000, VDP, áudio e integração do Z80 auxiliar. Modelar arbitragem e sincronização entre CPUs conforme as necessidades observáveis dos jogos.

### P5 — Sega CD

Compor a plataforma Mega Drive com CD-ROM, sub-CPU, memória compartilhada, áudio e ASICs específicos.

### P6 — 32X / Sega CD 32X

Adicionar SH-2, vídeo e áudio do 32X e composição com Mega Drive/Sega CD.

### P7 — Frontend Qt / PyO3

Consolidar launcher, biblioteca, configurações, telemetria e automação. A GUI não executa nem temporiza hardware.

### P8 — Compatibilidade e auditoria

Ampliar matriz de jogos, ROMs de diagnóstico, regressões, compatibilidade de revisões e validação em ambientes de host suportados.

### P9 — Validação sistêmica

Comprovar determinismo, save states, replay e sincronização de cada plataforma na granularidade realmente necessária. Não exigir um selo global de cycle-accuracy ou pin-accuracy como condição universal.

## 3. Z80 compartilhado — política de conclusão

O Z80 é um componente compartilhado, não uma CPU duplicada por console.

Critérios funcionais:
- instruções documentadas necessárias aos sistemas suportados;
- registradores, flags e efeitos arquiteturais corretos;
- prefixos e operações indexadas utilizadas pelo conjunto de instruções;
- interrupções e HALT conforme o uso da plataforma;
- regressões SG-1000 e SMS.

Critérios temporais prioritários:
- preservar duração nominal e ordem das instruções executadas por jogos;
- detalhar sequência temporal quando uma instrução/processo interagir com VDP, áudio, mapper, I/O, interrupção ou barramento de modo sensível ao instante;
- manter atomicidade para operações internas sem efeito intermediário observável;
- representar transações suspensíveis, WAIT, M-cycle/T-state e sinais derivados somente quando a plataforma exigir;
- não simular todos os pinos ou estados internos como meta independente.

Suítes externas como ZEXDOC/ZEXALL e traces de barramento são instrumentos de diagnóstico/validação. Não são autorização para ampliar indefinidamente o escopo nem substituem testes de integração nos consoles.

## 4. Método por tarefa

1. Auditar SHA da main, código, PRs, workflows e documentação relevante.
2. Identificar o comportamento observável que precisa ser corrigido.
3. Consultar documentação primária e referências independentes proporcionais ao risco.
4. Definir critérios de aceitação objetivos.
5. Implementar a menor mudança Rust que resolva o comportamento.
6. Executar testes direcionados e regressões dos consoles afetados.
7. Medir desempenho quando o caminho crítico for alterado.
8. Atualizar status/documentação e abrir PR.
9. Integrar somente com CI verde no SHA final.

Para tarefas complexas, pesquisas independentes podem ser conduzidas em três frentes (implementações de referência, documentação de hardware, arquitetura/testes). A pesquisa não deve criar uma obrigação de modelar fenômenos sem impacto no console.

## 5. Gates de qualidade

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Cada suíte é PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca é PASS. Registrar o SHA exato, jobs executados e jobs não executados.

## 6. Como determinar o estado atual

Este roadmap define a ordem e os critérios de saída; não declara, por si só, que uma fase ou componente está concluído. Para cada nova tarefa:

1. Consultar o SHA mais recente da `main` e confirmar o código efetivamente integrado.
2. Consultar PRs abertas e distinguir explicitamente Draft, aberta, aprovada e mesclada. Uma PR não mesclada não integra a `main`.
3. Conferir os workflows e os resultados vinculados ao SHA exato; jobs SKIPPED ou NOT RUN não são PASS.
4. Reconciliar o [estado dos componentes](component-status.md), o [scorecard Z80](z80-completion-scorecard.md) e a [matriz DD/FD](z80-dd-fd-semantic-matrix.md) com o código e os testes atuais antes de usá-los como evidência.

Não manter snapshots de PRs abertas neste roadmap. Auditorias datadas e relatórios de PR preservam evidências históricas, não definem prioridade nem estado integrado. O contrato de aceitação final é acompanhado pela [PR permanente de conclusão #533](https://github.com/leonardo201800478/sega-emulator/pull/533); ela não substitui esta rota normativa.

## 7. Autoridade documental

- [Escopo e princípios arquiteturais](project-scope.md)
- [Estado dos componentes](component-status.md)
- [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [ADRs de arquitetura](architecture/adr/README.md)

Documentos históricos podem explicar como uma proposta surgiu, mas não devem ser tratados como requisitos vigentes quando conflitam com esta política. Planos de sprint avulsos ou roadmaps reconciliados fora do repositório (incluindo cópias anexadas em conversas) são referências históricas, não uma segunda rota de execução; tarefas antigas que exijam microsequenciamento universal devem ser reinterpretadas pela regra temporal-para-gameplay/atômica-para-o-restante. Não manter nem atualizar cronogramas paralelos.
