# Sega Emulator

Projeto de emulação de alta precisão com implementação de referência em Python, começando pelo Sega SG-1000.

## Estado atual

- **CPU:** núcleo Z80 em auditoria diferencial; temporização ainda não é ciclo a ciclo.
- **Máquina SG-1000:** CPU, ROM/RAM, VDP TMS9918A inicial e joystick em desenvolvimento.
- **Interface experimental:** menus Qt 6/PySide6 para abrir ROMs, pausar/continuar, reiniciar, alterar escala e consultar diagnóstico. A entrada de controles usa backend SDL3/PySDL3; Pygame permanece separado para a saída de áudio.
- **Carregador:** arquivos `.sg`, `.rom`, `.bin` e ROMs dentro de ZIP; ZIP com múltiplas ROMs exige seleção explícita.
- **Diagnóstico:** logs JSONL/texto por sessão, SHA-256 da ROM, checkpoints de execução e stack traces.
- **Áudio:** PSG SN76489AN integrado à máquina, com saída mono nos frontends Pygame e Qt; a fidelidade sonora contra hardware real continua pendente.

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

## Iniciar a interface experimental

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator
```

A interface Qt abre sem ROM carregada. Use **Arquivo → Abrir ROM** para selecionar uma ROM local.

Menus disponíveis: **Arquivo**, **Emulação** (pausar/continuar e reiniciar), **Vídeo** (escalas de teste), **Controles** (configuração dos gamepads), **Diagnóstico** (logs e estado da máquina) e **Ajuda**. A SDL3 enumera dispositivos a cada ciclo de entrada para reconhecer conexão/desconexão durante a sessão; controles sem mapeamento SDL são lidos pela API de joystick bruto quando possível. Se um controle conectado for removido durante a execução, a emulação é pausada antes do próximo frame para evitar continuar sem o dispositivo esperado. Use **Controles → Dispositivos detectados** para consultar a disponibilidade do backend e a lista atualizada; o log da sessão também registra o estado inicial.

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

## Backend SG-1000 Rust experimental no Qt

O frontend Qt permite alternar entre **Python (referência)** e **Rust (experimental)** em **Emulação → Selecionar núcleo Z80…**. A seleção fica persistida nas configurações Qt. Se houver uma ROM carregada, trocar de núcleo recria a máquina para começar em estado limpo.

O backend Python permanece como padrão e referência de compatibilidade. Ao selecionar Rust, a extensão nativa fornece:

- núcleo Z80 e barramento de memória sem chamadas a métodos Python por leitura/escrita de byte;
- mapa de ROM espelhada, RAM interna e RAM de cartucho por perfil;
- VDP TMS9918A com portas, raster, VBlank, status de sprites e renderização dos modos gráficos modelados;
- PSG SN76489AN e sincronização de writes de áudio por T-state;
- máquina SG-1000 nativa que coordena `step`, `run_frame`, portas de I/O, interrupções e avanço de CPU/VDP/PSG em Rust.

**O runtime ainda não é totalmente independente de Python internamente.** O frontend e a API de aplicação continuam em Python, e a máquina Rust coordena componentes encapsulados em objetos PyO3; o GIL permanece adquirido durante um quadro. A etapa atual elimina a coordenação Python por instrução e callbacks Python nas portas, mas ainda não move todos os estados para structs Rust puros nem libera o GIL. O modelo temporal usa T-states nominais e não equivale a emulação ciclo a ciclo. A implementação Python permanece disponível como referência diferencial; não há fallback silencioso quando uma instrução Rust não é suportada.

Os getters `vram` e `registers` do VDP nativo retornam cópias para diagnóstico. Alterar diretamente essas cópias não altera o estado interno do VDP; use as portas modeladas para modificar o hardware. Esse contrato será revisto antes de tornar o backend Rust padrão.

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

O script compila a extensão PyO3 em Release, verifica a importação de `NativeSG1000Machine` e abre o frontend Qt. Na interface, selecione **Emulação → Selecionar núcleo Z80… → Rust (experimental)**; a compilação sozinha não troca o backend selecionado nas configurações.

Reabra o emulador e selecione **Emulação → Selecionar núcleo Z80… → Rust (experimental)**. Se a extensão não estiver instalada, a interface explica como instalá-la e preserva o backend anterior.

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

## Documentação

- [Arquitetura multi-sistema](docs/architecture.md)
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

## Limitações e próximos passos

1. Reproduzir e corrigir a renderização incorreta observada com ROM real.
2. Validar a entrada de joystick com hardware e jogos compatíveis.
3. Validar amostras, ruído e latência do PSG contra referências independentes e hardware real.
4. Aumentar a precisão temporal CPU–VDP com regressões reproduzíveis.

ROMs comerciais e logs locais não devem ser incluídos no repositório. Use homebrew ou cópias obtidas legalmente no ambiente local.
