# Política de CI e validação

**Baseline de auditoria:** `main` em `82ccad8eb9cd11d78dfddd168f307f90012426b5` (2026-11-11).
**Escopo:** runtime Rust, hardware, hosts transitórios e migração Slint/WGPU/CPAL/SDL3.

Esta política é subordinada aos ADRs vigentes e ao código integrado na `main`. PRs Draft, branches e validações parciais não alteram o estado integrado.

## 1. Princípios

1. A CI seleciona a menor suíte que cubra o código alterado e seus consumidores reais.
2. A seleção por caminhos nunca pode ocultar uma regressão: dependências compartilhadas acionam a união dos gates dos consumidores.
3. Testes de hardware/runtime permanecem obrigatórios nas alterações pertinentes.
4. Hosts legados são preservados por validação condicional ou opt-in; não são destino arquitetural.
5. Um job `SKIPPED` nunca é `PASS`. O gate agregado exige `success` para jobs aplicáveis e `skipped` para jobs não aplicáveis.
6. Cada resultado deve ser associado ao SHA exato avaliado.
7. Builds de aplicação são Release e usam lockfiles versionados com `--locked`.
8. Execução física em GPU, dispositivos de áudio ou gamepads é registrada separadamente; compilação e testes headless não comprovam operação física.

## 2. Workflows ativos

| Workflow | Responsabilidade |
|---|---|
| `.github/workflows/ci.yml` | Classificação de impacto, regressão Rust, hosts condicionais, desktop Slint/WGPU, higiene documental e lint de workflows |
| `.github/workflows/sms-debugger-runtime.yml` | Compatibilidade do SMS runtime com o host/debugger independente |

As alterações em `.github/workflows/**`, `.github/actions/**` ou `.github/scripts/**` acionam o gate `workflow-lint` e a regressão do workspace Rust. A análise estática dos workflows utiliza actionlint v1.7.12, instalado por release versionada e SHA-256 verificado.

## 3. Matriz de rastreabilidade

| Componente | Validação obrigatória | Workflow | Condição |
|---|---|---|---|
| Runtime Rust | fmt, Clippy Release, testes unitários e integração | CI / Rust workspace | Crates Rust gerais, manifests, lockfiles e configuração global |
| Z80 CPU/ALU/bus | fmt, Clippy, testes semânticos e integração SMS/SG-1000 | CI / Z80 focused | Alterações nos módulos Z80 e integrações classificadas |
| Z80 vetores externos | SingleStepTests DD/FD indexados, revisão de estado arquitetural e T-states | CI / Z80 focused | Alterações Z80 |
| PSG | fmt, Clippy e testes do PSG e consoles | CI / PSG focused | Alterações PSG |
| VDP | fmt, Clippy e testes do VDP e consoles | CI / VDP focused | Alterações VDP |
| MachineScheduler/temporal core | Regressão do workspace e replay determinístico | CI / Scheduler stress | Alterações temporais ou opt-in por label |
| Input portátil | fmt, Clippy e testes do runtime de entrada e consoles | CI / Input focused | Alterações input |
| Slint/WGPU 30 | fmt, Clippy, testes e build Windows x64 Release | CI / Slint WGPU | Alterações em `native/sega_desktop` ou dependências do workspace |
| Vulkan físico | Janela, device/queue, textura externa, composição, resize e teardown em GPU real | Validação manual R1 | Antes de declarar R1 concluída |
| Áudio legado | fmt, Clippy e testes Windows/Linux do host | CI / Legacy audio | Alterações no host ou dependências afetadas; também opt-in |
| Aplicação egui legada | fmt, Clippy, testes e build Windows x64 Release | CI / Legacy Windows host | Alterações na aplicação ou dependências transitórias afetadas; também opt-in |
| D3D12 legado | Clippy e testes Windows do runtime gráfico | CI / Legacy Windows host | Alterações no runtime gráfico/VDP ou opt-in |
| CPAL | Dispositivos, formatos, streams, callback e underruns | Gate futuro R3 | Somente após implementação |
| SDL3 | Teclado, gamepads e mapeamento de controles | Gate futuro R4 | Somente após integração |
| SMS debugger GUI egui | `NOT RUN`; o núcleo debugger_core continua coberto no workspace Rust | Nenhum gate padrão | Host de desenvolvimento legado; reativação exige lockfile e gate opt-in |
| Documentação | `git diff --check` | CI / Documentation | Alterações Markdown, MDX, RST ou TXT |
| Workflows | actionlint e validação do gate agregado | CI / Workflow lint | Alterações nos workflows e arquivos auxiliares |
| PowerShell tooling | Parser PowerShell nos scripts alterados e auditoria de workspaces/lockfiles | CI / Windows tooling | Alterações em `scripts/**/*.ps1` ou `atualizar_recompilar_sega_emulator.ps1` |
| Lockfiles | `cargo metadata --locked` e existência dos lockfiles | CI / Rust workspace e lockfile artifact | Alterações de manifests, lockfiles ou refresh manual |

## 4. Regras de impacto e dependências

- Z80-only executa a suíte focada Z80, incluindo os testes integrados SMS/SG-1000 e vetores externos pertinentes. Não exige compilar hosts gráficos ou de áudio sem relação com a alteração.
- PSG, VDP e input têm jobs de pacote com integração de console. Eles substituem a regressão genérica do workspace somente quando a alteração está isolada nesses domínios.
- MachineScheduler e temporal core executam a regressão do workspace e o stress replay determinístico. Consumidores legados afetados também são validados quando necessário.
- Alterações em `native/observability` acionam os consumidores pertinentes.
- Alterações em `rust/runtime/sms-runtime` acionam a regressão Rust, o host legado dependente quando aplicável e o workflow do debugger.
- Alterações no host de áudio legado executam os testes Windows e Linux; não significam que CPAL esteja integrado.
- Alterações de workflow executam actionlint e a regressão do workspace, mas não forçam compilações de hosts legados sem relação com a alteração.
- Alterações de documentação executam higiene de diff, sem build Rust indiscriminado.

Os filtros que combinam inclusões e exclusões usam `predicate-quantifier: some-with-excludes`. O gate agregado compara explicitamente o resultado real de cada job com sua aplicabilidade esperada.

## 5. Hosts transitórios e funcionalidades ainda não integradas

### egui/eframe, D3D12 e WASAPI/XAudio2

São hosts de transição. Permanecem disponíveis para preservação e regressão quando suas dependências são afetadas, ou por execução manual opt-in. Não devem bloquear alterações exclusivas da nova camada Slint/WGPU por APIs sem relação com o patch.

### CPAL

CPAL ainda não possui integração funcional nesta baseline. Não criar testes que afirmem enumerar dispositivos, criar streams ou tratar underruns antes de existir a implementação R3. A saída de áudio emulada e o host físico são responsabilidades distintas.

### SDL3

SDL3 ainda não está integrado. O runtime portátil de input existente continua testado como núcleo, mas isso não é validação SDL3. Não chamar os testes atuais de testes SDL3 nem testar APIs inexistentes.

### Scripts de desenvolvimento e hosts legados

- `scripts/project.ps1` é a referência para auditoria de workspaces e lockfiles.
- `scripts/build-app.ps1` e `atualizar_recompilar_sega_emulator.ps1` constroem o shell egui legado; não são gates da nova GUI Slint.
- `scripts/compile-fast.ps1` é uma verificação local de conveniência, não substitui os gates Release da CI.
- `scripts/install_sdl3_runtime.ps1` apenas prepara manualmente a DLL SDL3 oficial com checksum fixado. Não comprova integração SDL3 e não é executado pela CI.
- `scripts/common.ps1` fornece a inicialização MSVC compartilhada pelos scripts Windows.

A CI analisa sintaticamente scripts PowerShell alterados, incluindo o helper de build na raiz, e executa `project.ps1 audit` e `project.ps1 locks` no gate de tooling.

## 6. Testes que devem permanecer protegidos

Inventário de arquivos de integração encontrado na baseline:

| Domínio | Arquivos |
|---|---:|
| Z80 CPU | 30 |
| Z80 integrado ao SMS | 48 |
| SMS temporal/estado/interrupções | 4 |
| MachineScheduler | 3 |
| VDP | 1 |
| SMS runtime API | 1 |
| **Total** | **87** |

Esse inventário conta arquivos de integração versionados; não inclui testes unitários internos aos módulos Rust.

O teste `native/machine_scheduler/tests/stress_replay.rs` é executado como teste ignorado por um job dedicado, com limite de memória. Não remover o gate nem classificar a ausência de execução como aprovação.

A validação externa DD/FD existente é um conjunto de vetores SingleStepTests específico. Ela não é ZEXDOC/ZEXALL e não altera a regra de que E8 só começa após o fechamento demonstrável de E7.

Não foram encontrados arquivos de testes Python na árvore desta baseline. Caso ferramentas Python voltem a fazer parte do backend, deverão receber gates próprios, sem substituir os testes Rust.

## 7. Lockfiles e workspaces

Workspaces com lockfile independente devem ser auditados individualmente:

- `Cargo.lock` — workspace Rust raiz, incluindo `native/sega_desktop` quando integrado.
- `native/audio_output/Cargo.lock` — host de áudio legado.
- `native/sega_app/Cargo.lock` — aplicação egui/eframe legada.

O host GUI `native/sms_debugger` é um utilitário legado e fica explicitamente `NOT RUN` na CI padrão. O núcleo `debugger_core` permanece coberto pelo workspace raiz. Se o host GUI voltar a ser suportado, deverá receber lockfile versionado e gate opt-in próprio antes de ser reativado.

O refresh de lockfiles é uma operação manual/opt-in que produz artefato temporário para revisão. O workflow não recebe permissão `contents: write`, não faz commit e não executa push automático.

`scripts/project.ps1` deve descobrir os membros do workspace raiz via `cargo metadata` e auditar cada workspace independente explicitamente. Manifests fora dos workspaces reconhecidos são classificados como `NOT RUN`, nunca como validados.

## 8. Segurança e permissões

- Permissões padrão mínimas; jobs de validação usam `contents: read`.
- Checkout com `persist-credentials: false`.
- Actions externas referenciadas por SHA imutável e com versão anotada.
- Downloads de ferramentas são versionados e verificados por checksum.
- Nenhum workflow de CI de rotina recebe permissão de escrita para atualizar branches.
- Alterações de workflows devem ser revisadas como código executável, incluindo filtros, expressões, permissões e scripts inline.

## 9. Classificação de resultados

| Estado | Significado |
|---|---|
| PASS | Gate aplicável executado e concluído com sucesso no SHA registrado |
| FAIL | Gate aplicável executado e falhou |
| BLOCKED | Gate não pôde ser executado por pré-requisito ausente |
| NOT RUN | Validação não executada; sem evidência de aprovação |
| SKIPPED | Job não executado por condição de impacto; não equivale a PASS |

A PR só pode ser integrada quando todos os gates aplicáveis ao diff estiverem em `success`, os não aplicáveis estiverem realmente `skipped`, e não houver bloqueios técnicos pendentes. A autorização de merge não substitui os critérios de aceitação.
