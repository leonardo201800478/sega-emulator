> **Atualização de estado (2026-10-27):** o hardware SG-1000 ativo é Rust-only. A implementação Python antiga e ferramentas/testes que dependiam dela estão em `legacy/python_reference/`; não fazem parte do runtime e os comandos históricos abaixo não são caminhos de execução ativos.

# Plano de migração do runtime para Rust

## Decisão

**Estado atualizado (2026-10-27):** a migração do runtime SG-1000 para Rust foi aplicada na PR #140 (ainda em revisão). A interface Qt/PySide6 e os carregadores de ROM/perfis permanecem em Python; o hardware ativo está em `native/sg1000_core`.

O objetivo é que a execução da emulação — CPU, barramentos, memória, dispositivos, sincronização e composição da máquina — ocorra em Rust sem chamadas Python por acesso de memória ou por instrução. PyO3 será a fronteira entre o runtime Rust e a interface Qt, não o mecanismo interno de comunicação entre componentes de hardware.

## Estado inicial confirmado

- `main` no commit `593c2dc8fdb1be55c548a3979d24b8d49c589b52`.
- O núcleo Z80 Rust já existe em `native/z80_core`, com ALU separada em `native/z80_alu`.
- A extensão PyO3 atual encaminha operações para a máquina Rust consolidada; não recebe um barramento Python por acesso de memória. O GIL continua presente na fronteira de chamadas da aplicação.
- A antiga máquina SG-1000 Python integrava Z80, barramento/memória, TMS9918A, SN76489 e sincronização; foi arquivada em `legacy/python_reference/`.
- Baseline local informado nesta sessão: 2.317 testes aprovados; os quatro testes direcionados do backend Rust também passaram.
- Benchmark sintético local anterior à migração nativa, com 100.000 instruções e 7 repetições: CPU isolada Rust 1,078× a velocidade Python; máquina completa Rust 0,95× (cerca de 5% mais lenta). O workload é `JP 0000h`, não representa desempenho de jogos. O usuário informou depois que o resultado melhorou após o build Release, mas não há uma saída numérica completa registrada aqui para atribuir um novo fator.
- Esses resultados são evidência de throughput em carga sintética, não prova de compatibilidade universal, precisão ciclo a ciclo ou FPS em jogos.

## Escopo

### Migrar para Rust

- Runtime da CPU e estado de execução.
- Barramento, mapa de memória, ROM/RAM e portas de I/O.
- Máquina SG-1000 e sincronização temporal.
- VDP TMS9918A: registradores, VRAM, status, interrupções e renderização.
- PSG SN76489AN: registradores, divisores, tons, ruído, amostragem e estado determinístico.
- Entrada e integração de periféricos.
- Carregamento/validação de ROM, CLI/headless e lógica de sessão necessária ao runtime.
- APIs de estado, reset, pause/step e diagnóstico necessárias à integração com a UI.
- O frontend Pygame Python será descontinuado quando o Qt passar a consumir o runtime Rust; não manteremos dois frontends Python como objetivo final.

### Permanecer em Python

- Frontend Qt/PySide6, widgets, menus, janelas e ciclo de vida da UI.
- Testes pytest, harnesses diferenciais, scripts de benchmark, auditorias e ferramentas de desenvolvimento, salvo quando um componente de execução exigir mudança.
- Durante a migração, a implementação Python de referência permanece disponível apenas para testes diferenciais e comparação; não será chamada como fallback silencioso pelo runtime Rust.

A interface Qt é a única interface gráfica ativa. O frontend Pygame e a execução headless pela CLI foram arquivados; a CLI ativa limita-se a iniciar Qt e carregar/listar ROMs.

## Sequência de implementação

### Fase 0 — Contratos e baseline

- Inventariar módulos Python de runtime, dependências, contratos públicos e consumidores Qt/Pygame/CLI.
- Fixar testes de referência para estado da CPU, memória, portas, quadros, estado do VDP e amostras do PSG.
- Registrar quais testes são funcionais, diferenciais, determinísticos e de desempenho.
- Não declarar precisão de ciclo a ciclo sem testes temporais específicos.

**Saída:** matriz de módulos, contratos e critérios de aceitação antes de alterar o comportamento.

### Fase 1 — Núcleo Rust sem callbacks Python

- Criar um crate de runtime Rust que possua CPU, memória/barramento e I/O nativamente.
- Reutilizar o código Z80 Rust existente sem duplicar a implementação.
- Substituir o acesso de barramento via `Py<PyAny>` por interfaces/tipos Rust nativos no caminho de execução.
- Expor à UI uma API PyO3 de alto nível, como criação da máquina, `step/frame`, reset, estado e framebuffer.
- Manter o backend Python como referência diferencial até a fase passar.

**Aceitação:** mesma sequência de instruções e estado observável nos vetores de referência; nenhuma chamada Python por leitura/escrita de memória ou por instrução; testes Rust e Python de integração aprovados.

### Fase 2 — Máquina e temporização SG-1000

- Migrar a composição da máquina e a agenda/sincronização de componentes.
- Tornar explícitos os contratos de clocks, T-states e eventos sem presumir que ciclos nominais equivalem a ciclos de barramento.
- Comparar estado e hashes determinísticos em checkpoints equivalentes.

**Aceitação:** execuções repetidas determinísticas e regressões aprovadas; desvios temporais documentados.

### Fase 3 — VDP TMS9918A

- Migrar registradores, VRAM, portas, status, modos gráficos, sprites e geração de framebuffer.
- Preservar a semântica existente inicialmente; corrigir fidelidade em alterações separadas, sustentadas por testes.
- Usar testes sintéticos e cenas homebrew; comparar snapshots com referências somente quando ROM/revisão, entradas e instante forem equivalentes.

**Aceitação:** testes por modo, registradores, wraparound de VRAM, status e sprites; regressões de framebuffer reproduzíveis.

### Fase 4 — PSG SN76489AN e áudio

- Migrar estado dos registradores, divisores, canais de tom, ruído/LFSR, atenuação, acumulação e buffer de amostras.
- Separar geração digital, temporização e saída analógica/frontend.
- Validar frequências, formas de onda, ruído, determinismo, buffer e regressões de amostras/espectro.

**Aceitação:** vetores de áudio reproduzíveis e sem mudanças silenciosas na cadência temporal.

### Fase 5 — Entrada, carregamento e serviços de runtime

- Migrar a lógica de portas/joystick e contratos necessários à máquina.
- Migrar carregamento e validação de ROM para uma biblioteca Rust com erros tipados; manter caminhos e seleção de arquivos na UI Qt.
- Migrar logs/checkpoints necessários ao estado do runtime sem serializar ou compartilhar ROMs comerciais.

**Aceitação:** compatibilidade dos formatos atualmente suportados e testes de erro, seleção e integridade.

### Fase 6 — Qt como frontend Python fino

- Trocar a dependência direta da máquina Python por uma API de alto nível da extensão Rust.
- Manter a UI e o comportamento de pause, reset, seleção de núcleo/estado, entrada e apresentação.
- Garantir que chamadas longas do runtime não bloqueiem desnecessariamente a UI; o modelo de threads será definido com sincronização explícita.
- Não mudar clocks lógicos para compensar VSync, VRR ou buffering.

**Aceitação:** smoke tests Qt e validação manual de abrir ROM, executar, pausar, resetar, entrada, vídeo, áudio e logs.

### Fase 7 — Remoção do runtime Python

- Remover caminhos de runtime Python somente após paridade suficiente e migração dos consumidores.
- Manter referências Python sob `tests/reference` ou mecanismo equivalente apenas quando necessárias à validação diferencial.
- Atualizar instalação, empacotamento, CI, documentação e matriz de plataformas.
- Não remover a referência Python antes de haver evidência para substituí-la.

**Aceitação:** instalação limpa no Windows, build Rust Release, fmt/Clippy, testes Rust, pytest de integração, Ruff, smoke test Qt e benchmark comparável; nenhuma dependência de runtime em módulos Python de emulação fora da camada Qt.

## Regras de segurança da migração

1. Uma fase por PR, com mudanças pequenas e reversíveis.
2. Nunca migrar e alterar a fidelidade de hardware na mesma mudança, salvo correção necessária demonstrada por teste.
3. Python/Rust devem ser comparados com estado inicial, ROM/teste, sequência de entradas e limites de avanço equivalentes.
4. Não aceitar ganho de velocidade se houver divergência funcional ou temporal.
5. Nenhum fallback silencioso para Python quando o backend Rust não suportar uma operação.
6. Não versionar ROMs comerciais, logs locais, perfis ou artefatos de build.
7. Benchmarks devem informar carga, aquecimento, repetições, mediana e perfil de build; não extrapolar instruções/s diretamente para FPS.
8. Cada PR deve informar arquivos alterados, testes realmente executados, limitações e próximo marco.

## Estado da execução — 25/10/2026

A Fase 0 foi registrada e as duas primeiras fatias de migração estão implementadas nesta série de PRs.

### Fase 1 — componentes nativos

- `native/z80_core/src/memory.rs`: barramento nativo com ROM espelhada, RAM interna espelhada e RAM de cartucho configurável.
- `native/z80_core/src/vdp.rs`: portas, registradores, VRAM, temporização raster, VBlank, status de sprites e renderização dos quatro modos gráficos modelados.
- `native/z80_core/src/audio.rs`: PSG SN76489AN e sincronizador de writes por T-state.
- `RustZ80Cpu.with_native_memory`: o caminho nativo acessa a memória sem invocar métodos Python por byte.
- `tests/test_sg1000_cpu_backend.py`, `tests/test_tms9918_rust_parity.py` e `tests/test_sn76489_rust_parity.py`: integração CPU/memória e testes diferenciais de VDP, PSG e sincronização.

### Fase 2 — coordenação nativa

A [PR #93](https://github.com/leonardo201800478/sega-emulator/pull/93) adiciona `native/z80_core/src/machine.rs`:

- `NativeSG1000Machine` coordena `step` e `run_frame` em Rust.
- `NativeSG1000Io` roteia portas para VDP, PSG e joystick; o Z80 chama o roteamento nativo diretamente, sem callback a métodos Python.
- A API Python `SG1000Machine` mantém seus atributos e delega operações de máquina ao runtime Rust quando `cpu_core="rust"`.
- `tests/test_sg1000_native_machine.py` compara instruções, portas, joystick, estado do PSG e execução de quadro com a referência Python.

### Validação automatizada

Workflow Windows **Qt selectable Rust Z80 core**, commit `7bb95c0f55dfeb0332b9a5ea6b3da54d1bdee43c`:

- `cargo fmt --check`: aprovado.
- Clippy com `-D warnings`: aprovado.
- Testes unitários Rust: 7 aprovados.
- Build Release e instalação da extensão PyO3: aprovados.
- Ruff e formatação Python: aprovados.
- Testes direcionados Qt/backend Rust: **1.674 aprovados em 2,18 s**.
- Suíte completa pytest com extensão nativa instalada: **2.329 aprovados em 9,00 s**.
- CI geral e Performance Diagnostic: aprovados.

O teste diferencial da fase anterior identificou uma diferença de representação no PyO3 (`Vec<u8>` exposto como `bytes` em vez de lista de inteiros). Os getters do PSG foram ajustados para preservar o contrato esperado pela referência Python.

### Limitações conhecidas

- A coordenação do laço e das portas está em Rust, mas os componentes ainda são objetos PyO3; o GIL continua adquirido durante o quadro. Esta fase não libera o GIL nem move todos os estados para structs Rust puros.
- Os getters `vram` e `registers` do VDP retornam cópias; mutações nessas cópias não alteram o estado nativo.
- A temporização usa T-states nominais e writes do VDP ainda não têm fila temporal própria. Não declarar precisão ciclo a ciclo.

## Próximo passo

Criar uma terceira fatia para remover handles PyO3 do caminho interno: CPU, barramento, VDP, PSG e sincronizador devem ser possuídos por uma estrutura Rust nativa, expondo snapshots e operações de alto nível ao Qt. Em seguida, avaliar a liberação do GIL durante `run_frame` com sincronização explícita. Antes de tornar Rust padrão, resolver o contrato dos snapshots e validar com homebrew/testes reproduzíveis.
