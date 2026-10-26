# ADR 0007 — Rust-first para o núcleo de emulação

- **Status:** aprovado como diretriz arquitetural.
- **Data:** 2026-10-26.
- **Escopo:** novos componentes, migração do SG-1000 e sistemas futuros.

## Contexto

O repositório contém runtime de emulação em Python, uma extensão PyO3 e o crate standalone `native/sg1000_core`. Manter dois runtimes completos indefinidamente aumenta o custo de manutenção, cria divergências semânticas e multiplica a validação necessária. Ao mesmo tempo, reescrever tudo sem migração incremental arrisca contratos já usados pela UI e pelos testes.

O projeto usa [Gearsystem](https://github.com/drhelius/Gearsystem) como espelho de estudo para SG-1000/Master System/Game Gear. Gearsystem declara licença GPL-3.0; a referência arquitetural não autoriza cópia de código ou assets.

## Decisão

1. Rust será a implementação oficial do núcleo: CPU, barramento/memória, I/O, VDP, PSG, agenda temporal e runtime da máquina.
2. Python continuará como frontend Qt/PySide6, configuração, automação, testes, auditoria, benchmark e tooling.
3. Implementações Python de referência serão seletivas, mantidas somente quando fornecerem valor concreto de pesquisa ou comparação; não é meta conservar dois emuladores completos.
4. Componentes críticos novos nascerão em Rust quando contratos e critérios de validação estiverem claros. Migrações existentes ocorrerão por fatias pequenas, reversíveis e testáveis, sem reescrita indiscriminada.
5. PyO3/Maturin formarão uma fronteira de API de alto nível. CPU, barramento e dispositivos não devem comunicar-se por callbacks Python dentro do caminho crítico.
6. O backend Rust não se torna padrão apenas por compilar ou parecer mais rápido. Exige regressões funcionais, determinismo e comparação diferencial adequada.
7. Não será introduzido JIT ou paralelismo artificial sem profiling e justificativa técnica.

## Validação e aceite

Cada componente migrado deve ter contrato explícito, vetores de teste reproduzíveis e regressões. Quando há referência Python, comparar estado inicial equivalente, sequência de entradas e checkpoints: registradores/flags, ciclos, portas, VDP/PSG, framebuffer e áudio conforme o escopo. Preferir fontes técnicas e oráculos independentes para as expectativas.

Executar checks Rust (`fmt`, Clippy sem warnings e testes), checks Python (pytest/Ruff) e integração específica afetada. Benchmarks devem usar a mesma carga, build, aquecimento e repetições e não substituem testes funcionais. Não declarar precisão ciclo a ciclo sem modelo e evidência de ciclos de barramento.

## Efeito

- O README e a documentação de arquitetura devem refletir Rust-first e deixar as limitações atuais claras.
- O próximo marco é consolidar o SG-1000 no crate Rust independente, sem duplicar estruturas existentes.
- Master System/Game Gear só avançam após definição das diferenças da plataforma; Mega Drive exige núcleos separados para 68000 e Z80.
- Qualquer mudança de licença ou decisão de reutilizar código externo exige avaliação própria, não coberta por este ADR.
