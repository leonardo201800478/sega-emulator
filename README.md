> **Estado do SMS — 2026-10-29, base `b4a85c82b1d975a126f1dcbd3084dac51249b398`:** o runtime SMS nativo em Rust já integra CPU Z80, memória/cartucho, I/O, VDP Mode 4 e PSG e está conectado ao frontend Qt/PySide6 via PyO3. Isso não significa conclusão de fidelidade: a renderização do VDP é orientada a scanlines, os T-states da CPU são nominais e o scheduler mestre/precisão ciclo a ciclo continuam pendentes. A síntese YM2413/FM também não está implementada. Consulte o [estado técnico detalhado do SMS](docs/audits/sms-current-technical-status-2026-10-29.md), auditado nesse SHA. Resultados de testes/workflows antigos permanecem vinculados aos respectivos candidatos e não são automaticamente PASS para esta base.

# Sega Emulator

## Atualização do runtime de áudio Rust — 2026-10-31

As PRs [#299](https://github.com/leonardo201800478/sega-emulator/pull/299), [#302](https://github.com/leonardo201800478/sega-emulator/pull/302) e [#303](https://github.com/leonardo201800478/sega-emulator/pull/303) integraram na `main` a sessão DSP e o fluxo PCM Rust para SG-1000/SMS. Python/PyO3 não transporta buffers PCM durante gameplay. O mantenedor relatou teste auditivo positivo nos dois cores e nos dois backends (WASAPI e XAudio2): som mais limpo, menos engasgos e ruídos, timbre percebido como muito próximo ao Ares e volume mais alto. É evidência observacional, não benchmark, medição de loudness, latência ou prova de equivalência ao hardware. O pump ainda ocorre após `run_frame()` no fluxo Qt; desacoplamento do timer, validação física prolongada, hotplug, FM/YM2413, estéreo nativo completo e filtros analógicos calibrados continuam pendentes. Base da main: `9c6f2b75c4994f1eb3eb7ccc0cd64c7c4d2b2de2`. Consulte [arquitetura e validação do áudio](docs/architecture/native-audio-runtime.md#10-resultado-do-port-e-validação-auditiva-manual-2026-10-31).


Projeto de emulação de alta precisão com hardware ativo implementado em Rust. Python na aplicação fica restrito à interface Qt, carregamento de ROMs/perfis e integração fina com o runtime nativo; ferramentas e testes são infraestrutura, não um runtime de emulação. O primeiro alvo é o Sega SG-1000.

## Marco SG-1000 — concluído no escopo funcional validado (2026-10-28)

A correção final do mapeamento de RAM dos cartuchos DahJee/Jumbo Type A foi integrada à `main` pela [PR #141](https://github.com/leonardo201800478/sega-emulator/pull/141), no commit de merge `f51c0fac8dfd850a761e65de48709a2e7002d11d`. O issue de investigação [#136](https://github.com/leonardo201800478/sega-emulator/issues/136) foi encerrado como concluído.

**Compatibilidade funcional manual — PASS, conforme relato do mantenedor:**
- King's Valley (Taiwan)
- Knightmare (Taiwan)
- The Legend of Kage (Taiwan)
- Othello (Japan)
- Pippols (Taiwan)
- Star Soldier (Taiwan)
- Tank Battalion (Taiwan)
- TwinBee (Taiwan)

O mantenedor relata que os oito jogos executaram corretamente após a correção e que os demais jogos anteriormente funcionais continuam operando normalmente. A alteração mapeia 8 KiB de RAM externa Type A em `0x2000–0x3FFF`, preservando a RAM interna de 1 KiB em `0xC000–0xFFFF`; o perfil de Othello permanece separado.

**Checks locais registrados na PR #141 — PASS:** build Release da extensão via Maturin, 114 testes Rust, 192 testes Python, `cargo fmt --all -- --check` e `cargo clippy --workspace --all-targets -- -D warnings`.

**Limites da evidência:** testes diferenciais contra Genesis Plus GX/MAME/Ares e os checks `ruff check`, `ruff format --check`, `compileall` e `pip check` ficaram **NOT RUN** nesta validação. A consulta das execuções de workflow para o commit de merge não retornou runs; CI para esse SHA está **NOT VERIFIED**. Este marco registra compatibilidade funcional observada manualmente, não compatibilidade universal, precisão ciclo a ciclo ou equivalência comprovada com hardware real. Nenhuma ROM comercial foi adicionada ao repositório.

Detalhes técnicos e testes: [perfis de RAM DahJee/Jumbo e Othello](docs/hardware/sg1000-dahjee-othello-cartridge-profiles.md). O próximo marco é o Master System em Rust, mantendo as regressões SG-1000 como requisito.

## Estado atual

- **Marco SG-1000:** CPU, memória, I/O, VDP TMS9918A, PSG SN76489AN, entrada e runtime da máquina estão integrados no fluxo da aplicação. A validação funcional alcançada é satisfatória para o primeiro marco, mas não equivale a temporização ciclo a ciclo nem certifica todas as ROMs.
- **CPU Z80:** execução por instrução com T-states nominais e auditoria diferencial contínua; ainda não é um modelo completo ciclo a ciclo de barramento.
- **Frontend Qt 6/PySide6:** janela com menus, barra de ações, ROMs recentes, seleção de drivers de entrada/áudio/vídeo, configurações e diagnóstico. O modo imersivo de tela cheia é dedicado ao jogo: oculta menus, barra de ações e status; F11/Alt+Enter alternam o modo e Esc retorna à janela. A superfície solicita Vulkan pelo Qt Quick e mantém fallback raster explícito se o driver não inicializar; a confirmação do backend efetivo deve ser feita nos logs da máquina de destino. O pacing usa o período nominal NTSC (~59,92 Hz) sem alterar os T-states lógicos.
- **Configurações de áudio e vídeo:** saída mono do PSG, ativação, volume, taxa do mixer (22,05/44,1/48 kHz), buffer (256–2.048 amostras), quatro filtros de apresentação (Nearest, Bilinear, Dither Blending e CRT/NTSC), escala inteira e preferências de VSync/VRR/G-Sync e sincronismo A/V. Dither Blending detecta blocos 2×2 em xadrez e só os mistura quando os pares de cores são consistentes; não aplica blur global. CRT/NTSC oferece scanlines, máscara de fósforo, curvatura e modos RGB/Composite/RF aproximados por processamento espacial CPU/Numpy, compartilhados pelos backends Qt Quick e Raster. Composite/RF não modelam fase temporal, subportadora por linha nem a cadeia elétrica real; curvatura e efeitos avançados precisam de validação de desempenho no Windows. VSync/VRR ainda não são garantidos pelo backend efetivo; selecionar uma preferência não confirma que esteja ativa. O modo pelo relógio de áudio é experimental e ainda não altera o scheduler. O FIR de 63 taps reduz aliasing digital; não representa a resposta analógica do chip.
- **Carregador:** arquivos `.sg`, `.rom`, `.bin` e ROMs dentro de ZIP; ZIP com múltiplas ROMs exige seleção explícita.
- **Diagnóstico:** logs JSONL/texto por sessão, SHA-256 da ROM, checkpoints de execução e stack traces.
- **Áudio:** PSG SN76489AN integrado; sessão DSP, resampling, filas e entrega PCM são executados em Rust. A sessão foi movida para native/audio_runtime, com contrato PcmSink independente de PyO3 e testes headless com sink determinístico. As APIs Python/PyO3 de drenagem PCM, conversão e escrita direta no backend foram removidas; Qt envia apenas comandos de configuração/ciclo de vida e consulta métricas. WASAPI Shared/event-driven tenta IAudioClient3 (fallback Shared convencional); XAudio2 permanece alternativa. O pump ainda é acionado ao concluir run_frame() chamado pelo pacing Qt; a autonomia do scheduler/produção de áudio em relação ao timer de apresentação continua pendente. Fontes estéreo nativas e filtros analógicos calibrados por chip também continuam pendentes. Em teste manual anterior, o som foi considerado próximo ao Gearsystem; isso é avaliação observacional, não certificação de fidelidade ou latência. O FIR é filtragem digital e não modela a resposta analógica do chip. Consulte o [runtime de áudio multi-sistema](docs/architecture/native-audio-runtime.md), o [roadmap de fidelidade do PSG](docs/architecture/psg-fidelity-roadmap.md) e o [contrato dos backends de saída](docs/architecture/audio-output-backends.md).
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
.\\.venv\\Scripts\\python.exe -m pip install -e ".[dev,qt]"
```

## Executar e depurar pelo VS Code (Windows)

Abra a **raiz do repositório** no VS Code. O projeto fornece configurações em `.vscode/launch.json` e tarefas em `.vscode/tasks.json`.

Requisitos: Windows x64, Python 3.14 x64, Rust stable e Visual Studio Build Tools com a carga de trabalho **Desenvolvimento para desktop com C++** (MSVC).

1. No VS Code, execute **Terminal → Run Task… → Sega Emulator: Create Python 3.14 venv** uma vez.
2. Execute **Terminal → Run Task… → Sega Emulator: Install development dependencies**.
3. Pressione **F5** e escolha **Sega Emulator: Qt (desenvolvimento)**. A tarefa prévia compila e instala o runtime Rust/PyO3 em Release; em seguida o debugger inicia o módulo `sega_emulator`.
4. Para abrir uma ROM local, escolha **Sega Emulator: Qt com ROM** e informe o caminho completo quando solicitado.
5. Para os testes Qt direcionados, execute **Terminal → Run Task… → Sega Emulator: Run Qt tests**.

Se a compilação nativa falhar, consulte o primeiro erro de Rust/MSVC no terminal. O frontend ativo exige o runtime Rust; não há fallback de emulação em Python. Use apenas ROMs que você tenha direito de executar. A tarefa de testes usa Qt offscreen e não abre uma janela interativa.

## Iniciar a interface Qt

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator
```

A interface Qt abre sem ROM carregada. Use **Arquivo → Abrir ROM** para selecionar uma ROM local.

Menus disponíveis: **Arquivo**, **Emulação**, **Áudio**, **Drivers**, **Vídeo**, **Configurações → Áudio e vídeo**, **Configurações → Controles**, **Diagnóstico** e **Ajuda**. O menu **Áudio** reúne configurações de saída, seleção de backend/APIs, diagnóstico e gravação VGM. Em **Drivers → APIs de entrada, áudio e vídeo**, selecione **Automático (XInput → SDL3)**, **XInput** ou **SDL3 Gamepad** para input; **WASAPI nativo (Rust)**, **XAudio2 nativo (Rust)** ou áudio desativado; e **Automático (Vulkan → Qt Raster)**, Vulkan ou Qt Raster para apresentação. As preferências são globais e persistentes. Os núcleos de cada console devem receber snapshots de input normalizados; nenhuma API do host deve ser chamada no caminho por instrução. Para jogar sem elementos da aplicação, use **Vídeo → Tela cheia** ou pressione **F11** / **Alt+Enter**; o modo imersivo remove menus, barra de ações, status e margens ao redor da imagem. Pressione **Esc** para voltar à janela normal. Em Áudio e vídeo, a aba **Saída de áudio** controla ativação, taxa do mixer e buffer; **PSG / Hardware** exibe o perfil fixo do SN76489AN do SG-1000 e o ganho; **Vídeo** controla o filtro e a escala inteira. Clock, divisor e variante do chip permanecem fixos. O FIR é filtragem digital anti-aliasing, não um modelo analógico do hardware. A SDL3 enumera dispositivos a cada ciclo de entrada para reconhecer conexão/desconexão durante a sessão; controles sem mapeamento SDL são lidos pela API de joystick bruto quando possível. Se um controle conectado for removido durante a execução, a emulação é pausada antes do próximo frame para evitar continuar sem o dispositivo esperado. Use **Configurações → Controles → Dispositivos detectados** para consultar a disponibilidade do backend e a lista atualizada; o log da sessão também registra o estado inicial.

Para iniciar diretamente uma ROM:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.sg"
```

Também é possível apontar para um ZIP:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.zip"
```

Se o ZIP contiver várias ROMs compatíveis, a interface permite selecionar explicitamente o membro. O conteúdo não é extraído automaticamente.

## Estado do marco SG-1000 e primeiro release

`native/sg1000_core` é o núcleo oficial de hardware SG-1000 em Rust puro. `native/z80_core` é a fronteira PyO3/Maturin; a máquina integrada delega CPU, memória, I/O, VDP, PSG e execução ao núcleo Rust. Isso não implica precisão ciclo a ciclo: o modelo temporal continua baseado em instruções e T-states nominais.

## Runtime SG-1000 — arquitetura vigente

O frontend Qt usa uma fachada Python fina que encaminha as operações para `NativeSG1000Machine`, implementada sobre `native/sg1000_core`. Não há seleção de backend Python nem fallback para emulação Python. As implementações antigas de CPU, memória/barramento, VDP, PSG, sincronização e máquina foram movidas para `legacy/python_reference/`: ficam fora do pacote executável e não devem ser importadas por aplicação, testes ou workflows ativos.

O hardware ativo em Rust inclui:
- CPU Z80 e ALU;
- memória, ROM espelhada, RAM interna e RAM de cartucho configurada por perfil;
- barramento e portas de I/O;
- VDP TMS9918A, raster, VBlank, status e renderização;
- PSG SN76489AN, eventos temporizados e geração de amostras digitais;
- entrada, interrupções e coordenação da máquina.

Python de produção fica restrito à interface Qt e backends de host, carregamento de ROMs e perfis de cartucho, configuração/logging de aplicação e a fachada de integração. Ferramentas/testes Python devem validar Rust; não podem usar as implementações em `legacy/` como runtime ou oráculo ativo.

PyO3 expõe snapshots e operações da máquina. O GIL permanece adquirido durante as chamadas da extensão; isso não implica callbacks por instrução nem precisão ciclo a ciclo. O modelo temporal atual é baseado em instruções e T-states nominais.

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

Se a extensão não estiver instalada, a interface informa que o runtime nativo é necessário e encerra a inicialização; não existe fallback para emulação Python.

As fatias [PR #92](https://github.com/leonardo201800478/sega-emulator/pull/92) e [PR #93](https://github.com/leonardo201800478/sega-emulator/pull/93) passaram no workflow Windows. A PR #93 validou `rustfmt`, Clippy, testes Rust, build Release, **1.674 testes direcionados** e **2.329 testes de regressão** com a extensão instalada. Isso valida os caminhos cobertos, mas não demonstra compatibilidade universal, precisão ciclo a ciclo ou ganho de desempenho em todos os jogos.

### Opções de carregamento

```powershell
.\.venv\Scripts\python.exe -m sega_emulator --help
.\.venv\Scripts\python.exe -m sega_emulator --version
.\.venv\Scripts\python.exe -m sega_emulator --list-roms ".\roms\colecao.zip"
```

A CLI abre a interface Qt, aceita uma ROM opcional e lista membros compatíveis de um ZIP sem extraí-los. Não há frontend Pygame nem runner headless no pacote ativo.

No frontend Qt, **Velocidade lógica** compara T-states processados por segundo com o clock nominal. **Envio à UI (FPS)** mede a taxa de quadros enviados ao widget; não mede diretamente a apresentação física do monitor. Os logs incluem a janela de medição e p50/p95/p99 do tempo de processamento do quadro no lado da aplicação. Quadros descartados pelo compositor e underruns WASAPI permanecem não medidos sem contadores nativos confiáveis.

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
- [Atualização v0.1.1 — sessão de áudio Rust-only](docs/releases/v0.1.1.md)
- [Arquitetura multi-sistema e política Rust-first](docs/architecture.md)
- [Reorganização Rust-first e núcleo Z80 compartilhado](docs/architecture/migrations/shared-z80-runtime-reorganization.md)
- [Cronograma da reorganização e charter dos agentes](docs/architecture/migrations/reorganization-roadmap-and-agent-charter.md)
- [Contrato global de drivers de entrada, áudio e vídeo](docs/architecture/host-driver-apis.md)
- [Arquitetura dos crates Rust nativos, dependências e limites de CI](docs/architecture/native-rust-crates.md)
- [Backends de apresentação de vídeo e validação Vulkan](docs/architecture/vulkan-presentation-qt-quick.md)
- [Instruções para agentes de código](AGENTS.md)
- [Política permanente de desenvolvimento, fidelidade e entrega](docs/development-and-validation-policy.md)
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

Compilação e execução rápida:
.\atualizar_recompilar_sega_emulator.ps1 -Launch
python -m sega_emulator
## Ordem arquitetural de áudio

- [Ordem direta obrigatória: áudio exclusivamente em Rust](docs/architecture/audio-rust-only-directive.md)
