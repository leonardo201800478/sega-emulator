> **Estado vigente (2026-10-27):** o hardware SG-1000 ativo está em Rust. Implementações Python anteriores e ferramentas que dependiam delas foram arquivadas em `legacy/` e não fazem parte do runtime ou da validação ativa. Trechos que descrevem o estado anterior são históricos.

# Build local do backend Rust e validação do frontend Qt no Windows

**Projeto:** `leonardo201800478/sega-emulator`  
**Branch de trabalho:** `feat/qt-modern-settings`  
**Plataforma validada pelo usuário:** Windows x64, PowerShell, VS Code  
**Escopo:** compilar/instalar a extensão Rust via PyO3/Maturin, validar o frontend Qt e executar as verificações de qualidade locais.

> Este documento registra os comandos executados e os resultados informados pelo usuário. Não afirma que a abertura interativa, reprodução de áudio em dispositivo físico ou compatibilidade de uma ROM específica já foram verificadas.

## 1. Estado inicial e objetivo

O objetivo foi preparar o checkout local para executar o frontend Qt com o backend Rust experimental do SG-1000, sem confundir a extensão nativa com um executável Rust independente.

A arquitetura atual é híbrida:

- **Frontend e aplicação:** Python com Qt 6/PySide6.
- **Binding:** PyO3, empacotado/instalado com Maturin.
- **Núcleo nativo:** crate `native/z80_core`, pacote Cargo `sega-z80-core`, módulo de extensão `sega_z80_core`.
- **Workspace Cargo da raiz:** inclui `native/sg1000_core` e `native/z80_alu`, mas exclui `native/z80_core` e não possui uma seção `[package]` própria.

A compilação do crate nativo não transforma o frontend em um programa Rust puro: o ponto de entrada da interface continua sendo Python.

## 2. Ambiente local

O ambiente informado durante a sessão:

- Diretório: `C:\Users\leost\sega-emulator`
- Shell: PowerShell
- Ambiente virtual ativo: `.venv`
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- Cargo: `1.99.1`
- Maturin: `1.15.0`
- Python: CPython 3.14, ambiente virtual do projeto
- Branch: `feat/qt-modern-settings`

Antes da compilação, o estado informado pelo usuário era sincronizado com `origin/feat/qt-modern-settings`, no commit `3c339dd30f9cd9dc82d5781cd68b3f2f273ff989`, com árvore de trabalho limpa.

### Nota sobre referências Git

Em uma tentativa anterior de `git fetch origin`, o Git informou erros de bloqueio/atualização para referências remotas de branches. Não foi usado `git reset --hard` nem outra operação destrutiva para contornar isso. O usuário depois confirmou a branch `feat/qt-modern-settings`, o HEAD e a referência remota no mesmo commit, e árvore de trabalho limpa.

## 3. Por que o primeiro comando Maturin falhou

O primeiro comando tentado foi:

```powershell
maturin develop --release
```

Ele foi executado a partir da raiz do repositório. A falha ocorreu porque o `Cargo.toml` da raiz é um **manifesto de workspace**, não um pacote Cargo com seção `[package]`. O arquivo de raiz declara os membros do workspace e exclui explicitamente `native/z80_core`; por isso, Maturin precisa receber o manifesto do crate de extensão.

Não é necessário alterar o workspace para corrigir esse comando. A solução é apontar diretamente para o manifesto do crate nativo.

## 4. Compilar e instalar a extensão Rust

Execute a partir da raiz do repositório, com a `.venv` ativada e com as dependências Python do projeto já instaladas.

### 4.1 Instalar Maturin, caso necessário

```powershell
python -m pip install maturin
```

### 4.2 Compilar em Release e instalar no ambiente virtual

```powershell
maturin develop --manifest-path native/z80_core/Cargo.toml --features python-extension --release
```

O comando bem-sucedido informado nesta sessão:

- compilou dependências e crates, incluindo `sega-z80-alu`, `sega-sg1000-core` e `sega-z80-core`;
- terminou o perfil `release` otimizado;
- construiu um wheel para CPython 3.14 / Windows x64;
- instalou `sega-z80-core 0.1.0` no ambiente virtual.

Saída relevante reportada:

```text
Finished `release` profile [optimized] target(s) in 12.01s
Built wheel for CPython 3.14 ... sega_z80_core-0.1.0-cp314-cp314-win_amd64.whl
Installed sega-z80-core-0.1.0
```

### 4.3 Avisos do pip observados

Durante a instalação, o pip emitiu avisos repetidos:

```text
Ignoring invalid distribution ~ip (C:\Users\leost\sega-emulator\.venv\Lib\site-packages)
```

A instalação terminou com sucesso apesar dos avisos. Não foi feita limpeza manual do diretório `site-packages`; não apague arquivos de distribuição sem investigar a origem do aviso.

## 5. Verificar o import Python

O usuário executou:

```powershell
python -c "import sega_z80_core; print('Extensão Rust carregada:', sega_z80_core.__file__)"
```

Saída reportada:

```text
Extensão Rust carregada: C:\Users\leost\sega-emulator\.venv\Lib\site-packages\sega_z80_core\__init__.py
```

Isso confirma que o pacote Python `sega_z80_core` pode ser importado do ambiente virtual. **Isoladamente, o caminho para `__init__.py` não prova que o binário compilado foi carregado nem que a máquina em execução está selecionada como Rust.** A compilação/instalação foi confirmada pela saída do Maturin; a seleção e o uso do backend devem ser verificados na interface e nos logs disponíveis.

## 6. Testes focados do frontend Qt

O usuário executou:

```powershell
python -m pytest tests/test_qt_audio_output.py tests/test_qt_settings_dialog.py tests/test_qt_main_window.py tests/test_qt_preferences.py -q
```

Resultado informado:

```text
28 passed in 0.59s
```

Os testes cobrem os módulos focados de saída de áudio, diálogo de configurações, janela principal e persistência de preferências.

## 7. Regressão completa e qualidade

Depois da compilação e dos testes focados, foram executados os três comandos abaixo, na raiz do repositório.

### 7.1 Suíte completa

```powershell
python -m pytest -q
```

Resultado informado:

```text
2341 passed in 7.36s
```

Não foram reportadas falhas nem testes ignorados nessa execução local.

### 7.2 Lint

```powershell
python -m ruff check .
```

Resultado informado:

```text
All checks passed!
```

### 7.3 Verificação de formatação

```powershell
python -m ruff format --check .
```

Resultado informado:

```text
116 files already formatted
```

### Resumo dos resultados locais

| Verificação | Resultado |
|---|---|
| Build Rust Release e instalação via Maturin | Concluídos com sucesso |
| Import de `sega_z80_core` | Concluído |
| Testes focados Qt | 28 aprovados |
| Suíte completa pytest | 2.341 aprovados |
| Ruff lint | Aprovado |
| Ruff format check | 116 arquivos formatados |

Os resultados de testes e comandos nesta seção foram fornecidos pelo usuário na sessão. Não foram executados novamente pelo GitHub nesta documentação.

## 8. Como abrir o emulador com o frontend Qt

Na raiz do repositório, com a `.venv` ativada:

```powershell
python -m sega_emulator
```

Este é o ponto de entrada documentado no README. A interface Qt deve abrir sem ROM carregada; use **Arquivo → Abrir ROM** para selecionar uma ROM local obtida legalmente.

Para iniciar uma ROM diretamente:

```powershell
python -m sega_emulator "C:\caminho\para\jogo.sg"
```

O carregador também suporta ROMs `.rom`, `.bin` e arquivos ZIP compatíveis, conforme documentado no README.

Alternativamente, o ponto de entrada Python da interface pode ser invocado diretamente:

```powershell
python -c "from sega_emulator.frontend.qt_frontend import run; run()"
```

### Confirmar a seleção do backend Rust

O frontend integra a máquina SG-1000 e permite selecionar o backend pelo menu **Emulação → Selecionar backend de emulação…**. Escolha **Rust (runtime nativo experimental)** se não estiver selecionado. O código da branch inclui uma migração de preferências que define Rust como seleção inicial uma única vez, mas uma configuração já persistida pode influenciar a seleção atual.

A compilação por si só não garante que a sessão esteja usando Rust. Verifique a seleção exibida pela interface e observe mensagens de erro/logs. O backend Rust permanece experimental e pode não suportar todas as instruções Z80 necessárias para todas as ROMs.

## 9. Correção do excesso de velocidade no frontend Qt

Na revisão do sintoma de emulação acima de 600%, foi identificado no frontend Qt o agendamento sem limite:

```python
timer.setInterval(0)
```

Um `QTimer` com intervalo zero pede ao event loop para executar o callback assim que possível. Como cada callback executa um quadro inteiro com `machine.run_frame()`, a máquina avançava tão rápido quanto o computador conseguia processá-la; o indicador de velocidade refletia esse avanço acelerado.

A correção adiciona `src/sega_emulator/frontend/qt/frame_pacer.py`, com a classe `FramePacer`, e altera o frontend para:

- usar um timer Qt preciso de 1 ms para manter a responsividade do event loop;
- permitir o início de um quadro apenas quando vence o prazo de pacing;
- usar o período NTSC estimado por `(228 * 262) / Z80_CLOCK_HZ`, aproximadamente 59,92 quadros/s;
- não executar rajadas de recuperação se um quadro ultrapassar o prazo;
- reiniciar o prazo ao carregar ROM ou reinicializar a máquina.

O pacing é aplicado na camada de apresentação/execução do frontend. Não modifica os T-states calculados pelo núcleo nem introduz atrasos artificiais entre instruções CPU/VDP/PSG. Se a máquina não conseguir concluir um quadro dentro do orçamento, a velocidade efetiva poderá ficar abaixo de 100%, em vez de executar em velocidade ilimitada.

Foi acrescentado `tests/test_qt_frame_pacer.py` para verificar primeiro quadro imediato, prazo mínimo entre quadros, reset e ausência de rajadas de recuperação. Os testes ainda precisam ser executados no ambiente local/CI após esta alteração; não declarar o defeito resolvido definitivamente até confirmar o comportamento visual e a velocidade indicada no Windows.

## 10. Checklist de validação manual ainda pendente

A suíte automatizada aprovada não substitui os seguintes testes manuais. Marque cada item apenas após executá-lo no Windows.

- [ ] Abrir o frontend Qt e confirmar que a janela permanece ativa sem exceções.
- [ ] Carregar uma ROM homebrew ou outra ROM obtida legalmente.
- [ ] Confirmar explicitamente a seleção **Rust (runtime nativo experimental)**.
- [ ] Confirmar avanço da emulação, atualização de quadro e resposta aos controles.
- [ ] Abrir **Configurações → Áudio e vídeo**, alterar as preferências e confirmar persistência após reiniciar.
- [ ] Testar áudio em dispositivo físico, se habilitado, e observar estalos, interrupções e latência.
- [ ] Testar escala inteira, proporção 4:3 e filtros nearest-neighbor/bilinear.
- [ ] Verificar os logs em caso de falha e guardar apenas artefatos que não distribuam ROMs protegidas.

**Estado no momento da redação:** os testes automatizados locais estão aprovados segundo a saída fornecida; a abertura interativa do frontend e a validação com ROM/áudio físico ainda não foram confirmadas nesta conversa.

## 11. Limitações e interpretação correta

- O frontend continua sendo uma aplicação Python/Qt integrada ao backend Rust, não um executável independente inteiramente escrito em Rust.
- O backend é experimental; passar na suíte atual não prova compatibilidade universal com jogos.
- Os resultados não demonstram precisão ciclo a ciclo do Z80 nem fidelidade analógica do PSG.
- Não foi medido input lag de ponta a ponta nem feita comparação visual com hardware real nesta etapa.
- O comando de import confirma a importabilidade do pacote Python, mas a seleção efetiva do backend deve ser confirmada em runtime.
- Não foi criado nem versionado qualquer ROM comercial durante este processo.

## 12. Sequência resumida para repetir a validação

```powershell
# A partir da raiz, com .venv ativada
python -m pip install maturin
maturin develop --manifest-path native/z80_core/Cargo.toml --features python-extension --release

python -c "import sega_z80_core; print('Extensão Rust carregada:', sega_z80_core.__file__)"

python -m pytest tests/test_qt_audio_output.py tests/test_qt_settings_dialog.py tests/test_qt_main_window.py tests/test_qt_preferences.py -q
python -m pytest -q
python -m ruff check .
python -m ruff format --check .

# Iniciar a interface Qt para a verificação manual
python -m sega_emulator
```

Consulte também o [relatório da PR #98](../validation/qt-frontend-pr98.md) para o histórico da implementação Qt e a validação executada no GitHub Actions.
