# Escopo do projeto — Rust-only

**Baseline documental:** main `3ad1781f84b7824b5aa889a583ae6d4545987ec1` (2026-11-07).  
**Normas:** ADR 0010, ADR-0016 e [roadmap mestre](project-roadmap.md).

## Objetivo

Construir um emulador Sega modular, determinístico e temporalmente fiel, com backend e aplicação integralmente em Rust. O primeiro runtime de produto é o Master System temporal. A interface desktop é egui/eframe.

## Escopo ativo

- Z80 compartilhado em Rust, sem cópias por console.
- SMS temporal usando MachineScheduler, barramento/memória/mapper SMS, VDP temporal e SN76489 temporal.
- Worker SMS em `rust/runtime/sms-runtime`, com ownership exclusivo da máquina e comandos coarse-grained.
- Hosts nativos Rust para vídeo D3D12, áudio WASAPI/XAudio2 e input.
- Shell egui/eframe responsável pela apresentação e envio de comandos de sessão.
- Testes Rust semânticos, temporais, diferenciais e de integração; testes físicos classificados separadamente.
- Evidências vinculadas a SHA, ambiente, comando e escopo em `docs/evidence/`.

## Fora do escopo nesta etapa

- SG-1000, Game Gear, Mega Drive, Sega CD e 32X como novos runtimes de produto executáveis antes da conclusão de P1.
- Biblioteca visual completa de ROMs, scraper, catálogo online, frontend rico e painéis extensos de configuração.
- Python, PySide6/Qt, PyO3, Maturin ou fallback para qualquer runtime legado.
- JIT, frame skipping, hacks de timing e otimizações especulativas.
- Simulação elétrica/transistores/pinos sem relação comprovada com comportamento observável.

## Estado integrado

- Shell nativo egui/eframe integrado na main.
- Worker SMS temporal Rust disponível.
- Z80 compartilhado, Scheduler e componentes de hardware reutilizados.
- PR #615 integrou melhoria de filtragem de saída SN76489 e reset do ruído.
- A campanha longa do Scheduler está documentada como PASS para os testes executados; não é prova de integração temporal sistêmica.

## Integração em andamento — não integrada

- PR #613: ligação do shell ao runtime SMS temporal, ROM e viewport D3D12.
- PR #614: áudio WASAPI/XAudio2 e input teclado/SDL3; branch empilhada sobre #613.
- PR #616: auditoria documental de boot/cartuchos.

Consultar o [status atual](project-status-2026-11-07.md) antes de assumir que qualquer PR foi integrada.

## Limites conhecidos

A implementação temporal ainda é parcial. A campanha isolada do Scheduler não comprova VDP raster/fetch, WAIT real em todos os caminhos, Save States persistentes completos, compatibilidade ampla, saída física de áudio/vídeo ou gameplay. Essas lacunas devem permanecer explícitas até haver evidência própria.

## Prioridades de implementação

1. Integrar e validar a fatia SMS do shell/runtime/vídeo/áudio/input por PRs empilhadas, sem duplicar worker ou Scheduler.
2. Executar gates Rust e validação física Windows no HEAD final.
3. Validar ROM sintética/homebrew, lifecycle, determinismo e restauração de estado.
4. Fechar auditoria semântica Z80 E7; E8 permanece bloqueada até o fechamento.
5. Avançar para SG-1000 (P2) somente após P1.
