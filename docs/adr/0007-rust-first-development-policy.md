# ADR 0007 — Política Rust-first para o núcleo de emulação

- **Status:** aprovado.
- **Data:** 2026-10-26.
- **Escopo:** desenvolvimento futuro e evolução incremental do Sega Emulator.

## Contexto

O projeto começou com Python como implementação de referência e agora possui um runtime nativo Rust integrado ao frontend Python por PyO3. A experiência de desenvolvimento e o desempenho observado justificam definir uma estratégia estável, evitando que cada componente seja implementado integralmente duas vezes.

A arquitetura deve continuar acessível para desenvolvimento, determinística, testável e funcional a cada marco. A decisão não declara que o hardware atualmente modelado já possui precisão ciclo a ciclo nem que todo o estado da máquina já está em Rust puro.

## Decisão

1. **Rust é a linguagem principal do núcleo de hardware.** Novos componentes de CPU, barramento, memória, temporização, VDP, PSG, I/O, interrupções e scheduler devem ser implementados diretamente em Rust quando o contrato funcional e os critérios de aceitação estiverem claros.
2. **Python continua no frontend e nas ferramentas.** Qt/PySide, configuração, carregamento/orquestração da aplicação, scripts de desenvolvimento, análise, automação e testes podem permanecer em Python.
3. **Python de referência é seletivo, não obrigatório.** Criar protótipos Python quando o comportamento de hardware ainda for incerto ou quando uma implementação independente melhorar a validação. Não criar uma segunda implementação completa por rotina; remover ou congelar protótipos após a migração quando deixarem de agregar valor.
4. **Rust passa a ser a fonte oficial do componente após validação.** Manter contratos explícitos, vetores de teste reutilizáveis e testes de equivalência quando houver duas implementações. Não assumir tradução automática entre linguagens.
5. **Bindings PyO3/Maturin devem ser finos.** Evitar callbacks Python por operação de hardware, cópias desnecessárias e travessias repetidas da fronteira Rust/Python. A evolução para structs Rust puros e eventual liberação do GIL exige mudanças explícitas, seguras e testadas.
6. **Determinismo antes de paralelismo.** CPU, barramento, VDP e áudio devem seguir um modelo temporal explícito. Não executar componentes temporalmente acoplados em paralelo sem um modelo de sincronização demonstrável.
7. **Sem JIT por antecipação.** Profiling e benchmarks representativos devem preceder otimizações adicionais. Ganhos reportados precisam de metodologia reproduzível antes de serem apresentados como resultados gerais.
8. **Migração incremental.** Preservar o marco SG-1000 funcional e evitar reescritas extensas sem benefício demonstrado. Não ampliar para outros consoles até que as dependências e critérios do marco atual estejam claros.

## Política de testes e CI

Toda mudança de comportamento deve trazer testes antes ou junto da implementação. Os testes devem validar o comportamento do hardware, não apenas repetir a estrutura interna do código.

- Rust puro: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked`.
- Crate de extensão `native/z80_core`: executar verificações usando `--manifest-path native/z80_core/Cargo.toml`, construir a extensão com Maturin e executar testes Python de integração/paridade relevantes.
- Python: `pytest`, `ruff check` e `ruff format --check`.
- Escolher workflows de CI pelo escopo: `sg1000-three-agent-validation.yml` para o núcleo Rust SG-1000; `rust-sg1000-core.yml` para qualidade do workspace Rust; `qt-rust-z80-core.yml` para a extensão PyO3 e integração Qt; workflows `agent-*` para regressão, integridade/determinismo e auditoria de desempenho Python.
- Os jobs denominados “Agent” são gates de validação automatizada no GitHub Actions; seus nomes não provam que um agente autônomo escreveu, revisou ou corrigiu código.
- Não afirmar que um teste passou sem consultar logs/status do run correspondente. Registrar separadamente testes locais, checks CI, verificações não executadas e validações manuais.
- Não enfraquecer ou remover testes para obter um CI verde. Investigar regressões e corrigir a causa; quando um teste documentar hipótese ainda não comprovada, rotulá-la claramente.

## Consequências

**Benefícios esperados:** menor custo por operação no caminho crítico, uma implementação oficial do hardware, contratos mais claros e menor duplicação desnecessária.

**Custos e limites:** exige aprender Rust progressivamente, manter a fronteira PyO3 e criar vetores de teste robustos. A decisão não implica paralelismo automático, ausência do GIL, precisão ciclo a ciclo ou equivalência com hardware real.

## Critérios para considerar uma migração concluída

- Contrato e limites documentados.
- Testes unitários de estados, flags, temporização e casos-limite pertinentes.
- Testes de integração e regressão relevantes aprovados.
- Testes diferenciais quando houver implementação de referência útil.
- `fmt`, Clippy e build aprovados para os crates afetados.
- Workflow GitHub Actions aplicável concluído com sucesso, ou limitação claramente reportada.
- README e documentação refletem o estado real, sem declarar capacidades não validadas.
