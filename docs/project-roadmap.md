# Roadmap mestre — SG-1000 + Master System

**Status:** única rota de implementação vigente do projeto.
**Prioridade ativa:** preservar e consolidar SG-1000 + Master System (SMS), com integração temporal progressiva.
**Baseline auditada:** `main` em `6c8e18b1c0a134ee56e8282d1cbc7660373100f8` (2026-11-02, após integração da PR #418/T5a).
**Runtime-alvo:** Rust executa hardware, áudio, vídeo, entrada e tempo emulado. Python/Qt é plano de controle, interface e tooling. A migração de cada backend deve ser comprovada no código; esta diretriz não declara a arquitetura-alvo concluída.

> Este arquivo substitui todos os roadmaps, cronogramas de migração e cartas de planejamento anteriores. Não manter planos paralelos por subsistema. ADRs, especificações e auditorias continuam válidos como documentação técnica, mas não podem criar fases ou prioridades concorrentes. O trabalho executável é acompanhado por issues e PRs vinculadas às fases abaixo.

## 1. Escopo ativo e limites

### Dentro do escopo ativo

- SG-1000 e SG-1000 II como linha de preservação e regressão.
- Master System / Mark III, com distinção explícita entre modelos, revisões, ASICs e regiões quando houver evidência.
- Consolidação do Z80, barramentos, mappers, I/O, VDPs, PSG e integração temporal.
- Autonomia progressiva do runtime Rust para áudio, vídeo, entrada, pacing e execução.
- Frontend Python/Qt reduzido a aplicação, controle e telemetria.
- Matriz de compatibilidade, testes determinísticos e evidência vinculada a SHA.

### Fora do escopo de implementação até o marco M4

Game Gear como plataforma independente, Mega Drive/Genesis, Sega CD, 32X, SVP, Neptune e sistemas integrados de terceiros. Não iniciar implementação dessas plataformas durante o fechamento SG-1000 + SMS. Documentação histórica ou pesquisa desses sistemas não os torna escopo ativo. Uma expansão futura exige revisão formal deste roadmap por PR, após o M4.

JIT, frame-skip, paralelismo especulativo e otimizações sem baseline também permanecem proibidos.

## 2. Regras arquiteturais

1. Rust é o único runtime de hardware. Nenhum hardware, estado de registrador, ciclo, barramento, VDP ou chip de áudio é executado em Python.
2. Um único Master Scheduler é a autoridade temporal por máquina composta. Chips não mantêm loops temporais autônomos.
3. Python/Qt não avança a máquina, não transporta PCM e não participa do caminho de input ou apresentação de pixels quando o backend Rust correspondente estiver integrado.
4. PyO3 é uma fronteira coarse-grained de controle e telemetria; sem callbacks por instrução, pixel ou amostra.
5. SG-1000 e SMS compartilham mecanismos somente quando equivalência estiver demonstrada. TMS9918A e SMS VDP, mapas, I/O e perfis de hardware permanecem específicos.
6. A migração é port-first: preservar comportamento por testes/golden masters antes de refatorar. Mudanças estruturais e semânticas devem ser separadas.
7. T-states nominais não são execução ciclo a ciclo. Não declarar precisão temporal sem integração e evidência.
8. Perfil, firmware ou mapper desconhecido gera erro explícito; nunca fallback silencioso.
9. ROMs comerciais, BIOS proprietárias e dumps protegidos não entram no Git.
10. Mudanças não triviais seguem branch + PR. Não commitar diretamente na main, não fazer force-push e não integrar sem revisão e gates verdes.

## 3. Arquitetura de destino

O documento de arquitetura recebido propõe a seguinte separação lógica. Os nomes são conceitos de destino, não declaração de crates já existentes:

- **Domínio:** Z80, TMS9918A, SMS VDP, PSG, YM2413, mappers, Scheduler, SG-1000 e SMS.
- **Aplicação:** sessão, thread de emulação, comandos, pacing e portas de host.
- **Infraestrutura Rust:** áudio, vídeo, input, storage e host nulo/headless.
- **Interfaces:** binário Rust autônomo e fronteira PyO3; Qt como launcher/painel de controle.
- **Tooling:** golden masters, vetores, oráculos independentes, trace diff, matriz de compatibilidade e auditorias.

A arquitetura de destino deve ser alcançada incrementalmente, sem criar crates vazios ou mover código apenas para reproduzir uma árvore idealizada. O inventário real e o status por componente estão em [component-status.md](component-status.md).

### Fronteira de host

- Rust deve tornar-se um executável completo, capaz de iniciar ROM, executar a máquina, produzir áudio/vídeo e ler input sem Python.
- Áudio usa uma porta `AudioSink`; vídeo usa `VideoSink`; input usa `InputSource`; storage usa `Storage`; o relógio de host é isolado do tempo emulado.
- Host-null/headless é o alvo de determinismo e golden masters.
- O pacer mapeia tempo emulado para tempo real; nunca decide quais ciclos ou eventos de hardware acontecem.
- A janela de jogo pertence ao host Rust na arquitetura final. Qt permanece launcher, biblioteca, preferências e telemetria.

## 4. Roadmap único P0–P9

### P0 — Baseline e golden masters

- Confirmar SHA, árvore, workspace, APIs, testes, workflows e PRs concorrentes.
- Inventariar cada componente SG-1000/SMS: implementação, consumidor, testes, evidência e pendências.
- Capturar framebuffer, PCM e checkpoints de estado com entradas reproduzíveis e identificação/hash de ROM.
- Repetir capturas para verificar determinismo.
- Classificar lacunas como código ausente, integração pendente, teste ausente ou documentação desatualizada.

**Saída:** baseline rastreável e matriz de componentes; não cristalizar golden master divergente.

### P1 — Esqueleto arquitetural e portões

- Consolidar contratos mínimos compartilhados de Bus, Clock/ClockDomain, Event/Timestamp, IRQ, Snapshot e erros.
- Preparar host-null e testkit para leitura/comparação de golden masters e traces.
- Criar portões CI para dependências arquiteturais e fronteiras Python/Rust.
- Contratos temporais iniciais devem ser rotulados como nominais enquanto não sustentarem execução temporal real.

**Saída:** contratos mínimos e CI capazes de bloquear dependências proibidas.

### P2 — Port as-is de SG-1000 e SMS

- Portar SG-1000 primeiro, preservando TMS9918A, PSG, memória, I/O, entrada e testes.
- Portar SMS em PR separado, preservando VDP Mode 4, perfil PSG, mappers, memória, I/O e testes.
- Manter os executores Z80 específicos até a fase P3.
- Adaptar interfaces sem alterar semântica de hardware.
- Manter a fachada de compatibilidade atual durante a migração.

**Saída M1:** os dois sistemas preservam golden masters e testes portados; nenhuma regressão inexplicada.

### P3 — Z80 compartilhado

- Extrair uma única CPU Z80 a partir do executor SMS, após o port estrutural.
- Parametrizar o barramento e criar adaptadores SG-1000 e SMS.
- Portar a suíte de testes e executar comparação diferencial entre executor antigo e novo.
- Remover duplicações somente quando ambos os sistemas usarem o novo núcleo e não houver desvio semântico inexplicado.

**Saída:** uma implementação de CPU em produção, consumida pelos dois consoles.

### P4 — Engine e hosts Rust standalone

- Criar engine de sessão, thread de emulação, comandos, filas SPSC, triple buffer e configuração.
- Integrar janela Rust, input e saída de áudio.
- Implementar pacer com métricas de underrun, fila, cadência e deriva.
- Integrar storage e binário `sega-emu`.
- Remover o avanço da máquina por QTimer/loop Python.

**Saída M2:** SG-1000 e SMS executam no binário Rust sem Python; host-null e smoke passam.

### P5 — Controle PyO3 e painel Python

- Reduzir PyO3 a controle de ciclo de vida e telemetria somente leitura.
- Reorganizar o frontend em domain/application/adapters/ui, reaproveitando serviços existentes.
- Manter um único adaptador Python responsável por importar a extensão.
- Validar troca de sistema, pause, stop, reset, falhas e encerramento.
- Decidir por ADR se PyO3 permanece em processo ou se haverá processo Rust separado.

**Saída M3:** painel opcional; runtime segue funcional sem ele.

### P6 — Barramento, mappers, I/O e fundação Scheduler-driven (prioridade antecipada)

Esta fase passa a ser a frente ativa de integração temporal, autorizada formalmente em 2026-11-02. A antecipação é incremental: não exige desligar o executor legado, não presume que contratos em revisão estejam aprovados e não antecipa sem evidência a semântica elétrica do SMS.

- Manter uma única origem para os contratos temporais: a PR #418 absorve `BusTransaction`, `BusOperationResult`, os tipos de progresso e a ADR-001 originalmente propostos na #396. A #396 é supersedida e não deve ser integrada em paralelo.
- Completar o estado retomável serializável do Z80 para os safe points implementados, mantendo o executor atômico como referência diferencial.
- Construir o coordenador da máquina que usa o `MachineScheduler` como única autoridade temporal, consumindo deadlines e dispatches sem mover lógica de dispositivo para o Scheduler.
- Migrar o barramento em etapas para transações suspensíveis, com efeitos exatamente-uma-vez, WAIT somente quando modelado por fixture/evidência e sem busy-wait.
- Validar mapas, espelhamento, ROM/RAM/VRAM, decodificação, limites, reset, mapper e I/O sem regressões.
- Manter o runtime legado disponível até que a substituição de cada caminho tenha equivalência comprovada.
- Não conectar a CPU ao VDP scanline legado como atalho; o destino de raster/IRQ é o novo VDP temporal.

**Registro de execução (2026-11-02):**
- T0: a #396 foi a proposta original dos contratos. Na T5a, `contract.rs`, exports e ADR-001 foram absorvidos pela #418 como fonte única; a #396 foi fechada como supersedida, sem merge.
- T1: PRs #412/#413 — contadores checked e codec binário versionado do estado retomável suportado.
- T2: PR #414 — coordenador SMS sobre `MachineScheduler`, com escala temporal explícita.
- T3: barramento de memória transacional, efeitos no deadline, acknowledgement único e WAIT de fixture, incorporado à stack T4.
- T4: PR #416 — integrada em `main` no SHA `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`; head validado `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`; CI #41499228458 PASS em todos os quatro jobs sobre merge ref `cea276159efd382e8289085be4b5f67dccecc753`.
- T5a: PR #418 — código SHA `0c178ed6c8da6aeddf8e25682a2ebc620eec786f`; run `41536695747` PASS nos quatro jobs. O restore valida endereço efetivo `(HL)`, payload com CRC válido e progresso `instruction_tstates` contra o Scheduler; `SmsTemporalMachineState` versão 6 preserva `instruction_started_at`. Execução local: NOT RUN. A atualização documental posterior é validada no CI do SHA final da PR.

**Estado:** T0–T5a integrados incrementalmente; T4 permanece em `SHORT_PASS`. T5a/PR #418 está integrada em `main` pelo merge SHA `6c8e18b1c0a134ee56e8282d1cbc7660373100f8`. O escopo continua restrito ao subconjunto retomável SMS e não representa CPU Z80 completa nem `TEMPORAL_PASS`. P6 permanece ativa para completar a integração temporal, validar mapas/mappers/I/O e ampliar a cobertura de estado e dispositivos, incluindo SG-1000. O executor atômico legado permanece como referência diferencial.

**T5a — PR #418:** expor índices M-cycle/T-state estáveis no microsequenciador para as operações já suportadas e mapear para os contratos canônicos `BusTransaction`/`BusOperationResult` mantidos em `sega-temporal-core`. A #418 absorve a implementação e a ADR-001 da #396; a #396 é supersedida. Ownership, fase, timestamp, restore e conclusão transacional devem permanecer coerentes. Não inferir fases elétricas nem ampliar opcodes nesta etapa.

**Saída T5a (integrada):** metadados M/T serializáveis e mapeamento CPU/bus/Scheduler para o subconjunto retomável existente no SMS, com trace, snapshot/replay e diferencial. SG-1000 ainda não consome esse caminho temporal. Isso não equivale a `TEMPORAL_PASS`. Evidência: PR #418, HEAD `6e73c0655521941c9e4ab7384f4508954306561a`, CI de PR `41538464875` (4/4 PASS); merge em `main` `6c8e18b1c0a134ee56e8282d1cbc7660373100f8`. CI pós-merge `41540515497`: Rust, Python e Windows PASS; Scheduler deterministic stress SKIPPED no evento push, portanto não contado como PASS pós-merge. Auditoria técnica pós-merge do Agente 2: PENDENTE.

### P7 — Fechamento semântico e expansão da integração temporal

- Z80: completar BASE, CB, ED, DD/FD, DDCB/FDCB, flags, I/R, WZ/MEMPTR, INT/NMI, IM 0/1/2, EI delay, HALT, RETI/RETN e instruções de bloco, integrando cada família ao microsequenciador sem remover a referência atômica antes da validação.
- VDPs: avançar o novo SMS1 VDP em raster/fetch/pixel pipeline, sprites, colisão/overflow, status, IRQ, scroll, CRAM e efeitos raster; TMS9918A permanece específico do SG-1000.
- PSG: consolidar divisores, ruído, atenuação, latches, reamostragem e determinismo no caminho Scheduler-driven da máquina.
- YM2413 como etapa própria, condicionada à compatibilidade básica e à evidência de clock/fase.
- WAIT, arbitragem e simultaneidade só recebem semântica física quando sustentadas por documentação primária ou captura reproduzível.
- Cada correção semântica em PR separada, com oráculo independente e atualização justificada de golden master.

**Saída M4:** fechamento funcional SG-1000 + SMS com cobertura semântica e integração temporal parcial demonstrada; não declarar `TEMPORAL_PASS`.

### P8 — Compatibilidade e robustez

- Ampliar a matriz de jogos/homebrew com identidade, cenário, SHA candidato, estado e limitações.
- Executar smoke, regressões longas, troca de mídia e replay determinístico.
- Validar Windows x64, PyO3/Maturin, backends físicos de áudio/vídeo/input e diagnóstico.
- Medir desempenho somente depois de baseline funcional.

**Saída:** compatibilidade delimitada por evidência; sem alegações universais.

### P9 — Comprovação sistêmica e TEMPORAL_PASS

- Fechar a integração Scheduler-driven iniciada em P6 e expandida em P7: CPU, barramento, VDP, PSG, I/O e IRQ.
- Comprovar cronologia causal entre transações e eventos, incluindo WAIT/arbitragem onde houver evidência suficiente.
- Validar chunk invariance, ordenação, determinismo entre execuções equivalentes, snapshots em safe points/transações pendentes e replay.
- Comparar traces runtime com oráculos independentes ou evidência física adequada, registrando ancestralidade, SHA, primeira divergência e limitações.
- Executar regressões completas SG-1000/SMS e gates no mesmo SHA candidato.

**Saída M5:** `TEMPORAL_PASS` somente após integração e evidência temporal completa. Contratos, Scheduler isolado, testes unitários ou aparência visual não satisfazem este marco.

## 5. Golden masters, oráculos e compatibilidade

Golden master responde: “o port preservou o comportamento observado no baseline?”. Não prova que o comportamento esteja correto.

Oráculo independente responde: “o comportamento observado corresponde à especificação ou a uma referência independente?”. Não pode derivar circularmente do mesmo código sob teste.

Cada golden master deve registrar ROM/homebrew identificada por SHA-256, entradas scriptadas, hash de framebuffer por frame, hash de PCM e checkpoints de estado da CPU. ROM comercial permanece local; o repositório guarda apenas identificadores, hashes e passos de reprodução.

A matriz de compatibilidade deve distinguir:
- `BOOT_CONFIRMED`
- `GAMEPLAY_SMOKE`
- `REGRESSION_PASS`
- `VALIDATED`

Boot não equivale a gameplay; gameplay smoke não equivale a compatibilidade universal.

## 6. Estados e gates

Estados de componente:
- `IMPLEMENTED`
- `QUICK_PASS`
- `SHORT_PASS`
- `LONG_PASS`
- `INTEGRATION_PENDING`
- `TEMPORAL_PASS`

Estados de suíte: `PASS`, `FAIL`, `BLOCKED`, `NOT RUN` ou `SKIPPED`, sempre vinculados ao SHA candidato. Check ignorado ou executado em SHA anterior não é aprovação.

Gates obrigatórios:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Build Release/Maturin, testes Windows x64, GPU, áudio e input físicos são adicionais conforme a fatia. Não declarar TEMPORAL_PASS sem integração real.

## 7. Autoridade documental

1. **`docs/project-roadmap.md`:** única fonte de escopo ativo, fases e ordem de implementação.
2. **`docs/project-scope.md`:** princípios, limites e fronteiras arquiteturais; não cria fases.
3. **`docs/component-status.md`:** fotografia dos componentes implementados, integrações pendentes e evidências conhecidas; deve ser atualizada quando uma PR alterar estado.
4. **ADRs:** decisões arquiteturais, com status aprovado/proposto/substituído; não são roadmaps.
5. **Especificações e auditorias:** comportamento, evidências, incertezas e critérios técnicos; não criam cronogramas.
6. **Issues e PRs:** execução e acompanhamento das tarefas, sempre referenciando uma fase deste roadmap.

**Proibido manter roadmaps paralelos**, inclusive por chip, plataforma, mapper, áudio, vídeo, Qt ou pesquisa de hardware. Os documentos de roadmap antigos foram removidos nesta consolidação. Documentos técnicos e componentes existentes permanecem. Referências a roadmaps removidos devem ser corrigidas; documentos históricos que ainda forem necessários devem ser explicitamente marcados como históricos e nunca usados como instrução vigente.

## 8. Plataformas futuras

Game Gear, Mega Drive/Genesis, Sega CD, 32X, SVP e sistemas integrados de terceiros não fazem parte da execução ativa deste roadmap. Após M4, sua inclusão dependerá de nova revisão aprovada do roadmap, baseada no inventário e na arquitetura efetivamente consolidada. Não criar cronogramas ou branches de implementação dessas plataformas antes dessa decisão.


## 9. Registro de integração T4 na main — 2026-11-02

- PR #416 integrada por merge commit em `main`: `3f4a71d7b7bbc1863c20cd3293466405cb3f579b`.
- Head da PR validado: `bfdc7d2fbbcf7712091df9960a7e7e593c18afea`.
- CI de PR #41499228458, sobre merge ref `cea276159efd382e8289085be4b5f67dccecc753`: Rust fmt/clippy/workspace tests, Python Ruff/pytest, Windows x64 vídeo/áudio e Scheduler deterministic stress short — todos PASS.
- CI pós-merge #41499594576: Rust e Python PASS; Windows x64 vídeo/áudio PASS quando concluído; Scheduler stress SKIPPED por ser push em main sem o label de PR. O stress short já havia PASS no CI de PR do mesmo candidato.
- Logs do workspace: SG-1000 42/42; SMS 156/156; Z80 83/83. Os testes integrados novos de overflow/acknowledgement, duração do ciclo interno, diferencial, trace e replay também passaram.
- Testes locais: NOT RUN. Não houve execução de ROM homebrew/golden master nem das suítes externas Z80 nesta integração.
- PRs #412 e #415 têm seus heads como ancestrais do SHA integrado; #412 foi reconhecida pelo GitHub como merged. PRs #413 e #414 foram encerradas como supersedidas: seus heads divergiam da stack consolidada, mas os entregáveis funcionais de snapshot/codec e coordinator estão no diff integrado de #416. Os hashes específicos dos commits divergentes de #413/#414 não foram preservados como ancestrais; a consolidação preservou o conteúdo funcional, não toda a linhagem histórica dessas duas branches.

Estados: T1 `IMPLEMENTED / QUICK_PASS`; T2 `IMPLEMENTED / QUICK_PASS`; T3 `IMPLEMENTED / QUICK_PASS`; T4 `IMPLEMENTED / SHORT_PASS`; T5a `IMPLEMENTED / QUICK_PASS` (PR #418 integrada; CI da PR 4/4 PASS; CI pós-merge com Scheduler stress SKIPPED); SMS temporal completo `INTEGRATION_PENDING`; auditoria técnica pós-merge T5a `PENDING`; `TEMPORAL_PASS` NÃO declarado.
