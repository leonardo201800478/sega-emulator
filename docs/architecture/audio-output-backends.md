> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust (`native/sg1000_core`). A aplicação Qt usa uma fachada Python fina para PyO3 e carregadores de ROM/perfil. Implementações de hardware Python, ferramentas e testes que dependiam delas foram arquivados sob `legacy/` e não são executados pelo projeto ativo. Descrições de estados anteriores neste documento são históricas, não instruções atuais.

# Contrato de configuração para saída de áudio

> Estado: o frontend Qt já possui um primeiro backend WASAPI compartilhado, usando a implementação WASAPI do PortAudio por meio de `sounddevice`. XAudio2 e SDL3 como backends de saída ainda não estão implementados. Esta fatia não é uma implementação COM/WASAPI direta em Rust e não valida latência em hardware Windows.

## Objetivo

Definir um contrato de configuração validado e independente do dispositivo para a saída de áudio. O backend WASAPI inicial já abre o dispositivo no frontend Qt; a configuração e os testes automatizados não substituem a validação física. Nenhum backend pode alterar o estado determinístico do hardware emulado.

## Separação de responsabilidades

1. **Núcleo de áudio do console:** registradores, divisores, osciladores, ruído, mistura digital e clocks documentados para o chip emulado.
2. **Adaptador PCM:** converte a saída determinística do núcleo para um fluxo PCM contínuo. A conversão de frequência e o eventual filtro analógico são etapas explícitas, testáveis e separadas do clock do console.
3. **Gerenciador de saída:** carrega preferências, seleciona o backend, escolhe o dispositivo, trata falhas de inicialização e recolhe métricas de underrun.
4. **Backend:** entrega PCM ao sistema operacional. Não executa CPU, PSG ou VDP, e não define o ritmo lógico da emulação.

A callback de áudio não deve executar emulação, acessar a interface gráfica, fazer I/O bloqueante ou alocar buffers de tamanho imprevisível. O fluxo entre emulação e backend precisa de uma política explícita de buffer e sincronização.

## Backends

### WASAPI (Qt — implementação inicial)

O frontend Qt seleciona explicitamente o host API WASAPI por `sounddevice`/PortAudio e abre o dispositivo de saída padrão desse host em modo compartilhado, com conversão automática permitida. O produtor PCM usa uma fila limitada; a callback de saída apenas consome PCM e preenche lacunas com silêncio, sem executar a emulação. Blocos antigos podem ser descartados quando a fila fica cheia para evitar crescimento de latência. O backend conta callbacks com underrun e blocos descartados.

**Limite de implementação:** o acesso à API Windows passa pelo host WASAPI do PortAudio; esta etapa não implementa diretamente as interfaces COM de WASAPI em Rust. Modo exclusivo, escolha de dispositivo pela UI, leitura das métricas na interface e validação física de latência/underrun permanecem pendentes. O valor configurado de buffer é solicitado ao backend, mas ainda não é apresentado como latência real medida.

### XAudio2

Backend nativo Windows orientado a jogos, com grafo de vozes e buffers dinâmicos. Deve usar uma implementação mantida e testável da API nativa; não tratar um pacote Python de nome semelhante como binding de XAudio2 sem confirmar sua finalidade. A taxa da voz de origem e a taxa do dispositivo são conceitos separados, e conversões devem ser contabilizadas.

### SDL3

Backend multiplataforma baseado em dispositivos e fluxos de áudio SDL3. Deve ser implementado com uma binding SDL3 real e versionada. Não reutilizar o mixer SDL2 do Pygame como se fosse SDL3: o frontend Pygame foi removido do pacote ativo; a interface suportada é Qt/PySide6.

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
3. Validar o backend WASAPI/PortAudio em Windows real, incluindo underruns, encerramento, reconexão, formatos e latência medida.
4. Se a necessidade de controle nativo justificar o custo, avaliar a migração do adaptador PortAudio para WASAPI direto em Rust, com ownership explícito e testes no mesmo SHA.
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
- python-sounddevice, [Platform-specific settings / WASAPI](https://python-sounddevice.readthedocs.io/en/0.5.3/api/platform-specific-settings.html).
