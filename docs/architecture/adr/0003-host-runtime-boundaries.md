# ADR-0003 — fronteiras entre runtime Rust e host Qt/PyO3

- **Status:** Proposto para implementação
- **Data:** 2026-10-31
- **Escopo:** input, vídeo, áudio, frontend, PyO3 e ferramentas

## Contexto

O objetivo do projeto é manter todo hardware emulado no runtime Rust. Python/PySide6 é responsável pela interface, configuração, carregamento de ROMs, ferramentas, automação e diagnóstico. PyO3/Maturin deve ser uma fronteira fina.

As PRs abertas oferecem implementações e requisitos distintos: #313 contém input Rust/SDL3; #300 documenta a migração anterior de input; #301 documenta a migração de vídeo; #298 registra 35 decisões funcionais Qt; #297 adiciona uma ferramenta Python que executa o core Rust em instâncias SMS isoladas.

## Decisão

### Qt/Python

- Qt/PySide6 mantém janela, menus, diálogos, acessibilidade, configuração, seleção/carregamento de mídia, logs da aplicação e ferramentas.
- Python não implementa nem agenda CPU, memória, barramento, VDP, PSG, interrupções ou temporização de hardware.
- PyO3 expõe comandos, configurações, eventos e snapshots coarse-grained. Não há callbacks Python por instrução, acesso de memória, pixel ou amostra PCM.
- Evitar cópias grandes de framebuffer/PCM no caminho normal.
- Preservar os contratos funcionais de #298, incluindo Pause mantendo a ROM/sessão e Stop encerrando a sessão e descarregando a ROM sem fechar Qt.

### Input

- Captura e normalização física pertencem ao runtime Rust/backend nativo.
- Eventos de input devem carregar timestamp monotônico de origem, sequência e estado normalizado. O adaptador da máquina converte/associa esses eventos à timeline emulada com política explícita e determinística.
- O timestamp do host nunca substitui `MachineTime` e nunca avança CPU/dispositivos por conta própria.
- A implementação #313 é a candidata principal para a primeira fatia Windows/SDL3. O contrato atual de `DeviceReading.timestamp_ns` é host-monotonic; o scheduler deve fazer a conversão na fronteira.
- A chamada PyO3 `poll_actions` por frame é transitória até avaliação de custo e contrato final de eventos. Não é polling Python de APIs físicas, mas seu custo e latência precisam ser medidos. O alvo é evitar dependência do loop Qt para determinar a ordem temporal dos eventos de gameplay.
- Não declarar compatibilidade física, force feedback, rumble, hotplug ou redução de input lag sem matriz de dispositivos e medições.

### Vídeo

- Rust será dono do renderer, recursos, filtros e apresentação quando a migração for implementada.
- Qt fornece a janela/surface e eventos de UI, com ownership, thread e lifetime explícitos.
- VSync/VRR/frame pacing são políticas de apresentação; nunca avançam o relógio lógico da máquina.
- Backend solicitado, disponível, inicializado, efetivamente ativo e fallback aplicado devem ser estados distintos e diagnosticáveis.
- Avaliar D3D11/12, Vulkan, wgpu ou QRhi por interoperabilidade, compatibilidade e medições; não escolher backend por preferência sem profiling.

### Áudio

- Geração digital, mixagem/resampling e sessão de saída permanecem Rust-owned.
- Worker WASAPI/XAudio2/PortAudio/SDL não controla o scheduler nem avança CPU/chips.
- Qt recebe estado e telemetria, não amostras PCM no caminho normal.
- Diferenciar latência estimada de latência ponta a ponta medida; registrar underruns e backend efetivo.

### Ferramentas e auditorias

- Ferramentas Python podem orquestrar instâncias Rust isoladas e consumir APIs públicas/headless.
- A #297 deve continuar sem frame pacing de apresentação, sem alterar a sessão ativa e sem gravar bytes de ROM/BIOS em relatórios/logs.
- A ferramenta não deve depender de internals privados de SMS; adaptar quando a API pública de máquina/scheduler estiver estabilizada.

## Critérios de aceitação

- [ ] Nenhum runtime Python de hardware ativo em produção.
- [ ] Nenhuma chamada a APIs físicas de input por Python.
- [ ] Fronteira PyO3 sem callbacks por instrução/acesso/pixel/amostra.
- [ ] Input host normalizado e convertido para eventos ordenados da máquina.
- [ ] Frame pacing/VSync/VRR não avançam o scheduler.
- [ ] Áudio e vídeo desacoplados do relógio lógico.
- [ ] Lifecycle, threads, ownership e shutdown dos backends testados.
- [ ] Diagnóstico distingue backend solicitado, disponível, inicializado, ativo e fallback.
- [ ] Testes de integração e medições no Windows comprovam as alegações de latência/compatibilidade.
- [ ] #297 utiliza instância isolada e não expõe dados de ROM/BIOS.

## Consequências

A #313 pode ser integrada sem esperar pela migração de vídeo, mas precisa de um adaptador temporal e validação física. A #300 permanece referência de critérios, não uma implementação concorrente. A #301 e #298 são requisitos de arquitetura/UX, não evidência de runtime migrado. A #297 permanece ferramenta de diagnóstico e deve acompanhar a evolução da API pública.

## Referências internas

- [F0 — inventário remoto e reconciliação das PRs](../migrations/f0-remote-inventory-2026-10-31.md)
- [PR #313 — input Rust/SDL3](https://github.com/leonardo201800478/sega-emulator/pull/313)
- [PR #300 — plano anterior de input](https://github.com/leonardo201800478/sega-emulator/pull/300)
- [PR #301 — plano de vídeo](https://github.com/leonardo201800478/sega-emulator/pull/301)
- [PR #298 — modelo funcional Qt](https://github.com/leonardo201800478/sega-emulator/pull/298)
- [PR #297 — auditoria em lote SMS](https://github.com/leonardo201800478/sega-emulator/pull/297)
