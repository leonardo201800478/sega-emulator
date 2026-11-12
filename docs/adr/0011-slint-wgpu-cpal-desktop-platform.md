# ADR 0011 — Replataforma desktop com Slint, WGPU e CPAL

- **Status:** ACCEPTED; arquitetura-alvo normativa. Integração incremental em R2; R3/R4/R5 condicionadas aos respectivos gates.
- **Revisão:** 2026-11-11.
- **Baseline:** main SHA 392a5c67e85a11a5a980d8f7055bcc8a12fe3784.
- **Escopo:** substituir gradualmente GUI e adaptadores host; preservar runtime Rust.
- **Precedência:** complementa ADR 0010 para frontend/hosts. Não altera CPU, VDP, PSG, barramentos, Scheduler, ownership ou determinismo.

## 1. Decisão

A replataforma ocorre no repositório existente. Não criar outro repositório nem reescrever hardware.

| Responsabilidade | Tecnologia |
|---|---|
| GUI | Slint 1.18.x |
| API gráfica | WGPU 30.x |
| Backend inicial | Vulkan via WGPU, Windows x64 |
| Saída física de áudio | CPAL 0.18.2 |
| Entrada/gamepads | SDL3 após auditoria |
| Runtime/hardware | Rust existente |

WGPU é a única API gráfica do produto; Vulkan é backend WGPU, não API paralela. R1 deve selecionar e registrar Vulkan explicitamente, sem fallback silencioso. CPAL é a API de saída de áudio; WASAPI pode ser backend interno no Windows.

O caminho Slint/WGPU é o frontend desktop-alvo. CPAL é a única API de saída física definida para a nova arquitetura; SDL3 é a entrada futura após auditoria. Não criar novos consumidores dos hosts anteriores. A remoção física dos módulos ainda usados pelo aplicativo antigo fica condicionada a R3/R5 e aos gates de regressão. O inventário transitório é mantido em docs/architecture/migrations/legacy-host-retirement.md.

## 2. Fronteiras

Slint cuida de janelas, menus, diálogos, biblioteca, configurações e interação. WGPU cuida de device/queue/surface, texturas, composição e apresentação. CPAL cuida do stream físico. SDL3 coleta input.

Essas camadas não executam instruções, avançam clocks, implementam VDP/PSG, possuem estado mutável da máquina ou usam callbacks por instrução. O runtime permanece proprietário da máquina SMS, CPU, Scheduler, VDP, PSG e estado temporal. A UI usa comandos coarse-grained como load_rom, start, pause, stop, reset e step_frame.

## 3. Versões e compatibilidade

Versões de referência:
- Slint 1.18.x; integração WGPU 30 exposta como API/feature instável. A POC habilita explicitamente `renderer-femtovg-wgpu` e `unstable-wgpu-30`, além de `backend-winit`, `std` e `compat-1-18`.
- WGPU 30.x.
- CPAL 0.18.2.

Fixar versões reproduzíveis no Cargo.lock. Fixar a série Slint 1.18 durante a POC e revalidar antes de atualizar. Verificar documentação oficial, changelog, MSRV, licença e compatibilidade.

Referências:
- https://docs.slint.dev/latest/docs/rust/slint/wgpu_30/
- https://docs.slint.dev/latest/docs/rust/slint/wgpu_30/enum.WGPUConfiguration
- https://docs.slint.dev/latest/docs/rust/slint/wgpu_30/struct.WGPUSettings
- https://wgpu.rs/doc/wgpu/
- https://docs.rs/cpal/0.18.2/cpal/

## 4. R1 — POC gráfica mínima

R1 verifica as possibilidades reais das APIs. Não integra o runtime SMS nem constitui frontend de produto.

Critérios:
1. Aplicação mínima Slint 1.18.
2. Inicialização WGPU 30.
3. Slint usa o renderer FemtoVG baseado em WGPU 30 (`renderer-femtovg-wgpu`).
4. Vulkan explicitamente selecionado por `WGPUSettings.backends = Backends::VULKAN`; confirmar `Device::adapter_info().backend`.
5. Mesmo wgpu::Device e wgpu::Queue compartilhados, obtidos pelo rendering notifier Slint.
6. Textura externa RGBA8 com TEXTURE_BINDING, RENDER_ATTACHMENT e COPY_DST, atualizada continuamente pela queue compartilhada.
7. Composição da textura e UI Slint na mesma janela.
8. Resize, recriação de surface e shutdown limpo.
9. Build e execução Release no Windows x64.

A POC seleciona explicitamente o backend `winit` e o renderer `femtovg-wgpu` via `BackendSelector::backend_name()` e `renderer_name()`, além de usar `WGPUConfiguration::Automatic(WGPUSettings)` para restringir o backend WGPU subjacente a Vulkan. `Window::set_rendering_notifier()` fornece o device/queue efetivos do renderer. A textura é importada por `Image::try_from<wgpu::Texture>()` e recuperada para atualização por `Image::to_wgpu_30_texture()`. O caminho manual permanece uma alternativa a avaliar se o renderer automático não satisfizer o ciclo de vida/presentação. Não presumir compatibilidade apenas pela documentação.

**Fora de R1:** SmsTemporalWorker, ROM real, FrameMailbox, menu de jogo, pausa do Scheduler, HUD, CPAL, SDL3, HDR, VRR e paridade do frontend. Não alterar runtime, CPU, VDP, PSG ou Scheduler. Não exigir zero-copy físico; evitar cópias redundantes e demonstrar ownership correto.

**Gate:** execução real com Vulkan confirmado, textura atualizada e composição Slint/WGPU na mesma janela. Análise documental ou CI de jobs não relacionados não é PASS de integração.

## 5. R2–R5

- **R2 — vídeo real:** integrar frames SMS pelo FrameMailbox/contrato portátil, preservando SmsTemporalWorker e Scheduler; validar formato, ownership, sincronização e lifecycle sem alterar timing.
- **R3 — CPAL:** enumerar dispositivos/formatos; criar, executar e encerrar streams; reproduzir PCM real; tratar remoção/reconexão, erros e underruns. Callback eficiente; thread de áudio não controla Scheduler.
- **R4 — SDL3:** auditar e integrar teclado/gamepads, hotplug e mapeamentos.
- **R5 — frontend:** consolidar launcher, biblioteca, configurações, menus, HUD, logs e telemetria. Demonstrar paridade antes de remover egui/eframe e hosts antigos.

Cada fase tem gates independentes. Não ampliar uma POC para antecipar fases posteriores. O corte completo dos hosts anteriores não é autorizado por mera integração R2; depende de CPAL R3 e consolidação R5.

## 6. Matriz de validação CI

A CI é orientada pelo impacto real dos arquivos alterados, não por uma matriz fixa de todas as APIs históricas.

- Mudanças em `native/sega_desktop/**` executam o gate Windows x64 Release da POC Slint/WGPU 30/Vulkan.
- Mudanças no runtime e nos componentes compartilhados continuam sujeitas aos gates Rust de workspace e aos testes focados correspondentes.
- egui/eframe, D3D12 e WASAPI/XAudio2 são hosts transitórios. Seus gates só executam quando os respectivos diretórios de implementação são alterados ou quando a validação legada é solicitada manualmente.
- O job de input valida o runtime genérico de entrada; não representa validação de SDL3, que ainda não foi integrado.
- CPAL e SDL3 terão gates próprios quando suas integrações forem implementadas. Não executar nem declarar PASS de APIs ainda ausentes.
- Alterações exclusivas em workflows não acionam por si só a bateria completa de testes de hardware. Alterações globais de manifests/lockfiles continuam executando a regressão Rust de workspace.

A matriz não remove a baseline legada nem declara paridade. Ela evita que uma POC da nova arquitetura dependa de APIs que não fazem parte do seu escopo.

## 7. Preservação

MachineScheduler permanece autoridade de clocks/eventos. UI, renderização, áudio físico e input não comandam tempo virtual. Preservar ordem causal, WAIT states, operações retomáveis e estado necessário à restauração. Execução contínua e em blocos deve ser equivalente. Não duplicar worker, CPU, PSG, VDP ou Scheduler.

## 8. Licenças

Antes de distribuir binários, revisar licenças/notices de Slint, WGPU, CPAL e dependências transitivas. Registrar obrigações em THIRD_PARTY_NOTICES.md; confirmar termos oficiais vigentes para o uso pretendido.

## 9. Estado

- Arquitetura e capacidades documentadas justificam a POC.
- POC integrada compilada/executada: NOT RUN na revisão inicial; implementação submetida em PR própria para validação Windows Release.
- Vulkan em hardware Windows: NOT RUN.
- Textura externa e composição: NOT RUN.
- Frames reais SMS: NOT RUN.
- CPAL/SDL3 na nova arquitetura: NOT RUN.
- egui/eframe e hosts antigos continuam na baseline.

Esta ADR autoriza a POC, não declara migração implementada. Conclusões exigem evidência no HEAD final da PR.
