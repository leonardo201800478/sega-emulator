# Gate de conclusão do Z80 do Master System antes do patch Z80A do SG-1000

**Tipo:** plano bloqueado / PR de preparação.  
**Estado:** BLOCKED — não implementar nem ativar o patch SG-1000 enquanto todos os critérios abaixo não estiverem comprovados.  
**Base registrada:** `main` em `315d665d4b8295fc5427976249db136c587ae37b` (merge da PR #156, 2026-10-28).

## Objetivo

Preparar a futura mudança que completará o Z80A do SG-1000 somente depois que o núcleo Z80 do Master System tiver cobertura funcional completa e evidência reproduzível. Esta PR registra o gate e não altera CPU, barramento, temporização, frontend ou runtime de nenhuma máquina.

O patch futuro deverá ser pequeno, baseado em diferenças verificadas entre o contrato do Z80A usado no SG-1000 e a implementação Z80 já validada no SMS. Não duplicar um segundo núcleo completo nem reescrever componentes sem necessidade.

## Regra de ativação

A futura implementação do patch SG-1000 só pode começar quando todos os itens obrigatórios de **Z80 SMS — gate de conclusão** estiverem marcados como PASS e houver evidência associada ao SHA exato validado. Uma suíte verde, por si só, não demonstra completude se faltarem famílias de instruções, estados ou cenários de interrupção.

Até lá:
- manter esta PR como **Draft**;
- não adicionar o patch do SG-1000 a esta branch;
- não converter a PR para Ready for review;
- não mesclar esta PR como se o gate técnico já tivesse sido cumprido.

O GitHub não converte uma Draft PR em Ready automaticamente com base em critérios escritos no Markdown. A transição deverá ser manual após a revisão das evidências; se for desejada uma trava automatizada, será necessário implementar e validar um workflow explícito separado.

## Z80 SMS — gate de conclusão

### 1. ISA e semântica

- [ ] Inventariar a matriz completa de opcodes base e prefixados: CB, ED, DD, FD, DDCB e FDCB.
- [ ] Cada opcode definido para o modelo Z80 adotado tem implementação correta; aliases e instruções não documentadas são classificados e tratados deliberadamente, sem lacunas silenciosas.
- [ ] Verificar combinações de prefixos repetidos, prefixos ignorados, instruções indefinidas e comportamento de retorno adotado, com decisão documentada e fontes.
- [ ] Cobrir aritmética 8/16-bit, flags documentadas e não documentadas, DAA, BIT, operações de bloco, wraparound e efeitos colaterais em memória.
- [ ] Resolver ou justificar com evidência independente todas as divergências conhecidas. O vetor CPIR com divergência de flag X não pode continuar ignorado sem uma decisão técnica documentada e aprovada.

### 2. Estado da CPU e interrupções

- [ ] Validar registradores principais e alternativos, IX/IY, I/R, PC/SP, WZ/MEMPTR, Q e demais latches que façam parte do modelo.
- [ ] Testar reset, HALT, EI delay, DI, NMI, IRQ, modos IM 0/1/2, IFFs, acknowledge e retorno de interrupção.
- [ ] Testar limites de pilha, wraparound de endereço e ordem/efeitos de leituras e escritas.
- [ ] Garantir que snapshots/reset e execução repetida preservem determinismo.

### 3. Oráculos e cobertura diferencial

- [ ] Expandir os vetores independentes por família de instrução, condições de flags e limites; manter fixtures fixas, versionadas e com procedência/licença documentadas.
- [ ] Comparar estado final completo exposto, memória escrita nas regiões relevantes, PC/SP, flags/latches e T-states agregados.
- [ ] Nenhuma divergência inexplicada ou teste ignorado de comportamento relevante permanece no conjunto de aceitação.
- [ ] Documentar limitações do oráculo e investigar a primeira divergência; não gerar a expectativa pelo próprio código sob teste.
- [ ] Cobrir opcodes individualmente e combinações de prefixos; um contador de testes ou uma matriz de classificação não substitui a verificação semântica.

### 4. Temporalidade e barramento

- [ ] Documentar explicitamente a fidelidade temporal realmente implementada.
- [ ] Validar T-states por instrução contra referências independentes para os caminhos e condições suportados.
- [ ] Se o núcleo for declarado ciclo a ciclo, acrescentar testes que comparem sequência e instante das transações de barramento, WAIT/contensão quando aplicável e eventos de interrupção; sem isso, não declarar precisão ciclo a ciclo.
- [ ] Não confundir T-states agregados corretos com validação temporal completa do barramento.

### 5. Integração e não regressão

- [ ] Executar `cargo fmt --all -- --check`.
- [ ] Executar `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- [ ] Executar `cargo test --workspace --locked`.
- [ ] Executar os workflows aplicáveis do SMS e da CI geral no SHA candidato exato; verificar logs e não contar jobs ignorados como aprovação.
- [ ] Executar testes de integração da extensão PyO3/Maturin se a fronteira nativa for afetada.
- [ ] Confirmar que o SG-1000 permanece sem regressões e que a CPU SMS não depende do runtime do SG-1000.
- [ ] Registrar SHA, comandos/workflows, resultados PASS/FAIL/BLOCKED/NOT RUN e limitações.

## Trabalho permitido antes de liberar o gate

É permitido completar a auditoria e os testes do Z80 SMS, corrigir divergências e atualizar a documentação do SMS em PRs separadas. Não é permitido introduzir a mudança comportamental Z80A do SG-1000 antes de cumprir todos os critérios obrigatórios acima.

## Plano do patch Z80A do SG-1000, após aprovação

1. Confirmar SHA atualizado de `main`, branch e estado das PRs.
2. Auditar a implementação Z80A atualmente usada pelo SG-1000 e identificar as lacunas concretas, com testes que falhem antes da correção.
3. Definir a menor alteração compatível com o contrato existente, preservando integração e API.
4. Implementar o patch em branch/PR própria, sem misturar migração de linguagem e mudanças de fidelidade não relacionadas.
5. Rodar testes direcionados e toda a validação obrigatória Rust/Python pertinente; comparar o SG-1000 antes/depois e investigar a primeira divergência.
6. Mesclar somente após CI verde no SHA candidato, revisão do diff e evidência de ausência de regressões.

## Evidência atual

A matriz de referência registrada no roadmap SMS em 2026-10-28 contém 67 casos representados: 66 ativos e um caso CPIR explicitamente ignorado por divergência ainda não resolvida de flag não documentada X. A amostra cobre famílias relevantes, mas não é cobertura exaustiva da ISA. A validação anterior passou nos SHAs candidatos das PRs #146 e #156; isso não demonstra, por si só, completude funcional do Z80 nem validação ciclo a ciclo.

Portanto, o gate inicia **BLOCKED**. A contagem de vetores e o status dos workflows deverão ser atualizados com evidências no SHA atual antes de qualquer ativação.
