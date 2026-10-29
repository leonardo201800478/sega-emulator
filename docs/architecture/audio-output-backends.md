> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust (`native/sg1000_core`). A aplicação Qt usa uma fachada Python fina para PyO3 e carregadores de ROM/perfil. Implementações de hardware Python, ferramentas e testes que dependiam delas foram arquivados sob `legacy/` e não são executados pelo projeto ativo. Descrições de estados anteriores neste documento são históricas, não instruções atuais.

# Contrato de configuração para saída de áudio

> Estado: nesta branch, o frontend Qt usa uma fachada PyO3 para o backend WASAPI compartilhado implementado em Rust via crate wasapi. O fluxo é event-driven e usa uma fila limitada. SDL3 como backend de saída ainda não está implementado. WASAPI e XAudio2 precisam passar CI e validação física de estabilidade, reconexão e latência em Windows antes de serem considerados validados.

## Objetivo

Definir um contrato de configuração validado e independente do dispositivo para a saída de áudio. O backend WASAPI inicial já abre o dispositivo no frontend Qt; a configuração e os testes automatizados não substituem a validação física. Nenhum backend pode alterar o estado determinístico do hardware emulado.

## Integração atual dos núcleos

- **SG-1000:** o frontend Qt usa `SG1000Machine.drain_audio_pcm()` e encaminha PCM16 para o backend nativo WASAPI/XAudio2. Quando a saída está desativada ou falha, as amostras pendentes são descartadas sem alterar T-states.
- **Master System:** `NativeSMSMachine` gera amostras PSG no núcleo Rust e expõe `drain_audio_pcm()` pela extensão PyO3. A fachada `SMSMachine` em `sega_emulator.machine.sms` permite usar o mesmo `PcmResampler` nativo e a mesma saída WASAPI/XAudio2. A integração do áudio à futura execução de frames/interface de gameplay do SMS continua pendente; a interface Qt atual ainda inicia apenas SG-1000.
- Ambos os caminhos usam a mesma taxa de entrada PCM do adaptador (44,1 kHz), e a taxa de saída configurada não altera clocks, registradores nem T-states do núcleo.

## Separação de responsabilidades

1. **Núcleo de áudio do console:** registradores, divisores, osciladores, ruído, mistura digital e clocks documentados para o chip emulado.
2. **Adaptador PCM:** converte a saída determinística do núcleo para um fluxo PCM contínuo. A conversão de frequência e o eventual filtro analógico são etapas explícitas, testáveis e separadas do clock do console.
3. **Gerenciador de saída:** carrega preferências, seleciona o backend, escolhe o dispositivo, trata falhas de inicialização e recolhe métricas de underrun.
4. **Backend:** entrega PCM ao sistema operacional. Não executa CPU, PSG ou VDP, e não define o ritmo lógico da emulação.

A callback de áudio não deve executar emulação, acessar a interface gráfica, fazer I/O bloqueante ou alocar buffers de tamanho imprevisível. O fluxo entre emulação e backend precisa de uma política explícita de buffer e sincronização.

## Backends

### WASAPI (backend nativo Rust — primeira implementação)

A Qt cria e controla a fachada PyO3; a inicialização COM, enumeração do dispositivo padrão, stream compartilhado event-driven, fila PCM limitada, preenchimento de underruns com silêncio e encerramento ocorrem em Rust (native/audio_output). A taxa de amostragem e o tamanho de buffer são configurações de saída e não alteram o clock do PSG.

**Limites de validação:** esta implementação ainda não foi certificada em dispositivo Windows real. A política atual descarta o novo bloco quando a fila limitada está cheia; a contagem de blocos descartados e underruns é mantida no backend Rust. O tamanho configurado é uma solicitação de duração do buffer; no modo compartilhado, o Windows determina o período efetivo e a latência real ainda não foi medida. Reconexão/hot-plug, latência efetiva, estabilidade em sessões longas e fallback explícito ainda precisam de testes no Windows. O backend não executa emulação nem chama Python na thread de áudio.

### XAudio2 (backend nativo Rust — primeira implementação)

A Qt seleciona o backend por meio da fachada PyO3. O worker Rust cria o engine XAudio2, a mastering voice e uma source voice mono PCM16; submete buffers que permanecem vivos até o engine informar que foram consumidos. A fila de entrada é limitada, e a aplicação mede blocos descartados e episódios de underrun. A implementação usa polling de estado em uma thread Rust dedicada; ainda precisa de avaliação de CPU/latência e de um mecanismo de notificação mais eficiente se as medições justificarem.

**Limites de validação:** compilação Windows, dispositivo padrão, remoção/reconexão, encerramento e estabilidade prolongada ainda precisam ser verificados no SHA final. Não declarar equivalência de latência com WASAPI sem medições equivalentes.

### SDL3

Backend multiplataforma baseado em dispositivos e fluxos de áudio SDL3. Deve ser implementado com uma binding SDL3 real e versionada. Não reutilizar o mixer SDL2 do Pygame como se fosse SDL3: o frontend Pygame foi removido do pacote ativo; a interface suportada é Qt/PySide6.

## Configurações expostas

- Backends atualmente selecionáveis: WASAPI compartilhado nativo em Rust e XAudio2 nativo em Rust. SDL3 é uma meta futura e não deve aparecer como opção selecionável até estar implementado e testado.
- Dispositivo de saída: padrão do sistema ou dispositivo escolhido.
- Frequência PCM: 22050, 32000, 44100, 48000, 88200 ou 96000 Hz, apenas se suportada.
- Formato: PCM signed 16-bit, PCM 24-bit quando suportado, ou float 32-bit quando suportado.
- Canais de saída: mono ou estéreo; duplicar mono para estéreo é uma operação da saída, não uma mudança no chip.
- Perfil de latência: estável, equilibrado ou baixa latência; buffer manual quando a API permitir.
- WASAPI: compartilhado/exclusivo e conversão automática em modo compartilhado quando exposta pelo backend.
- Fallback: habilitado/desabilitado, com mensagem explícita e registro da configuração realmente aberta.

A UI deve desabilitar opções incompatíveis com o backend ativo, exibir erros de abertura e mostrar a configuração efetiva. Não alterar silenciosamente a frequência, o formato ou o modo selecionado sem comunicar o fallback.

## Padrões por console

Não existe uma frequência PCM histórica que represente por si só o “som real” de um console. O hardware gera sinais contínuos determinados por clocks, registradores e circuitos; a frequência de amostragem é uma escolha da cadeia moderna de captura/saída.

Portanto, cada perfil de console deverá especificar somente fatos sustentados por documentação ou validação: clock do chip, topologia de canais, mistura digital e, quando houver evidência suficiente, resposta analógica/filtros. A taxa PCM padrão será escolhida para compatibilidade e qualidade da saída atual, não apresentada como propriedade do hardware original. Para o SG-1000, 48000 Hz é um ponto inicial prático para saída Windows, sujeito a teste auditivo e medição; não substitui a validação do modelo SN76489AN.

## Plano incremental

1. Formalizar este contrato e manter o núcleo PSG independente do dispositivo de áudio.
2. Implementar configurações validadas e persistentes, incluindo perfil por console e preferências globais.
3. Validar o backend WASAPI nativo em Windows real, incluindo underruns, encerramento, reconexão, formatos e latência medida.
4. Validar o backend XAudio2 nativo em Rust como alternativa explícita, incluindo ownership dos buffers, uso de CPU e testes no mesmo SHA.
5. Manter a integração de áudio do frontend Qt separada do hardware PSG Rust e validar o wheel/driver no Windows.
6. Implementar XAudio2 por binding nativo verificável; medir estabilidade e latência.
7. Integrar escolha de dispositivo e métricas na interface Qt, sem expor configurações não implementadas.
8. Executar testes de contrato sem hardware em CI e testes físicos separados no Windows.

## Critérios de aceitação

- Nenhum backend muda os clocks lógicos, os registradores ou a sequência determinística do PSG.
- A mesma sequência PCM de entrada é entregue aos backends, salvo conversões explicitamente configuradas.
- Testes cobrem validação de configurações, fallback, buffer vazio/cheio, encerramento e falhas do dispositivo.
- A CI Linux valida o contrato sem depender de hardware de áudio.
- A validação final de latência, underruns, reconexão de dispositivos e modo exclusivo é executada em Windows real e relatada com backend, dispositivo, formato e buffer efetivos.
- O projeto não declara “latência baixa” ou equivalência sonora com hardware real sem medição e evidências.

## Referências técnicas

- Microsoft, [XAudio2 Introduction](https://learn.microsoft.com/en-us/windows/win32/xaudio2/xaudio2-introduction).
- Microsoft, [XAudio2 sample-rate conversions](https://learn.microsoft.com/en-us/windows/win32/xaudio2/xaudio2-sample-rate-conversions).
- SDL, [SDL_AudioStream](https://wiki.libsdl.org/SDL3/SDL_AudioStream).
- Microsoft, [WASAPI low-latency audio](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/low-latency-audio).
