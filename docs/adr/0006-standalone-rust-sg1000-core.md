> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust (`native/sg1000_core`). A aplicação Qt usa uma fachada Python fina para PyO3 e carregadores de ROM/perfil. Implementações de hardware Python, ferramentas e testes que dependiam delas foram arquivados sob `legacy/` e não são executados pelo projeto ativo. Descrições de estados anteriores neste documento são históricas, não instruções atuais.

# ADR 0006 — Núcleo SG-1000 independente em Rust

- **Status:** aprovado para implementação incremental.
- **Data:** 2026-10-25.
- **Branch de trabalho:** `feat/rust-standalone-sg1000-test`.

## Contexto

O runtime nativo existente está exposto via PyO3 e coordena CPU, memória, VDP, PSG e I/O por objetos Python/PyO3. Isso permite selecionar Rust no frontend Qt, mas não é um núcleo integralmente independente de Python, nem oferece atualmente um executável Rust standalone do SG-1000.

O objetivo desta etapa é permitir compilar, testar e executar o núcleo SG-1000 sem inicializar Python. O frontend Qt pode permanecer em Python durante a migração.

## Decisão

Criar o crate puro Rust `native/sg1000_core`, sem dependência de PyO3, como destino da migração. O crate de extensão `native/z80_core` será tratado como adaptador de compatibilidade até os contratos e testes do novo núcleo estabilizarem.

Migrar na seguinte ordem, mantendo cada etapa testável:
1. Mapa de memória/cartucho SG-1000 e seus perfis explícitos.
2. Estado e execução do Z80, extraindo a lógica de CPU para structs Rust e contratos de memória/I/O independentes de Python.
3. Portas SG-1000, VDP TMS9918A, PSG SN76489 e sincronização de eventos.
4. Scheduler/máquina SG-1000 que avance CPU e periféricos em ordem determinística.
5. Executável headless para ROM legal/homebrew, limite de quadros, hash de framebuffer/estado e relatório de contadores.
6. Bindings PyO3 finos para integrar o frontend existente sem duplicar a lógica de emulação.

## Critérios de aceitação para o núcleo completo de teste

- `cargo test`, `cargo fmt --check` e `cargo clippy -- -D warnings` aprovados no crate standalone.
- O núcleo compila e executa sem importar Python ou inicializar o interpretador.
- Testes unitários de CPU, memória, VDP, PSG, portas, interrupções e scheduler.
- Teste headless determinístico: mesma ROM homebrew, configuração e entradas produzem os mesmos contadores e hashes.
- Testes diferenciais Python/Rust para estados observáveis; divergências conhecidas registradas como testes ou limitações explícitas.
- Sem alegação de precisão ciclo a ciclo até existir modelo e evidência de eventos de barramento nesse nível.
- O backend Rust não se torna padrão apenas por compilar; exige validação funcional e regressão comparativa.

## Estado inicial

A primeira fatia implementa o mapa de ROM/RAM em Rust puro. CPU, VDP, PSG, portas, scheduler e executável headless ainda precisam ser extraídos/implementados no novo crate. A extensão PyO3 existente permanece inalterada nesta etapa.
## Progresso — primeira fatia de CPU puro Rust

- Adicionado o contrato `CpuBus` para acesso a memória e portas sem dependência de Python.
- Adicionados `Z80Registers` e `Z80` ao crate `sega-sg1000-core`, com uma fatia explícita de instruções base, contagem de T-states e erro para opcode ainda não implementado.
- O workspace Rust inclui agora o contrato e a implementação inicial para que possam ser compilados/testados sem construir a extensão PyO3.
- Esta fatia **não** substitui ainda a CPU do runtime nem torna Rust completo: prefixos, interrupções, demais opcodes, integração dos periféricos e scheduler continuam pendentes. Não habilitar como backend padrão até a migração e validação diferencial.
- Validação focada prevista: compilação do workspace e os três testes unitários da nova fatia; sem executar a suíte extensa Python.

## Progresso — máquina headless inicial

- Adicionado `Sg1000Machine`, que conecta o Z80 parcial ao mapa de memória por meio do contrato `CpuBus`.
- A máquina pode executar um número fixo de instruções e reportar T-states nominais, sem carregar Python ou PyO3.
- Testes unitários verificam execução de ROM sintética, escrita em RAM, determinismo entre duas instâncias e erro explícito para opcode não suportado.
- Limitação mantida: VDP, PSG, portas SG-1000, interrupções e scheduler ainda não estão integrados; não usar como emulador completo.
