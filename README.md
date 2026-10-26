# Sega Emulator

Projeto de emulação de alta precisão para a família Sega. **Estratégia arquitetural oficial: núcleo de emulação em Rust; frontend Qt/PySide6 e ferramentas em Python.** A migração é incremental e guiada por contratos, testes diferenciais e CI, não por reescrita integral nem pela obrigação de manter dois runtimes completos.

O primeiro marco é SG-1000; a expansão seguirá gradualmente para Master System/Game Gear, Mega Drive/Genesis, Sega CD e 32X. O Gearsystem é uma referência externa de estudo para SG-1000/SMS/Game Gear: https://github.com/drhelius/Gearsystem. Seu código declara GPL-3.0 e não deve ser copiado sem análise explícita da compatibilidade de licença.

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

## Estratégia de linguagens e estado do runtime

- **Rust é a implementação oficial do hardware:** CPU Z80, barramento/memória, I/O, VDP, PSG, sincronização e runtime da máquina.
- **Python permanece na aplicação:** Qt/PySide6, configuração da UI, testes pytest, testes diferenciais, auditorias, ferramentas e benchmarks.
- **Referência Python seletiva:** mantida temporariamente apenas quando contribui para pesquisa ou comparação. Não é objetivo manter dois emuladores completos. O backend Rust só passa a ser padrão após critérios funcionais e regressões aprovados.
- **PyO3/Maturin são a fronteira de integração.** Chamadas Python por instrução/acesso a byte devem sair do caminho crítico. GIL e threads só serão alterados com modelo explícito de sincronização.
- O crate `native/sg1000_core` é o destino standalone, ainda parcial. A extensão `native/z80_core` oferece integração experimental com Qt, mas a documentação técnica registra limitações temporais e de ownership. Não afirmar que já existe um núcleo SG-1000 completo ou precisão ciclo a ciclo.

Decisão aprovada: [ADR 0007 — Rust-first para o núcleo](docs/adr/0007-rust-first-emulation-core.md). Plano incremental: [migração do runtime para Rust](docs/rust-runtime-migration-plan.md). Diretrizes para implementação e validação, inclusive workflows/agentes, estão em [.github/copilot-instructions.md](.github/copilot-instructions.md).

## Núcleo SG-1000 standalone em Rust (em desenvolvimento)

O crate puro `native/sg1000_core` é o destino do runtime independente de Python. Ele já contém contrato de barramento, memória ROM/RAM e uma fatia inicial de CPU Z80 e máquina headless; ainda **não** representa um SG-1000 completo. VDP, PSG, portas de I/O e scheduler precisam ser integrados/validados no crate standalone. Executar ROM sintética ou compilar não comprova compatibilidade com jogos.

## Backend Rust experimental no Qt

A interface Qt permite selecionar o backend Z80 Python (referência) ou Rust (experimental). O runtime experimental já conecta CPU, memória e componentes de VDP/PSG em uma extensão PyO3, mas não equivale ao crate standalone completo. O frontend, configurações e API da aplicação continuam em Python.

Limitações que permanecem explícitas:
- Parte da coordenação nativa usa objetos PyO3; o GIL permanece adquirido durante o quadro. A máquina ainda precisa migrar seus estados e dispositivos para estruturas Rust puras.
- A temporização atual usa T-states nominais; isso não equivale a um modelo de cada ciclo de barramento nem comprova precisão ciclo a ciclo.
- Getters de diagnóstico como `vram`/registradores podem retornar cópias; mutá-las não altera o estado interno.
- Nenhum fallback para Python deve mascarar instruções/recursos não suportados no backend Rust.

Para compilar e instalar a extensão no Windows/PowerShell, com o ambiente virtual e dependências instalados:
```powershell
python -m pip install maturin
Push-Location native/z80_core
maturin develop --release --features python-extension
Pop-Location
```

Para compilar e abrir a interface:
```powershell
.\scripts\test-rust-in-game.ps1
```

Os resultados registrados nas PRs #92/#93 e no workflow Qt referem-se aos commits testados naquele momento, não ao HEAD atual. Sempre consulte o status de CI da revisão candidata antes de tratar os resultados históricos como atuais.

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
