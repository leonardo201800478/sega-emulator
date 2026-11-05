# Z80 compartilhado — scorecard de conclusão

**Baseline documental consultada:** main em f4e428ad31c0d2e214e8ed15aab8fa705b1c795e (2026-11-05).  
**Política vigente:** [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).  
**Escopo:** um único núcleo Z80 compartilhado pelos consoles que realmente utilizam essa CPU.

Este scorecard mede o que é necessário para executar corretamente SG-1000 e Master System. Não define uma meta de simulação elétrica, cobertura integral de pinos ou microsequenciamento de cada instrução sem dependência de plataforma.

## 1. Critérios funcionais

| Área | Critério |
|---|---|
| Instruções documentadas | Implementar corretamente as instruções que o software dos consoles suportados pode executar |
| Registradores e flags | Preservar os resultados arquiteturais e os efeitos observáveis |
| Prefixos | Tratar corretamente CB, ED, DD, FD, DDCB e FDCB nos caminhos suportados |
| Interrupções | Reproduzir os modos e as transições usados pela plataforma |
| HALT / reset | Preservar comportamento funcional e retomada correta |
| Estado compartilhado | Uma implementação Rust, sem cópias de CPU por console |
| Regressão | Alterações no Z80 não podem quebrar SMS nem SG-1000 |

A matriz [DD/FD semântica](z80-dd-fd-semantic-matrix.md) permanece referência técnica para instruções indexadas. Ela não transforma cada particularidade não documentada em requisito universal.

## 2. Estado conhecido na baseline consultada

- O núcleo Z80 compartilhado está em native/z80_cpu.
- E3A1/E3A2 e E4A/E4B/E4C constam como integradas nas auditorias recentes.
- E5 DDCB/FDCB e E6 resolução geral de prefixos constam como integradas na main.
- E7.3/HALT T4 consta como integrada.
- O fechamento semântico global ainda não foi comprovado como concluído.
- O caminho de barramento/temporização está implementado apenas em fatias; não há prova de integração temporal sistêmica completa.

Esta lista é uma fotografia documental. Antes de alterar código, confirmar os arquivos, testes e workflows no SHA atual.

## 3. Precisão temporal — somente quando necessária

Detalhes adicionais entram no runtime somente se um console ou periférico suportado depender deles:

| Recurso | Regra |
|---|---|
| M-cycles / T-states | Usar quando necessários para ordenar uma interação observável |
| WAIT | Implementar conforme a ligação e o uso real na plataforma |
| INT / NMI | Preservar os instantes de amostragem que alterem o comportamento do console |
| BUSREQ / BUSACK | Não implementar por completude abstrata; exigir consumidor real |
| RETI / daisy chain | Implementar notificação externa se houver controlador compatível no sistema suportado |
| M1 / RFSH e outros sinais | Modelar apenas se algum consumidor depender deles |
| Pinos físicos | Não exigir representação individual nem atualização universal por clock |
| Estados elétricos/analógicos | Fora do escopo |

Uma representação diagnóstica de sinais pode ser derivada do estado do core, mas nunca deve criar uma segunda CPU ou caminho de execução.

## 4. Testes e validação

- Testes unitários por instrução/família e por estado arquitetural alterado.
- Testes de integração com os barramentos e periféricos dos consoles suportados.
- Testes de regressão SG-1000 e SMS.
- Testes diferenciais e ROMs de diagnóstico quando agregarem evidência independente.
- Testes temporais detalhados apenas para interações cuja ordem/duração altere o resultado observável.
- Desempenho Release medido quando houver mudança no caminho crítico.

ZEXDOC, ZEXALL, traces completos de pinos e validação de silício podem ser ferramentas auxiliares, não gates universais de produto. Nenhum resultado de teste deve ser declarado sem execução no SHA correspondente.

## 5. Trabalho temporal em pausa

As PRs #512, #514 e #526 permanecem abertas/Draft na consulta de 2026-11-05 e não estão integradas na main. Seus contratos de M-cycle/T-state e timeline racional não são tarefas ativas até que uma dependência concreta seja demonstrada sob a ADR-0016.

A PR #527 é uma proposta documental não integrada. Não usar os relatórios dessa PR como autoridade normativa.

## 6. Regra de atualização

Atualizar este scorecard somente quando uma mudança integrada, um teste executado ou uma evidência de hardware alterar o estado de um critério funcional relevante. Não criar fases, percentuais de completude ou bloqueios de precisão elétrica sem necessidade comprovada.
