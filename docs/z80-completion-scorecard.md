# Z80 compartilhado — scorecard de conclusão

**Baseline reconciliada:** main em `1da784f556b9524f5a6f43ce8a7c525e68354f9e` (2026-11-05, merge PR #536); fotografia vinculada a esse SHA.  
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
- A entrega de VBlank IRQ e Line IRQ do SMS ao Z80 está coberta pelos testes de integração da PR #531, integrada nesta baseline.
- A semântica do pino RESET do Z80 está coberta por testes unitários e de integração SMS na PR #536: PC/I/R zerados, IFF1/IFF2 desabilitados, IM 0, HALT/NMI/EI delay limpos; registradores gerais/alternativos, IX/IY, SP, linha INT externa e T-states da máquina preservados.
- A duração elétrica mínima do pulso RESET não é simulada: o reset de host é uma transição atômica de estado, conforme ADR-0016; não há consumidor de gameplay demonstrado para a largura do pulso.
- A subquestão de flags de I/O em bloco foi caracterizada para o perfil Zilog NMOS por pesquisa documental/experimental e comparação do executor; CMOS/clones permanecem não comprovados. A auditoria semântica global E7.7 continua aberta. Ver [auditoria complementar de flags de I/O em bloco](audits/z80-e7-block-io-flags-2026-11-05.md).
- O caminho de barramento/temporização está implementado apenas em fatias; não há prova de integração temporal sistêmica completa.

Esta lista é uma fotografia histórica. Antes de alterar código ou declarar qualquer critério concluído, confirmar os arquivos, testes, PRs e workflows no SHA atual da `main`. PR aberta ou Draft não equivale a implementação integrada.

## 3. Política temporal do Z80 — prioridade de gameplay e baseline atômica

A semântica de cada instrução suportada deve ser correta, independentemente do caminho de execução. A implementação atômica é suficiente como padrão para instruções cujo estado intermediário não seja observável pelo console. A precisão temporal detalhada é prioritária para sequências executadas por jogos quando a ordem/duração altera uma interação com o hardware.

| Caminho | Nível obrigatório | Critério |
|---|---|---|
| ALU/registradores internos sem acesso externo | Atômico | Resultado, flags, PC/R e duração nominal correta; sem microfases obrigatórias |
| Fetch/decode e prefixos | Atômico por instrução como base | Bytes consumidos, seleção de prefixo, PC/R e semântica exatos; detalhar M1 somente se consumidor exigir |
| Acesso a RAM/ROM sem contenção ou efeito temporal | Atômico | Endereço, dado, side effects e duração nominal |
| Escritas/leitura de I/O e registradores VDP/PSG | Temporal prioritário quando o instante afeta gameplay | Ordenação relativa à CPU/VDP/áudio e efeito no instante correto |
| Mapper/banco de cartucho | Atômico por transação, temporal se a troca interagir com execução em andamento | Estado de banco e ordem observável corretos |
| INT/NMI, EI delay, HALT, acknowledge e leitura de status | Temporal nos pontos de amostragem/aceitação observáveis | Prioridade, fronteira de instrução e efeitos colaterais corretos |
| Raster/H-counter, scroll e writes intralinha | Temporal quando jogos usam a interação | Reproduzir a menor fronteira temporal demonstrada |
| WAIT/arbitragem/DMA | Condicional | Exigir ligação real no console e efeito reproduzível |
| Pinos e fases elétricas | Fora do escopo | Não são requisito de conclusão |

**Regra de leitura do scorecard:** `IMPLEMENTED_ATOMIC` pode satisfazer integralmente o requisito funcional de uma família. `NOT_IMPLEMENTED` no microsequenciador indica apenas ausência daquele caminho temporal, não ausência da instrução no executor atômico. Só registrar uma lacuna temporal como bloqueio quando houver consumidor de gameplay e evidência que exija essa granularidade.

Uma representação diagnóstica de sinais pode ser derivada do estado do core, mas nunca deve criar uma segunda CPU ou caminho de execução.

## 4. Testes e validação

- Testes unitários por instrução/família e por estado arquitetural alterado.
- Testes de integração com os barramentos e periféricos dos consoles suportados.
- Testes de regressão SG-1000 e SMS.
- Testes diferenciais e ROMs de diagnóstico quando agregarem evidência independente.
- A nova matriz `z80_ed_block_io_flag_reference_model.rs` cobre 48.000 combinações determinísticas com um modelo de referência derivado de pesquisa NMOS; não é oráculo de silício e só será PASS após CI no HEAD candidato.
- Testes temporais detalhados apenas para interações cuja ordem/duração altere o resultado observável.
- Desempenho Release medido quando houver mudança no caminho crítico.

ZEXDOC, ZEXALL, traces completos de pinos e validação de silício podem ser ferramentas auxiliares, não gates universais de produto. Nenhum resultado de teste deve ser declarado sem execução no SHA correspondente.

## 5. Trabalho temporal em pausa

As PRs #512, #514 e #526 permanecem abertas/Draft na consulta de 2026-11-05 e não estão integradas na main. Seus contratos de M-cycle/T-state e timeline racional não são tarefas ativas até que uma dependência concreta seja demonstrada sob a ADR-0016.

A PR #527 é uma proposta documental não integrada. Não usar os relatórios dessa PR como autoridade normativa.

## 6. Regra de atualização

Atualizar este scorecard somente quando uma mudança integrada, um teste executado ou uma evidência de hardware alterar o estado de um critério funcional relevante. Não criar fases, percentuais de completude ou bloqueios de precisão elétrica sem necessidade comprovada.
