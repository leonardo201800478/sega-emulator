# Sega Emulator — Emulação de Alta Precisão

Projeto modular de emulação de hardware Sega, com implementação de referência em Python, testes automatizados e evolução incremental. O primeiro alvo é o **SG-1000**; não se pretende implementar todas as plataformas simultaneamente.

> **Estado do projeto (24/10/2026):** núcleo em desenvolvimento. A execução da interface ou de uma ROM não constitui, isoladamente, prova de compatibilidade. A precisão ciclo a ciclo e a compatibilidade abrangente com jogos comerciais ainda não foram demonstradas.

## Estado atual

- **CPU Z80:** núcleo Python com decodificação para instruções base e famílias de prefixos; em auditoria semântica e diferencial. Os T-states retornados são nominais: a CPU ainda não agenda cada acesso de barramento em cada ciclo.
- **Máquina SG-1000:** integração inicial de CPU, ROM/RAM, VDP TMS9918A e entradas de joystick em desenvolvimento.
- **Memória:** 1 KiB de RAM espelhada em `C000h–FFFFh`; ROM linear e espelhamento simplificado de cartuchos pequenos. Banking, SRAM e hardware especial de cartuchos não estão implementados.
- **Vídeo:** renderizador funcional inicial do TMS9918A com modos gráficos, portas, VRAM, status e sprites. A avaliação de sprites ocorre durante a renderização do quadro, não em tempo real por scanline.
- **Sprites — PR #33:** opção `sprite_limit_enabled=True` por padrão; `False` remove o limite visual de quatro sprites por linha para diagnóstico, mas mantém o status de overflow. PR aberta em draft, ainda sem merge. Veja [PR #33](https://github.com/leonardo201800478/sega-emulator/pull/33) e o [registro de continuidade](docs/project-handoff-2026-10-24.md).
- **Áudio:** PSG ainda não integrado à máquina; fidelidade sonora e sincronização audiovisual continuam pendentes.
- **Frontend:** interface experimental Qt 6/PySide6; Pygame é uma opção de frontend. Não representa ainda uma experiência final de jogo.
- **Diagnóstico:** logs JSONL/texto por sessão, SHA-256 da ROM, checkpoints de execução e stack traces.
- **ROMs:** carregamento de `.sg`, `.rom`, `.bin` e ROMs dentro de ZIP. ZIP com múltiplas ROMs compatíveis exige seleção explícita.

## Objetivos e critérios técnicos

1. Preservar determinismo: a mesma ROM, estado inicial e sequência de entradas devem produzir a mesma sequência de estados e resultados.
2. Diferenciar execução funcional de precisão temporal. Ciclos nominais não equivalem a micro-operações ou acessos de barramento ciclo a ciclo.
3. Construir testes unitários, de integração, regressão, determinismo e comparação diferencial antes de afirmar compatibilidade.
4. Manter o núcleo desacoplado do frontend e das APIs de apresentação.
5. Usar Python como referência legível. Considerar Rust seletivamente após profiling, contratos estáveis e testes de equivalência.
6. Não distribuir ROMs comerciais, BIOS protegidas ou material sem licença adequada.

## Requisitos de desenvolvimento

- Windows x64
- Python 3.14.x x64 (ambiente usado no projeto: Python 3.14.7)
- Git for Windows, PowerShell e VS Code (recomendados)

## Instalação no PowerShell

Na raiz do repositório:

```powershell
py -3.14 -m venv .venv
.\\.venv\\Scripts\\python.exe -m pip install --upgrade pip
.\\.venv\\Scripts\\python.exe -m pip install -e ".[dev,qt,frontend]"
```

## Executar a interface experimental

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator
```

A interface Qt abre sem ROM carregada. Use **Arquivo → Abrir ROM** para selecionar uma ROM local.

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

## Diagnóstico headless

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator "C:\\caminho\\para\\jogo.sg" --headless-frames 600
```

O comando informa quadro, PC, instruções, T-states e SHA-256. Isso ajuda a reproduzir uma execução, mas não prova compatibilidade visual, sonora ou de jogabilidade.

## Logs da sessão

Por padrão, os arquivos ficam em `logs\\session-<data-hora>.jsonl` e `.log`. Para escolher outro diretório:

```powershell
.\\.venv\\Scripts\\python.exe -m sega_emulator --log-dir "C:\\temp\\sega-logs"
```

O JSONL é destinado à análise automatizada; o log textual é para leitura rápida. Logs locais de execução não devem ser versionados.

## Testes e qualidade

Execute a partir da raiz:

```powershell
.\\.venv\\Scripts\\python.exe -m pytest -q
.\\.venv\\Scripts\\python.exe -m ruff check .
.\\.venv\\Scripts\\python.exe -m ruff format --check .
git diff --check
```

Não declarar testes aprovados sem execução observável e resultados registrados. Em alterações via GitHub, usar os resultados do GitHub Actions; não presumir que a suíte local foi executada.

## Documentação

- [Registro de continuidade e estado para nova conversa](docs/project-handoff-2026-10-24.md)
- [Arquitetura multi-sistema](docs/architecture.md)
- [Pesquisa de compatibilidade de hardware Sega](docs/research/hardware-compatibility-roadmap-2026-10.md)
- [Notas de hardware SG-1000](docs/hardware/sg1000.md)
- [Memória, portas e barramentos SG-1000](docs/hardware/sg1000-memory-buses.md)
- [Catálogo prioritário de compatibilidade SG-1000](docs/hardware/sg1000-game-compatibility.md)
- [Marco do primeiro jogo executável](docs/sg1000-first-playable.md)
- [Auditoria de cobertura da CPU Z80](docs/z80-coverage.md)
- [Auditoria técnica da CPU Z80](docs/z80-code-audit-2026-10-22.md)
- [Plano de resolução das lacunas Z80](docs/z80-gap-resolution-plan.md)
- [Renderização e avaliação de sprites TMS9918A](docs/hardware/tms9918a-sprite-rendering.md)

## Roadmap incremental

1. **Estabilizar a PR #33:** confirmar todos os checks, revisar as regressões de sprites e manter a PR sem merge até haver evidência suficiente.
2. **Investigar falhas de CI nas PRs abertas:** recuperar os logs individualmente; não assumir uma causa comum sem evidência.
3. **Consolidar CPU Z80:** fechar lacunas semânticas documentadas, melhorar testes diferenciais e só então especificar a execução temporal de barramento.
4. **Consolidar VDP:** validar status, sprites, modos gráficos e limites contra referências independentes; planejar temporização por scanline como etapa separada.
5. **Integrar PSG:** definir modelo de registradores, divisores, mistura e temporização com testes de frequência e regressão.
6. **Validar o sistema completo:** homebrew e ROMs legalmente obtidas, testes de determinismo e comparação de áudio/vídeo.
7. **Otimizar com evidências:** profiling antes de mudanças de desempenho ou migração seletiva para Rust.

## Limitações conhecidas

- Ainda não há prova de emulação Z80 ciclo a ciclo.
- O VDP não modela contenção, pipeline de pixels ou efeitos de escritas no meio da linha/quadro.
- A integração de joystick precisa de validação com jogos e hardware.
- PSG e sincronização audiovisual ainda não estão integrados.
- Não declarar compatibilidade geral com jogos reais apenas porque o emulador inicia ou apresenta uma imagem.

Use homebrew, testes sintéticos e material legalmente obtido. Não adicione ROMs comerciais, BIOS protegidas ou logs de execução locais ao repositório.
