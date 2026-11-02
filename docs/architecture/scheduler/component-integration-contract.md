# Contrato de integração de componentes temporais

## 1. Propósito

Este documento define o que um componente deve oferecer para participar da timeline de uma máquina. É um contrato arquitetural; não congela uma trait Rust antes de validar ownership, monomorfização, custo e semântica.

## 2. Identidade e ownership

Cada instância possui:
- identidade estável no perfil da máquina;
- zero ou mais domínios de clock explicitamente registrados;
- estado funcional e temporal próprio;
- estado de execução restaurável;
- lista de eventos que pode produzir;
- fronteiras seguras de suspensão/retomada.

O Scheduler é dono da timeline global. O componente é dono de seu estado interno. A máquina é dona da composição e das interações.

## 3. Capacidades conceituais exigidas

Um componente temporal integrado deve permitir à máquina:
1. conhecer o próximo deadline/fronteira observável;
2. avançar deterministicamente até um deadline que não ultrapasse;
3. preservar fase e estado interno ao suspender;
4. expor eventos produzidos com timestamp e payload tipado;
5. aceitar transações externas no instante definido pelo perfil;
6. indicar se está bloqueado, pausado ou aguardando recurso;
7. exportar/importar seu estado sem depender do host.

Não se exige que cada oscilador interno gere um evento global por ciclo. Avanço interno em lote é permitido se equivalência for provada.

## 4. Safe point

Safe point é uma fronteira na qual a execução pode parar sem perder informação causal nem duplicar efeitos. Deve preservar:
- instante absoluto e fase local;
- micro-op/operação em andamento;
- endereço, dados e direção de transação pendente;
- latência restante ou deadline de conclusão;
- WAIT/bloqueio e condição de retomada;
- eventos emitidos ainda não consumidos;
- estado de interrupção/latch relevante;
- identificador/sequence necessário para não aplicar uma operação duas vezes.

Não é automaticamente seguro suspender apenas ao fim de instrução ou scanline.

## 5. Transações

Uma transação deve distinguir, conforme o dispositivo:
- solicitação;
- instante de apresentação do endereço/dados;
- instante de amostragem/leitura;
- instante de efetivação/escrita;
- conclusão;
- duração/WAIT;
- efeitos colaterais e evento resultante.

O Scheduler atual não define esses campos nem resolve arbitragem. O contrato concreto pertence ao bus adapter e ao perfil do sistema. A proposta de tipos da PR #396 deve ser tratada como revisão pendente até ser integrada e auditada.

## 6. Eventos

Eventos devem ser valores tipados, serializáveis por codec determinístico, sem closures ou ponteiros. Payload deve identificar origem/destino de forma estável quando necessário. O produtor não escolhe uma prioridade para contornar a causalidade; prioridades são definidas no perfil.

## 7. Pausa, reset e clock gating

Não representar pausa ou clock parado por razão de clock zero. A razão é positiva no modelo atual. Pausa, reset, suspensão e clock gating são estados explícitos com semântica definida pelo hardware/perfil. A política de reset deve esclarecer se cada domínio reinicia, continua ou muda de fase.

## 8. Save state

O snapshot deve permitir continuação equivalente após restauração. Capturar também estado parcial e transações pendentes. A operação deve ser atômica: se qualquer componente falhar na validação, não expor máquina parcialmente restaurada.

## 9. Critérios de aceitação por componente

- clock/ratio/fase documentados e rastreáveis;
- deadline e safe points definidos;
- nenhuma passagem por eventos externos;
- chunk invariance demonstrada;
- eventos determinísticos e ordenados;
- estado de suspensão/retomada serializável;
- snapshot/restore com replay equivalente;
- teste diferencial contra caminho de referência;
- sem regressão nos sistemas já funcionais.
