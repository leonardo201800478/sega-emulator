# Runtime de áudio nativo — arquitetura e migração

- **Decisão:** WASAPI Shared é o backend padrão no Windows; XAudio2 é a alternativa nativa.
- **Estado da migração:** em andamento. A documentação não implica implementação concluída.
- **Escopo:** SG-1000 → Master System/Game Gear → Mega Drive/Genesis → Sega CD → 32X. QSound é uma extensão DSP opcional, não um chip nativo desses consoles.
- **Princípio:** todo o runtime de áudio e a comunicação com o dispositivo do Windows pertencem ao Rust. Python/PySide6 é somente frontend de configuração, controle e diagnóstico.

## 1. Decisões adotadas

1. **WASAPI Shared/event-driven:** backend padrão do Windows, preservando coexistência com outras aplicações e o mixer do sistema.
2. **XAudio2:** backend alternativo nativo, mantido porque já existe no projeto; não é um segundo mixer nem um caminho simultâneo.
3. **WASAPI Shared + IAudioClient3:** caminho preferencial quando a interface e o formato forem aceitos. Consultar períodos suportados, limitar o período solicitado ao intervalo do endpoint e alinhá-lo ao período fundamental. Se a interface ou inicialização falhar, usar WASAPI Shared/event-driven convencional com conversão automática. IAudioClient3 não implica modo exclusivo nem garante latência ponta a ponta menor. WASAPI Exclusive continua fora do caminho padrão.
4. **Rust é dono do ciclo de vida:** abertura, worker, fila, resampling, mixagem, recuperação de dispositivo, fallback, métricas e encerramento.
5. **Python não transporta PCM:** nenhum bytes, vetor de amostras ou buffer PCM atravessa PyO3 no caminho ativo. PyO3 expõe comandos coarse-grained de configuração/ciclo de vida e snapshots/telemetria.
6. **O scheduler do hardware é a autoridade temporal:** clocks e eventos dos chips não dependem da taxa de saída, do timer Qt, do período WASAPI ou da frequência de apresentação de vídeo.
7. **Não adicionar CPAL, miniaudio, Rodio ou Kira ao caminho principal:** não há benefício demonstrado que justifique uma terceira abstração de saída ou um runtime paralelo. SDL3 não é backend de áudio selecionado nesta decisão.

## 2. Escopo de hardware por marco

A matriz indica o escopo arquitetural previsto, não afirma que todos os dispositivos já estejam implementados ou validados.

| Plataforma | Dispositivos/fontes a acomodar | Observações de integração |
|---|---|---|
| SG-1000 | PSG da família SN76489 | Saída mono conforme perfil/revisão documentado; preservar estado e clocks do PSG. |
| Master System | PSG SN76489A; FM YM2413 nas variantes compatíveis | FM é opcional por perfil de hardware/expansão. Não presumir YM2413 em todo SMS. |
| Game Gear | PSG SN76489A | Roteamento estéreo específico do Game Gear pertence ao dispositivo/perfil, não ao balance do host. |
| Mega Drive/Genesis | PSG; YM2612 ou YM3438 conforme revisão; DAC/PCM pelo canal 6 | Preservar diferenças de chip/revisão, mistura e roteamento estéreo. |
| Sega CD | Áudio herdado do Mega Drive; PCM RF5C164; CDDA | Sincronizar PCM/CDDA com a timeline do sistema e eventos do CD. |
| 32X | Áudio herdado do Mega Drive; PWM estéreo | Modelar FIFO, registradores, clock e interação com SH-2 conforme evidência técnica. |
| Extensão opcional | DSP/QSound e fontes PCM multicanal | QSound não é chip interno obrigatório desses consoles. Tratar como perfil/módulo independente para eventual escopo arcade ou pesquisa. |

A matriz não autoriza antecipar implementação de FM, PCM, CDDA, PWM ou QSound antes da consolidação do runtime e da auditoria de cada dispositivo. Cada chip exige especificação própria, revisão/clock correto, vetores determinísticos e oráculo independente.

## 3. Camadas e contratos

### 3.1 Dispositivos emulados

Cada dispositivo é parte do hardware da máquina e permanece independente do Windows e da UI. Sua interface representa registradores, eventos, clock, estado interno e contribuições de áudio. O dispositivo não abre streams nem conhece WASAPI/XAudio2.

- PSG: divisores, tons, ruído, atenuação e particularidades por console/revisão.
- FM: operadores, envelopes, temporização, tabelas e comportamento dos YM2413/YM2612/YM3438.
- PCM/DAC: canais, endereçamento, reprodução, volume, pan e limites de cada dispositivo.
- CDDA: transporte, setores/frames de áudio e sincronização com a emulação do CD.
- PWM: FIFO, registradores, frequência e comportamento de underrun próprios do 32X.
- DSP/QSound: processamento especializado, espacialização e PCM multicanal quando houver perfil de hardware aplicável.

### 3.2 Timeline e scheduler

O scheduler Rust é a autoridade dos ticks emulados. Escritas em registradores, eventos de DMA/FIFO e mudanças de estado devem ser aplicados em ordem temporal definida. A saída consome os resultados sem avançar ou corrigir clocks do hardware.

A arquitetura evolui para timestamps/ticks explícitos e ordenação determinística entre CPU, barramento, dispositivos e áudio. A campanha de temporização ciclo a ciclo pode ser incremental, mas amostras geradas por frame não equivalem a precisão ciclo a ciclo.

### 3.3 Mixer e formato interno

O módulo native/audio_runtime/src/dsp.rs fornece mixer DSP Rust independente do host: soma de múltiplas fontes mono, ganho por fonte, ganho mestre e buffer reutilizável. A soma preserva headroom; saturação/quantização permanece no resampler PCM. Há também filtro passa-baixa de primeira ordem opcional, desativado por padrão. Seus coeficientes não representam automaticamente o circuito de nenhum console: cada resposta analógica exige evidência e calibração específicas.

O mixer comum combina fontes heterogêneas sem apagar suas propriedades:
- canais mono/estéreo e roteamento específico do console;
- ganho por dispositivo/canal e headroom;
- pan do hardware separado do balance global do host;
- soma, clipping/saturação e conversões explicitamente definidas;
- PCM, CDDA e fontes de taxa variável;
- resampling para a taxa solicitada pelo dispositivo.

O contrato interno deve evoluir de blocos mono PCM16 para formato de mixagem capaz de comportar estéreo e múltiplas fontes. Não converter prematuramente cada chip para PCM16 se isso reduzir precisão ou impedir o mixer. A representação (ponto fixo ou flutuante) será decidida com análise de faixa dinâmica, determinismo cross-platform e testes de equivalência; o resampler atual não será substituído sem baseline.

### 3.4 Saída Windows

O crate sega-audio-output é a camada de dispositivo do host. Consome blocos já mixados, converte apenas o necessário para o formato negociado e entrega ao Windows.
- **WASAPI Shared/event-driven + IAudioClient3:** padrão preferencial. Consultar GetSharedModeEnginePeriod, escolher período dentro de min/max e múltiplo do fundamental, então inicializar com InitializeSharedAudioStream. Se interface/consulta/inicialização não estiver disponível, fallback para Initialize via EventsShared com autoconversão. Tamanho solicitado não é latência medida.
- **XAudio2:** alternativa. Ownership dos buffers até callback de conclusão; callbacks mínimos e não bloqueantes. Evitar polling se callbacks confiáveis cobrirem o ciclo de vida.
- **WASAPI Exclusive:** investigação posterior, opt-in, com fallback claro e validação de drivers. IAudioClient3 é usado no Shared quando disponível; fallback mantém compatibilidade.
- **Device loss/hotplug:** worker Rust detecta falhas, reenumera/reabre conforme política e reporta falha persistente sem bloquear a UI.
- **Fila:** limitada, com política explícita de overflow/underrun e métricas separadas.
- **Shutdown:** parar produção, sinalizar worker, liberar buffers após o backend deixar de usá-los e aguardar término sem deadlock.

A saída possui backends Rust WASAPI/XAudio2 e filas limitadas. A sessão DSP/resampler agora reside em native/audio_runtime/src/session.rs, atrás do contrato PcmSink, sem dependência de PyO3 ou APIs do host. As classes PyO3 antigas para resampler, escrita PCM e drenagem de amostras foram removidas. O adaptador Rust da extensão instancia o sink WASAPI/XAudio2 e entrega amostras nativas à sessão; nenhum buffer PCM é exportado para Python.

## 4. Fronteira PyO3 e Qt

Python/PySide6 pode selecionar WASAPI (padrão), XAudio2 ou áudio desativado; configurar opções suportadas; iniciar, parar, pausar e reconfigurar uma sessão; consultar snapshots, backend efetivo, dispositivo, underruns, blocos descartados e erros.

Python/PySide6 não pode drenar amostras de PSG/FM/PCM/CDDA/PWM por frame; converter, mixar, resamplear, balancear ou empacotar PCM; copiar PCM para bytes ou chamar write_pcm por frame; executar callback de áudio ou controlar thread de tempo real.

O primeiro corte pode processar áudio internamente em Rust ao final de cada lote/frame solicitado pelo frontend. O destino é a sessão/scheduler Rust coordenar produção e fila independentemente do timer Qt. Chamada coarse-grained para avançar máquina não equivale a transportar PCM pelo Python.

## 5. Estado inventariado no início da PR #299

Referência: main SHA 2318fb35715c49c26e63f71debadacdb1816c79c; branch inicial migration/native-audio-runtime SHA 705d08e6f6ee5afd3c1c17392db4754586688590.

- native/audio_output: backends WASAPI e XAudio2 em Rust.
- native/audio_runtime: resampler PCM em Rust.
- native/z80_core/src/audio.rs: fachadas PyO3 para resampler e saídas; métodos atuais aceitam/retornam PCM Python e fazem cópia para Vec<u8>.
- native/z80_core/src/machine.rs e sms_machine.rs: expõem drenagem de amostras/PCM e descarte para Python.
- src/sega_emulator/frontend/qt_frontend.py: envia comandos de configuração/ciclo de vida e chama run_frame pelo pacing; não recebe amostras nem PCM.
- native/z80_core/src/audio.rs: adapta os backends WASAPI/XAudio2 ao PcmSink Rust, sem classes PyO3 de transporte PCM.
- As antigas fachadas Python audio_output.py e wasapi_audio.py foram removidas; não há transporte PCM no frontend.

Esse estado é inventário, não aprovação arquitetural. O caminho PCM via Python deve ser removido após transição com testes equivalentes.

## 6. Plano de migração incremental

### Fase A — contrato e documentação
- Aprovar WASAPI Shared padrão e XAudio2 alternativo.
- Consolidar matriz de hardware e contratos independentes dos consoles.
- Atualizar ADRs, arquitetura global, documentação dos crates, drivers e sincronização A/V.

### Fase B — sessão nativa Rust
- Introduzir sessão de áudio Rust que possua mixer DSP multi-fonte, resampler e backend selecionado.
- Integrar IAudioClient3 ao WASAPI Shared com consulta/alinhamento de período e fallback convencional.
- Integrar sessão aos wrappers/máquinas Rust SG-1000 e SMS sem expor buffers PCM.
- Processar amostras dentro de Rust, usando comandos PyO3 apenas para configuração/telemetria.
- Preservar VGM/diagnóstico como fluxos separados; não usar gravação como transporte de áudio.

### Fase C — remoção do caminho Python
- [x] Remover NativePcmResampler, drain_audio_pcm, drain_audio_samples e descarte PCM da API PyO3.
- [x] Remover as fachadas Python de resampling e escrita PCM; testes do pipeline migrados para Rust.
- Reconfiguração de volume/backend deve ocorrer por comandos de sessão Rust, com rollback/fallback definido.

### Fase D — runtime independente da UI
- Desacoplar produção/consumo de áudio do timer de apresentação Qt.
- Definir ownership entre máquina, scheduler, sessão e worker; não criar segunda execução da máquina.
- Tratar pausa, reset, carregamento de ROM, troca de sistema, dispositivo perdido e shutdown.
- Medir latência ponta a ponta e estabilidade com workload repetível.

### Fase E — expansão de dispositivos
- Reutilizar runtime para FM, DAC/PCM, CDDA e PWM à medida que cada console for implementado.
- Acrescentar DSP/QSound apenas em perfil de hardware separado e mediante decisão explícita de escopo.
- Validar cada dispositivo antes de habilitar no caminho de gameplay.

## 7. Critérios de aceitação e evidências

- Nenhum PCM ou amostra de áudio cruza PyO3 durante gameplay.
- Nenhum resampling, mixagem, ganho ou escrita no backend ocorre em Python.
- WASAPI Shared é o default; XAudio2 permanece selecionável como alternativa.
- Falha de WASAPI não inicia silenciosamente outro backend; fallback deve ser explícito e observável.
- Testes Rust cobrem resampler, mixer multi-fonte, ganho, filtro DSP opcional, filas, overflow, underrun e shutdown.
- Testes de integração verificam SG-1000 e SMS, áudio habilitado/desabilitado e reconfiguração.
- CI Windows compila/testa WASAPI e XAudio2; testes físicos documentam Windows, dispositivo, driver, formato/período efetivos e estabilidade.
- Benchmarks comparam baseline/candidato equivalentes, registrando mediana, variação, CPU, underruns e latência.
- Estados de validação separados: IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING, TEMPORAL_PASS.
- Não declarar migração completa enquanto o frontend transportar PCM ou a saída real não for validada em Windows.

## 8. Referências técnicas

- Microsoft WASAPI: https://learn.microsoft.com/windows/win32/coreaudio/wasapi
- Microsoft IAudioClient3: https://learn.microsoft.com/windows/win32/api/audioclient/nn-audioclient-iaudioclient3
- Microsoft XAudio2: https://learn.microsoft.com/windows/win32/xaudio2/xaudio2-introduction
- Microsoft XAudio2 callbacks: https://learn.microsoft.com/windows/win32/xaudio2/xaudio2-callbacks
- Rust wasapi: https://docs.rs/wasapi/
- Rust windows: https://docs.rs/windows/
- Rust rtrb: https://docs.rs/rtrb/
- Versões efetivas das crates e workflows do SHA candidato prevalecem sobre exemplos externos.

## 9. Estado da implementação na atualização v0.1.1

**Candidato:** branch feat/rust-owned-audio-runtime, derivado da integração Qt/áudio; versão de pacote 0.1.1.

### IMPLEMENTED — sessão Rust independente de PyO3

- NativeAudioSession<S: PcmSink> está no crate puro sega-audio-runtime; não importa PyO3, Python ou backend específico.
- Os adaptadores Rust SG-1000/SMS conectam a sessão ao sink WASAPI/XAudio2 nativo.
- O caminho Qt não tem API para drenar amostras/PCM nem para chamar write_pcm; apenas configura, fecha e consulta métricas.
- Ao concluir run_frame, o adaptador Rust drena as amostras internamente e as encaminha à sessão.
- Falha de escrita desativa a sessão e é reportada ao frontend sem interromper a emulação.
- Testes headless Rust validam o pipeline da sessão com sink determinístico e integração de geração PSG dos dois sistemas.
- Reset, troca de ROM e reconfiguração fecham/recriam a sessão.

### Evidência histórica no candidato aad95bd26b7e90a38b250ef865fafe51f2aa1c8f (não transferível)

- QUICK_PASS: PASS — CI Linux, Ruff format/lint, build Maturin, smoke test SMS e 257 testes aprovados (11 ignorados).
- SHORT_PASS: INTEGRATION_PENDING — validação funcional Python PASS (224 aprovados/21 ignorados); integridade/repetibilidade PASS (duas execuções); Qt Frontend Rebuild PASS (66 aprovados/1 ignorado, controladores 10 aprovados, suíte completa 321 aprovados/10 ignorados); UI/Controller PASS.
- Windows Rust integration: IN_PROGRESS no workflow https://github.com/leonardo201800478/sega-emulator/actions/runs/41158473737. Rust formatting, formatação do crate audio_output, Clippy e testes do crate audio_output PASS; Clippy do z80_core está em execução; testes Rust do z80_core, build Maturin Windows e integração ainda pendentes.
- LONG_PASS: NOT RUN.
- TEMPORAL_PASS: NOT RUN; a execução ainda é em lotes/frame e não constitui produção de áudio desacoplada do timer Qt.
- A drenagem PCM, conversão PCM e escrita direta de PCM por PyO3 foram removidas; as fachadas Python audio_output.py e wasapi_audio.py foram removidas. O frontend ainda chama run_frame() pelo pacing Qt; esse acoplamento temporal é uma fase separada e continua pendente.
- WASAPI/XAudio2 em hardware físico, latência ponta a ponta, underruns sob carga e reconexão: INTEGRATION_PENDING.

### Validação do candidato v0.1.1 — feat/rust-owned-audio-runtime

- QUICK_PASS: NOT RUN — aguardando GitHub Actions no SHA candidato exato.
- SHORT_PASS: INTEGRATION_PENDING — build Maturin, integração Qt e regressões de gameplay dependem dos workflows deste candidato.
- LONG_PASS: NOT RUN.
- TEMPORAL_PASS: NOT RUN — o pump ainda é acionado ao final de run_frame() pelo fluxo Qt.
- WASAPI/XAudio2 físico, hotplug, sessões longas e latência ponta a ponta: NOT RUN.

Os resultados históricos acima pertencem somente ao SHA antigo e não aprovam este candidato. Não classificar a migração completa: a autonomia do scheduler frente ao timer Qt e a validação física Windows ainda estão pendentes.
