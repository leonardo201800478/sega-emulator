# Plano de migração do runtime para Rust

## Decisão

O runtime de emulação do projeto será migrado incrementalmente de Python para Rust. A interface Qt/PySide6 permanece em Python. A migração será feita em fatias pequenas, cada uma compilável, testável e revisável; não haverá reescrita integral em um único commit.

O objetivo é que a execução da emulação — CPU, barramentos, memória, dispositivos, sincronização e composição da máquina — ocorra em Rust sem chamadas Python por acesso de memória ou por instrução. PyO3 será a fronteira entre o runtime Rust e a interface Qt, não o mecanismo interno de comunicação entre componentes de hardware.

## Estado inicial confirmado

- `main` no commit `593c2dc8fdb1be55c548a3979d24b8d49c589b52`.
- O núcleo Z80 Rust já existe em `native/z80_core`, com ALU separada em `native/z80_alu`.
- A extensão PyO3 atual recebe objetos Python de barramento; portanto, o núcleo ainda pode cruzar a fronteira Rust/Python durante a execução. A migração deve eliminar essas chamadas do caminho crítico.
- A máquina SG-1000 Python integra Z80, barramento/memória, TMS9918A, SN76489 e sincronização.
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

A meta final é manter apenas o frontend Qt em Python. O frontend Pygame/SDL e a CLI Python são caminhos de aplicação a retirar ou substituir por Rust; a saída de áudio e a entrada nativas serão ligadas ao runtime sem alterar os clocks lógicos. Essa consolidação ocorrerá depois de os contratos de hardware e a API Rust estarem estáveis.

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

A Fase 0 teve uma primeira revisão estática de contratos e consumidores. A Fase 1 está em andamento nesta branch:

- `native/z80_core/src/memory.rs`: barramento nativo com ROM espelhada, RAM interna espelhada e RAM de cartucho configurável, com testes unitários Rust.
- `native/z80_core/src/lib.rs`: `RustZ80Cpu.with_native_memory`; o caminho nativo acessa a memória sem invocar métodos Python por byte. O CPU ainda mantém objetos PyO3 para memória e I/O, portanto isto ainda não é uma máquina Rust integral.
- `native/z80_core/src/vdp.rs`: portas, registradores, VRAM, temporização raster, VBlank, status de sprites e renderização dos quatro modos gráficos modelados.
- `native/z80_core/src/audio.rs`: PSG SN76489AN e sincronizador de writes por T-state nativos.
- `src/sega_emulator/machine/sg1000.py`: o backend Rust seleciona memória, VDP, PSG e sincronizador nativos; o backend Python permanece como referência.
- `tests/test_sg1000_cpu_backend.py`, `tests/test_tms9918_rust_parity.py` e `tests/test_sn76489_rust_parity.py`: integração CPU/memória e comparações diferenciais de VDP, PSG e sincronização de áudio.

### Limitações conhecidas

- O laço de execução da máquina e a arbitragem de I/O ainda são coordenados por Python; operações de porta da CPU ainda atravessam o objeto PyO3 de I/O.
- Os getters `vram` e `registers` do VDP nativo retornam cópias. Leituras de diagnóstico funcionam, mas mutações feitas diretamente nessas cópias não alteram o estado nativo. A compatibilidade desse contrato precisa ser resolvida antes de tornar o backend Rust padrão.
- A temporização ainda não é ciclo a ciclo: o avanço dos dispositivos usa T-states nominais e writes do VDP ainda não são enfileirados em uma linha temporal própria.

### Validação automatizada

- No commit `73f4298`, o workflow Windows falhou em `cargo fmt --check`; a formatação foi corrigida em `b921490`.
- No commit `a01b551`, a formatação passou, mas o Clippy falhou ao compilar os testes porque um dos usos dos antigos nomes de constantes de áudio não havia sido atualizado.
- O commit `605e466` corrige a referência restante. O workflow **Qt selectable Rust Z80 core** e os agentes de validação estão executando novamente contra esse estado; **ainda não há resultado final confirmado para esse commit**.
- Os workflows genéricos de Python reportaram 637 aprovados e 1.663 ignorados, pois não instalam a extensão Rust. Esses resultados não validam os novos componentes nativos.

Nenhum resultado histórico é automaticamente atribuído ao HEAD atual. Não considerar a fatia aprovada até o workflow Windows construir a extensão Release e executar os testes diferenciais com ela instalada.

## Próximo passo

Concluir e auditar o workflow Windows desta fatia. Se fmt, Clippy, testes Rust, build Release, paridade direcionada e regressão completa passarem, a próxima implementação será mover o I/O e o laço CPU–VDP–PSG para uma máquina Rust. Essa etapa deverá preservar a ordem dos eventos temporais e as interrupções. Não declarar o runtime integralmente Rust até a coordenação deixar de depender de Python fora do frontend Qt.
