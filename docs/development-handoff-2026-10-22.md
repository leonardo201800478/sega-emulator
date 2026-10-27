> **Estado vigente (2026-10-27):** runtime SG-1000 ativo em Rust. O hardware Python antigo e os testes/ferramentas que dependiam dele estão arquivados em `legacy/python_reference/` e não devem ser executados/importados. Este documento preserva observações históricas; comandos apontando para os caminhos arquivados não são comandos ativos.

# Registro de desenvolvimento e ponto de retomada — 22/10/2026

## Objetivo deste registro

Preservar o estado confirmado do projeto Sega Emulator ao encerrar a sessão, para que a próxima sessão continue deste ponto sem repetir a sincronização nem recomeçar a auditoria.

## Repositório e estado Git confirmado

- Repositório: [leonardo201800478/sega-emulator](https://github.com/leonardo201800478/sega-emulator)
- Branch principal: `main`
- HEAD confirmado: `d6228f24b4f562d5b366296c0494f00c23def8c9`
- Commit: [style(test): format indexed opcode test program](https://github.com/leonardo201800478/sega-emulator/commit/d6228f24b4f562d5b366296c0494f00c23def8c9)
- Estado local reportado pelo usuário: `main...origin/main`, sem alterações pendentes.
- A branch de trabalho `cpu/z80-ed-absolute-word-transfers` continua existindo localmente e no remoto; não foi excluída nem mesclada nesta sessão.
- O clone local foi sincronizado com `git switch main` e `git pull --ff-only origin main`. A atualização avançou de `dccb4fc` para `d6228f2` sem reescrever histórico.

### Incidente de sincronização resolvido

Inicialmente, o terminal estava na branch `cpu/z80-ed-absolute-word-transfers`. Por isso, `git pull` sem argumentos apenas verificava essa branch, não a `main`. A referência local `origin/main` também estava temporariamente inconsistente durante um `fetch`. A inspeção de branches e a troca explícita para `main` resolveram a situação. Não foi necessário reset, rebase, merge manual ou exclusão de branch.

## Verificações locais executadas pelo usuário

Ambiente informado:
- Windows (`win32`)
- Python 3.14.7
- pytest 9.1.1
- Ambiente virtual: `.venv`

Resultados copiados do terminal em 22/10/2026:
- `python -m pytest -v`: **262 passed in 0.60s**
- `ruff check .`: **All checks passed!**
- `ruff format --check .`: **19 files already formatted**
- `git diff --check`: sem saída de erro
- `git status -sb`: `## main...origin/main`
- `git log -1 --oneline`: `d6228f2 (HEAD -> main, origin/main, origin/HEAD) style(test): format indexed opcode test program`

Esses são resultados locais fornecidos pelo usuário, não uma execução feita por esta sessão em um ambiente Windows remoto. O último CI previamente confirmado também passou; o resultado do CI e a execução local são evidências distintas.

## Conteúdo atualmente presente na main

No avanço para `d6228f2`, o Git informou 10 arquivos alterados:
- `.github/workflows/ci.yml`
- `README.md`
- `docs/architecture.md`
- `docs/codebase-audit.md`
- `docs/z80-coverage.md`
- `src/sega_emulator/__main__.py`
- `src/sega_emulator/bus/interfaces.py`
- `src/sega_emulator/cpu/z80.py`
- `tests/test_smoke.py`
- `tests/unit/test_z80.py`

O workflow de CI executa Ruff format check, Ruff lint e pytest em Python 3.14 no GitHub Actions.

## Estado técnico confirmado

### Base de projeto
- Implementação de referência em Python, com módulos de CPU, barramento/memória e testes.
- RAM com 1 KiB de armazenamento físico e espelhamento conforme os testes atuais.
- Barramento inicial com ROM de cartucho somente leitura e RAM.
- Contratos de barramento lógico em `src/sega_emulator/bus/interfaces.py`, incluindo contrato de I/O separado.
- Entrada do pacote ainda informa o estado de scaffold: a máquina SG-1000 completa não está integrada.

### CPU Z80
A implementação expandiu-se para cobrir caminhos de execução das famílias base, CB, ED, DD/FD e DD/FD-CB, incluindo:
- instruções base e operações de registradores/ALU;
- rotações, shifts, BIT, RES e SET;
- operações ED, transferências de palavras, I/O, operações de bloco e operações especiais;
- IX/IY, registradores altos/baixos e endereçamento indexado;
- HALT, DI/EI e um modelo inicial de NMI/INT;
- contagem nominal de T-states por instrução.

A existência de caminhos de execução e a aprovação dos testes atuais **não demonstram** validação exaustiva de todas as entradas, compatibilidade geral com programas, nem precisão ciclo a ciclo.

### Limitações importantes
- O modelo de ciclos soma T-states nominais por instrução; não agenda cada acesso de memória/I/O em ciclos individuais.
- Ainda não há validação diferencial abrangente com uma suíte Z80 de referência.
- Flags de operações de bloco, comportamentos não documentados, prefixos repetidos, casos especiais de IX/IY e arbitragem de interrupções precisam de comparação independente.
- IM 0 aceita atualmente apenas opcodes RST fornecidos no barramento de dados; outros opcodes são rejeitados explicitamente.
- Não há ainda mapa real de portas SG-1000, VDP, PSG, entrada integrada, sincronização audiovisual ou frontend.
- O projeto, portanto, ainda não é um emulador SG-1000 funcional completo.
- A licença do projeto continua sendo uma decisão pendente documentada no README.

## Próximas ações recomendadas

Executar em ordem incremental, sem começar ainda uma integração ampla de dispositivos:

1. **Corrigir inconsistências documentais** em `docs/z80-coverage.md` e `docs/codebase-audit.md`. A cobertura Z80 contém pelo menos uma descrição antiga dizendo que HALT não está implementado, embora a própria matriz e os testes atuais mostrem o contrário; revisar também menções antigas à contagem de testes e à inexistência de CI.
2. **Validar diferencialmente o Z80** contra uma suíte de referência confiável e legalmente utilizável. Confirmar licença e procedência antes de copiar ou incorporar código/testes externos.
3. Priorizar resultados, flags, PC/SP, wraparound, ciclos nominais, instruções ED de bloco, DD/FD e interrupções. Registrar divergências reproduzíveis como testes de regressão.
4. Corrigir as divergências encontradas com mudanças pequenas; atualizar testes e matriz de cobertura na mesma alteração.
5. Só depois de estabelecer uma base de CPU mais confiável, especificar o modelo temporal/barramento necessário à integração do SG-1000.
6. Integrar RAM, CPU, portas de I/O, VDP, PSG, entrada e execução principal em etapas independentes, com testes unitários e de integração.
7. Decidir e documentar a licença do projeto antes de distribuir o código como produto.

## Comandos locais para repetir as verificações

No PowerShell do VS Code:

```powershell
cd C:\Users\leost\sega-emulator
git switch main
git pull --ff-only origin main
.\.venv\Scripts\Activate.ps1
python -m pytest -v
ruff check .
ruff format --check .
git diff --check
git status -sb
git log -1 --oneline
```

Se `git pull --ff-only` falhar por divergência, parar e inspecionar `git status -sb`, `git branch -avv` e `git log --oneline --decorate --graph --all -20`. Não usar `git reset --hard` nem rebase sem identificar primeiro os commits e alterações locais.

## Critérios para declarar avanço

- Distinguir testes sintéticos aprovados de validação diferencial externa.
- Não declarar CPU “completa” até cobrir e validar os opcodes documentados aplicáveis ao Z80 escolhido.
- Não declarar precisão ciclo a ciclo enquanto os acessos ao barramento e a temporização dos dispositivos não forem modelados e testados nesse nível.
- Confirmar cada commit remoto e link antes de informar que uma mudança foi publicada.
- Manter o trabalho diretamente na `main`, conforme a preferência atual do usuário; não abrir PRs por padrão.

## Nota de retomada

Ao retomar, começar verificando HEAD remoto e o estado dos documentos, depois corrigir as inconsistências da documentação Z80 antes de implementar novas famílias de hardware. Não repetir a sincronização que já foi concluída se o clone continuar em `main` no commit `d6228f2` ou em um descendente dele.
