# ADR 0007 — Política Rust-first para o núcleo de emulação

- **Status:** aprovado.
- **Data:** 2026-10-26.
- **Escopo:** desenvolvimento futuro e evolução incremental do Sega Emulator.

## Contexto

O projeto começou com Python como implementação de referência e agora possui um runtime nativo Rust integrado ao frontend Python por PyO3. A experiência de desenvolvimento e o desempenho observado justificam definir uma estratégia estável, evitando que cada componente seja implementado integralmente duas vezes.

A arquitetura deve continuar determinística, testável e funcional a cada marco. O hardware SG-1000 ativo é implementado em Rust; a aplicação Qt usa uma fachada PyO3 fina. A decisão não declara precisão ciclo a ciclo.

## Decisão

1. **Rust é a linguagem principal do núcleo de hardware.** Novos componentes de CPU, barramento, memória, temporização, VDP, PSG, I/O, interrupções e scheduler devem ser implementados diretamente em Rust quando o contrato funcional e os critérios de aceitação estiverem claros.
2. **Python não implementa hardware ativo.** A aplicação Python fica restrita à interface Qt e seus backends de host, carregamento de ROMs/perfis, configuração/logging e fachada PyO3. Testes e ferramentas podem usar Python como infraestrutura, mas exercitam Rust e não importam o código em `legacy/`.
3. **Referências Python antigas são somente arquivo.** As implementações substituídas estão em `legacy/python_reference/`, fora do pacote executável. Não criar novas referências Python de hardware nem usá-las como oráculo em testes ativos.
4. **Rust é a única fonte ativa do hardware.** Manter contratos explícitos, vetores independentes e regressões Rust. Não importar implementações arquivadas para equivalência automática.
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


## Aditivo — fluxo obrigatório de completude e validação (2026-10-28)

A decisão Rust-first permanece vigente e passa a ser executada com o fluxo obrigatório descrito em [Política permanente de desenvolvimento, fidelidade e entrega](../development-and-validation-policy.md):

1. Inventariar o estado real do componente no HEAD atual.
2. Completar lacunas funcionais com fatias compiláveis em Rust, sem reescrita integral por padrão.
3. Rodar checks rápidos por fatia; depois validação independente abrangente curta e, após corrigir divergências, testes longos.
4. Liberar o componente para o próximo marco somente com critérios e evidências registrados.
5. Reservar a campanha completa de temporização ciclo a ciclo para a máquina SMS integrada, mantendo desde o início os contratos necessários para eventos de barramento e sincronização.

O projeto limita a três agentes de IA simultâneos. A pesquisa deve priorizar documentação técnica oficial e usar código de emuladores abertos para consulta de lógica/arquitetura, sem cópia de código. Toda emulação ativa continua em Rust; Python permanece em Qt/PySide6, ferramentas e testes. APIs de host de baixo nível são escolhidas por evidência e medição, não por adoção especulativa.

Para alterações que afetem gameplay, o merge em `main` exige checks pertinentes no SHA candidato e evidência de teste in-game; caso não esteja disponível no ambiente de automação, o gate fica pendente até confirmação do mantenedor.