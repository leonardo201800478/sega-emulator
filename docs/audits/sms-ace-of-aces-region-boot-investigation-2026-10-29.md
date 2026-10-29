# Investigação: Ace of Aces (Europe, Brazil) (En) não inicia no SMS

**Data:** 2026-10-29  
**Base observada:** `main` em `b4a85c82b1d975a126f1dcbd3084dac51249b398`  
**Status:** INVESTIGATION_PENDING — causa ainda não confirmada.

## Sintoma reportado

A ROM identificada como `Ace of Aces (Europe, Brazil) (En)` não inicia no Sega Emulator. O usuário suspeita de configuração/região.

O nome do dump sugere uma edição europeia/brasileira, mas o nome do arquivo não é evidência suficiente para definir o hardware esperado, o conteúdo do cabeçalho ou o modo de vídeo correto. A região é uma hipótese prioritária, não uma conclusão.

## Objetivo

Encontrar a primeira divergência que impede o boot e determinar se ela está relacionada à seleção automática/manual de região, ao perfil regional do SMS, à leitura de metadados da ROM, ao carregamento/mapeamento do cartucho ou a outra camada do runtime.

## Hipóteses a distinguir

1. **Detecção da região:** o modo automático pode não inferir o perfil esperado a partir desta imagem, ou a ROM pode não conter metadados reconhecidos.
2. **Perfil regional incompleto/inconsistente:** seleção Europe/PAL pode não configurar de forma coerente o clock do sistema, total de linhas, contador de linha, pacing e perfil do VDP. Separar parâmetros de vídeo do host de parâmetros de hardware.
3. **Região escolhida na interface não chega ao core:** verificar persistência da configuração, troca de sistema, carregamento de ROM e perfil efetivamente entregue ao runtime Rust.
4. **Cartucho ou dump:** tamanho, padding/cabeçalho, checksum e mapeamento de bancos podem impedir o boot independentemente da região.
5. **CPU/reset ou periféricos:** estado inicial do Z80, interrupções, portas, VDP ou PSG podem desviar antes de o jogo mostrar imagem.
6. **Problema de apresentação confundido com boot:** verificar se a CPU progride e o frame muda, mesmo que a saída visual fique preta, congelada ou fora de sincronismo.

## Plano de investigação

### 1. Fixar o artefato e a configuração

Na máquina de teste, registrar localmente, sem compartilhar a ROM:
- tamanho em bytes e SHA-256;
- metadados/cabeçalho e checksum, se presentes;
- modo selecionado na interface: Automatic, Europe, USA ou Japan;
- sistema/modelo do console, região do core e parâmetros de vídeo efetivos;
- resultado observável em cada modo: tela preta, logo/tela inicial, congelamento, reinício, áudio, ou ausência de execução.

Repetir o teste com a mesma cópia e a mesma sequência de inicialização. Não adicionar a ROM ao repositório nem publicar dumps de memória que revelem seu conteúdo.

### 2. Auditar o fluxo de região no repositório

Seguir a configuração de ponta a ponta:
- valor salvo na interface Qt/PySide6 e valor padrão;
- caminho usado para resolver `Automatic`;
- perfil de máquina selecionado durante o carregamento;
- conversão Python/PyO3 e parâmetro passado ao core Rust;
- clock/perfil do SMS, total de linhas, counters e pacing;
- estado reinicializado ao trocar região ou carregar outra ROM;
- consistência entre região de emulação e temporização de apresentação.

Confirmar o comportamento real no código atual; não inferir que a opção Europe equivale a corrigir o boot sem rastrear o perfil efetivo.

### 3. Separar região de carregamento e execução

Comparar os resultados dos quatro modos, sem alterar simultaneamente outros parâmetros. Se o boot falhar em todos, priorizar carregamento, mapper, reset e tracing inicial do Z80. Se somente um perfil funcionar, registrar exatamente qual parâmetro difere e construir um teste unitário para a política de seleção.

Coletar checkpoints limitados:
- PC e primeiros opcodes após reset;
- evolução do PC e instruções por frame;
- acessos relevantes ao mapper/memória;
- estado/status do VDP e interrupções;
- indicação de que a execução avança mesmo sem quadro visível.

Não publicar conteúdo bruto da ROM. Os logs devem registrar endereços, estados e hashes, não sequências completas de bytes de ROM.

### 4. Comparar com referência

Quando possível, testar o mesmo dump em GearSystem e Ares e registrar região/modelo configurados. A comparação deve separar:
- se a CPU executa;
- se o cartucho é reconhecido;
- se há saída de vídeo;
- se o jogo efetivamente alcança o estado de boot esperado.

Não tratar o nome da ROM ou um único emulador como prova conclusiva.

### 5. Regressão e correção

Se a causa for a política regional, adicionar testes para Automatic/Europe/USA/Japan, passagem de configuração ao core e parâmetros do perfil PAL/NTSC. Se a causa for independente de região, abrir uma fatia específica para a camada identificada. Usar fixtures sintéticas/licenciadas; não incorporar a ROM comercial.

## Critérios de aceitação

- [ ] Registrar SHA-256/tamanho local do dump e as opções de região usadas, sem versionar a ROM.
- [ ] Demonstrar o perfil regional realmente entregue ao core Rust.
- [ ] Testar Automatic, Europe, USA e Japan isoladamente.
- [ ] Identificar a primeira divergência: carregamento, mapper, reset/Z80, VDP/interrupções ou apresentação.
- [ ] Demonstrar com evidência se a região causa a falha ou apenas altera o modo de vídeo/temporização.
- [ ] Adicionar teste determinístico que reproduza a causa sem ROM comercial.
- [ ] Executar testes direcionados e suíte pertinente no SHA candidato.
- [ ] Validar manualmente no Windows com a mesma ROM e registrar o resultado.

## Estado de validação

- **IMPLEMENTED:** especificação documental da investigação.
- **QUICK_PASS:** NOT RUN.
- **SHORT_PASS:** NOT RUN.
- **LONG_PASS:** NOT RUN.
- **INTEGRATION_PENDING:** SIM.
- **Boot manual:** PENDING; não executado nesta sessão.
- **CI:** deve ser verificado no SHA candidato; status ausente/pendente não é aprovação.
- **TEMPORAL_PASS:** não alcançado nem pretendido nesta fatia.

## Limites

- Não modificar `main` diretamente.
- Não aplicar workaround pelo nome do jogo.
- Não presumir que toda ROM identificada como Europe/Brazil exige um único perfil sem verificar metadados e comportamento.
- Não confundir pacing de vídeo do host com temporização real do hardware.
- Não incluir ROM comercial, BIOS protegida ou logs com conteúdo da ROM no Git.

## Fontes técnicas a consultar

- Fluxo de seleção de região e perfil SMS no repositório Sega Emulator.
- Documentação técnica do Sega Master System para regiões, temporização e comportamento do cabeçalho de ROM.
- Código público do GearSystem e Ares para comparar o comportamento, sem copiar código nem presumir independência dos oráculos.

Esta PR documenta o plano e não declara a causa nem uma correção.
