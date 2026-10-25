# Protocolo de validação in-game — SG-1000 / núcleo Python

## Objetivo

Preparar uma sessão reproduzível para avaliar a execução de uma ROM SG-1000 no núcleo Python após a integração do Z80. O protocolo separa:
1. inicialização e execução headless;
2. observação visual e controles no frontend;
3. comparação de uma mesma cena no ARES;
4. registro de evidências e criação de regressões.

Este protocolo não presume compatibilidade antes da execução. Não distribua ROMs comerciais; use homebrew ou ROMs que você tenha direito de utilizar.

## 1. Fixar o ambiente e a ROM

Execute na raiz do clone atualizado, em PowerShell. Os comandos abaixo não modificam a ROM.

```powershell
git status --short --branch
git rev-parse HEAD
python --version
$Rom = (Resolve-Path "C:\caminho\para\jogo.sg").Path
$RomHash = (Get-FileHash -Algorithm SHA256 $Rom).Hash.ToLower()
"ROM=$Rom"
"SHA256=$RomHash"
```

Registre o hash SHA-256, tamanho do arquivo, nome do jogo/região/revisão conhecida e a origem legal da ROM. Não coloque o conteúdo da ROM no repositório nem em artefatos públicos.

## 2. Verificação curta sem interface

Instale as dependências de desenvolvimento se necessário e execute primeiro a suíte curta:

```powershell
python -m pip install -e ".[dev,qt,frontend]"
python -m pytest -q
```

Depois faça uma execução de 600 quadros:

```powershell
$Case = "artifacts\in-game\$($RomHash.Substring(0,12))"
New-Item -ItemType Directory -Force $Case | Out-Null
python -m sega_emulator $Rom --headless-frames 600 --log-dir "$Case\logs" 2>&1 |
    Tee-Object "$Case\headless.txt"
if ($LASTEXITCODE -ne 0) { throw "A execução headless falhou (exit code $LASTEXITCODE)." }
```

Registre saída, logs e código de saída. Uma execução concluída não comprova que o jogo esteja visualmente correto nem jogável. Se houver exceção por opcode não implementado, preserve o traceback e o PC/opcode informado; isso se torna um caso reproduzível.

## 3. Teste visual e de jogabilidade no emulador

Abra a mesma ROM no frontend Qt:

```powershell
python -m sega_emulator $Rom
```

Execute o teste por pelo menos 10 minutos ou até alcançar um marco claro do jogo. Anote:
- inicialização, tela de título e transições;
- resposta de direções e botões;
- sprites, tiles, paleta, scroll e flicker;
- som, interrupções, travamentos, resets e mensagens de erro;
- local/ação exata em que aparece cada divergência.

Não altere a ROM entre as execuções. Registre qualquer configuração não padrão. Para investigar uma cena, use **F8** ou **Diagnóstico → Capturar snapshot do VDP** se essa opção estiver disponível na versão em execução; guarde o snapshot junto aos logs.

## 4. Comparação controlada com ARES

1. Abra a mesma ROM e confirme o mesmo hash/tamanho nos dois ambientes.
2. Use a mesma região/revisão e, quando possível, o mesmo estado inicial.
3. Compare a mesma cena e o mesmo momento de entrada; evite comparar screenshots de cenas ou frames diferentes.
4. Capture screenshot do Sega Emulator e do ARES com resolução e proporção documentadas.
5. Para divergências gráficas, preserve as duas imagens originais e anote se a diferença é de conteúdo, posição, paleta, temporização ou apenas escala/apresentação.
6. Para divergências funcionais, anote sequência de botões, número aproximado de quadros/tempo e resultado esperado versus observado.

Não use diferença visual isolada como prova automática de erro do VDP: primeiro elimine diferenças de frame, estado de jogo, escala, overscan e configuração do emulador de referência.

## 5. Estrutura de evidências

Mantenha os dados localmente (a pasta `artifacts/` pode conter caminhos e screenshots privados e não deve ser publicada automaticamente):

```text
artifacts/in-game/<sha-prefix>/
  headless.txt
  logs/
  screenshots/
    sega-emulator/
    ares/
  snapshots/
  observations.md
```

Modelo para `observations.md`:

```markdown
# Caso de validação in-game

- Jogo / região / revisão:
- ROM SHA-256:
- Tamanho da ROM:
- Commit testado:
- Python:
- Versão/configuração do ARES:
- Data e duração:

## Resultado headless
- Código de saída:
- Quadros / PC / instruções / T-states:
- Exceções:

## Cenários
| ID | Estado inicial / ação | Sega Emulator | ARES | Classificação |
|---|---|---|---|---|
| SG1-001 | | | | |

## Evidências
- Screenshot Sega Emulator:
- Screenshot ARES:
- Logs/snapshot:

## Próxima regressão
- Passos mínimos para reproduzir:
- Resultado esperado:
- Resultado observado:
```

## Critérios de avaliação

- **Inicialização:** ROM aceita, sem exceção inesperada.
- **Execução:** quadros avançam sem travamento inesperado.
- **Vídeo:** imagem coerente e comparação feita na mesma cena.
- **Entrada:** comandos básicos produzem a resposta esperada.
- **Determinismo:** duas execuções equivalentes devem ser comparadas com o mesmo hash da ROM e condições iniciais; diferenças de framebuffer devem ser investigadas, não presumidas como erro.
- **Conclusão:** qualquer divergência relevante vira um teste de regressão pequeno e reproduzível.

## Limites conhecidos

A CPU executa instruções e o VDP avança pelos T-states nominais retornados; isso não é sincronização completa por T-state nem prova de precisão ciclo a ciclo. A saída headless confirma avanço do modelo, não fidelidade visual ou jogabilidade. Não declarar compatibilidade plena até existirem evidências repetíveis para o jogo e os cenários testados.
