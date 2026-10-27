> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust. O código Python antigo de hardware, os testes/ferramentas dependentes e os workflows históricos foram arquivados sob `legacy/`; não são importados nem executados pelo projeto ativo. Este documento mantém observações históricas quando indicadas.

# Arquitetura dos crates Rust nativos

## Objetivo e escopo

Este documento descreve a organização real dos crates Rust no commit de referência desta alteração. Distingue o workspace principal dos crates usados pela aplicação Qt e da extensão experimental PyO3. Não é uma declaração de completude funcional nem de precisão ciclo a ciclo. O hardware ativo do SG-1000 é Rust-only; Python fornece a UI Qt, carregadores e a fronteira de aplicação.

## Visão geral

| Caminho | Papel | Workspace raiz? | Estado e dependências |
|---|---|---:|---|
| `native/sg1000_core` (`sega-sg1000-core`) | Biblioteca standalone específica do SG-1000: memória, CPU Z80 e estrutura inicial de máquina | Sim | Parcial e incremental. Depende de `sega-z80-alu` e `sega-z80-bus`. Não equivale sozinho ao runtime integral usado pela aplicação Qt. |
| `native/sms_core` (`sega-sms-core`) | Biblioteca de máquina específica do Master System | Sim | Inicial/parcial: configurações, imagem de cartucho e memória/mapa. CPU, I/O, VDP, áudio e scheduler ainda não estão implementados como runtime completo. Depende de `sega-z80-bus`. |
| `native/z80_alu` (`sega-z80-alu`) | Biblioteca compartilhada de primitivas ALU Z80 e executável de vetores | Sim | Consumida pelo SG-1000 e pela extensão experimental. Não é um runtime Z80 completo: não decodifica instruções nem modela ciclos/barramento. |
| `native/z80_bus` (`sega-z80-bus`) | Contrato compartilhado de acesso à memória e portas para um Z80 | Sim | Dependência de SG-1000 e SMS. É hardware-neutral, mas a implementação de mapa e dispositivos pertence a cada máquina. Os valores padrão de portas são placeholders, não especificação física universal. |
| `native/z80_core` (`sega-z80-core`) | Biblioteca Rust/PyO3, `cdylib`/ `rlib`, usada como adaptador obrigatório entre a UI Qt e o núcleo Rust consolidado | **Não** | Excluída explicitamente do workspace principal. Tem `Cargo.toml`, `Cargo.lock` e configuração Maturin próprios. Depende de `sega-z80-alu` e `sega-sg1000-core`; fornece integração Rust/Python para a máquina SG-1000, sem substituir a API de aplicação Python. |

Os nomes de pacote e os nomes dos módulos importáveis não são idênticos em todos os casos: por exemplo, `sega-z80-bus` expõe o crate Rust `sega_z80_bus`.

## Workspace principal e caminhos de compilação

O `Cargo.toml` raiz usa resolver 3. Os membros e os `default-members` são os mesmos quatro crates:

- `native/sg1000_core`
- `native/sms_core`
- `native/z80_alu`
- `native/z80_bus`

A extensão `native/z80_core` consta em `exclude` e não participa de `cargo test --workspace`, `cargo clippy --workspace` ou `cargo fmt --all` executados na raiz. Isso é intencional para manter separado seu grafo de build/dependências PyO3, não significa que o crate seja código morto.

### Workspace principal (PowerShell, raiz do repositório)

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Para focar um componente sem sair do workspace:

```powershell
cargo test -p sega-sg1000-core --locked
cargo test -p sega-sms-core --locked
cargo test -p sega-z80-alu --locked
cargo test -p sega-z80-bus --locked
```

O `Cargo.lock` raiz fixa o grafo do workspace principal. Um teste do workspace passa por todos os testes incluídos nos membros; não prova que o SMS ou a máquina standalone sejam completos.

### Extensão experimental PyO3/Maturin

Build/testes Rust do crate isolado, a partir da raiz:

```powershell
cargo fmt --manifest-path native/z80_core/Cargo.toml --all -- --check
cargo clippy --manifest-path native/z80_core/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path native/z80_core/Cargo.toml --locked
```

Build Release instalável e testes de integração com a extensão:

```powershell
python -m pip install maturin
Push-Location native/z80_core
maturin build --release --features python-extension --out dist
Pop-Location
$wheel = Get-ChildItem native/z80_core/dist/*.whl | Select-Object -First 1
python -m pip install $wheel.FullName
python -m pytest tests/test_rust_runtime_selection.py tests/test_sg1000_native_machine.py tests/test_vgm_machine_integration.py -q
```

No Windows, o workflow `.github/workflows/qt-rust-z80-core.yml` também instala `.[dev,qt]`, constrói o wheel Release, instala-o e roda Ruff e um conjunto direcionado de testes Qt/backend/paridade. O caminho `maturin develop --release --features python-extension` descrito no README de raiz é uma alternativa para ambiente local de desenvolvimento.

## Seleção do backend na aplicação Qt

`src/sega_emulator/machine/sg1000.py` é uma fachada fina e Rust-only que importa `NativeSG1000Machine` de `sega_z80_core`; não aceita seletor de backend nem contém lógica de hardware. Preferências antigas de `cpu_core` são migradas para `rust` e não selecionam outro runtime.

Papéis distintos:

- **Python:** frontend Qt/PySide6 e backends de host, carregamento de ROM/perfis, configuração/logging, fachada PyO3 e ferramentas/testes que exercitam Rust. Implementações de hardware antigas estão em `legacy/python_reference/` e não são usadas.
- **Rust standalone do workspace:** crates progressivamente independentes para componentes e máquinas, com tipos Rust e contratos explicitados.
- **Rust/PyO3:** adaptador de integração da aplicação Qt; o estado e a execução do hardware pertencem a `native/sg1000_core`.

Manter `native/z80_core` como fronteira PyO3 enquanto a UI Qt depender dela; o hardware não deve voltar a ser implementado nesse crate nem em Python.

## Papel dos workflows atuais

| Workflow | Cobertura efetiva relevante | Limite conhecido |
|---|---|---|
| `.github/workflows/rust-sg1000-core.yml` | `fmt`, Clippy para todos os membros do workspace e `cargo test --workspace --locked`; filtros para manifestos raiz/lock e SG-1000/ALU | O filtro de PR/push não incluía `native/z80_bus/**`, apesar de SG-1000 depender do crate. Esse caminho deve disparar validação do workspace. |
| `.github/workflows/sg1000-three-agent-validation.yml` | Testes direcionados de CPU, memória e máquina SG-1000; depois qualidade e testes completos do workspace | É uma sequência de jobs CI, não revisão autônoma por IA; não prova precisão temporal além dos testes executados. Já inclui `z80_bus` no filtro. |
| `.github/workflows/sms-core-fast.yml` | Formatação global, Clippy e testes de `sega-sms-core` | Adequado para o alvo SMS rápido; não substitui os testes integrais de workspace. Já inclui `z80_bus`. |
| `.github/workflows/qt-rust-z80-core.yml` | Formatação/Clippy/testes do crate excluído, build Release/Maturin, Ruff e pytest direcionado para Qt/backend/paridade | O filtro precisa acompanhar os crates upstream dos quais depende, inclusive `native/z80_bus/**` e mudanças de manifests/lock relevantes. O push está limitado a uma branch de desenvolvimento específica; a cobertura de PR é o caminho principal para revisão. |
| `.github/workflows/ci.yml` | Ruff nos arquivos Python alterados em PR/push (ou projeto inteiro quando acionado manualmente conforme lógica do workflow) e `python -m pytest` | Não compila Rust nem constrói a extensão Maturin. |
| `legacy/workflows/z80-extended-audit.yml` | Workflow histórico preservado apenas para consulta | **Arquivado; não executa**. Os testes ativos do núcleo usam os workflows Rust e a integração Qt/PyO3.

Consulte os arquivos de workflow para comandos exatos e estado do run do SHA candidato. Paths são parte do contrato de CI: todo arquivo consumido por um teste/build que pretenda validar alteração precisa ser incluído no filtro relevante, ou a validação correspondente deve ser deliberadamente global.

## Estado dos filtros após a organização

- O workflow geral de SG-1000 executa o workspace completo e deve disparar quando mudar `native/z80_bus/**`.
- O workflow Qt/Rust deve disparar para `native/z80_bus/**`, porque esse crate faz parte do grafo de dependências do núcleo standalone usado pela extensão.
- Alterações em `Cargo.toml`/ `Cargo.lock` raiz devem acionar os workflows que validam o workspace e as relações de dependência.
- Mudanças no manifesto/lock do `native/z80_core` e em seus fontes continuam sob o workflow isolado da extensão.
- O CI Python não valida o grafo Rust; são complementares, não intercambiáveis.

## Diretórios `target`

O repositório possui uma entrada de diretório vazia `native/z80_alu/target` apontada pela API como árvore Git vazia (tree SHA igual ao SHA canônico da árvore vazia: `4b825dc642cb6eb9a060e54bf8d69288fbee4904`). Isso é um resíduo rastreado de diretório, não artefato compilado. A raiz `.gitignore` já cobre `**/target/`. A remoção desse diretório vazio não apaga código de build; deve ser feita apenas como limpeza rastreada explícita, separada de qualquer alvo comportamental. Nenhuma limpeza em outros caminhos `target` deve ser feita sem confirmar a entrada Git correspondente.

## Critérios de aceitação desta organização

1. Manter a separação entre os quatro membros do workspace e a extensão PyO3 excluída.
2. Documentar dependências e níveis reais de conclusão sem afirmar que crates parciais constituem um emulador completo.
3. Alinhar README de `z80_alu` ao uso efetivo por SG-1000 e `z80_core`.
4. Incluir `native/z80_bus/**` nos filtros dos workflows que validam dependentes relevantes.
5. Fazer os caminhos de validação de workspace e extensão explícitos e reproduzíveis.
6. Não alterar instruções, resultados da emulação, mapas, temporização ou seleção default da UI.
7. Validar o diff no SHA candidato e verificar os workflows do GitHub Actions antes de declarar aprovação.

## Referências de implementação

- Workspace: `Cargo.toml`, `Cargo.lock`.
- Crates: manifestos em `native/{sg1000_core,sms_core,z80_alu,z80_bus,z80_core}/Cargo.toml`.
- Integração Qt: `src/sega_emulator/machine/sg1000.py`, preferências/frontend Qt e `.github/workflows/qt-rust-z80-core.yml`.
- Política de implementação: `docs/architecture.md`, `docs/adr/0007-rust-first-development-policy.md`.
