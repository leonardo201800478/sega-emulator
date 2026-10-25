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

A Fase 0 teve uma primeira revisão estática de contratos e consumidores. A Fase 1 começou nesta branch com:
- `native/z80_core/src/memory.rs`: barramento nativo de ROM espelhada, RAM interna espelhada e RAM de cartucho configurável, com testes unitários Rust.
- `native/z80_core/src/lib.rs`: novo construtor `RustZ80Cpu.with_native_memory`; o caminho nativo lê/escreve memória sem chamar métodos Python por acesso. O construtor antigo com barramento Python permanece para compatibilidade e testes diferenciais.
- `src/sega_emulator/machine/sg1000.py`: o backend Rust passa a usar o barramento nativo; o backend Python permanece inalterado.
- `tests/test_sg1000_cpu_backend.py`: teste de integração para instruções que escrevem e leem RAM através do backend Rust.

**Limitação desta fatia:** o VDP, PSG, sincronização da máquina, I/O e laço de execução por instrução ainda são Python; I/O continua atravessando PyO3. Portanto, esta alteração não conclui a Fase 1 inteira nem satisfaz a meta final de runtime totalmente Rust.

### Validação automatizada

No commit `85329fb92c49cf51f2699c0b69be946f899c4101`, o workflow Windows **Qt selectable Rust Z80 core** passou:
- `cargo fmt --check`: aprovado.
- Clippy com `-D warnings`: aprovado.
- Testes unitários Rust: 7 aprovados, nenhum falhou.
- Build Release e instalação da extensão PyO3: aprovados.
- Ruff (lint e formatação Python): aprovado.
- Testes direcionados Qt/backend Rust: 1.671 aprovados.
- Suíte completa pytest: 2.317 aprovados.
- CI geral, validação funcional, integridade/determinismo, auditoria de desempenho/cobertura, diagnóstico de desempenho e auditoria diferencial SingleStepTests: aprovados.
- A auditoria Z80 estendida ainda estava pendente no momento deste registro.

Esses resultados validam o incremento atual, não a migração completa nem precisão ciclo a ciclo.

## Próximo passo

Iniciar a fase de integração temporal nativa: portar para Rust a arbitragem de I/O e o estado temporal mínimo do VDP (contadores, status e condição de interrupção) junto com o laço da máquina. Só então permitir avanço em lotes, com limites alinhados aos eventos do hardware. A renderização completa do VDP e o PSG vêm depois, em fases próprias. Não agrupar instruções enquanto isso puder alterar a ordem dos eventos ou a entrega de interrupções.
