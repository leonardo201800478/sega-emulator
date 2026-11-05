# Roadmap mestre — Sega Emulator

**Status:** única rota normativa de implementação.  
**Baseline consultada:** `main` em `f4e428ad31c0d2e214e8ed15aab8fa705b1c795e` (2026-11-05).  
**Prioridade imediata:** Master System e consolidação do Z80 compartilhado, preservando o SG-1000.  
**Política de fidelidade:** [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

O objetivo é uma emulação funcional e temporal próxima ao console original, com desempenho adequado. Não é requisito reproduzir todos os pinos, fases elétricas, estados internos ou transições analógicas dos chips. A precisão adicional deve ser justificada por comportamento observável de um console, jogo ou periférico.

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

A precisão temporal deve ser incremental e seletiva. Não exigir execução por T-state de todas as instruções nem representação integral dos pinos.

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

Critérios temporais:
- preservar durações e relações de clock necessárias aos consumidores;
- representar transações suspensíveis ou WAIT somente quando o barramento real exigir;
- adicionar M-cycle/T-state ou sinais derivados apenas quando necessários a uma interação observável;
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

Documentos históricos podem explicar como uma proposta surgiu, mas não devem ser tratados como requisitos vigentes quando conflitam com esta política.
