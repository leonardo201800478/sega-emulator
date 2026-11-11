# Sprint R1 — POC Slint + WGPU 30 + Vulkan

**Início de referência:** 2026-11-11
**Baseline:** main SHA 392a5c67e85a11a5a980d8f7055bcc8a12fe3784
**ADR:** 0011 — Replataforma desktop
**Status inicial:** POC executável NOT RUN.

## Objetivo

Comprovar integração executável entre Slint 1.18, WGPU 30 e Vulkan no Windows x64: janela Slint, device/queue compartilhados, textura externa WGPU atualizada e composição na mesma janela. Esta sprint não implementa frontend final nem integra o runtime do emulador.

## Escopo

- Verificar APIs oficiais e versões.
- Criar a aplicação isolada em `native/sega_desktop`, sem dependência do runtime SMS.
- Habilitar `backend-winit`, `renderer-femtovg-wgpu` e `unstable-wgpu-30` no Slint 1.18.
- Inicializar WGPU 30 com Vulkan explícito por `WGPUSettings.backends`.
- Obter o wgpu::Device e wgpu::Queue do renderer Slint pelo rendering notifier e confirmar backend com `Device::adapter_info()`.
- Criar textura RGBA8 com TEXTURE_BINDING, RENDER_ATTACHMENT e COPY_DST.
- Importar a textura via `Image::try_from`, atualizá-la pela queue compartilhada e compô-la com a UI Slint.
- Validar resize, surface lifecycle, erros e shutdown.
- Build e execução Release Windows x64.
- Registrar adapter, backend, versões, features, comandos e limitações.

## Fora do escopo

SmsTemporalWorker, FrameMailbox, ROM, CPU, VDP, PSG, Scheduler, menus de jogo, pausa, HUD, CPAL, SDL3, HDR, VRR, biblioteca e remoção de egui/eframe. Não ampliar R1 para antecipar R2–R5.

## Critérios de aceite

| ID | Critério | Resultado |
|---|---|---|
| R1-01 | Janela Slint inicia | Execução Windows Release |
| R1-02 | WGPU 30 inicializa | Device/queue criados |
| R1-03 | Vulkan selecionado | Backend/adapter registrados |
| R1-04 | Integração Slint/WGPU | Mesmo device e queue comprovados |
| R1-05 | Textura externa | Conteúdo atualizado continuamente |
| R1-06 | Composição | Textura e UI na mesma janela |
| R1-07 | Lifecycle | Resize, surface recreation e shutdown |
| R1-08 | Reprodutibilidade | Build Release e instruções reproduzíveis |
| R1-09 | Backend Slint | Renderer FemtoVG WGPU selecionado, sem fallback |
| R1-10 | Teardown | Recursos externos liberados/recriados sem uso após teardown |

Documentação das APIs comprova viabilidade teórica, não substitui os testes R1-01 a R1-08.

## Plano

1. Confirmar APIs Slint 1.18/WGPU 30 e renderer FemtoVG WGPU.
2. Adicionar `native/sega_desktop` ao workspace Rust, com dependência Slint apenas no target Windows.
3. Selecionar explicitamente `winit` + `femtovg-wgpu`, restringir WGPU a Vulkan e validar adapter/backend no rendering notifier.
4. Criar/importar textura externa e atualizá-la pela queue compartilhada.
5. Exibir textura e UI na mesma janela e solicitar redraw periódico.
6. Tratar teardown/recriação do renderer, resize e shutdown.
7. Executar fmt, Clippy e testes direcionados.
8. Build Release Windows x64 no CI.
9. Executar manualmente em hardware Windows/Vulkan; registrar GPU, driver, SHA e logs.
10. Abrir PR; merge exige autorização explícita.

## Atualização operacional — 2026-11-11

- `Cargo.lock` da workspace raiz foi regenerado com `cargo update --workspace` e verificado com `cargo metadata --locked --format-version 1 --no-deps`.
- O conteúdo gerado corresponde exatamente ao arquivo entregue para integração (138818 bytes; Git blob SHA `de69e313a1a6822c297f989888545113b53f8bc3`; SHA-256 `c5ea77bbe0fcaa7b6d98cb4aaa9dad74e8e148a715c2bb4ccc77dfe4f777ad1a`).
- A atualização está no branch da PR #747. A CI precisa reexecutar Clippy, testes e build Release no HEAD atualizado.
- A execução física em GPU Vulkan continua NOT RUN até validação no Windows x64.

## Gates

cargo fmt --all -- --check
cargo clippy --release --workspace --all-targets --locked -- -D warnings
cargo test --release --workspace --locked
Build Release Windows x64; execução Vulkan real; composição Slint/WGPU; atualização contínua da textura. O CI comprova compilação/testes de unidade, mas não substitui a execução manual com GPU e driver Vulkan.

Classificar cada item PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não é PASS. Sem GPU/driver compatível, execução física fica BLOCKED/NOT RUN.

## Entregáveis

- [x] Escopo documental definido.
- [x] POC Slint inicia.
- [x] WGPU 30/Vulkan inicializa.
- [x] Device/queue compartilhados comprovados.
- [x] Textura externa atualizada.
- [x] Composição na mesma janela.
- [x] Resize responsivo e shutdown validados; recriação in-process permanece NOT RUN.
- [x] Build Release Windows x64 no CI.
- [x] Execução física Windows/Vulkan registrada.
- [x] Evidências físicas e CI registradas; PR #751 revisada.

R2 (frames SMS) começa somente após aprovação de R1. CPAL permanece R3.


## Validação física inicial — Windows x64 / 2026-11-11

Evidências fornecidas pelo usuário durante execução local da Release:

- Janela Slint abriu no Windows 11.
- O status exibido identificou backend Vulkan e adaptador NVIDIA GeForce RTX 5070.
- A mensagem da aplicação confirmou o compartilhamento de device/queue entre Slint e WGPU 30.
- A textura externa exibiu barras de cor, marcador branco móvel e variação vertical de tonalidade.
- O contador observado chegou a 6.305 atualizações.
- Maximização, minimização/restauração, redimensionamento vertical e encerramento/reabertura foram exercitados sem travamento.
- Na primeira versão, a imagem permanecia em tamanho fixo de 640 × 480 logical pixels; a janela podia ser maximizada, mas a área apresentada não ocupava a largura disponível.

Classificação após a evidência manual inicial:

| ID | Resultado | Evidência / ressalva |
|---|---|---|
| R1-01 | PASS | Janela abriu em Windows 11 x64 |
| R1-02 | PASS | Status Vulkan e device/queue ativos |
| R1-03 | PASS | NVIDIA GeForce RTX 5070 identificada com backend Vulkan |
| R1-04 | PASS | Status da aplicação confirmou device/queue compartilhados |
| R1-05 | PASS | Textura animada; contador observado em 6.305 |
| R1-06 | PASS | Textura e textos Slint na mesma janela |
| R1-07 | PARTIAL | Maximizar, minimizar/restaurar e resize vertical sem falhas; imagem inicialmente fixa, correção responsiva em validação |
| R1-08 | PASS CI | Build automatizado da versão mergeada; nova alteração de instrumentação requer CI própria |
| R1-09 | PASS | Backend WGPU Vulkan explícito; execução mostrou Vulkan |
| R1-10 | PARTIAL | Encerramento/reabertura sem travamento; falta inspecionar eventos RenderingTeardown/RenderingSetup no log |

Não foi fornecida captura de informações do driver, SHA-256 do executável ou arquivo de log da execução inicial. Esses dados permanecem pendentes e não devem ser inferidos.

## Instrumentação de lifecycle e layout responsivo — branch de validação

A alteração subsequente em `native/sega_desktop/src/main.rs`:

- Remove as dimensões fixas da área visual da imagem e usa tamanho preferido, limites mínimos e `vertical-stretch`, preservando `image-fit: contain`.
- Exibe contadores de `RenderingSetup` e `RenderingTeardown` na janela.
- Emite eventos estruturados `R1_LIFECYCLE` em stdout/stderr para captura com `Tee-Object`.
- Mantém a liberação da fila compartilhada e da imagem Slint no evento `RenderingTeardown`.
- Não tenta simular uma recriação de superfície nem afirma que resize dispara teardown; o ciclo de superfície pertence ao backend Winit/WGPU.

Validação pendente para esta alteração: fmt, Clippy, testes, build Release Windows x64, teste manual do preenchimento responsivo e inspeção do log de lifecycle. R1-07 e R1-10 permanecem PARTIAL até essas evidências.


## Reteste físico do usuário — resize e teardown / 2026-11-11

Novas evidências enviadas pelo usuário após a build de validação:

- Duas capturas demonstram a textura redimensionando junto com a janela, mantendo sua proporção e preenchendo a área útil disponível.
- O usuário reportou animação fluida.
- A janela identificou novamente Vulkan e NVIDIA GeForce RTX 5070.
- Os contadores mostraram `Renderer setup: 1 | teardown: 0` enquanto a janela estava ativa.
- O log registrou `RenderingSetup count=1` e `RenderingTeardown count=1` no encerramento.
- Na primeira execução, o processo entrou em panic no encerramento: `RefCell already mutably borrowed`, em `i-slint-backend-winit-1.18.1/winitwindowadapter.rs:1852`. O segundo encerramento observado não reproduziu o panic.

### Análise e correção

A causa imediata era a mutação síncrona de propriedades Slint dentro do callback de `RenderingTeardown` (e, por consistência, também em `RenderingSetup`). O callback é executado enquanto o backend Winit mantém empréstimo interno do adaptador; alterar uma propriedade pode disparar atualização/redraw reentrante e tentar obter novo empréstimo mutável do mesmo `RefCell`.

Correção na branch desta PR:

- O callback de lifecycle limita-se a atualizar contadores, liberar a fila externa e preparar os dados de UI.
- Atualizações de propriedades Slint são agendadas com `Timer::single_shot(Duration::ZERO, ...)`, para ocorrerem depois que o notifier retornar.
- A imagem é limpa de forma diferida no teardown.
- O tratamento de erro de backend, falha de importação e API incompatível também usa atualização diferida.

A API `Timer::single_shot` está documentada no Slint 1.18.1. Esta correção evita a mutação reentrante observada; o encerramento precisa ser repetido com a nova build para confirmar o resultado.

| Critério | Estado após evidência do usuário |
|---|---|
| Resize responsivo e proporção | PASS |
| Fluidez visual observada | PASS qualitativo |
| Eventos Setup/Teardown emitidos | PASS |
| Encerramento sem panic | FAIL na primeira execução; segunda execução sem reprodução |
| Correção diferida do notifier | Implementada; CI e reteste pendentes |

Não declarar R1 concluída até a CI desta revisão passar e o usuário confirmar encerramento repetido sem panic na nova build.


## Reteste físico após correção do callback — Windows x64 / 2026-11-11

O usuário executou cinco inicializações independentes da Release corrigida e forneceu os cinco pares de eventos:

```text
R1_LIFECYCLE event=RenderingSetup count=1 backend=Vulkan adapter=NVIDIA GeForce RTX 5070
R1_LIFECYCLE event=RenderingTeardown count=1
```

Resultado observado em todas as cinco execuções:
- `RenderingSetup` executado uma vez e identificando Vulkan / NVIDIA GeForce RTX 5070.
- `RenderingTeardown` executado uma vez.
- Nenhum panic, erro ou travamento nos cinco encerramentos.
- A versão corrigida também foi observada com a textura expandindo junto com a janela, mantendo a proporção.
- O usuário relatou renderização fluida.

CI do commit `b5275dd5c095794d44c260719627c77424c81175` concluída com sucesso: Rust workspace regression, Windows x64 Slint/WGPU 30/Vulkan POC, documentation patch hygiene e Rust quality/unit regression. Jobs não aplicáveis foram SKIPPED e não são contados como PASS.

Atualização de classificação:
- R1-07: PASS para resize responsivo e estabilidade observada em Windows x64.
- R1-10: PASS para teardown no encerramento e ausência de panic em cinco execuções independentes.
- Recriação do renderer dentro do mesmo processo: NOT RUN; os cinco pares setup/teardown correspondem a cinco processos independentes e não comprovam reconfiguração/recriação in-process.

A validação física confirma o caminho de execução e encerramento no hardware do usuário. Não há evidência de teste de perda/recriação de superfície ou de renderer no mesmo processo; não declarar esse cenário validado.


## Fechamento operacional de R1 e handoff para R2 — 2026-11-11

### Resultado consolidado

O usuário executou a aplicação Release em Windows 11 x64 com NVIDIA GeForce RTX 5070. As evidências visuais e os logs confirmam:
- Vulkan como backend ativo, sem fallback.
- Slint e WGPU 30 usando o mesmo device/queue do renderer.
- Textura WGPU externa importada e atualizada continuamente.
- Composição da textura e UI Slint na mesma janela.
- Textura responsiva ao resize, mantendo proporção; animação relatada como fluida.
- Minimizar/restaurar e encerrar/reabrir sem travamento.
- Após a correção de mutação reentrante, cinco execuções independentes produziram exatamente um RenderingSetup e um RenderingTeardown cada, sem panic.

### Correção crítica de lifecycle

A primeira execução instrumentada revelou panic `RefCell already mutably borrowed` no backend Winit ao modificar propriedades Slint sincronamente dentro do rendering notifier. A implementação foi alterada para diferir as mutações de UI via `Timer::single_shot(Duration::ZERO, ...)`. Cinco encerramentos independentes da versão corrigida passaram sem panic.

### CI final da revisão

O commit final da PR #751 é `2e00496165fa02144f11cf16df1b1eff3198dd61`. A CI correspondente executou os gates de workspace, Windows x64 Slint/WGPU 30/Vulkan e higiene documental; os gates aplicáveis devem estar todos concluídos com sucesso antes do merge. SKIPPED não conta como PASS.

### Matriz final honesta

| Critério | Estado | Observação |
|---|---|---|
| R1-01 Janela Slint | PASS | Execução física Windows 11 |
| R1-02 WGPU 30 | PASS | Device/queue ativos |
| R1-03 Vulkan | PASS | RTX 5070 identificada |
| R1-04 Integração Slint/WGPU | PASS | Compartilhamento confirmado |
| R1-05 Textura externa | PASS | Atualização contínua |
| R1-06 Composição | PASS | Textura + UI na mesma janela |
| R1-07 Resize/lifecycle | PARTIAL | Resize responsivo e shutdown aprovados; recriação de superfície/renderer in-process NOT RUN |
| R1-08 Build reproduzível | PASS | CI Windows x64 e build Release |
| R1-09 Backend sem fallback | PASS | Vulkan explicitamente selecionado e observado |
| R1-10 Teardown/recriação | PARTIAL | Teardown no shutdown aprovado em cinco processos; recriação do renderer no mesmo processo NOT RUN |

### Decisão de transição

A POC R1 está aprovada como baseline gráfica operacional para iniciar a integração R2, com uma limitação explicitamente aberta: recriação do renderer/superfície dentro do mesmo processo ainda não foi exercitada. Essa limitação deve permanecer como risco técnico rastreado na R2 e ser coberta antes de considerar o host de produção concluído. Não afirmar que surface recreation foi validada.

### Escopo obrigatório de R2

R2 integra frames reais do SMS ao caminho de apresentação Slint/WGPU, mantendo:
- `SmsTemporalWorker` como proprietário da máquina e da execução contínua.
- `MachineScheduler` como autoridade temporal única.
- CPU, VDP, PSG, barramentos, interrupções, timing e estados exclusivamente no backend Rust.
- Host/GUI apenas com comandos coarse-grained e consumo de frames publicados.
- Transporte de frames com ownership, sincronização e backpressure explícitos; evitar cópias desnecessárias somente após provar segurança e correção.
- Sem callback por instrução, sem controle de ciclos pela GUI e sem espera bloqueante no thread de UI.
- Sem CPAL nesta etapa (R3) e sem SDL3 (R4).

Antes de alterar código, o responsável por R2 deve auditar o fluxo atual de `FrameMailbox`, `VideoFrame`, `PixelFormat`, `SmsTemporalWorker`, `native/video_output_wgpu` e o contrato atual de `native/sega_desktop`; não presumir APIs ou ownership sem ler as implementações.
