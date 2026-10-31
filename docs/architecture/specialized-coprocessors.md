# Arquitetura de processadores auxiliares, DSPs e coprocessadores especializados

**Estado:** proposta arquitetural normativa para implementação futura  
**Escopo imediato:** documentação e contratos conceituais; nenhuma implementação de coprocessador é declarada  
**Runtime de hardware:** exclusivamente Rust  
**Autoridade temporal:** scheduler único da máquina, no runtime Rust

## 1. Objetivo

Definir como o Sega Emulator representará processadores especializados que ampliam a capacidade de um console, especialmente chips programáveis instalados em cartuchos ou módulos de expansão. O exemplo de referência é o **Sega Virtua Processor (SVP)** do cartucho de *Virtua Racing* para Mega Drive.

Esta categoria não se refere aos chips convencionais de vídeo, áudio ou I/O. Também não significa que toda ALU ou multiplicador integrado a uma CPU deva virar um dispositivo independente.

O objetivo é permitir que cada coprocessador tenha sua própria arquitetura, estado, memória, registradores, protocolo de comunicação e comportamento temporal, sem contaminar os núcleos de CPU existentes nem introduzir um executor genérico que esconda diferenças reais.

## 2. Terminologia e classificação

O domínio é denominado **Centro de Processadores e Coprocessadores Especializados**. “Centro” representa uma responsabilidade arquitetural de inventário, configuração e integração; não deve se tornar uma classe monolítica que execute todos os dispositivos.

| Categoria | Responsabilidade | Exemplos / tratamento |
|---|---|---|
| CPU de propósito geral | Executa o programa principal ou uma CPU auxiliar de plataforma | Motorola 68000, Z80 e SH-2; cada arquitetura conserva seu próprio núcleo |
| Coprocessador programável | Executa instruções próprias para acelerar tarefas específicas | SVP / DSP da família SSP1601 |
| DSP ou acelerador matemático | Executa operações aritméticas/sinal/geométricas especializadas, com ISA e estado próprios | DSPs de cartucho; não presumir que sejam compatíveis entre si |
| Acelerador gráfico programável | Executa código e produz dados que serão consumidos pelo pipeline gráfico da plataforma | SVP como gerador de dados gráficos para o Mega Drive |
| ASIC programável ou híbrido | Combina lógica fixa, processador, memória e interfaces específicas | Deve ser decomposto em blocos observáveis, preservando as dependências físicas relevantes |

**Fora desta categoria:** ALUs internas de CPUs, VDPs convencionais, PSG/FM, controladores comuns e funções puramente passivas. Se um chip especializado incorporar esses blocos, eles serão modelados dentro do dispositivo quando fizerem parte do seu comportamento observável.

Um chip não será classificado apenas pelo nome comercial. O catálogo distinguirá fabricante/designação física, arquitetura interna conhecida, revisão, placa/cartucho e grau de confiança das informações.

## 3. Princípios obrigatórios

1. **Um núcleo por arquitetura real.** SVP/SSP1601, SH-2 e outros processadores não serão executados por um interpretador universal de instruções.
2. **Rust é o único runtime.** CPU auxiliar, DSP, memória interna, registradores, DMA, interrupções e temporização pertencem a Rust. Python/Qt limita-se à interface e a comandos de ciclo de vida em nível de máquina.
3. **O dispositivo não possui scheduler independente.** O scheduler mestre da máquina controla todos os domínios de clock e decide a ordem dos eventos.
4. **Barramento explícito.** Comunicação com CPU principal, memória compartilhada, registradores, portas e interrupções deve ocorrer por contratos de barramento da plataforma, não por chamadas diretas entre núcleos.
5. **Tempo determinístico.** Nenhum thread paralelo, relógio de parede, timer Python ou callback do host pode avançar o coprocessador.
6. **Estado completo.** Registradores, pipeline/estado de execução, memórias internas, flags, latches, eventos pendentes e estado das interfaces devem integrar save states quando observáveis.
7. **Incerteza explícita.** Fatos documentados, comportamento observado, inferências e hipóteses serão registrados separadamente. Desconhecido não será convertido em valor inventado.
8. **Implementação incremental.** Um registro no catálogo ou um stub não significa compatibilidade funcional.

## 4. Posição na arquitetura

A plataforma instancia o coprocessador como um dispositivo de hardware independente e conecta seus recursos aos contratos concretos da máquina.

```text
ROM / imagem do cartucho
        |
        v
Identificação + perfil de placa
        |
        v
Configuração da plataforma Mega Drive
        |
        +---------------- CPU 68000
        |                      |
        |                 barramento MD
        |                      |
        +---- cartucho --------+
                 |
          interface do cartucho
                 |
        +--------+----------------------+
        |                               |
   ROM do cartucho                Dispositivo SVP
                                       |
                             +---------+----------+
                             | DSP / estado       |
                             | ROM/IRAM internas  |
                             | RAMs internas      |
                             | registradores      |
                             | interface externa  |
                             +---------+----------+
                                       |
                              DRAM do cartucho /
                              portas e sinais SVP
                                       |
                              dados gráficos gerados
                                       |
                              DMA / barramento MD
                                       |
                                      VDP
```

O diagrama é conceitual: o mapa, a direção e a temporização de cada transferência precisam ser confirmados para a revisão de placa e o comportamento emulado. Não representa uma especificação elétrica completa.

### 4.1 Separação de responsabilidades

- **Núcleo do processador:** ISA, registradores, flags, pipeline, execução e exceções próprias.
- **Memória do coprocessador:** ROM interna, RAM de instruções, RAM de dados, memória externa e regras de acesso.
- **Interface do dispositivo:** registradores mapeados, portas, mailboxes, sinais de busy/ready, interrupções e protocolos de comando.
- **Adaptador de plataforma:** decodificação de endereços, ligação ao barramento, arbitragem, waits, DMA e conexão com outros dispositivos.
- **Scheduler mestre:** avanço temporal, domínios de clock, ordenação dos eventos e sincronização entre CPU, coprocessador e VDP.
- **Save state:** captura e restauração atômica do estado do dispositivo e dos eventos associados.

O núcleo do DSP não deve conhecer diretamente o frontend, o carregador de ROM, o VDP concreto ou APIs do Windows. O adaptador da plataforma fornece os recursos necessários por contratos tipados.

## 5. Contratos de integração

Os contratos abaixo definem responsabilidades, não uma API Rust final. Os tipos e traits concretos só devem ser introduzidos quando uma implementação real exigir seus detalhes.

### 5.1 Identidade e perfil

Cada instância deve ser configurada por um perfil explícito que possa registrar:

- identificador estável do dispositivo e família de arquitetura;
- fabricante/designação física e revisão conhecida;
- placa/cartucho e revisão regional, quando relevante;
- regiões de ROM/RAM, registradores e portas;
- fontes e relações de clock documentadas;
- sinais de reset, interrupção, busy/ready e arbitragem;
- requisitos de memória externa e comportamento de DMA;
- procedência, licença e confiança das evidências técnicas.

A identificação de hardware deve preferir hashes e metadados verificáveis de ROM/placa. Nome do jogo ou texto de cabeçalho, isoladamente, não autoriza ativar um coprocessador. Casos desconhecidos ou ambíguos devem permanecer como UNKNOWN/AMBIGUOUS, sem selecionar silenciosamente um perfil.

### 5.2 Barramento e comunicação

A integração deve suportar, conforme o chip exigir:

- janelas de memória e registradores mapeados;
- memória compartilhada com regras explícitas de acesso;
- portas de comando, mailbox ou handshake;
- requisições de leitura/escrita ao barramento;
- DMA e arbitragem de propriedade do barramento;
- linhas de interrupção e sinais de estado;
- estados de espera e contenção.

Não se deve presumir que todos os coprocessadores compartilhem a mesma interface. Um dispositivo pode usar memória mapeada, outro comandos, outro acesso compartilhado ou uma combinação desses mecanismos.

### 5.3 Execução e tempo

O scheduler mestre é a única autoridade temporal. Cada dispositivo declara seu domínio de clock e os eventos/recursos de que depende. Relações entre clocks devem ser representadas por razões inteiras ou acumuladores de fase determinísticos, conforme a necessidade comprovada.

O scheduler deve poder ordenar, sem ambiguidade:

- avanço de CPU principal e coprocessador;
- requisições e concessões de barramento;
- acessos a memória compartilhada;
- waits, DMA e contenção;
- interrupções, reset e handshakes;
- eventos de saída consumidos por outros componentes.

Empates temporais precisam de uma regra de ordenação documentada e testada. Não executar CPU e coprocessador em paralelo para “ganhar desempenho”: primeiro estabelecer semântica e determinismo. Qualquer otimização futura exige profiling e equivalência de resultados.

Uma contagem agregada de ciclos por instrução pode ser usada em uma fase preliminar, mas não autoriza declarar precisão ciclo a ciclo. Essa declaração depende de traces ordenados de barramento e comparação com evidência independente.

### 5.4 Save states e determinismo

O estado serializado precisa incluir, no mínimo, estado arquitetural do processador, memórias internas mutáveis, estado das interfaces, requisições pendentes, interrupções e fase temporal necessária para retomar a execução.

Critérios:
- salvar e restaurar no meio de uma operação;
- continuar com a mesma sequência de eventos e obter os mesmos resultados;
- rejeitar versões/perfis incompatíveis explicitamente;
- não depender de ponteiros de host, threads, relógios de parede ou estado do frontend.

## 6. Caso de referência: Sega Virtua Processor (SVP)

O SVP é o primeiro alvo conceitual desta categoria. Fontes técnicas independentes o descrevem como um DSP de 16 bits associado à família Samsung SSP1601, com lógica externa específica da Sega. O chip não deve ser tratado como um SSP1601 genérico sem validar as diferenças do dispositivo real.

### 6.1 Blocos a modelar

O modelo deve investigar e separar:

1. **Núcleo DSP:** instruções, registradores, flags, operações aritméticas, multiplicação/acumulação, fluxo de controle e exceções.
2. **Memória interna:** ROM interna de inicialização, RAM de instruções e bancos de RAM de dados, com tamanho e mapeamento confirmados por evidência.
3. **Memória externa do cartucho:** ROM do jogo, DRAM/RAM externa e decodificação de endereços.
4. **Registradores externos:** comandos, status, sincronização e sinais entre DSP e Mega Drive.
5. **Interface do cartucho:** geração/recepção de clock, reset e conexões com o barramento.
6. **Caminho de produção de dados gráficos:** geração dos dados/tile patterns e transferência para a memória de vídeo do Mega Drive, respeitando o mecanismo real de acesso e DMA.

Não incorporar números de clock, latências, tamanhos ou efeitos de registradores apenas por repetição em fontes secundárias. Fixar cada valor somente após rastrear a procedência e, quando possível, confrontar com hardware, documentação ou testes independentes.

### 6.2 Integração com o Mega Drive

- O SVP é um dispositivo do cartucho; não é uma extensão do núcleo 68000 nem do Z80.
- O 68000 acessa a interface do cartucho segundo o mapa e protocolo observados.
- O DSP executa seu próprio programa e interage com recursos internos/externos do cartucho.
- A memória compartilhada e os sinais de sincronização devem respeitar conflitos e waits observáveis.
- Dados gráficos gerados pelo SVP chegam ao VDP pelo caminho efetivamente usado pelo cartucho e pelo software; não criar uma ligação direta fictícia DSP→framebuffer.
- O VDP continua sendo o hardware de vídeo do Mega Drive. O SVP produz dados que o pipeline normal do console pode consumir.

### 6.3 Incertezas conhecidas

A pesquisa pública contém descrições de blocos internos e análises de registradores, mas também identifica sinais, lógica externa e comportamentos ainda não completamente compreendidos. A implementação deverá manter uma matriz CONFIRMED / OBSERVED / INFERRED / UNKNOWN por comportamento.

O SVP deve ser validado como chip integrado à placa do cartucho, não somente como um núcleo DSP isolado. Uma implementação que execute instruções, mas ignore a interface externa, memória compartilhada e sincronização, não é compatibilidade SVP.

## 7. Organização Rust proposta

A arquitetura lógica recomendada é:

```text
native/
  processor_devices/              # domínio futuro; criar quando houver código real
    device_contracts/              # contratos mínimos compartilhados
    svp/                           # núcleo e integração específica do SVP
      dsp/
      memory/
      external_interface/
      tests/
  megadrive_core/
    cartridge/
      profiles/
      bus_adapter/
    machine/
      scheduler/
      save_state/
```

Esta árvore é uma proposta de organização, não um compromisso de criar todos esses crates ou diretórios imediatamente. Antes de criar um crate compartilhado, verificar o workspace, dependências, contratos já existentes e consumidores. Não duplicar o núcleo de CPU nem os contratos de barramento que já existirem.

A implementação do SVP deve poder ser testada isoladamente com barramento e memória determinísticos, e depois integrada ao cartucho/Mega Drive sem alterar a semântica do núcleo 68000.

## 8. Catálogo extensível

O catálogo deve separar **família de dispositivo**, **implementação de núcleo**, **perfil de placa** e **perfil de software**. Uma mesma família pode ter variantes incompatíveis; uma mesma plataforma pode ter cartuchos sem coprocessador e cartuchos com coprocessador.

Campos recomendados:

| Campo | Finalidade |
|---|---|
| device_id | Identidade estável da instância/família |
| architecture | ISA ou classe funcional específica |
| silicon_revision | Revisão física conhecida ou UNKNOWN |
| board_profile | Mapa de memória, conexões e componentes da placa |
| rom_match | Hashes/tamanho/região e procedência da identificação |
| clock_domains | Domínios e relações temporais documentadas |
| memory_regions | ROM/RAM internas e externas, permissões e espelhamentos |
| bus_interfaces | Endereços, portas, protocolos e arbitragem |
| interrupts | Fontes, polaridade, prioridade e comportamento conhecido |
| save_state_version | Versão do formato de estado |
| evidence | Fontes, testes, confiança e incertezas |
| status | Estado real de implementação e validação |

O catálogo não executa dispositivos e não substitui o carregador de ROM. Ele fornece dados de configuração rastreáveis para a plataforma.

## 9. Estratégia de implementação e critérios de aceitação

A implementação real fica condicionada ao avanço do Mega Drive no roadmap. Não interromper a consolidação SMS/Z80 para iniciar um coprocessador de uma plataforma ainda não integrada.

### Fase 0 — pesquisa e especificação do SVP
- Consolidar documentação técnica, pesquisas de die, análises de registradores e evidências de hardware.
- Distinguir comportamento do DSP SSP1601 genérico de particularidades do SVP.
- Criar mapa de memória, registradores, reset, clock, protocolo, DMA e incertezas.
- Verificar procedência e licença de todos os materiais/testes.

### Fase 1 — núcleo DSP isolado
- Implementar a ISA identificada do modelo correto.
- Vetores fixos para ALU, multiplicação/acumulação, flags, endereçamento e wraparound.
- Testar ROM interna, RAM de instruções e RAM de dados.
- Usar oráculo independente; ancestralidade comum entre implementações deve ser registrada.

### Fase 2 — integração externa
- Modelar interface do cartucho, registradores, memória externa e sincronização.
- Testar leituras/escritas, handshake, status, reset e casos de contenção.
- Implementar waits, arbitragem e DMA conforme evidência, sem atalhos diretos para o VDP.

### Fase 3 — integração Mega Drive
- Ativar o perfil SVP somente para imagens/placas identificadas com segurança.
- Testar interação 68000↔SVP, cartucho↔barramento e dados destinados ao VDP.
- Preservar cartuchos Mega Drive sem coprocessador e todos os testes anteriores.

### Fase 4 — fidelidade e robustez
- Testes diferenciais, ROMs sintéticas/homebrew legais e testes de hardware quando disponíveis.
- Traces temporais de barramento, reset, interrupções, DMA, save/load e determinismo.
- Estresse e benchmark apenas depois da equivalência funcional.
- Validar no emulador com *Virtua Racing* e registrar a revisão/hash da ROM local sem versionar seu conteúdo.

### Critérios de aceitação

- Nenhum fallback de execução em Python.
- Nenhuma alteração no resultado do Mega Drive base quando o coprocessador não está presente.
- Perfil de cartucho selecionado por evidência inequívoca.
- Núcleo DSP e interface externa cobertos por testes independentes.
- Comunicação, memória compartilhada, waits e DMA reproduzíveis.
- Save/load no meio de atividade sem divergência.
- Execuções repetidas com estado e eventos idênticos produzem estado final e traces idênticos.
- Checks Rust e workflows executados no SHA candidato exato.
- Compatibilidade de jogo e precisão temporal declaradas somente após evidência própria.

Estados devem ser registrados separadamente: IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING e TEMPORAL_PASS. Um teste de instruções aprovado não implica integração, e integração funcional não implica TEMPORAL_PASS.

## 10. Referências técnicas e limites

As fontes abaixo orientam pesquisa e formulação de hipóteses; não substituem documentação de fabricante nem validação no hardware.

- [Copetti — Mega Drive / Genesis Architecture, seção Extra functions](https://www.copetti.org/pt-br/writings/consoles/mega-drive-genesis/): contextualiza o SVP como DSP de 16 bits associado ao SSP1601 e a geração de dados gráficos.
- [SVP Development Tools — jdesiloniz/svpdev](https://github.com/jdesiloniz/svpdev): projeto de ferramentas e pesquisa para executar código em hardware SVP; documentação e wiki destacam blocos internos e áreas ainda desconhecidas. Consultar licença e procedência antes de reutilizar qualquer artefato.
- [SVP decap and details](https://github-wiki-see.page/m/jdesiloniz/svpdev/wiki/SVP-decap-and-details): análise visual do die e hipóteses sobre ROM/RAM, clock e blocos DSP; separar observação de inferência.
- [External registers behaviour](https://github-wiki-see.page/m/jdesiloniz/svpdev/wiki/External-registers-behaviour): pesquisa do protocolo de registradores externos e sincronização; alguns comportamentos continuam em investigação.
- [Charles MacDonald — Sega Genesis hardware notes](https://www.neperos.com/article/picov96e5792f069): notas históricas de engenharia reversa, incluindo mapa preliminar e observações do cartucho Virtua Racing; tratar como evidência histórica, não especificação final.
- [MAME issue #7910 — SVP/SSP1601 boot process](https://github.com/mamedev/mame/issues/7910): discussão sobre o processo de inicialização do DSP; verificar o estado atual do código e a independência do oráculo antes de usá-lo em testes.

Nenhum código, teste, ROM, dump de BIOS ou asset dessas referências deve ser copiado sem auditoria de licença e procedência. Referências de emuladores são fontes de comportamento observado, não prova automática de equivalência com o hardware.

---

**Decisão arquitetural:** o Sega Emulator terá suporte extensível a coprocessadores e processadores especializados como dispositivos Rust independentes, integrados ao barramento e ao scheduler mestre da plataforma. O SVP será o primeiro estudo de caso quando a trilha Mega Drive estiver pronta; esta especificação não declara o SVP implementado.
