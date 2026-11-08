# Auditoria e registro de investigação — qualidade sonora SMS

**Data do registro:** 2026-11-08
**Repositório:** https://github.com/leonardo201800478/sega-emulator
**Branch-base auditada:** `main`
**SHA da main usado como base documental:** `ece2d634851313d9066e5d87d4b3235a9b8fcd9b`
**PR de observabilidade relacionada:** [#657](https://github.com/leonardo201800478/sega-emulator/pull/657)
**Issues relacionadas:** [#640](https://github.com/leonardo201800478/sega-emulator/issues/640), [#659](https://github.com/leonardo201800478/sega-emulator/issues/659)

> Este documento registra fatos observados, ações realizadas e hipóteses de investigação. Não representa correção do áudio nem comprovação da causa raiz.

## 1. Resumo executivo

Foram executadas rodadas manuais de gameplay do Sega Master System utilizando uma build com a telemetria PCM da PR #657. A telemetria tornou possível observar underruns e ocupação da FIFO, mas os testes mostraram que a qualidade sonora permanece ruim mesmo quando os underruns são inexistentes ou pouco frequentes.

Sintomas relatados pelo usuário:
- áudio fora de ritmo/descompassado;
- distorção audível;
- percepção de áudio ligeiramente lento;
- gameplay/velocidade visual aparentemente normal;
- qualidade ruim semelhante em WASAPI Shared e XAudio2.

A evidência indica que underrun da FIFO, isoladamente, não explica todos os sintomas. A cadeia PCM compartilhada pelos backends precisa ser investigada. A causa raiz permanece **não determinada**.

## 2. Estado de referência do repositório

Na consulta de 2026-11-08, o commit mais recente da `main` era:

- SHA: `ece2d634851313d9066e5d87d4b3235a9b8fcd9b`
- Commit: `fix(audio): discard resampler tail after output epoch flush (#658)`

O SHA acima é a base desta documentação, não uma afirmação de que a main permanecerá nesse estado após a data do registro.

### PR #657 — telemetria PCM OUT-F

A PR #657 foi disponibilizada para teste manual. Sua finalidade é expor métricas da FIFO PCM e dos sinks WASAPI/XAudio2. A implementação descrita na PR:
- mede ocupação/capacidade e high-water da FIFO;
- contabiliza frames aceitos, consumidos e descartados;
- contabiliza underrun em eventos e frames;
- propaga snapshots ao worker SMS e à GUI;
- estima backlog da FIFO, sem afirmar latência ponta a ponta.

A PR não altera PSG, resampler, clocks, MachineScheduler, prebuffer ou parâmetros de buffers. Portanto, trata-se de observabilidade, não de correção automática do áudio.

Na auditoria anterior da PR, o HEAD identificado era `3289904ffb16c367b1fe5b7a8d1e3d8386ec7a7b`. Os checks reportados naquela consulta incluíam sete jobs PASS e cinco SKIPPED (validação SDL3, Z80, PowerShell, stress determinístico do Scheduler e refresh manual de lockfiles). Esses resultados são históricos e devem ser reconsultados antes de qualquer decisão; SKIPPED não equivale a PASS.

## 3. Testes manuais reportados

### 3.1 Primeira captura de telemetria — WASAPI Shared

A captura de tela fornecida pelo usuário mostrou:

| Métrica | Valor visível |
|---|---:|
| FIFO ocupação/capacidade | 735 / 4096 frames |
| High-water | 2972 frames |
| Underrun events | 1006 |
| Underrun frames | 1.412.144 |
| Frames descartados | 0 |
| Blocos descartados | 0 |
| Frames publicados | 12.481 |

A interface também exibia aproximadamente 8.609 frames de jogo e 48.295.774 instruções Z80. Esses valores são apenas uma fotografia instantânea da execução; não permitem inferir taxa efetiva de áudio nem latência ponta a ponta.

### 3.2 Rodada posterior — comparação dos backends

O usuário relatou nova rodada com:
- **XAudio2:** nenhum underrun observado.
- **WASAPI Shared:** poucos underruns observados.
- **Ambos:** qualidade sonora ruim, descompassada e distorcida, com percepção de reprodução ligeiramente lenta.
- **Gameplay:** parecia ocorrer em velocidade normal.

Não foram fornecidos nesta rodada contadores numéricos completos, duração padronizada, captura de loopback, waveform ou medição independente da taxa efetiva de amostras. Portanto, a comparação é evidência manual qualitativa, não um benchmark quantitativo controlado.

## 4. Ações realizadas no GitHub

1. A PR [#657](https://github.com/leonardo201800478/sega-emulator/pull/657) recebeu um comentário com os resultados da nova rodada, distinguindo underruns de qualidade sonora.
2. Foi aberta a issue [#659](https://github.com/leonardo201800478/sega-emulator/issues/659), intitulada *Investigate SMS audio quality: timing drift, distortion and perceived slow playback across WASAPI and XAudio2*.
3. A issue #659 registra os sintomas, limitações da evidência, hipóteses, critérios de diagnóstico e restrições contra ajustes especulativos.
4. A issue #659 foi relacionada explicitamente à PR #657 e à issue [#640](https://github.com/leonardo201800478/sega-emulator/issues/640).
5. Foi preparado um prompt de continuidade para uma sessão dedicada à investigação e auditoria da cadeia completa de áudio.

IDs e estado das interações devem ser reconsultados no GitHub se forem necessários para auditorias posteriores. A documentação não declara que a PR #657 foi mergeada nem que a issue #659 foi resolvida.

## 5. Interpretação técnica — evidência versus hipótese

### Evidência observada

- Os dois backends apresentam qualidade ruim segundo o relato do usuário.
- O XAudio2 não apresentou underruns na rodada posterior.
- O WASAPI apresentou poucos underruns na rodada posterior.
- A percepção de lentidão foi atribuída ao áudio; a velocidade visual do jogo parecia normal.

### Inferência sustentada, ainda não causa raiz

Underrun da FIFO não é explicação suficiente para a totalidade do defeito, pois sintomas semelhantes persistiram no XAudio2 sem underruns observados e no WASAPI com poucos underruns.

Isso justifica investigar componentes anteriores à FIFO ou compartilhados entre os backends. Não prova que o PSG ou o resampler estejam defeituosos.

### Hipóteses em aberto

- conversão de clock do PSG para taxa de amostragem;
- acumuladores, divisores, arredondamento e deriva;
- geração e cadência de amostras;
- continuidade de fase e estado persistente do resampler;
- tamanho e intervalo dos blocos PCM;
- sincronização entre avanço da máquina, frames e produção de áudio;
- formato PCM, canais, signedness, amplitude, clipping e conversões;
- consumo da FIFO e temporização específica dos hosts;
- taxa nominal versus taxa efetiva de produção e consumo.

Nenhuma hipótese acima deve ser apresentada como causa confirmada antes de teste reproduzível.

## 6. Plano de investigação

### Fase A — auditoria do caminho de dados

Mapear a execução real, do PSG até o dispositivo:

1. clocks da máquina e do PSG;
2. atualização dos canais e geração do sinal;
3. conversão para amostras;
4. resampler e estado entre blocos;
5. montagem/publicação dos blocos PCM;
6. worker e FIFO;
7. consumo pelo sink;
8. conversões e submissão WASAPI/XAudio2;
9. consumo efetivo do endpoint.

Para cada etapa registrar arquivo, função, tipo/unidade, cadência, estado persistente e limites arquiteturais. Não inferir comportamento apenas pelos nomes das funções.

### Fase B — captura em pontos de fronteira

Avaliar instrumentação temporária ou permanente para registrar:
- PCM imediatamente após o PSG;
- PCM após o resampler;
- PCM antes da FIFO;
- PCM consumido/submetido pelo backend;
- quantidade de amostras por frame e por segundo;
- timestamps monotônicos de produção/publicação/consumo;
- tamanho e intervalo dos blocos;
- descontinuidades, duplicações e perdas;
- min/max, RMS e clipping;
- sample rate nominal e taxa efetiva.

A captura deve permitir comparar a mesma sequência PCM nos dois backends, sem alterar a semântica temporal do hardware.

### Fase C — testes determinísticos

Criar testes para:
- taxa efetiva e conversão de clocks;
- invariância por chunking;
- continuidade de fase do resampler entre blocos;
- comportamento de flush/epoch;
- ausência de perda ou duplicação indevida;
- conversões de formato/canais;
- regressão dos caminhos PCM.

### Fase D — validação física

Reproduzir com mesma ROM legalmente utilizável, região, trecho, configuração e duração. Comparar WASAPI e XAudio2 e registrar:
- captura de loopback;
- métricas numéricas;
- comportamento auditivo;
- sincronismo com gameplay;
- ambiente, dispositivo e taxa configurada.

Testes unitários, CI e captura física são evidências diferentes e devem ser classificadas separadamente.

## 7. Restrições de engenharia

- Não aumentar buffers ou prebuffer por tentativa e erro.
- Não alterar clocks, PSG ou MachineScheduler sem evidência.
- Não mascarar pitch, velocidade, latência ou descompasso.
- Não introduzir dependência do renderer no Scheduler.
- Preservar determinismo, equivalência por chunking e save states.
- Preservar o caminho SG-1000.
- Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.
- Qualquer correção deve ser pequena, testável e submetida por branch/PR.

## 8. Critérios de encerramento

A investigação somente será considerada concluída quando:
1. houver causa raiz apoiada por evidência;
2. existir teste reproduzindo o defeito antes da correção;
3. a taxa efetiva estiver coerente com a taxa esperada;
4. não houver deriva temporal indevida;
5. continuidade do resampler estiver validada;
6. não houver perda/duplicação indevida de amostras;
7. a mesma sequência PCM for comparada nos dois backends;
8. testes automatizados e regressões estiverem classificados por suíte;
9. gameplay físico confirmar a melhora;
10. sincronismo determinístico e compatibilidade SG-1000 forem preservados.

## 9. Estado ao final deste registro

| Item | Estado |
|---|---|
| PR #657 — telemetria | Disponibilizada para teste; estado remoto deve ser revalidado |
| Comentário com nova evidência | Publicado na PR #657 |
| Issue #640 — underruns WASAPI | Aberta; causa não confirmada |
| Issue #659 — qualidade sonora | Aberta; investigação pendente |
| Teste manual WASAPI | Poucos underruns na rodada posterior; áudio ruim |
| Teste manual XAudio2 | Nenhum underrun observado na rodada posterior; áudio ruim |
| Causa raiz | NÃO DETERMINADA |
| Correção do áudio | NÃO IMPLEMENTADA nesta investigação |
| Validação automatizada da correção | NOT RUN |
| Validação física pós-correção | NOT RUN |

## 10. Próxima ação obrigatória

Começar pela auditoria do HEAD atual da main e das PRs/issues relacionadas. Em seguida, rastrear e medir a cadeia PCM compartilhada, priorizando determinar se a distorção/descompasso já existe antes da FIFO.

Este documento é um registro de investigação e evidência manual, não uma declaração de correção ou conclusão funcional.
