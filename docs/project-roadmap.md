# Roadmap mestre — Sega Emulator

**Status:** única rota normativa de implementação.  
**Baseline documental atualizada:** `main` em `c8da934d051f69f0fad01d133a923c26248a7250` (2026-11-05); esta alteração é documental.  
**Prioridade imediata:** Master System e consolidação do Z80 compartilhado, preservando o SG-1000.  
**Política de fidelidade:** [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

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

## 6. Estado observado na baseline consultada

- O repositório contém runtimes Rust para SG-1000 e Master System e uma CPU Z80 compartilhada.
- E5 DDCB/FDCB e E6 resolução geral de prefixos constam como integradas na main nas auditorias recentes.
- E7.3/HALT T4 consta como integrada. O fechamento semântico global e a integração temporal completa não estão comprovados.
- PRs #512, #514 e #526 permanecem propostas abertas/Draft na consulta; não são código integrado em main. A PR #527 também é documental e não foi integrada.
- O runtime não possui comprovação de cycle-accuracy sistêmica. Isso não é, por si só, um defeito: o critério passa a ser a fidelidade observável necessária aos consoles suportados.

A descrição acima é uma fotografia documental. Antes de qualquer implementação, conferir novamente main, PRs e workflows. O código integrado prevalece sobre status antigo de auditoria.

## 7. Autoridade documental

- [Escopo e princípios arquiteturais](project-scope.md)
- [Estado dos componentes](component-status.md)
- [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [ADRs de arquitetura](architecture/adr/README.md)

Documentos históricos podem explicar como uma proposta surgiu, mas não devem ser tratados como requisitos vigentes quando conflitam com esta política. Planos de sprint avulsos ou roadmaps reconciliados fora do repositório (incluindo cópias anexadas em conversas) são referências históricas, não uma segunda rota de execução; tarefas antigas que exijam microsequenciamento universal devem ser reinterpretadas pela regra temporal-para-gameplay/atômica-para-o-restante. Não manter nem atualizar cronogramas paralelos.
