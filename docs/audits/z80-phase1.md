# Fase 1 — auditoria diferencial do Z80

## Objetivo

Aumentar a cobertura empírica do núcleo Z80 antes de novas otimizações. Esta fase
usa duas fontes independentes:

- **SingleStepTests/Z80**: vetores JSON por opcode, fixados no commit
  `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` (licença MIT no upstream).
- **ZEXDOC/ZEXALL**: exercitadores externos em formato CP/M `.COM`; não são
  versionados neste repositório. O arquivo usado deve ser obtido legalmente e
  sua origem/licença registrada pelo operador.

## Auditoria SingleStepTests completa

No GitHub, abra **Actions → Z80 full SingleStepTests audit → Run workflow**.
O workflow baixa o corpus do commit fixado, executa a suíte unitária e depois
processa todos os arquivos JSON. Os relatórios JSON/Markdown e os metadados da
execução são publicados como artefato da execução.

Para executar localmente no ambiente de desenvolvimento:

```powershell
python -m pip install -e ".[dev]"
git clone https://github.com/SingleStepTests/z80.git external/SingleStepTests-z80
git -C external/SingleStepTests-z80 checkout ebe1875d48f374bcfd4b505d8eb8ee751568b5f7
python run_singlestep_full.py `
  --directory external/SingleStepTests-z80/v1 `
  --workers 2 `
  --report-json artifacts/singlestep-full.json `
  --report-md artifacts/singlestep-full.md
```

Para fazer um smoke test local, acrescente, por exemplo,
`--files 00 "cb 00" "ed 44"`. O diretório deve conter os JSON originais do
corpus; os arquivos de teste não devem ser modificados.

## ZEXDOC/ZEXALL

O harness `run_zex.py` prepara uma memória plana de 64 KiB,
intercepta as chamadas BDOS 2 e 9 e termina no warm boot. Exemplo:

```powershell
python run_zex.py C:\caminho\para\zexall.com --max-seconds 300
python run_zex.py C:\caminho\para\zexall.com --tests 0,1,2
```

O código de saída 3 indica interrupção por limite de tempo, não aprovação
completa. O relatório do operador deve registrar o nome e SHA-256 do binário,
a duração, a quantidade de testes concluídos e qualquer erro. Não publicar o
binário externo no repositório.

## O que os resultados provam — e o que não provam

O runner compara registradores/estado exposto, RAM, transações de I/O e o total
de T-states agregado retornado por `cpu.step()` com o número de entradas do
vetor. Isso não compara cada borda ou evento individual do barramento e não
prova precisão de ciclo. Campos não suportados e exceções são falhas da
auditoria, não aprovações.

Uma divergência só deve levar a mudança de CPU depois de confirmar a semântica
do vetor, o comportamento documentado do Z80 e, quando possível, um segundo
oráculo independente. Os resultados devem ser mantidos como artefatos com o
commit do emulador e o commit do corpus.
