# Escopo do projeto — Rust-only

## Objetivo

Construir um emulador Sega modular, determinístico e temporalmente fiel, com backend e aplicação integralmente em Rust. A prioridade de produto é o Master System temporal. A interface desktop é egui/eframe e começa com menus operacionais mínimos.

## Escopo ativo

- Z80 compartilhado em Rust, sem cópias por console.
- SMS temporal usando MachineScheduler, barramento/memória/mapper SMS, VDP temporal e SN76489 temporal.
- Worker SMS em `rust/runtime/sms-runtime`, com ownership exclusivo da máquina e comandos coarse-grained.
- Hosts nativos Rust para vídeo D3D12, áudio WASAPI/XAudio2 e input.
- Aplicação egui/eframe responsável somente pela apresentação e envio de comandos de sessão.
- Testes Rust semânticos, temporais, diferenciais e de integração; testes físicos classificados separadamente.

## Fora do escopo nesta etapa

- SG-1000, Game Gear, Mega Drive, Sega CD e 32X como runtimes executáveis.
- Biblioteca visual de ROMs, scraper, catálogo online, frontend rico e painéis de configuração extensos.
- Python, PySide6/Qt, PyO3, Maturin ou fallback para qualquer runtime legado.
- JIT, frame skipping, hacks de timing e otimizações especulativas.
- Simulação elétrica/transistores/pinos sem relação comprovada com comportamento observável.

## Estado da migração

A branch de migração extraiu o worker SMS para Rust puro e removeu a aplicação Python, os testes Python, tooling Python, extensão PyO3 e workflows Qt/Python. O SMS atômico antigo foi retirado do crate ativo: `native/sms_core` agora contém o caminho temporal e fachadas/reexports para os componentes compartilhados. A CI no HEAD exato ainda precisa validar as alterações mais recentes. O shell egui permanece demonstrativo e não está integrado ao runtime.

O SMS temporal reutiliza o único Z80/ALU, SN76489 e VDP compartilhados. A implementação continua parcial: VDP por scanline, WAIT fornecido por fixtures, ausência de Save States persistentes e integração física de host pendentes.

## Prioridades de implementação

1. Validar `rust/runtime/sms-runtime` em CI Linux e Windows/D3D12 no HEAD exato.
2. Mover os crates ativos para a raiz canônica `rust/` e consolidar workspace/lockfiles.
3. Integrar ROM, lifecycle, input, vídeo e áudio ao shell egui, sem transferir clocks à UI.
4. Remover o SG-1000 legado depois que o SMS estiver executável e validado.
5. Portar para Rust os harnesses externos essenciais, incluindo o runner ZEXDOC/ZEXALL, sem antecipar E8 antes do fechamento E7.
6. Executar validação física Windows e regressão determinística.
