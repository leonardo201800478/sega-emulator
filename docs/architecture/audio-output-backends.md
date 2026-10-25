# Contrato de configuração para saída de áudio

> Escopo desta PR: configurações validadas e serializáveis. Os backends WASAPI, XAudio2 e SDL3, o gerenciador de saída e a integração com a UI ainda **não estão implementados**; os itens abaixo são contratos e plano de trabalho, não capacidades já disponíveis.

## Objetivo

Definir um contrato de configuração validado e independente do dispositivo para uma futura saída WASAPI, XAudio2 ou SDL3. Esta PR não abre dispositivos nem entrega áudio físico. A saída futura não poderá alterar o estado determinístico do hardware emulado.

## Separação de responsabilidades

1. **Núcleo de áudio do console:** registradores, divisores, osciladores, ruído, mistura digital e clocks documentados para o chip emulado.
2. **Adaptador PCM:** converte a saída determinística do núcleo para um fluxo PCM contínuo. A conversão de frequência e o eventual filtro analógico são etapas explícitas, testáveis e separadas do clock do console.
3. **Gerenciador de saída:** carrega preferências, seleciona o backend, escolhe o dispositivo, trata falhas de inicialização e recolhe métricas de underrun.
4. **Backend:** entrega PCM ao sistema operacional. Não executa CPU, PSG ou VDP, e não define o ritmo lógico da emulação.

A callback de áudio não deve executar emulação, acessar a interface gráfica, fazer I/O bloqueante ou alocar buffers de tamanho imprevisível. O fluxo entre emulação e backend precisa de uma política explícita de buffer e sincronização.

## Backends

### WASAPI

Backend nativo para Windows. O modo compartilhado é o padrão inicial por compatibilidade com outros aplicativos. Modo exclusivo e processamento orientado a eventos são opções avançadas quando o dispositivo e a implementação suportarem. Não assumir que um buffer mínimo funciona em todos os endpoints; validar a configuração e reportar o valor efetivo.

### XAudio2

Backend nativo Windows orientado a jogos, com grafo de vozes e buffers dinâmicos. Deve usar uma implementação mantida e testável da API nativa; não tratar um pacote Python de nome semelhante como binding de XAudio2 sem confirmar sua finalidade. A taxa da voz de origem e a taxa do dispositivo são conceitos separados, e conversões devem ser contabilizadas.

### SDL3

Backend multiplataforma baseado em dispositivos e fluxos de áudio SDL3. Deve ser implementado com uma binding SDL3 real e versionada. Não reutilizar o mixer SDL2 do Pygame como se fosse SDL3: o frontend atual depende de pygame-ce, que fornece a integração SDL2.

## Configurações expostas

- Backend: WASAPI, XAudio2 ou SDL3.
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
3. Implementar WASAPI e medir latência/underruns no Windows; shared é o padrão inicial, exclusive opcional.
4. Implementar SDL3 com binding compatível com Python 3.14 e validar empacotamento no Windows.
5. Implementar XAudio2 por binding nativo verificável; medir estabilidade e latência.
6. Integrar as opções na interface Qt, mostrando capacidades do dispositivo e a configuração efetiva.
7. Executar testes de contrato sem hardware em CI e testes físicos separados no Windows.

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
- python-sounddevice, [Platform-specific settings / WASAPI](https://python-sounddevice.readthedocs.io/en/0.5.3/api/platform-specific-settings.html).
