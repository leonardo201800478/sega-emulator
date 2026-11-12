# Plano Modern Hi-Fi — SMS e SG-1000

**Status:** planejamento ativo; primeira auditoria estática concluída.
**Branch de trabalho:** `feat/modern-hifi-audio-foundation-20261112`
**Baseline de main:** `1f98f4caebc8bd617e3933a50aeb669d58800582` (2026-11-12).
**Baseline sonora anterior de comparação:** `f47f71830344b85f72329376505d400bfbfaae69`.
**Escopo inicial:** SN76489, pipeline PCM, saída nativa e preparação para CPAL 0.18.2.
**Não autoriza merge:** toda implementação será submetida a PR e validada antes de qualquer integração.

## 1. Diretriz do produto

O emulador deve representar o SG-1000/Master System como se o hardware original tivesse recebido um upgrade de saída de áudio Hi-Fi estéreo.

A intenção não é simular a televisão CRT, a transmissão RF, o cabo RF ou as limitações de áudio mono do equipamento histórico. Devemos preservar a identidade sonora e o comportamento digital dos dispositivos Sega, mas reconstruir e transportar o áudio com qualidade moderna.

### Regras normativas

1. O comportamento digital do SN76489 e os perfis SMS/SG-1000 permanecem distintos e não podem ser alterados apenas para produzir uma preferência sonora.
2. O MachineScheduler continua sendo a única autoridade temporal. A taxa de amostragem, o dispositivo físico e a fila de áudio nunca avançam ou corrigem clocks de hardware.
3. O alvo preferencial de produção é **48.000 Hz, estéreo, 24 bits**. A configuração efetiva será negociada com o dispositivo; nunca forçar formato não suportado.
4. A precisão interna e o processamento DSP devem permanecer em ponto flutuante de alta precisão ou representação equivalente. A redução para 24 bits ocorre somente na fronteira final de saída.
5. Para SMS e SG-1000, a fonte PSG permanece mono e é apresentada como dual-mono L/R. Não inventar separação estéreo, reverberação, chorus, widening ou canal LFE.
6. Sistemas 2.1 recebem o sinal estéreo normal; crossover e bass management pertencem ao sistema operacional, receiver ou amplificador.
7. Filtros de reconstrução/anti-aliasing não devem ser confundidos com modelos analógicos da placa. Nenhum filtro tonal arbitrário ou reforço de graves será ativado por padrão.
8. A qualidade auditiva, medições objetivas e determinismo são critérios de aceite. Mais fidelidade teórica não justifica regressão sonora.

## 2. Arquitetura-alvo

```text
MachineScheduler
    │ ClockAdvance — tempo de hardware inalterado
    ▼
SN76489 / perfis SMS e SG-1000
    │ fonte digital mono, amostras determinísticas
    ▼
Audio source / mixer (f64 ou precisão equivalente)
    │ linha temporal de áudio de referência: 48 kHz
    ▼
DSP e resampler de saída
    │ sem PCM16 intermediário
    ▼
PCM final / conversão de formato
    │ estéreo, I24 preferencial; F32 como alternativa negociada
    ▼
CPAL 0.18.2
    │ stream e formato realmente suportados
    ▼
Sistema de áudio / receiver / caixas / subwoofer
```

O perfil de saída é independente do clock do PSG. Uma troca de dispositivo pode reconstruir o resampler e a sessão de host, mas não pode modificar o estado digital do chip nem a linha temporal do Scheduler.

## 3. Auditoria inicial da main

### 3.1 Estado observado

| Área | Estado observado em 1f98f4c | Implicação |
|---|---|---|
| SN76489 | Core compartilhado com perfis SMS/SG-1000, FIR e amostragem fracionária | Preservar semântica; medir antes de trocar algoritmo |
| Fonte temporal SMS | `DEFAULT_AUDIO_SAMPLE_RATE = 44_100` em `native/sms_core/src/temporal_bus.rs` | Fonte ainda não está padronizada em 48 kHz |
| Worker SMS | `SMS_TEMPORAL_SAMPLE_RATE = 44_100` | Pacing de áudio usa 44,1 kHz |
| Mixer | Mono, precisão `f64`, ganho sem clipping | Bom ponto de partida; preservar headroom |
| PcmResampler | Entrada fixa em 44,1 kHz; FIR polifásico de 63 taps e 1024 fases | Generalizar taxa de entrada e preservar continuidade entre blocos |
| Quantização | `pcm16()` serializa PCM16 antes do sink | Gargalo real para uma saída 24-bit |
| Sessão | `PcmSink::write_pcm(Vec<u8>)` recebe bytes PCM16 | Contrato precisa deixar de pressupor PCM16 |
| Saída legada | `native/audio_output` expande mono PCM16 para frames estéreo | Dual-mono existe, mas ainda em PCM16 |
| CPAL | PR #770 aberta, draft, 55 commits, separada da main | Coordenar contrato PCM; não assumir que a PR já integra Hi-Fi 24-bit |

### 3.2 Interpolação do PSG: comparação com o baseline

A main atual usa interpolação fracionária entre duas saídas FIR. O baseline `f47f718` amostrava a saída filtrada no tick em que a fase de saída cruzava a fronteira.

A auditoria não encontrou prova de que a condição temporária zero nos ticks sem emissão cause defeito: nesses ticks o valor não é consumido; no tick de emissão, a saída filtrada anterior é recalculada antes de inserir a amostra atual no FIR.

**Conclusão:** a mudança de algoritmo é real; regressão sonora ainda não está demonstrada. Não alterar nem reverter esse caminho sem um teste A/B PCM reproduzível e métricas comparáveis.

## 4. Plano de execução e gates

### H0 — Inventário e baseline arquitetural

**Status: CONCLUÍDO (auditoria estática).**

- [x] Confirmar SHA da main e PR #770.
- [x] Identificar taxas de amostragem atuais.
- [x] Identificar o ponto de quantização PCM16.
- [x] Confirmar mono PSG e duplicação dual-mono na saída.
- [x] Comparar o caminho de amostragem atual com o baseline anterior.
- [ ] Executar testes locais/workflow no branch de trabalho.

### H1 — Referência sonora reproduzível

**Status: PRÓXIMA ETAPA.**

1. Criar gerador de traces de registradores e clocks, sem ROM comercial.
2. Gerar PCM determinístico para SMS e SG-1000, incluindo tom isolado, acordes de três tons, ruído periódico/branco, mudanças de volume e escrita de registradores durante execução.
3. Capturar o mesmo trace no baseline `f47f718` e na implementação atual.
4. Registrar WAV/PCM bruto, taxa, perfil, clock, sequência de writes e SHA do código.
5. Comparar RMS, pico, DC offset, frequência, resposta espectral, aliasing, transientes, ruído, clipping e erro normalizado.
6. Testar execução contínua versus blocos arbitrários e verificar igualdade bit a bit do estado e da sequência produzida.

**Gate H1:** fixtures reproduzíveis e relatório A/B. Nenhuma troca de algoritmo antes desse gate.

### H2 — Fechamento da síntese e reconstrução

1. Auditar divisor, fase de amostragem, FIR, resposta de frequência e atraso de grupo.
2. Validar a interpolação fracionária com o core real, não apenas com um sinal sintético auxiliar.
3. Medir transientes de escrita de tom/volume/ruído e os limites de período 0/1.
4. Comparar, com os mesmos traces, o FIR atual com um protótipo independente de síntese por eventos/Blip Buffer.
5. Usar Gearsystem e ARES como referências comparativas, não como oráculos absolutos.
6. Revisar licenças antes de qualquer reutilização de código; preferir implementação independente da arquitetura estudada.

**Gate H2:** resultado objetivo melhor ou equivalente ao baseline, sem mudança não fundamentada da semântica do PSG.

### H3 — Linha de áudio de referência em 48 kHz

1. Definir 48 kHz como taxa interna de produção de áudio, independente da taxa física do endpoint.
2. Generalizar o `PcmResampler` para taxa de entrada explícita, sem constante global implícita de 44,1 kHz.
3. Preservar determinismo, contagem de frames, flush e invariância por tamanho de bloco.
4. Evitar resampling redundante quando a taxa de saída negociada também for 48 kHz.
5. Manter `f64` até a fronteira final de conversão.

**Gate H3:** 48 kHz end-to-end em testes headless; equivalência entre blocos; sem alteração de clocks do Scheduler.

### H4 — Contrato PCM moderno e saída 24-bit

1. Substituir o contrato implícito PCM16 de `PcmSink` por frames/amostras com formato explícito, sem alocações no callback físico.
2. Manter PCM em precisão elevada até a saída; aplicar volume e saturação uma única vez na fronteira correta.
3. Negociar formato com o dispositivo: I24 quando suportado; F32 como alternativa de alta precisão; fallback documentado quando necessário.
4. Confirmar a semântica de 24 bits de CPAL: `I24/U24` usam armazenamento de 4 bytes, com 24 bits significativos.
5. Não assumir que todo endpoint suporta I24 ou 48 kHz.
6. Atualizar contadores/telemetria para refletir taxa, canais, formato e bits efetivamente negociados.

**Gate H4:** testes de conversão, saturação, silêncio, dual-mono, fallback e callback sem locks/alocação; CI Windows x64.

### H5 — Integração CPAL e experiência de saída

1. Coordenar com a PR #770 sem misturar a migração do backend com mudanças do núcleo SN76489.
2. Manter callback CPAL limitado à leitura da FIFO e conversão/cópia previamente preparada.
3. Validar pause/stop/reset/load/epoch, underrun, overflow, encerramento e reconexão.
4. Mostrar formato realmente negociado; não exibir “24-bit” se o endpoint estiver em F32 ou 16 bits sem explicar a representação.
5. Manter saída estéreo dual-mono para PSG SMS/SG-1000.

**Gate H5:** CI aplicável verde e testes físicos documentados. Nenhum merge automático.

### H6 — Validação física e aceite auditivo

- Testes prolongados em Windows x64 com saída 48 kHz estéreo.
- Testar endpoints I24 e F32 quando disponíveis, e fallback para 44,1 kHz.
- Medir underruns, drops, latência de fila e estabilidade; não confundir ocupação FIFO com latência ponta a ponta.
- Ouvir os mesmos traces/ROMs de referência no emulador, Gearsystem e ARES, com volume e cadeia de áudio equalizados.
- Confirmar ausência de clipping, ruído digital, estalos, canais desequilibrados e perda de transientes.
- Validar em caixas estéreo e sistema 2.1 com bass management externo.

**Gate H6:** relatório com dados, hardware, configurações, WAVs e resultados; sem alegar teste físico que não tenha sido executado.

## 5. Critérios de aceite globais

- [ ] Identidade sonora SMS e SG-1000 preservada.
- [ ] 48 kHz estéreo como perfil preferencial.
- [ ] 24 bits efetivos quando I24 estiver disponível; fallback transparente.
- [ ] Nenhuma redução para PCM16 antes da fronteira final.
- [ ] Nenhum LFE artificial ou processamento tonal obrigatório.
- [ ] Scheduler, CPU, VDP e barramentos independentes da saída física.
- [ ] Execução contínua e segmentada determinísticas.
- [ ] Sem clipping, underruns ou drops não contabilizados.
- [ ] Testes headless, CI e testes físicos classificados separadamente.

## 6. Riscos e decisões em aberto

| Risco/decisão | Tratamento |
|---|---|
| Mudança do source rate 44,1 → 48 kHz altera WAVs e save-state de áudio | Fazer como mudança explícita de versão/configuração e testar snapshots |
| PR #770 modifica `audio_output` e `session.rs` | Não editar os mesmos arquivos em branches paralelas sem coordenar o contrato |
| I24 não suportado em todos os drivers | Negociação por capacidades e fallback explícito |
| F32 não é PCM inteiro de 24 bits | Reportar o formato real; não rotular F32 como I24 |
| Blip Buffer pode mudar a resposta transitória | Comparação A/B antes de adoção |
| 24 bits não corrigem uma síntese ruim | Corrigir primeiro fonte, filtros e resampling |

## 7. Referências técnicas

- [CPAL 0.18.2 — documentação](https://docs.rs/cpal/0.18.2/cpal/)
- [CPAL — formatos I24/U24 e armazenamento](https://docs.rs/cpal/latest/src/cpal/sample_format.rs.html)
- [Gearsystem — Sms_Apu](https://github.com/drhelius/Gearsystem/blob/1be9a12b82cae84ddd6f9b7006d546abe2d58bd4/src/audio/Sms_Apu.cpp)
- [Gearsystem — Blip_Buffer](https://github.com/drhelius/Gearsystem/blob/1be9a12b82cae84ddd6f9b7006d546abe2d58bd4/src/audio/Blip_Buffer.h)
- [SMS Power](https://www.smspower.org/)

---
**Estado desta revisão:** planejamento e auditoria estática documentados. Não há alteração de algoritmo de áudio nesta etapa. Testes locais e validação física permanecem pendentes.
