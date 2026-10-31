# Migração do runtime de áudio nativo

A arquitetura normativa e a matriz de hardware estão em [Runtime de áudio nativo](../native-audio-runtime.md).

## Decisão aprovada

- WASAPI Shared/event-driven é o backend padrão no Windows.
- XAudio2 é o backend alternativo existente.
- WASAPI Shared tenta IAudioClient3 para negociar a periodicidade suportada; fallback para Shared convencional. WASAPI Exclusive permanece experimento posterior.
- Rust é proprietário de todo o runtime e do fluxo PCM; Python/PySide6 fica restrito a configuração, controle e telemetria.
- O escopo arquitetural contempla SG-1000, SMS/GG, Mega Drive, Sega CD e 32X. QSound é extensão DSP opcional, não componente obrigatório desses consoles.

## Plano de execução

1. [x] Inventário inicial de código, crates, fachadas PyO3, frontend e workflows.
2. [x] Decisão de backend e contrato arquitetural documentados.
3. [x] Criar sessão de áudio Rust que possua mixer DSP multi-fonte, resampler e backend (primeira fatia integrada à fonte PSG atual).
4. [x] Integrar sessão aos runtimes SG-1000 e SMS; o caminho Qt ativo não transporta PCM por PyO3.
5. [x] Integrar IAudioClient3 ao WASAPI Shared, com alinhamento min/max/fundamental e fallback convencional.
6. [x] Adicionar mixer DSP compartilhado em Rust e filtro passa-baixa opcional, desativado até calibração por chip.
7. [x] Remover APIs PyO3 antigas de drenagem/conversão e fachadas Python de PCM/saída.
8. [ ] Desacoplar a produção de áudio do timer Qt sem criar uma segunda execução da máquina.
9. [ ] Expor telemetria do período efetivo IAudioClient3 e validar fallback em endpoints sem suporte.
10. [ ] Expandir o mixer para fontes estéreo nativas e filtros por chip somente com referência técnica.
11. [ ] Validar Rust, Maturin, CI Windows, integração e hardware físico.

## Critérios de conclusão

- Nenhum PCM/amostra cruza Python/PyO3 durante gameplay.
- Nenhum processamento de áudio ou escrita no backend ocorre em Python.
- WASAPI Shared é o default e XAudio2 continua selecionável.
- Testes cobrem resampling, mixagem, filas, overflow/underrun, encerramento, falha e recuperação.
- SG-1000 e SMS preservam comportamento determinístico e gameplay.
- Latência, underruns e estabilidade são medidos com baseline equivalente; testes físicos são registrados separadamente.

## Estado

**IMPLEMENTED (sessão DSP Rust independente de PyO3 + remoção da API Python PCM + testes headless) / INTEGRATION_PENDING (scheduler independente do timer Qt e validação física)**

A sessão Rust independente de PyO3 está integrada aos adaptadores SG-1000/SMS; o Qt não transporta amostras nem PCM, e as APIs Python de drenagem/conversão foram removidas. Os resultados do candidato aad95bd26b7e90a38b250ef865fafe51f2aa1c8f são históricos e não se transferem para v0.1.1. As APIs legadas de PCM/PyO3 foram removidas; o pump ainda é chamado após run_frame(). QUICK_PASS=NOT RUN; SHORT_PASS=INTEGRATION_PENDING; LONG_PASS=NOT RUN; TEMPORAL_PASS=NOT RUN. Testes físicos WASAPI/XAudio2 ainda estão pendentes.


A negociação IAudioClient3 e as primitivas DSP foram incorporadas na PR #299; esta atualização moveu a sessão DSP para um crate Rust independente e removeu a fronteira Python de PCM. O mixer já está no caminho ativo da sessão Rust para a fonte PSG atual; ainda não há fontes FM/PCM/CDDA/PWM implementadas nem filtros analógicos habilitados. Latência ponta a ponta e compatibilidade real do IAudioClient3 permanecem pendentes de validação em hardware Windows.


## Validação da atualização v0.1.1

Os resultados citados acima são históricos e pertencem aos SHAs anteriores. Para a branch feat/rust-owned-audio-runtime:

- QUICK_PASS: NOT RUN — aguardando workflows no SHA candidato.
- SHORT_PASS: INTEGRATION_PENDING.
- LONG_PASS: NOT RUN.
- TEMPORAL_PASS: NOT RUN.
- Testes físicos WASAPI/XAudio2 e latência ponta a ponta: NOT RUN.
