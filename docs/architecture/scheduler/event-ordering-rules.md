# Ordenação de eventos, causalidade e delta cycles

## 1. Chave canônica atual

A fila ordena lexicograficamente por:

`(MachineTime, HardwarePhase, PlatformPriority, insertion_sequence)`

Menor valor em cada dimensão é despachado primeiro.

- `MachineTime`: instante absoluto inteiro.
- `HardwarePhase(u16)`: estrato ordinal de ordenação no perfil.
- `PlatformPriority(u16)`: prioridade de despacho de software.
- `insertion_sequence(u64)`: desempate estável atribuído pela fila.

A sequência é local à fila e preservada em snapshot. Registro de componente deve usar IDs estáveis próprios, não confundir ID com sequência.

## 2. O que as dimensões não significam

`HardwarePhase` não é timestamp fracionário, fase elétrica, borda de clock ou posição sub-tick. `PlatformPriority` não é prioridade elétrica de CPU, DMA ou VDP. Só evidência específica do hardware pode definir precedência física; o Scheduler fornece a ordenação determinística que o perfil escolhe.

## 3. Causalidade

Após despacho, a fila mantém watermark completo. Não aceita:
- timestamp anterior ao último despacho;
- evento no mesmo timestamp que regrida fase/prioridade;
- sequência duplicada ou esgotada;
- estado restaurado não canônico ou causalmente regressivo.

Eventos produzidos durante um despacho podem ser inseridos no mesmo timestamp se não regredirem a chave já consumida. A regra evita rollback causal, mas não determina a ordem física de sinais simultâneos.

## 4. Eventos simultâneos

Para cada colisão potencial (CPU write × VDP fetch, IRQ × acknowledge, DMA × CPU, PSG register write × divider edge), o perfil deve registrar:
1. sinais envolvidos;
2. timestamp e granularidade conhecidos;
3. fonte/medição;
4. ordem observada ou hipótese;
5. comportamento se a evidência for insuficiente;
6. teste que fixa a regra.

Não usar sequência de inserção como substituto de uma regra física ausente. A sequência só desempata eventos que o perfil já considera equivalentes.

## 5. Delta-event budget

`DeltaEventPolicy` limita o número de eventos no mesmo timestamp. O default implementado é 1024. Zero é inválido. Exceder o limite gera erro e deixa o evento pendente; aumentar o limite pode permitir retomada.

O limite é proteção contra loops de software, não limite do hardware, nem uma prioridade.

## 6. Perfil e estabilidade

Valores de fase/prioridade devem ser centralizados no perfil da máquina, documentados e testados. Não espalhar literais numéricos pelo código. Mudanças de ordenação são mudanças semânticas e exigem testes diferenciais, atualização de evidências e revisão arquitetural.

## 7. Persistência

Snapshot preserva eventos pendentes, sequência seguinte e watermark completo. Restore valida ordem canônica, unicidade, faixa de sequência e causalidade antes de reconstruir a heap.
