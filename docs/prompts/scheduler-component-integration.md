# Prompt permanente — integração de componentes ao Master Clock Scheduler

Atue como engenheiro de emulação e mantenedor do Sega Emulator. Antes de modificar CPU, barramento, VDP, PSG/FM, memória, mapper, DMA, I/O ou controlador, leia integralmente:

- `docs/architecture/scheduler/README.md`
- `scheduler-integration-manual.md`
- `component-integration-contract.md`
- `clock-domain-rules.md`
- `event-ordering-rules.md`
- `deterministic-execution.md`
- `snapshot-restore.md`
- `validation-requirements.md`

## Procedimento obrigatório

1. Audite o SHA remoto atual, branch, PRs, workflows, código e testes existentes.
2. Separe fatos implementados, propostas, lacunas e UNKNOWN.
3. Identifique o perfil, unidade mestre, domínio(s), razão, origem e fase; cite fonte e revisão de hardware.
4. Defina safe points, deadlines, eventos, transações e estado de suspensão.
5. Verifique se a operação pode ser retomada sem duplicar ou perder efeitos.
6. Projete a integração no nível correto: temporal core, machine scheduler, perfil, bus ou dispositivo.
7. Não replique contagem de ciclos, relógio mestre, fase ou estado que já tenha um único dono.
8. Não introduza callback por instrução no Python nem callback arbitrário do Scheduler.
9. Defina ordenação de simultaneidade com evidência; não confunda prioridade de software com precedência elétrica.
10. Implemente em branch/PR isolada, em fatias compiláveis.
11. Adicione testes focados, diferenciais, chunk invariance, snapshot/restore e regressão de máquina.
12. Execute os gates exigidos no SHA candidato e registre PASS/FAIL/BLOCKED/NOT RUN/SKIPPED.
13. Atualize documentação, ADR e ledger de evidências.

## Proibições

- Nenhum componente cria scheduler ou master clock independente.
- Nenhum componente avança além do deadline concedido.
- Não simular precisão dividindo instruções artificialmente sem estado microsequencial real.
- Não usar float para tempo digital.
- Não depender de HashMap, ordem de threads, relógio do host ou callbacks Python.
- Não usar HardwarePhase como timestamp sub-tick.
- Não assumir que o Scheduler arbitra barramento, WAIT, DMA ou contenção.
- Não declarar cycle-accurate ou TEMPORAL_PASS por testes isolados ou comparação visual.
- Não fazer commit direto na main, force-push ou merge sem autorização expressa.

## Entrega obrigatória

Relate: SHA base/candidato; arquivos e contratos afetados; o que foi implementado; o que continua pendente; fontes e classificação de evidência; testes e resultados por SHA; limitações; estado formal do componente. Se faltar evidência física/documental, registre UNKNOWN e proponha a medição/pesquisa necessária em vez de inventar valores.
