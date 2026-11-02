# Handoff — próxima etapa: desenho da integração temporal SMS/Z80/barramento

**Data de referência:** 2026-11-02  
**Repositório:** https://github.com/leonardo201800478/sega-emulator  
**Objetivo:** iniciar uma nova conversa e continuar exatamente do estado remoto documentado, sem reiniciar o trabalho concluído.

## 1. Papel

Atue como arquiteto de software, engenheiro de emulação e responsável técnico pelo Sega Emulator. A prioridade agora é preparar a primeira integração real entre a fundação temporal Rust e o caminho de execução do Master System (SMS), começando por inventário e contrato verificável. Não comece refatorando CPU/VDP/barramento antes de entender o código e as evidências.

## 2. Primeira operação obrigatória: inventário remoto

O GitHub é a fonte da verdade. Antes de qualquer conclusão ou edição:

1. Consulte SHA atual de `main`.
2. Consulte PRs #358, #359 e a PR documental desta etapa (se já criada): state, draft, base, head SHA, mergeability, checks e comentários.
3. Confirme se a cadeia de branches continua:
   - #358 → base `main`;
   - #359 → base branch de #358;
   - PR documental → base branch de #359.
4. Verifique se houve commits posteriores aos SHAs de referência abaixo.
5. Consulte o estado atual da issue #158 e seus comentários/ledger.
6. Localize e leia os documentos atuais:
   - `docs/evidence/adr-0013-snapshot-restore-validation.md`
   - `docs/architecture/machine-scheduler-technical-reference.md`
   - `docs/architecture/adr/0010-rust-runtime-temporal-ownership.md`
   - `docs/architecture/adr/0012-event-ordering-dimensions.md`
   - `docs/architecture/adr/0013-deterministic-scheduler-snapshot-restore.md`
   - `docs/architecture/adr/0014-temporal-device-execution-and-synchronization.md`
   - `docs/architecture/adr/README.md`
   - `docs/prompts/sms-temporal-integration-design-handoff-2026-11-02.md`
   - auditoria técnica SMS, roadmap SG-1000/SMS e documentação do caminho Z80/bus existente.
7. Inspecione código real no SHA/branch atual; não use o texto deste handoff como substituto da auditoria.

Se algum SHA ou estado divergir, preserve a história e atualize o plano com base no estado remoto. Não faça rebase/retarget automático de PRs empilhadas.

## 3. Estado técnico consolidado — ponto de partida

### Scheduler / Temporal Core

A fundação foi implementada na stack de PRs #358/#359:
- `native/temporal_core`: MachineTime, razões racionais, fase preservada, ClockEdgeTime e fila determinística.
- `native/machine_scheduler`: registro de clock domains, dispatch, avanço idle, snapshots/restore atômicos, codec versionado e replay diferencial.
- Ordenação: `(MachineTime, HardwarePhase, PlatformPriority, stable_sequence)`.
- `HardwarePhase` é estrato ordinal de ordenação; não é tempo fracionário, T-state nem fase elétrica medida.
- `ClockEdgeTime` calcula posição racional exata de borda, mas a fila atual despacha somente em `MachineTime` inteiro.
- O Scheduler é genérico; não executa instruções, não faz arbitragem de barramento e não chama CPU/VDP/PSG.
- A integração temporal de dispositivos continua pendente.

### PR #358

- URL: https://github.com/leonardo201800478/sega-emulator/pull/358
- Branch: `test/scheduler-deterministic-stress-harness`
- Head de referência: `0d8ad97e79ac50c4687e1400dd5bb434ca5f3607`
- Base `main`: `0da3098893cce4d80f7a22291d00488e4e7d0f9b`
- Estado observado em 2026-11-02: Open, Draft, não merged.
- CI ordinário workflow 41378338137: seis gates Rust/Python PASS nesse SHA.
- SHORT workflow 41379180394: etapa de stress concluiu 4 seeds × 20.000 = 80.000 eventos, limite 512 MiB, teste passou; workflow global CANCELLED e etapa de testes workspace Rust não completou. Atribuir SHORT_PASS somente à etapa stress.
- LONG workflow 41379203192: todos os jobs SUCCESS; 16 seeds × 250.000 = 4.000.000 eventos, limite 2 GiB, teste passou.
- O stress harness agenda eventos diretamente por MachineTime; não comprova interações entre domínios de dispositivos.

### PR #359

- URL: https://github.com/leonardo201800478/sega-emulator/pull/359
- Branch: `test/scheduler-multidomain-advance-replay`
- Base: branch #358.
- Head de referência: `a544fe367f67c1f6c6d622efcdb528e222ab53c3`
- Estado observado: Open, Draft, não merged.
- CI workflow 41381005224: jobs Rust quality/unit regression e Python lint/regression SUCCESS nesse SHA; stress job SKIPPED pela condição do workflow.
- Teste `native/machine_scheduler/tests/multi_domain_replay.rs`:
  - três domínios racionais registrados fora da ordem de ID;
  - eventos com timestamps duplicados;
  - ordem esperada fixa `[0, 1, 2, 4, 3, 5, 6]`;
  - comparação uninterrupted versus snapshot/restore;
  - chunks idle intermediários comparados com oracle aritmético independente;
  - overflow exige `SchedulerError::Clock(ClockError::ArithmeticOverflow)` e não altera estado.
- A cobertura usa somente timestamps inteiros MachineTime. Não prova sub-tick dispatch nem integração de dispositivos.

### PR documental desta etapa

A branch de documentação foi criada a partir do head #359:
- Branch: `docs/scheduler-progress-handoff-2026-11-02`
- Base SHA de criação: `a544fe367f67c1f6c6d622efcdb528e222ab53c3`
- A PR deve ser criada/consultada como stack sobre `test/scheduler-multidomain-advance-replay`, não diretamente sobre `main`.
- Confirme o head e o CI atual antes de confiar nos detalhes.

### Issue #158 — auditoria Z80

- URL: https://github.com/leonardo201800478/sega-emulator/issues/158
- Título: “Z80 SMS audit A — matriz completa de opcodes e semântica”.
- Estado observado em 2026-11-02: Open.
- Escopo: matriz dos espaços base/CB/ED/DD/FD/DDCB/FDCB; flags S/Z/H/PV/N/C e X/Y; WZ/MEMPTR, Q, registradores alternativos, IX/IY, prefixos e efeitos em memória; relatório de lacunas e vetores fixos.
- Não assumir que a auditoria de opcodes está concluída. Ler comentários recentes e ledger antes de integrar o core Z80 ao Scheduler.

## 4. Objetivo da nova etapa

Produzir um **design técnico verificável** para a primeira integração temporal SMS, sem implementar prematuramente timing físico.

O foco inicial é rastrear o caminho real:
1. Z80 instruction execution / T-state accounting;
2. memory reads/writes;
3. I/O reads/writes;
4. VDP/PSG advancement;
5. IRQ/NMI/reset e linhas de controle;
6. mapper/bank switching quando relevante;
7. fronteira PyO3 e chamadas de frame atuais.

Determinar onde o tempo é atualmente acumulado e quando efeitos de barramento se tornam visíveis. Identificar quais efeitos são aplicados retrospectivamente no fim da instrução e quais já ocorrem durante a execução.

## 5. Fontes e evidência de hardware

Pesquisar documentação primária e fontes técnicas confiáveis, priorizando SMSPower e documentação de hardware/medidas reproduzíveis. Consultar também os PDFs de manuais Z80 já anexados ao projeto/conversa, mas distinguir:
- especificação genérica do Z80;
- particularidades do barramento/placa SMS;
- comportamento de ASICs SMS1/SMS2 e variantes de VDP;
- diferenças de região NTSC/PAL.

Para cada afirmação temporal, registrar:
- URL/documento e página/seção;
- trecho ou descrição objetiva da evidência;
- se é especificação, medição, observação de emulador ou inferência;
- confiança e lacunas;
- como será transformada em teste independente.

Não inferir frequência, divisor, prioridade, wait state, contenção ou ordem elétrica somente porque a API atual exige um valor. Não atribuir valor físico a HardwarePhase sem fonte.

## 6. Entregáveis obrigatórios antes da implementação

### A. Inventário de execução atual
Diagrama de sequência real do SMS desde chamada de frame até CPU, bus, VDP, PSG e IRQ. Identificar arquivos, funções e linhas no SHA auditado.

### B. Matriz de transações
Tabela para cada operação relevante:
- opcode/micro-op ou origem;
- endereço/tipo (memory, I/O, interrupt acknowledge);
- instante em que o endereço/dados ficam observáveis;
- ciclos nominais conhecidos;
- wait states/contenda documentados;
- componente que atualmente avança;
- comportamento desejado;
- evidência e incerteza.

### C. Contrato mínimo CPU ↔ bus ↔ scheduler
Propor a menor interface que permita a CPU parar em fronteiras observáveis de barramento e retomar deterministicamente. Não congelar trait pública antes de mapear os casos:
- M1/opcode fetch;
- leitura/escrita de memória;
- leitura/escrita I/O;
- stack e operações read-modify-write;
- interrupções e acknowledge;
- HALT, prefixos, instruções repetidas;
- wait states;
- eventos simultâneos CPU/VDP.

Não presumir que uma instrução agregada possa ser aplicada retrospectivamente se isso alterar efeitos visíveis no barramento.

### D. Decisão sobre granularidade temporal
Determinar, com evidência, se SMS precisa:
- somente MachineTime inteiro;
- timestamp sub-tick racional;
- micro-op/safe-point com offsets inteiros;
- combinação de timestamp e fase ordinal.

Se sub-tick for necessário, abrir ADR específica antes de implementar. Não usar HardwarePhase como substituto de fração temporal.

### E. Plano de testes/oráculos
Definir traces determinísticos para:
- memória e I/O;
- sincronização CPU/VDP;
- leitura de status e clear-on-read;
- IRQ/VBlank/HBlank;
- wait states e contenda;
- snapshot/restore no meio de transação;
- replay idêntico;
- regressão SG-1000.
Usar Ares/Gearsystem ou hardware real como comparadores quando apropriado, sem tratar outro emulador como autoridade absoluta. Fixar ROM/test fixture por hash e licença.

### F. ADR / implementação fatiada
Atualizar ADR existente ou criar nova ADR somente após o design. Definir fatias pequenas, cada uma com:
- arquivos e APIs afetados;
- critério de aceitação;
- testes antes/depois;
- risco de regressão;
- gates CI no SHA exato.

## 7. Arquitetura imutável

- Backend de emulação 100% Rust.
- Python/PySide6 apenas GUI, configuração e tooling; sem clocks, ciclos, registradores mutáveis, H/VBlank, barramento ou callbacks por instrução.
- PyO3 deve ser fronteira de alto nível; sem callback por instrução.
- Scheduler genérico, sem semântica específica de SMS/Z80.
- Determinismo e correção de hardware prevalecem sobre desempenho.
- Sem JIT, frame skip, hacks de timing ou paralelismo especulativo.
- Sem floats para clocks/ciclos/fases.
- Não quebrar SG-1000; preservar testes e comportamento existente.
- Save states devem preservar estado temporal necessário à continuação determinística.

## 8. Git e validação

- Nunca commitar diretamente em `main`.
- Nunca force-push, reset destrutivo ou sobrescrever trabalho de terceiros.
- Mudanças não triviais em branch própria + PR Draft.
- Não mergear #358, #359 ou PR documental sem autorização explícita.
- Não rebasear branches empilhadas sem plano e confirmação dos heads.
- Gates obrigatórios no SHA candidato:
  ```text
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
  python -m pytest
  python -m ruff check .
  python -m ruff format --check .
  ```
- Marcar cada gate PASS/FAIL/BLOCKED/NOT RUN e sempre associar ao SHA exato.
- Não alegar execução local se não foi realizada.
- QUICK_PASS/SHORT_PASS/LONG_PASS/TEMPORAL_PASS têm escopos distintos. Não declarar TEMPORAL_PASS sem integração de dispositivos e oráculos independentes.

## 9. Próximo passo imediato

Após o inventário remoto e leitura dos documentos:
1. Inspecione o código atual do SMS/Z80/barramento e a issue #158.
2. Produza primeiro o diagrama do fluxo de execução e a matriz de transações.
3. Faça pesquisa de hardware para preencher somente o que for suportado por evidência.
4. Apresente findings, incertezas e critérios de aceitação.
5. Só então proponha a primeira fatia de implementação.

Não comece pelo scheduler novamente. E1–E7, stress SHORT/LONG e replay multi-domain já possuem evidência delimitada e documentada. O trabalho agora é descobrir como conectar a máquina real sem comprometer causalidade ou inventar timing.
