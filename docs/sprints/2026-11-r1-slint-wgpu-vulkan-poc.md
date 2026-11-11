# Sprint R1 — POC Slint + WGPU 30 + Vulkan

**Início de referência:** 2026-11-11  
**Baseline:** main SHA 392a5c67e85a11a5a980d8f7055bcc8a12fe3784  
**ADR:** 0011 — Replataforma desktop  
**Status inicial:** POC executável NOT RUN.

## Objetivo

Comprovar integração executável entre Slint 1.18, WGPU 30 e Vulkan no Windows x64: janela Slint, device/queue compartilhados, textura externa WGPU atualizada e composição na mesma janela. Esta sprint não implementa frontend final nem integra o runtime do emulador.

## Escopo

- Verificar APIs oficiais e versões.
- Criar aplicação mínima Rust/Slint.
- Inicializar WGPU 30 com Vulkan explícito.
- Compartilhar wgpu::Device e wgpu::Queue com Slint.
- Criar e atualizar textura de teste.
- Importar/compor textura na janela Slint.
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

Documentação das APIs comprova viabilidade teórica, não substitui os testes R1-01 a R1-08.

## Plano

1. Confirmar feature/API Slint WGPU 30, configuração manual e importação de textura.
2. Fixar dependências e features no manifest/lockfile.
3. Implementar janela e inicialização WGPU/Vulkan.
4. Comprovar device/queue compartilhados.
5. Atualizar textura animada e compor com Slint.
6. Tratar resize, surface lifecycle e shutdown.
7. Executar fmt, Clippy e testes aplicáveis antes da CI.
8. Build e execução Release Windows x64.
9. Registrar logs, SHA, ambiente e limitações.
10. Abrir PR; merge exige autorização explícita.

## Gates

cargo fmt --all -- --check  
cargo clippy --release --workspace --all-targets --locked -- -D warnings  
cargo test --release --workspace --locked  
Build Release Windows x64; execução Vulkan real; composição Slint/WGPU; atualização contínua da textura.

Classificar cada item PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não é PASS. Sem GPU/driver compatível, execução física fica BLOCKED/NOT RUN.

## Entregáveis

- [x] Escopo documental definido.
- [ ] POC Slint inicia.
- [ ] WGPU 30/Vulkan inicializa.
- [ ] Device/queue compartilhados comprovados.
- [ ] Textura externa atualizada.
- [ ] Composição na mesma janela.
- [ ] Resize e shutdown validados.
- [ ] Build/execução Release Windows x64.
- [ ] Evidência e PR revisadas.

R2 (frames SMS) começa somente após aprovação de R1. CPAL permanece R3.
