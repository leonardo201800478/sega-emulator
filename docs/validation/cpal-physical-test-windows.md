# Validação física de áudio CPAL — Windows x64

**Branch:** `feat/cpal-audio-backend-20261112`
**PR:** #770
**Estado:** roteiro preparado; execução física depende do PC Windows com dispositivo de áudio.

## Objetivo

Validar a saída PCM real do Sega Master System pelo adaptador CPAL 0.18.2. Esta campanha não altera nem calibra clocks do PSG, Scheduler ou runtime.

## 1. Baseline da main

A `main` no commit-base desta PR (`1f98f4caebc8bd617e3933a50aeb669d58800582`) contém o host Slint/WGPU R2, mas ainda não conecta saída física de áudio. O menu de áudio exibe WASAPI/XAudio2 como opções desabilitadas e informa que a saída física será integrada em R3.

Portanto, a passagem pela main valida que o checkout, o build do host, a janela Vulkan/WGPU e o carregamento de ROM estão funcionais. **Não se deve esperar som na main.** O som físico é avaliado somente na branch CPAL.

No terminal PowerShell do VSCode, na raiz do repositório:

```powershell
git status --short --branch
git fetch origin --prune
git switch main
git pull --ff-only origin main
git rev-parse HEAD
cargo run --release --manifest-path native/sega_desktop/Cargo.toml
```

Registre o SHA exibido. Abra a janela, carregue a mesma ROM SMS que será usada na branch CPAL e confirme vídeo/controles básicos. Feche o emulador antes de trocar de branch.

## 2. Atualizar para a branch CPAL

Se a branch ainda não existir localmente:

```powershell
git switch --track -c feat/cpal-audio-backend-20261112 origin/feat/cpal-audio-backend-20261112
```

Se ela já existir localmente:

```powershell
git switch feat/cpal-audio-backend-20261112
git pull --ff-only origin feat/cpal-audio-backend-20261112
```

Confirme que o HEAD inclui o commit de instrumentação desta campanha:

```powershell
git log -1 --oneline
```

Crie uma pasta para evidências e execute o host Release capturando stdout/stderr:

```powershell
New-Item -ItemType Directory -Force artifacts | Out-Null
cargo run --release --manifest-path native/sega_desktop/Cargo.toml 2>&1 | Tee-Object -FilePath .\\artifacts\\cpal-physical-test.log
```

Na inicialização do worker SMS, procure no log:

```text
R3_AUDIO CPAL output opened: sample_rate=... Hz, channels=..., queue_capacity=4096 frames
```

Se aparecer `R3_AUDIO CPAL unavailable; continuing without physical audio`, a saída física não foi aberta. Registre o erro completo e classifique a execução como FAIL para reprodução física; o vídeo pode continuar funcionando silenciosamente.

## 3. Preparação do Windows

- Selecione em Configurações > Sistema > Som o dispositivo de saída que será testado.
- Confirme que o dispositivo está conectado, habilitado e não está mutado.
- Feche outros emuladores e aplicações que possam monopolizar o dispositivo.
- Use a mesma ROM SMS e o mesmo dispositivo nos ensaios.
- Comece com volume moderado e prefira fones/caixas em volume seguro.

CPAL abre o dispositivo padrão do host. A seleção manual de dispositivo ainda não está exposta na GUI. O log informa sample rate e canais negociados; confirme o dispositivo padrão nas Configurações de Som do Windows.

## 4. Matriz de teste manual

Marque cada resultado como PASS, FAIL ou NOT RUN e anote observações.

| ID | Procedimento | Critério de aprovação |
|---|---|---|
| A01 | Iniciar ROM com áudio conhecido (música/efeitos PSG) | Som audível e estável; sem silêncio contínuo |
| A02 | Manter execução por 5 minutos | Sem travamentos, estalos recorrentes ou perda progressiva de áudio |
| A03 | Pausar por 10 segundos | Áudio deixa de avançar; sem reprodução contínua indevida |
| A04 | Retomar após pausa | Áudio retorna sem replay evidente de blocos antigos |
| A05 | Reset durante áudio | Máquina reinicia e áudio acompanha o novo estado |
| A06 | Stop e novo start | Stream retoma corretamente; sem áudio preso |
| A07 | Trocar para outra ROM SMS | Novo conteúdo sonoro; sem vazamento audível da ROM anterior |
| A08 | Rodar por 15–30 minutos | Sem degradação, underruns audíveis ou falhas |
| A09 | Fechar o emulador normalmente | Processo encerra sem crash ou dispositivo preso |

Não desconecte o dispositivo durante a primeira campanha. Hotplug/reconexão automática não está implementada e deve ser uma campanha separada, com o resultado esperado documentado como limitação conhecida.

## 5. Evidências a coletar

- SHA da `main` testada e SHA da branch CPAL.
- Modelo/nome do dispositivo de saída selecionado no Windows.
- Formato configurado no painel do Windows e sample rate/canais informados pelo log CPAL.
- ROM utilizada (nome e hash opcional; não anexar ROM protegida por direitos autorais).
- Resultado A01–A09, duração e observações de estalos, silêncio ou atraso.
- Arquivo `artifacts/cpal-physical-test.log`.
- Se possível, gravação curta de loopback do áudio do Windows para comparar pause/reset e identificar silêncio/estalos.

## 6. Limitações e interpretação

- A fila de aplicação tem capacidade de 4096 frames neste host. Isso não é o tamanho do buffer do driver; CPAL deixa o endpoint negociar seu buffer.
- CPAL oferece pause/play portável, mas não um flush genérico de amostras já entregues ao driver. Pode haver pequena cauda residual ao pausar/resetar.
- O callback CPAL consome PCM pronto e preenche underruns com silêncio; não executa o Scheduler nem hardware emulado.
- Esta campanha manual verifica a saída audível e o lifecycle. Não prova, sozinha, latência round-trip, ausência absoluta de underruns ou equivalência determinística do runtime.
- R3 só poderá ser declarada concluída depois de combinar CI, evidência física, telemetria/underruns e testes de lifecycle.

## Resultado

**Estado inicial:** NOT RUN — aguardando execução no Windows x64 do usuário.
