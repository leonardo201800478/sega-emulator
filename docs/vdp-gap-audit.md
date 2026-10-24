# Auditoria de lacunas do VDP TMS9918A — etapa inicial

## Escopo desta PR

Esta etapa adiciona regressões para três comportamentos do renderizador já implementado: colisão entre pixels opacos de sprites, overflow quando mais de quatro sprites intersectam uma linha e limpeza dos flags ao ler a porta de status.

## Lacunas ainda abertas

- validar os limites e prioridades de sprites contra testes independentes;
- cobrir integralmente as combinações de modos gráficos e máscaras de endereçamento;
- confirmar a semântica de interrupção de VBlank e de leitura/limpeza do status;
- separar temporização de linha/quadro da renderização final, com eventos de hardware reproduzíveis;
- modelar mudanças de registradores e acessos VRAM em momentos específicos do quadro;
- validar paleta, clipping e comportamento de sprites com uma referência independente;
- corrigir sincronização CPU–VDP: atualmente o relógio avança por T-states nominais acumulados, não por acessos de barramento/ciclos individuais.

A suíte de regressão não comprova precisão ciclo a ciclo nem substitui a comparação de imagens com um emulador de referência.
