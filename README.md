# Sega Emulator

Projeto de emulação de alta precisão com núcleo de hardware prioritariamente em Rust e frontend/ferramentas em Python. O primeiro alvo é o Sega SG-1000; a arquitetura evolui incrementalmente para outras plataformas Sega.

## Estado atual

- **Marco SG-1000:** CPU, memória, I/O, VDP TMS9918A, PSG SN76489AN, entrada e runtime da máquina estão integrados no fluxo da aplicação. A validação funcional alcançada é satisfatória para o primeiro marco, mas não equivale a temporização ciclo a ciclo nem certifica todas as ROMs.
- **CPU Z80:** execução por instrução com T-states nominais e auditoria diferencial contínua; ainda não é um modelo completo ciclo a ciclo de barramento.
- **Frontend Qt 6/PySide6:** janela com menus, barra de ações, ROMs recentes, seleção de drivers de entrada/áudio/vídeo, configurações e diagnóstico. O modo imersivo de tela cheia é dedicado ao jogo: oculta menus, barra de ações e status; F11/Alt+Enter alternam o modo e Esc retorna à janela. A superfície solicita Vulkan pelo Qt Quick e mantém fallback raster explícito se o driver não inicializar; a confirmação do backend efetivo deve ser feita nos logs da máquina de destino. O pacing usa o período nominal NTSC (~59,92 Hz) sem alterar os T-states lógicos.
- **Configurações de áudio e vídeo:** saída mono do PSG, ativação, volume, taxa do mixer (22,05/44,1/48 kHz), buffer (256–2.048 amostras), filtro de vídeo e escala inteira preferencial. O filtro de vídeo nearest/bilinear e a escala 4:3 são aplicados pela cena Qt Quick em Vulkan quando disponível; o caminho raster mantém a compatibilidade. O FIR de 63 taps reduz aliasing digital; não representa a resposta analógica do chip.
- **Carregador:** arquivos `.sg`, `.rom`, `.bin` e ROMs dentro de ZIP; ZIP com múltiplas ROMs exige seleção explícita.
- **Diagnóstico:** logs JSONL/texto por sessão, SHA-256 da ROM, checkpoints de execução e stack traces.
- **Áudio:** PSG SN76489AN integrado e saída PCM mono do frontend Qt por WASAPI compartilhado via PortAudio/sounddevice. Em teste manual recente, o som foi considerado muito próximo ao Gearsystem, um marco de qualidade percebida importante para o SG-1000. Ainda investigamos pequenos engasgos e possível deriva de sincronismo durante sessões prolongadas; essa observação não é uma certificação contra hardware real. O FIR é filtragem digital e não modela a resposta analógica do chip. Consulte o [roadmap de fidelidade do PSG](docs/architecture/psg-fidelity-roadmap.md) e o [contrato dos backends de saída](docs/architecture/audio-output-backends.md).
- **Captura de áudio:** gravação dos comandos temporizados do SN76489 em VGM pelo menu Diagnóstico; consulte o [procedimento de captura VGM](docs/validation/vgm-psg-capture.md) e o [histórico de versões](CHANGELOG.md).

## Requisitos

- Windows x64
- Python 3.14.x x64 (ambiente de desenvolvimento: Python 3.14.7)
- Git for Windows, PowerShell e VS Code (recomendados)

## Instalação no PowerShell

Na raiz do repositório:

```powershell
py -3.14 -m venv .venv
.\\.venv\\Scripts\\python.exe -m pip install --upgrade pip
.\\.venv\\Scripts\\python.exe -m pip install -e ".[dev,qt,frontend]"
```

## Iniciar a interface Qt

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator
```

A interface Qt abre sem ROM carregada. Use **Arquivo → Abrir ROM** para selecionar uma ROM local.

Menus disponíveis: **Arquivo**, **Emulação**, **Drivers**, **Vídeo**, **Configurações → Áudio e vídeo**, **Configurações → Controles**, **Diagnóstico** e **Ajuda**. Em **Drivers → APIs de entrada, áudio e vídeo**, selecione **Automático (XInput → SDL3)**, **XInput** ou **SDL3 Gamepad** para input; **PortAudio/WASAPI compartilhado** ou áudio desativado; e **Automático (Vulkan → Qt Raster)**, Vulkan ou Qt Raster para apresentação. As preferências são globais e persistentes. Os núcleos de cada console devem receber snapshots de input normalizados; nenhuma API do host deve ser chamada no caminho por instrução. Para jogar sem elementos da aplicação, use **Vídeo → Tela cheia** ou pressione **F11** / **Alt+Enter**; o modo imersivo remove menus, barra de ações, status e margens ao redor da imagem. Pressione **Esc** para voltar à janela normal. Em Áudio e vídeo, a aba **Saída de áudio** controla ativação, taxa do mixer e buffer; **PSG / Hardware** exibe o perfil fixo do SN76489AN do SG-1000 e o ganho; **Vídeo** controla o filtro e a escala inteira. Clock, divisor e variante do chip permanecem fixos. O FIR é filtragem digital anti-aliasing, não um modelo analógico do hardware. A SDL3 enumera dispositivos a cada ciclo de entrada para reconhecer conexão/desconexão durante a sessão; controles sem mapeamento SDL são lidos pela API de joystick bruto quando possível. Se um controle conectado for removido durante a execução, a emulação é pausada antes do próximo frame para evitar continuar sem o dispositivo esperado. Use **Configurações → Controles → Dispositivos detectados** para consultar a disponibilidade do backend e a lista atualizada; o log da sessão também registra o estado inicial.

Para iniciar diretamente uma ROM:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.sg"
```

Também é possível apontar para um ZIP:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.zip"
```

Se o ZIP contiver várias ROMs compatíveis, a interface permite selecionar explicitamente o membro. O conteúdo não é extraído automaticamente.

Para usar o frontend Pygame:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.sg" --ui pygame
```

## Estado do marco SG-1000 e primeiro release

O crate puro `native/sg1000_core` continua sendo uma extração experimental e parcial, distinta do runtime integrado usado pela aplicação. Ele ainda não representa sozinho o SG-1000 completo. Não confunda esse crate standalone com a máquina integrada no frontend Qt nem com uma alegação de fidelidade ciclo a ciclo.

## Runtime Rust do SG-1000 no Qt

O frontend Qt executa exclusivamente o runtime nativo Rust. A antiga opção de trocar para o runtime Python foi removida da interface e valores legados em QSettings são migrados para `rust` na inicialização. A implementação Python de referência permanece no repositório para testes diferenciais e desenvolvimento; não é uma opção de runtime de produção. Qt, execução headless e frontend Pygame iniciam a máquina com Rust. O caminho Rust carrega os módulos de hardware Python de forma lazy: eles não são importados nem instanciados durante uma sessão Rust normal.

A extensão nativa fornece:

- núcleo Z80 e barramento de memória sem chamadas a métodos Python por leitura/escrita de byte;
- mapa de ROM espelhada, RAM interna e RAM de cartucho por perfil;
- VDP TMS9918A com portas, raster, VBlank, status de sprites e renderização dos modos gráficos modelados;
- PSG SN76489AN e sincronização de writes de áudio por T-state;
- máquina SG-1000 nativa que coordena `step`, `run_frame`, portas de I/O, interrupções e avanço de CPU/VDP/PSG em Rust.

**O runtime ainda não é totalmente independente de Python internamente.** O frontend e a API de aplicação continuam em Python, e a máquina Rust coordena componentes encapsulados em objetos PyO3; o GIL permanece adquirido durante um quadro. A etapa atual elimina a coordenação Python por instrução e callbacks Python nas portas, mas ainda não move todos os estados para structs Rust puros nem libera o GIL. O modelo temporal usa T-states nominais e não equivale a emulação ciclo a ciclo. A implementação Python permanece disponível como referência diferencial; não há fallback silencioso quando uma instrução Rust não é suportada.

Os getters `vram` e `registers` do VDP nativo retornam cópias para diagnóstico. Alterar diretamente essas cópias não altera o estado interno do VDP; use as portas modeladas para modificar o hardware. O runtime Rust é obrigatório no Qt; as cópias retornadas são destinadas a diagnóstico.

Para compilar e instalar a extensão no Windows/PowerShell, com o ambiente virtual ativado e a partir da raiz do repositório:

```powershell
python -m pip install maturin
Push-Location native/z80_core
maturin develop --release --features python-extension
Pop-Location
```

Para compilar a extensão e abrir o emulador para teste manual em uma única etapa, use o script PowerShell (a partir da raiz do repositório, com `.venv` já criado e dependências `.[dev,qt,frontend]` instaladas):

```powershell
.\scripts\test-rust-in-game.ps1
```

Para abrir diretamente uma ROM local:

```powershell
.\scripts\test-rust-in-game.ps1 "C:\caminho\para\jogo.sg"
```

O script compila a extensão PyO3 em Release, verifica a importação de `NativeSG1000Machine` e abre o frontend Qt. A interface Qt usa o runtime Rust automaticamente; a compilação sozinha não comprova que uma ROM específica é compatível.

Se a extensão não estiver instalada, a interface informa que o runtime nativo é necessário e não faz fallback silencioso para o emulador Python.

As fatias [PR #92](https://github.com/leonardo201800478/sega-emulator/pull/92) e [PR #93](https://github.com/leonardo201800478/sega-emulator/pull/93) passaram no workflow Windows. A PR #93 validou `rustfmt`, Clippy, testes Rust, build Release, **1.674 testes direcionados** e **2.329 testes de regressão** com a extensão instalada. Isso valida os caminhos cobertos, mas não demonstra compatibilidade universal, precisão ciclo a ciclo ou ganho de desempenho em todos os jogos.

## Diagnóstico headless

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.sg" --headless-frames 600
```

O comando informa quadro, PC, instruções, T-states e SHA-256. Isso não prova, por si só, compatibilidade visual ou de jogabilidade.

### Perfil detalhado por fase (novo)

Gera uma medição-base sem instrumentação e um relatório JSON separado com tempos inclusivos de CPU, avanço do VDP, rasterização por linha, composição de sprites e escrita de pixels. O JSON inclui FPS-base, tempos por quadro instrumentado, contadores por modo de vídeo, estado dos registradores e SHA-256 do framebuffer por quadro.

```powershell
.\\.venv\\Scripts\\python.exe tools\\benchmarks\\runtime_diagnostics.py --rom "C:\\caminho\\para\\jogo.sg" --warmup-frames 30 --frames 120 --output runtime-diagnostic.json
```

**Importante:** o perfil instrumentado tem overhead. Os tempos de funções aninhadas são inclusivos; não devem ser somados entre si nem usados como FPS normal. O hash do framebuffer permite localizar o primeiro quadro divergente em execuções comparáveis, mas não determina sozinho se a imagem está correta. A ROM não é copiada para o relatório.

## Snapshot do VDP durante o jogo

Para investigar divergências visuais em uma cena real, execute a interface Qt com a ROM local e avance até a cena desejada. Pressione **F8** ou use **Diagnóstico → Capturar snapshot VDP (JSON + PNG)**. O emulador salva em `logs/vdp_snapshots/`:

- JSON com ROM identificada por SHA-256, PC, contadores da máquina, registradores do VDP, tabela bruta de atributos dos sprites, VRAM integral e hash do framebuffer.
- PNG do último framebuffer concluído.

Capture no mesmo ponto do jogo no Ares e no nosso emulador, mantendo ROM/revisão e sequência de entradas iguais. O snapshot é pontual: a VRAM/registradores são lidos no instante da captura, enquanto o PNG representa o último quadro concluído; portanto, não se deve presumir que ambos sejam um estado raster atomicamente sincronizado. O diagnóstico não altera a renderização e não deve ser usado como prova isolada de equivalência com hardware.

Os snapshots podem conter o estado completo da ROM em execução; mantenha-os locais e não os versione se a ROM for comercial.

## Logs da sessão

Por padrão, os arquivos ficam em `logs\\session-<data-hora>.jsonl` e `.log`. Para escolher outro diretório:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator --log-dir "C:\\temp\\sega-logs"
```

O JSONL é destinado à análise automatizada; o log textual é para leitura rápida. Os logs são artefatos locais de execução e não devem ser versionados.

## Testes

```powershell
.\\.venv\\Scripts\\python.exe -m pytest -q
.\\.venv\\Scripts\\python.exe -m ruff check .
.\\.venv\\Scripts\\python.exe -m ruff format --check .
git diff --check
```

## Licença e créditos

O Sega Emulator é distribuído sob a licença **MIT**. Consulte o arquivo [LICENSE](LICENSE) para os termos completos. O diálogo **Ajuda → Sobre o Sega Emulator** mostra a versão, o escopo atual do SG-1000, créditos tecnológicos e a licença.

O projeto é independente e não é afiliado à SEGA. SEGA e SG-1000 são marcas de seus respectivos titulares. Consulte os créditos e as limitações técnicas neste README e nos documentos de arquitetura antes de interpretar resultados como evidência de fidelidade absoluta.

## Documentação

- [Marco de qualidade do SG-1000 — áudio, vídeo, sincronismo e primeiro release](docs/milestones/sg1000-audio-video-milestone-2026-10-26.md)
- [Notas preliminares do release v0.1.0](docs/releases/v0.1.0.md)
- [Arquitetura multi-sistema e política Rust-first](docs/architecture.md)
- [Contrato global de drivers de entrada, áudio e vídeo](docs/architecture/host-driver-apis.md)
- [Arquitetura dos crates Rust nativos, dependências e limites de CI](docs/architecture/native-rust-crates.md)
- [Backends de apresentação de vídeo e validação Vulkan](docs/architecture/vulkan-presentation-qt-quick.md)
- [Instruções para agentes de código](AGENTS.md)
- [Prompt de orientações do projeto (ChatGPT; menos de 8.000 caracteres)](docs/project-instructions-prompt.md)
- [ADR 0007 — Política de desenvolvimento Rust-first](docs/adr/0007-rust-first-development-policy.md)
- [Pesquisa de compatibilidade de hardware Sega](docs/research/hardware-compatibility-roadmap-2026-10.md)
- [Notas de hardware SG-1000](docs/hardware/sg1000.md)
- [Modelo e plano de fidelidade do VDP TMS9918A](docs/hardware/tms9918a-model.md)
- [Backlog consolidado de validação da TMS9918A](docs/hardware/tms9918a-validation-backlog.md)
- [Memória, portas e barramentos SG-1000](docs/hardware/sg1000-memory-buses.md)
- [Modelo digital do PSG SN76489AN](docs/hardware/sn76489an.md)
- [Catálogo prioritário de compatibilidade SG-1000](docs/hardware/sg1000-game-compatibility.md)
- [Marco do primeiro jogo executável](docs/sg1000-first-playable.md)
- [Auditoria de cobertura da CPU Z80](docs/z80-coverage.md)
- [Auditoria técnica da CPU Z80](docs/z80-code-audit-2026-10-22.md)
- [Plano de resolução das lacunas Z80](docs/z80-gap-resolution-plan.md)

## Limitações conhecidas e próximos passos

1. Investigar os pequenos engasgos e o possível descompasso que aparecem após sessões longas, medindo fila PCM, underruns/overflows, duração dos frames e deriva temporal antes de alterar o scheduler.
2. Continuar a validação diferencial de vídeo e entrada com ROMs conhecidas, checkpoints reproduzíveis e referências independentes.
3. Expandir a cobertura de CPU/VDP/PSG sem declarar precisão ciclo a ciclo enquanto o modelo de barramento não a sustentar.
4. Validar a saída em diferentes dispositivos Windows e documentar as condições dos testes. A avaliação positiva atual é observacional e não substitui validação em hardware real.

ROMs comerciais e logs locais não devem ser incluídos no repositório. Use homebrew ou cópias obtidas legalmente no ambiente local.
