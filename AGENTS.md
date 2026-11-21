# Instruções para agentes de código — Sega Emulator

A fonte da verdade é a `main` remota, seus commits, testes e workflows. Antes de cada tarefa, consulte o HEAD, PRs, branches, diffs e CI atuais. Não reutilize SHAs ou conclusões antigas sem revalidação.

## Arquitetura normativa

- **Runtime/hardware:** 100% Rust. CPU Z80 compartilhada, memória, barramentos, mappers, VDPs, PSG/FM, periféricos, interrupções, Scheduler, timing, estados e execução pertencem ao backend.
- **GUI/host ativo:** Slint 1.18.x.
- **Renderização:** WGPU 30.x; Vulkan como backend inicial no Windows x64.
- **Áudio físico:** CPAL 0.18.2.
- **Input:** eventos nativos Slint para teclado e `sega-input-runtime`/SDL3 para gamepads, respeitando o estado integrado atual.
- O host gráfico anterior não é caminho ativo do produto. Código remanescente só pode ser tratado como legado/transitório até inventário de consumidores e remoção segura; não desenvolver novas funcionalidades nele.

## Separação de responsabilidades

A GUI cuida de janelas, menus, launcher, biblioteca, configurações, logs, telemetria e integração com o SO. É proibido implementar hardware, instruções, clocks, temporização ou callbacks por instrução na GUI.

A integração com runtime permanece coarse-grained: `load_rom()`, `start()`, `pause()`, `stop()`, `reset()`, `step_frame()`, snapshots e frames com ownership explícito. Preservar `SmsTemporalWorker`, `FrameMailbox`, `MachineScheduler` e componentes validados.

## Tempo e determinismo

- `MachineScheduler` é a autoridade única de clocks, eventos e ordem causal.
- Preservar sincronização CPU/VDP/áudio/barramentos, WAIT states reais quando demonstrados, operações suspensíveis/retomáveis e estado necessário à retomada.
- Execução contínua e em blocos deve ser equivalente nas dimensões testadas.
- Save States/replays devem preservar determinismo.
- Renderização, áudio físico e input não podem avançar nem corrigir clocks do hardware.
- Não criar Scheduler, CPU, PSG, VDP, worker ou pipeline paralelo por conveniência.

## Roadmap de plataformas

1. SMS1/SMS2.
2. SG-1000 — concluir a máquina temporal na arquitetura existente.
3. Game Gear como sistema próprio.
4. Expansões e acessórios 8-bit após Game Gear: YM2413 integrado do Master System japonês (não o FM Sound Unit externo do Mark III); Sega Card/My Card como mídia; 3D Glasses/SegaScope; periféricos de controle e interfaces de expansão conforme compatibilidade documentada. Card Catcher C-1000 está fora do escopo de implementação.
5. Mega Drive/Genesis.
6. Sega CD.
7. 32X.
8. Compatibilidade, consolidação e aceite final.

Não iniciar 16-bit antes do fechamento dos sistemas 8-bit e da fase de acessórios. Não reescrever arquitetura nem criar implementações paralelas para SG-1000.

## Z80

O Z80 compartilhado está encerrado no escopo praticável do perfil NMOS clássico utilizado pelos sistemas 8-bit suportados, com limitações explicitadas. E8/ZEXDOC/ZEXALL é validação complementar futura, não bloqueio artificial permanente. Não reabrir E7 sem bug, regressão ou evidência técnica concreta. CMOS/clones e precisão elétrica não cobertos continuam NOT_VERIFIED.

## Método e gates

1. Auditar main/PR/CI no SHA exato.
2. Consultar documentação primária, referências independentes e versões atuais das bibliotecas quando relevantes.
3. Definir contratos, ownership, estados, timing e critérios de aceite antes de implementar.
4. Implementar incrementos pequenos em branch/PR.
5. Executar `cargo fmt --all -- --check`, Clippy e testes direcionados; depois workflows aplicáveis.
6. Classificar PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca é PASS.
7. Auditar diff e preservar evidências históricas; nunca atribuir teste a SHA diferente.
8. Não fazer merge sem CI aplicável verde no HEAD final e autorização explícita.

## Uso do Codex

- **Usar o Codex exclusivamente para executar o merge de uma Pull Request**, depois que o agente concluir a auditoria, confirmar os gates aplicáveis no HEAD exato e verificar que a PR está mergeável.
- Não delegar ao Codex implementação, refatoração, depuração, pesquisa técnica, leitura/auditoria do repositório, execução de testes, análise de CI, revisão de diffs, criação/atualização de PRs, reconciliação de branches ou preparação de commits.
- O agente responsável pela tarefa deve conduzir diretamente essas atividades, registrar evidências e decidir tecnicamente se os critérios de merge foram satisfeitos.
- Antes de solicitar/executar o merge via Codex, confirmar novamente: repositório e número da PR, branch base, SHA atual do HEAD, checks obrigatórios e workflows específicos aplicáveis, comentários/revisões pendentes e ausência de conflitos.
- O merge só pode ocorrer com autorização explícita do usuário ou autorização permanente já registrada para aquela sequência, usando o SHA esperado do HEAD. Nunca contornar branch protection, checks obrigatórios ou revisões.
- Se qualquer gate estiver pendente, falhar, estiver associado a outro SHA ou não puder ser verificado, não acionar o Codex para merge. Corrigir ou aguardar e repetir a auditoria.
- Após o merge, o agente deve verificar o SHA resultante na branch base, o estado da PR e a integridade da árvore integrada. O uso do Codex termina na operação de merge; a verificação pós-merge continua sob responsabilidade do agente.

Não usar Python, Qt, PyO3 ou Maturin no runtime, frontend, CI ou tooling obrigatório. Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.
