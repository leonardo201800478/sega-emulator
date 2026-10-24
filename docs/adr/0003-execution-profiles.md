# ADR 0003 — Perfis de execução e fronteiras de precisão

- **Status:** Aprovada para implementação incremental.
- **Data:** 2026-10-24
- **Escopo inicial:** SG-1000, CPU Z80; os demais subsistemas são políticas declarativas até que existam implementações e testes.

## Objetivo

Oferecer uma escolha explícita entre fidelidade de referência e desempenho, sem habilitar aproximações temporais ou HLE silenciosamente. A configuração solicitada para desempenho é CPU LLVM JIT, VDP rápido, PSG HLE e temporização balanceada. Ela é um preset opt-in, não o padrão global.

## Presets

| Perfil | CPU | VDP | Áudio | Temporização |
| --- | --- | --- | --- | --- |
| `accurate` (padrão) | Interpretador Python de referência | Referência | Modelo de chip PSG | Estrita |
| `performance` (opt-in) | LLVM JIT Z80 | Caminhos rápidos explicitamente validados | HLE seletiva, se implementada | Balanceada, em pontos seguros |

Um preset expressa a intenção do usuário. O runtime só pode ativar uma função se declarar a capacidade correspondente. Enquanto um backend não estiver implementado e validado, a resolução deve voltar ao caminho de referência e expor o motivo.

## Definições operacionais

### CPU: interpretador e LLVM JIT

- Preservar `Z80Cpu` Python como oráculo de referência e fallback obrigatório.
- Traduzir blocos básicos para LLVM IR, inicialmente um subconjunto de opcodes Z80 bem delimitado.
- Encerrar blocos em saltos, HALT, instruções prefixadas ainda não suportadas, I/O, acesso a barramento com efeitos observáveis, interrupções e pontos de sincronização.
- Instruções não suportadas, contexto de memória não identificável e código auto-modificável devem retornar ao interpretador.
- A invalidação de cache deve cobrir qualquer escrita que possa modificar bytes de código traduzidos.
- Testes diferenciais devem comparar estado de registradores, flags, PC, HALT, ciclos nominais e efeitos de memória/I/O. Divergência desativa o caminho JIT no caso testado; nunca se deve corrigir o oráculo para corresponder ao JIT sem evidência independente.

### VDP: modo rápido

- Não aplicar HLE ao VDP como substituição geral.
- Primeiro perfilar e preservar a semântica da implementação de referência.
- Otimizações nativas são permitidas para rotinas puras de dados quando os resultados puderem ser comparados bit a bit.
- Qualquer aproximação de rasterização, acesso à VRAM, colisões, interrupções ou status de linha deve ser uma opção separada, documentada e coberta por testes de regressão visual/temporal.
- A temporização balanceada pode reduzir despacho/sincronização apenas em fronteiras demonstradas seguras; não altera a frequência lógica do hardware.

### PSG: HLE seletiva

- HLE não é sinônimo de gerar som simplificado. O caminho padrão mantém o modelo de chip e seus registradores.
- O preset pode selecionar HLE apenas para comportamentos de software ou firmware identificados, com condições de ativação explícitas e fallback para emulação do chip.
- Não ignorar escritas de registradores, divisores, ruído ou estado de osciladores.
- Validar por frequências, formas de onda, amostras e regressões espectrais quando houver uma implementação de HLE concreta.

### Temporização

- `strict`: manter os pontos de sincronização existentes e não introduzir aproximações novas.
- `balanced`: permitir agrupamento de trabalho apenas quando os efeitos observáveis e a ordem de eventos forem preservados.
- `relaxed`: permitir aproximações temporais declaradas e específicas; nunca é ativado implicitamente.
- Nenhum modo deve distribuir CPU, VDP e PSG em threads independentes sem um modelo de sincronização explícito.

## Política de capacidades e padrão

- O padrão é sempre `accurate`.
- O preset `performance` pode ser selecionado sem afirmar que todos os componentes já existem.
- O runtime precisa anunciar individualmente as capacidades de LLVM Z80 JIT, VDP rápido, PSG HLE e temporização balanceada/relaxada.
- Recursos não anunciados caem para a referência e devem gerar um motivo legível.
- Não alterar o padrão para desempenho antes de testes diferenciais, regressões e benchmarks reproduzíveis.

## Critérios de aceitação por etapa

1. Infraestrutura LLVM ORC compila e executa no Windows x64.
2. Tradutor Z80 de subconjunto é ligado ao estado da CPU e comparado diferencialmente com `Z80Cpu`.
3. A cobertura diferencial inclui valores de fronteira, flags, wrap de 8/16 bits, desvios, HALT, memória e fallback.
4. Benchmark com workload e hash registrados demonstra ganho mensurável antes de habilitar JIT por padrão.
5. VDP rápido e PSG HLE só são habilitados quando seus próprios critérios de aceitação forem cumpridos.
