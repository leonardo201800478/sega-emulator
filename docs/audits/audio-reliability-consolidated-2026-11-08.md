# Auditoria consolidada — confiabilidade de áudio e estado da integração SMS

**Data:** 2026-11-08
**Baseline auditada:** `main` — `9731b37098e7937fa92fffc15007a7c79aac4e4c`
**Escopo:** comparação das auditorias independentes (Claude, Gemini, DeepSeek e Manus), inspeção estática dos caminhos de áudio PCM/WASAPI/XAudio2, worker SMS, UI egui e documentação de roadmap/telemetria.
**Natureza:** revisão documental e estática. Não constitui correção, benchmark, teste de gameplay ou certificação de áudio.

## 1. Decisão executiva

A auditoria consolidada **não aprova o áudio para release**. A arquitetura separa corretamente a máquina temporal, o worker, o transporte PCM e os backends de host, mas há lacunas confirmadas de observabilidade e tratamento de falha. Além disso, a evidência manual mais recente da issue #659 demonstra que a qualidade sonora ruim persiste nos dois backends mesmo quando underruns são raros ou não observados.

A prioridade imediata passa a ser a cadeia compartilhada de geração e transformação PCM, antes de qualquer tuning específico de WASAPI/XAudio2. O problema não pode ser reduzido a underrun: o relato de distorção, descompasso e percepção de áudio lento em ambos os backends indica que a investigação precisa começar antes da FIFO e comparar a mesma sequência PCM em ambos.

**Decisão de planejamento:** congelar novas expansões de plataforma e tarefas não bloqueadoras do SMS enquanto a trilha de confiabilidade de áudio não atingir os gates definidos no scorecard `docs/audio/audio-reliability-scorecard.md`. Isso não suspende correções necessárias de segurança/estabilidade nem autoriza mudanças especulativas em clocks, PSG ou Scheduler.

## 2. Baseline e estado remoto

- `main` aponta para `9731b37098e7937fa92fffc15007a7c79aac4e4c`, merge da PR #664.
- PRs #613 (runtime SMS + viewport D3D12) e #614 (áudio nativo + teclado/SDL3) estão **MERGED**.
- PR #657 (telemetria PCM OUT-F) está **CLOSED, NOT MERGED**. Não tratá-la como origem integrada por si só; os elementos de telemetria observados no código e na documentação devem ser atribuídos aos commits efetivamente presentes na main.
- PR #658 (descarte de cauda do resampler após flush/epoch) está **MERGED**, merge SHA `ece2d634851313d9066e5d87d4b3235a9b8fcd9b`.
- PR #661 (amostragem SN76489 em posições fracionárias do clock de saída) está **MERGED**, merge SHA `af24e77018257b77293a28e2f8e1ac12d0804aa4`.
- PRs #643, #644 e #645 permanecem **OPEN**, com base antiga `f47f71830344b85f72329376505d400bfbfaae69`, `mergeable=false` e sobreposição com arquivos da integração da aplicação. Não fazem parte da baseline; exigem reconciliação independente.
- Issue #640 permanece aberta: underruns WASAPI relatados, causa raiz não confirmada.
- Issue #659 permanece aberta: qualidade ruim/descompasso/distorção em WASAPI e XAudio2; causa raiz não determinada.
- Issue #641 permanece aberta e fora da trilha de áudio: corrupção visual no menu de save, sem localização da primeira fronteira de divergência.

### CI da baseline

Workflow `42484594809`, evento push, SHA exato `9731b37098e7937fa92fffc15007a7c79aac4e4c`, conclusão geral `success`.

| Suíte / job | Resultado |
|---|---|
| Rust quality and unit regression | PASS |
| Documentation patch hygiene | PASS |
| Change impact classification | PASS |
| Rust workspace regression | SKIPPED — NOT RUN |
| Linux audio-output host validation | SKIPPED — NOT RUN |
| Windows WASAPI and XAudio2 validation | SKIPPED — NOT RUN |
| Windows x64 D3D12 compile | SKIPPED — NOT RUN |
| Windows SDL3 input validation | SKIPPED — NOT RUN |
| Scheduler deterministic stress | SKIPPED — NOT RUN |
| Z80 focused validation | SKIPPED — NOT RUN |
| Windows PowerShell tooling syntax | SKIPPED — NOT RUN |
| Refresh Cargo lockfiles (manual artifact) | SKIPPED — NOT RUN |

A conclusão geral `success` significa que o workflow terminou sem falha nos jobs executados; não transforma jobs ignorados em validação aprovada. A auditoria independente também não executou build, testes locais, benchmark, gameplay ou teste físico.

## 3. Método de consolidação das quatro auditorias

As quatro revisões independentes foram confrontadas com inspeção direta dos arquivos e com os documentos versionados. Divergências factuais foram arbitradas pela evidência do código:

- **Gemini:** identificou corretamente o risco de foco/consumo de teclado e a natureza não transacional do snapshot de métricas; apontou duplicidade de paths recentes como melhoria de baixa prioridade.
- **DeepSeek:** levantou corretamente a questão de foco e snapshot; suas afirmações de que a telemetria PCM não existia, que a API de métricas do worker era privada e que não havia teste de histórico recente foram refutadas pela inspeção. O painel Diagnostics exibe parte das métricas, a API do worker é pública e existe teste unitário de move-to-front/limite do histórico. A execução desse teste na CI da baseline, porém, não foi comprovada.
- **Claude:** confirmou o risco de foco, apontou o caminho de abertura de ROM que pode falhar por persistência do diretório antes de carregar o arquivo e identificou a omissão da starvation XAudio2 na métrica comum. Algumas preocupações de bloqueio síncrono e foco da HWND filha permanecem hipóteses que exigem reprodução.
- **Manus:** apresentou o inventário mais amplo de falhas e limitações do áudio, worker e UI. A classificação abaixo preserva a distinção entre condição de código confirmada e efeito físico ainda não reproduzido.

A classificação P1/P2/P3 deste documento é prioridade de engenharia para o plano atual, não uma alegação de severidade universal ou de falha reproduzida em todos os hosts.

## 4. Achados consolidados

“Confirmado” significa que a condição foi identificada no código/documentação inspecionados. Não significa que o sintoma físico foi reproduzido.

| ID | Achado | Prioridade | Estado da evidência | Direção |
|---|---|---:|---|---|
| AUD-01 | Starvation do endpoint XAudio2 não chega ao snapshot comum de underrun | P1 | Código confirmado; sintoma físico não medido | Separar FIFO underflow, endpoint starvation e silêncio entregue; propagar contadores por backend |
| AUD-02 | XAudio2 pode continuar aceitando PCM após falha terminal do worker | P1 | Código confirmado; falha não reproduzida em host | Tornar falha terminal visível e rejeitar escrita após falha/encerramento |
| AUD-03 | Ocupação/telemetria pode ficar obsoleta após flush, pause, stop, reset ou load | P1 | Código confirmado | Atualizar snapshot após transição confirmada e contabilizar flush em contador próprio |
| AUD-04 | Áudio ruim em ambos os backends mesmo com poucos/nenhum underrun | P1 | Relato manual registrado na #659; causa não determinada | Auditar geração, taxa efetiva, resampler, cadência e formato antes da FIFO |
| AUD-05 | Taxa exposta é taxa configurada, não necessariamente taxa efetiva do endpoint | P2 | Código confirmado | Rotular corretamente; expor formato efetivo apenas quando consultável |
| AUD-06 | Métricas FIFO não equivalem a áudio reproduzido nem latência ponta a ponta | P2 | Semântica confirmada | Distinguir frames retirados da FIFO, buffers em voo, frames tocados e latência medida |
| AUD-07 | Falha em SubmitSourceBuffer pode perder PCM já retirado da FIFO | P2 | Caminho de código confirmado; falha não reproduzida | Reportar falha e perda; não re-enfileirar ingenuamente pelo lado consumidor SPSC |
| AUD-08 | Documentação promete campos de telemetria que a UI não mostra integralmente | P2 | Divergência documental confirmada | Alinhar documentação e UI; não ampliar UI sem contrato de semântica |
| AUD-09 | Underrun WASAPI contado por evento em um caminho legado, sem quantificar a falta | P1 | Código e issue #640; números históricos variam por build | Medir frames/silêncio e duração dos episódios por backend |
| AUD-10 | Atalhos/entrada de jogo não verificam explicitamente consumo e foco de teclado | P2 | Código confirmado; interferência empírica não reproduzida | Separar input gameplay, atalhos e gamepad; testar transições de foco |
| AUD-11 | StepFrame pode preservar erro antigo após recuperação bem-sucedida | P2 | Código confirmado | Distinguir budget excedido de falha fatal e limpar erro em sucesso |
| AUD-12 | Falha ao persistir último diretório pode impedir a abertura da ROM selecionada | P2 | Caminho de controle confirmado | Persistência auxiliar não pode bloquear carga válida; testar falha de filesystem |
| AUD-13 | Caminho exibido pode divergir da ROM ativa após tentativa de carga malsucedida | P2 | Código confirmado | Separar path candidato, ROM ativa e erro de tentativa |
| AUD-14 | Alteração de região na UI não reconfigura worker já criado | P3 | Código confirmado | Desabilitar/explicar ou aplicar transição explícita |
| AUD-15 | Histórico recente usa escrita direta e deduplicação textual | P3 | Código confirmado; corrupção não reproduzida | Escrita temporária + rename e normalização segura de paths |
| AUD-16 | Save State do core não está exposto ponta a ponta pelo worker/UI | P3 (fora do gate de áudio) | Lacuna funcional confirmada | Planejar como trilha separada, incluindo epoch/áudio e round-trip |
| AUD-17 | Possível bloqueio síncrono da UI e foco da HWND filha D3D12 | P3 | Hipótese não reproduzida | Não refatorar sem medição/reprodução |
| AUD-18 | Issues #640 e #659 não têm causa raiz demonstrada | P1 | Estado remoto confirmado | Permanecer abertas até evidência e critérios de aceite completos |

### Semântica que não pode ser confundida

1. **FIFO underflow:** consumidor da FIFO não dispõe de frames suficientes para preencher a solicitação; pode inserir silêncio.
2. **XAudio2 endpoint starvation:** voz/buffers submetidos esgotaram; evento não informa sozinho quantos frames de silêncio foram efetivamente ouvidos.
3. **WASAPI padding/period:** estado do buffer do endpoint e período solicitado; não equivale à ocupação da FIFO.
4. **Frames aceitos:** publicados na FIFO.
5. **Frames consumidos:** retirados da FIFO/entregues ao estágio seguinte; não significam reprodução audível.
6. **Frames descartados:** rejeitados por FIFO cheia; perdas por flush, falha de submissão ou término precisam de contadores distintos.
7. **Backlog estimado:** duração equivalente somente da FIFO, não latência de driver, mixer, endpoint ou DAC.
8. **Sample rate configurado:** solicitação/contrato do stream; não prova a taxa física ou efetiva negociada.

Não somar eventos de starvation e frames de underrun, nem inferir frames silenciosos a partir de contagem de eventos. Não usar uma fotografia isolada de atomics independentes como balanço transacional.

## 5. Evidência manual de qualidade sonora — issue #659

A issue #659 registra, após a disponibilização da telemetria PCM:
- XAudio2: nenhum underrun observado na rodada posterior;
- WASAPI Shared: poucos underruns;
- ambos: áudio percebido como distorcido, fora de ritmo e ligeiramente lento;
- gameplay/velocidade visual aparentemente normal.

Não há ainda captura de loopback, waveform, medição independente da taxa efetiva, timestamps correlacionados, ou comparação da mesma sequência PCM nos dois backends. A conclusão suportada é limitada: **underrun da FIFO não explica sozinho os sintomas relatados**. Não se conclui que o PSG, resampler ou Scheduler sejam a causa.

A investigação deve comparar pontos de fronteira: saída do PSG, após conversão/resampler, antes da FIFO e após entrega ao backend. A primeira fronteira em que o sinal divergir será a região de investigação prioritária.

## 6. Plano de execução por etapas

### AUDIO-00 — Congelar baseline e contrato de medição
- Revalidar SHA, PRs, issues, workflows e dependências no início da tarefa.
- Definir unidades, semântica e epoch de cada contador.
- Criar cenário de referência repetível: ROM legal/homebrew, região, trecho, duração, taxa e dispositivo.
- Não alterar clocks, buffers, prebuffer ou taxa por tentativa e erro.

### AUDIO-01 — Rastrear geração e taxa efetiva (prioridade máxima)
- Mapear PSG → mixer/conversão → resampler → blocos PCM → worker → FIFO → backend.
- Registrar taxa nominal, taxa efetiva calculável, frames por frame de emulação, tamanho/intervalo de blocos e continuidade de fase.
- Verificar arredondamento, acumuladores, divisores, deriva, canais, signedness, amplitude e clipping.
- Produzir captura determinística offline nos limites de estágio; comparar a mesma sequência nos dois sinks.
- Critério: identificar a primeira fronteira que diverge, ou demonstrar que os buffers PCM são idênticos até a entrada dos backends.

### AUDIO-02 — Resampler, epoch e invariância
- Testar continuidade de fase entre blocos e após flush/reset/load.
- Testar equivalência entre processamento contínuo e chunking variado.
- Testar taxa efetiva, ausência de duplicação/perda indevida e conversões de formato.
- Não corrigir pitch/velocidade alterando clock do PSG ou do Scheduler.

### AUDIO-03 — Transporte FIFO e backends
- Corrigir contabilidade de FIFO underflow, frames silenciosos, overflow e flush.
- Instrumentar separadamente starvation XAudio2, WASAPI period/padding e buffers em voo quando disponíveis.
- Rejeitar escrita após falha terminal do worker.
- Tratar falha de submissão sem perda silenciosa nem falsa confirmação.
- Manter callback/thread de áudio sem UI, logging bloqueante, alocação desnecessária ou controle do Scheduler.

### AUDIO-04 — Transições e lifecycle do worker
- Validar Pause/Resume/Stop/Reset/Load ROM/Close e StepFrame.
- Definir silêncio físico, invalidação de epoch, flush e prebuffer na retomada.
- Atualizar telemetria após transição confirmada.
- Separar avanço determinístico do estado de áudio da política de apresentação física durante StepFrame.

### AUDIO-05 — Validação física comparativa
- Windows x64 com WASAPI Shared e XAudio2, mesmo dispositivo quando possível.
- Mesma ROM/hash local, região, trecho, volume, configuração e duração.
- Captura de loopback e métricas sincronizadas; comparar forma de onda, taxa efetiva, clipping, gaps, pitch e sincronismo.
- Repetir execução prolongada e transições lifecycle.
- Resultado por backend: PASS / FAIL / BLOCKED / NOT RUN.

## 7. Critérios de aceite para liberar a prioridade seguinte

A trilha de áudio só deixa de bloquear a evolução do SMS quando:
1. causa raiz dos sintomas da #659 for demonstrada ou os limites restantes forem explicitamente aceitos com evidência;
2. taxa e cadência PCM forem coerentes com o relógio esperado, sem deriva não explicada;
3. resampler mantiver fase e equivalência sob chunking e transições de epoch;
4. a mesma sequência PCM for comparada nos dois backends;
5. não houver perda/duplicação silenciosa em FIFO, flush, falha ou submit;
6. falhas terminais dos workers forem observáveis e impedirem novas escritas aceitas;
7. métricas separarem FIFO, endpoint e reprodução efetiva;
8. testes automatizados e gates Rust passarem no SHA candidato;
9. validação física Windows for concluída para cada backend, com evidência reproduzível;
10. SG-1000 e determinismo da máquina permanecerem sem regressão.

## 8. Restrições permanentes

- MachineScheduler continua sendo a autoridade do tempo emulado.
- A thread de áudio consome PCM; não avança nem corrige o Scheduler.
- Não ajustar clocks do PSG, Z80 ou VDP para sincronizar com o endpoint.
- Não usar frame skipping, JIT, hacks temporais ou correções especulativas.
- Não aumentar buffer/prebuffer sem baseline quantitativa e hipótese demonstrável.
- Não transformar a UI em proprietária de estado de hardware.
- Preservar o runtime compartilhado e SG-1000.
- ROMs comerciais, BIOS proprietárias e dumps protegidos nunca entram no Git.

## 9. Estado final desta auditoria

| Área | Classificação |
|---|---|
| Baseline main | CONFIRMADA: `9731b37098e7937fa92fffc15007a7c79aac4e4c` |
| Inspeção estática de áudio/worker/UI | CONCLUÍDA no escopo declarado |
| Causa raiz de qualidade sonora (#659) | NÃO DETERMINADA |
| Causa raiz de underruns (#640) | NÃO DETERMINADA |
| Correção de áudio decorrente desta auditoria | NÃO IMPLEMENTADA |
| Build/testes locais desta auditoria | NOT RUN |
| CI especializado de áudio no SHA da main | SKIPPED / NOT RUN |
| Validação física pós-correção | NOT RUN |
| Aprovação para release de áudio | NÃO APROVADO |

## 10. Fontes internas de evidência

- `docs/audits/sms-audio-quality-investigation-2026-11-08.md`
- `docs/audits/sms-ui-pcm-telemetry-integration-2026-11-08.md`
- `docs/audio/native-audio-output-audit-2026-11-07.md`
- `docs/audio/out-f-output-telemetry.md`
- Issue #640 — underruns WASAPI.
- Issue #659 — qualidade sonora em ambos os backends.
- Issue #641 — corrupção visual, acompanhada separadamente.
- Auditorias independentes Claude, Gemini, DeepSeek e Manus recebidas em 2026-11-08.

## 11. Acompanhamento após a auditoria — PR #666

A primeira rodada de correções está na [PR #666](https://github.com/leonardo201800478/sega-emulator/pull/666), branch `fix/audio-host-failure-telemetry-20261108`, HEAD `29916f7ddb27a069b385a57e53d04760de5e01ed`. A PR permanece aberta e não integrada na main.

Correções implementadas: rejeição de novas escritas após estado terminal do backend; falha de `SubmitSourceBuffer` terminal e com contagem de frames perdidos; starvation XAudio2 separado do underrun FIFO; frames descartados em flush contabilizados; snapshots atualizados após lifecycle; telemetria terminal preservada antes do fechamento da sessão; GUI e contratos OUT-F atualizados.

Validação CI do HEAD: workflow `42489225235` SUCCESS. PASS: Rust workspace regression (fmt/Clippy/testes), Linux audio-output, Windows WASAPI/XAudio2 (fmt/Clippy/testes), formatação/Clippy da aplicação Windows, documentação e classificação de impacto. SKIPPED: Scheduler stress, Z80 focused, SDL3, PowerShell e refresh manual de lockfiles.

**Limitação essencial:** nenhuma validação física, captura loopback ou comparação acústica foi realizada. A causa raiz da issue #659 (distorção/ritmo lento nos dois backends) e da #640 (underruns WASAPI) continua UNKNOWN. As correções de telemetria/falha não equivalem à correção da qualidade sonora. O próximo passo obrigatório é a comparação física controlada WASAPI Shared versus XAudio2.
