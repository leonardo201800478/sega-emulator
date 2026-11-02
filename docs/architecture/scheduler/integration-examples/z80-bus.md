# Exemplo de integração: Z80 e barramento

**Status atual:** proposta de integração; o caminho SMS/SG-1000 ainda executa instruções de forma atômica e não é dirigido operacionalmente pelo Scheduler.

## 1. Responsabilidades

- Z80: estado arquitetural completo, microsequencer, M-cycles/T-states e safe points.
- Bus adapter: traduz micro-op em transação, resolve memória/I/O/mapper, WAIT e efeitos colaterais.
- Scheduler: avança timeline/domínios até fronteiras autorizadas e despacha eventos.
- Máquina: coordena CPU, bus, VDP, IRQ e eventos simultâneos conforme perfil.

O Scheduler não decodifica opcode, endereço, ciclo M1 ou sinal Z80.

## 2. Fluxo conceitual

1. CPU emite uma micro-operação/solicitação sem aplicar efeito externo prematuramente.
2. Bus adapter classifica ciclo e calcula plano temporal conforme perfil e evidência.
3. Máquina agenda o próximo limite/evento necessário.
4. Scheduler avança domínios e retorna despacho.
5. Máquina resolve eventos concorrentes e chama o bus adapter para amostrar/efetivar a operação no instante correto.
6. CPU recebe completion e retoma do estado serializado.
7. WAIT suspende a transação sem repetir efeitos já aplicados.

## 3. Estado mínimo suspensível

Opcode/prefixo, micro-op atual, registradores, PC/SP, flags, bancos, interrupções, HALT/EI delay, endereço/dados/direção pendentes, fase do ciclo, WAIT e evento de conclusão. O conjunto exato depende do executor Z80 e deve ser inventariado antes da API.

## 4. Limites de evidência

Manual ZiLOG descreve timing genérico Z80, não toda a integração elétrica SMS. O perfil SMS precisa documentar relação CPU/VDP, WAIT, refresh, acknowledge e simultaneidade com fontes adequadas. Não transformar `PlatformPriority` em prioridade física.

## 5. Critérios de aceitação

- execução retomável sem perda/duplicação;
- teste diferencial com executor atômico;
- transações pendentes em save/restore;
- WAIT com retomada futura;
- eventos IRQ/VDP simultâneos testados;
- mesmo trace para execução contínua e particionada.
