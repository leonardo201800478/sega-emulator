# Auditoria geral do código

**Data:** 22/10/2026  
**Branch de revisão:** `docs/codebase-audit-2026-10`  
**Base auditada:** branch `main` no início desta revisão  
**Natureza:** revisão estática de código, testes, documentação e configuração. Não equivale a validação contra hardware real.

## Resumo executivo

O repositório tem uma base Python pequena, determinística e testável: RAM de 1 KiB espelhada, barramento com ROM de cartucho e um subconjunto executável do Z80. O código delimita várias limitações importantes e rejeita opcodes ainda não implementados em vez de simular sucesso silenciosamente.

O estado é adequado para continuar o desenvolvimento incremental do SG-1000, mas **ainda não é um emulador funcional completo**. Não existe integração de máquina que conecte CPU, barramento, VDP, PSG, portas de I/O, entrada e saída audiovisual. A CPU não implementa todos os opcodes, interrupções ou execução ciclo a ciclo.

Na revisão estática dos módulos listados abaixo, não foi confirmado um defeito aritmético que justifique uma alteração imediata no núcleo Z80. Isso não prova correção integral: a cobertura continua parcial e os testes atuais são predominantemente sintéticos.

## Escopo inspecionado

- `src/sega_emulator/__init__.py`: versão do pacote.
- `src/sega_emulator/__main__.py`: ponto de entrada.
- `src/sega_emulator/bus/interfaces.py`: protocolo estrutural de barramento.
- `src/sega_emulator/bus/memory.py`: armazenamento e espelhamento da RAM.
- `src/sega_emulator/bus/memory_bus.py`: mapeamento inicial de ROM/RAM e validações.
- `src/sega_emulator/cpu/z80.py`: registradores, flags, helpers e decodificação do subconjunto Z80.
- `tests/test_smoke.py`, `tests/unit/test_memory.py`, `tests/unit/test_memory_bus.py`, `tests/unit/test_memory_bus_interface.py` e `tests/unit/test_z80.py`.
- `pyproject.toml`, `.gitignore`, `README.md`, `docs/architecture.md`, `docs/hardware/sg1000.md`, `docs/z80-coverage.md` e `docs/firmware-inventory.md`.

## Achados e prioridades

### P0 — Nenhum defeito crítico confirmado nesta revisão estática

Não foi feita alegação de compatibilidade geral, precisão ciclo a ciclo ou validação contra hardware real. A ausência de um defeito confirmado não deve ser interpretada como prova de ausência de bugs.

### P1 — Falta integração de uma máquina executável

**Observação:** o ponto de entrada só informa o estado do projeto. A CPU e a memória são componentes isolados; não há ciclo principal que avance CPU e periféricos de forma coordenada.

**Impacto:** instalar e executar o pacote não inicia um jogo nem demonstra compatibilidade SG-1000.

**Ação:** manter o ponto de entrada explicitamente descrito como scaffold até existir uma integração mínima e testada. A mensagem foi ajustada para distinguir “subconjunto de componentes implementado” de “máquina integrada”.

### P1 — CPU Z80 incompleta

**Observação:** o decoder cobre famílias selecionadas, mas deixa instruções-base e prefixos por implementar. Entre as lacunas importantes estão HALT, CB, IX/IY, muitas instruções ED, I/O e controle de interrupções DI/EI/IM/RETI/RETN.

**Impacto:** programas que usem instruções ausentes terminam com `UnsupportedOpcodeError`; ROMs comerciais não devem ser usadas como critério único de validação.

**Ação:** usar `docs/z80-coverage.md` como inventário vivo, implementar lotes coerentes, e testar resultado, flags, PC/SP, wraparound e T-states nominais por família.

### P1 — Temporização ainda não é ciclo a ciclo

**Observação:** `step()` retorna uma quantidade nominal de T-states por instrução e soma em `total_cycles`; as leituras/escritas do barramento não são agendadas individualmente nesses ciclos.

**Impacto:** não há base para alegar contenção, estados de espera, sincronização com VDP/PSG ou precisão temporal de hardware.

**Ação:** antes de adicionar periféricos temporizados, definir um modelo explícito de avanço temporal e de eventos de barramento, mantendo o núcleo determinístico.

### P1 — Mapa de memória é uma política inicial, não um decodificador completo

**Observação:** a imagem de cartucho é mapeada linearmente desde `0x0000`, sem banking ou espelhamento; lacunas são consideradas não mapeadas. Escritas na ROM geram `ReadOnlyMemoryError`.

**Impacto:** esse comportamento torna erros de software visíveis, mas não representa todos os efeitos elétricos de escrita em ROM nem todas as configurações de cartucho.

**Ação:** preservar a política atual enquanto ela ajuda os testes; ao ampliar o hardware, separar o comportamento de diagnóstico do modelo físico e documentar banking, espelhamento e expansões conforme evidência.

### P2 — Testes são uma boa base, mas não medem cobertura de hardware completa

**Observação:** os testes de CPU usam programas sintéticos curtos e verificam famílias e casos selecionados. Há testes de RAM/barramento, mas não existe ainda uma suíte de diagnóstico Z80 de referência nem testes de integração de uma máquina SG-1000 completa. O teste de armazenamento físico da RAM acessa o atributo privado `_data`.

**Impacto:** 220 testes aprovados demonstram que os casos codificados passam; não provam compatibilidade com todas as entradas, instruções ou temporizações.

**Ação:** acrescentar testes de limites e propriedades por família, testes de integração quando existir uma máquina, e diagnósticos de referência legalmente obtidos. Preferir verificar o comportamento público da RAM a depender de `_data` quando isso não exigir expor detalhes internos.

### P2 — Ausência de integração contínua confirmada na configuração revisada

**Observação:** `README.md` identifica CI como pendência e a configuração inspecionada não define workflow de CI.

**Impacto:** os testes e linters dependem da execução manual local.

**Ação:** adicionar workflow Windows/Python compatível com a versão-alvo para executar pytest, Ruff check e Ruff format check. Fazer isso como uma mudança isolada, depois de confirmar os requisitos de runners disponíveis.

### P2 — Licença do projeto ainda pendente

**Observação:** o README deixa a escolha de licença como decisão pendente.

**Impacto:** os direitos de uso, modificação e redistribuição do código do próprio projeto ficam indefinidos. A licença do emulador também não concede direitos sobre ROMs comerciais ou BIOS.

**Ação:** decidir a licença antes de publicar distribuições ou aceitar contribuições externas; manter ROMs e firmware protegidos fora do repositório.

### P3 — Decoder Z80 concentrado em um único módulo

**Observação:** `z80.py` reúne estado dos registradores, flags, operações aritméticas, pilha, condições e decodificação de instruções.

**Impacto:** o arquivo crescerá com os prefixos, aumentando o custo de revisão e o risco de regressão. No tamanho atual, uma divisão prematura pode criar abstrações sem benefício demonstrado.

**Ação:** não refatorar por estética agora. Reavaliar a separação do decoder por famílias quando a expansão de CB/ED/DD/FD tornar a manutenção significativamente mais difícil; preservar a API e testes de equivalência.

## Avaliação por componente

| Componente | Estado observado | Limite principal |
|---|---|---|
| Pacote e ambiente | Python 3.14, setuptools, pytest e Ruff fixados no `pyproject.toml` | Python 3.14 é o intervalo declarado; outras versões não estão cobertas |
| RAM | 1024 bytes, inicialização explícita, endereços de RAM validados e espelhamento de 1 KiB | Não modela estado elétrico de power-on ou temporização |
| Barramento | ROM opcional, RAM espelhada, erros explícitos para regiões não mapeadas e escrita em ROM | Sem dispositivos de I/O, wait states, banking ou efeitos de barramento |
| Contrato `MemoryBus` | Protocolo Python com leitura/escrita de byte | Só expressa operações lógicas; não expressa tempo ou interrupções |
| CPU Z80 | Subconjunto determinístico com registradores alternativos, flags, pilha, saltos, ALU e algumas instruções ED | Conjunto de instruções e interrupções incompletos; `step()` não é ciclo a ciclo |
| Documentação | README, arquitetura, notas SG-1000, inventário Z80 e firmware local | Deve continuar sincronizada com cada lote e diferenciar implementado de validado |

## Resultado de qualidade disponível

Na validação local reportada em 22/10/2026, antes desta alteração documental:

- `pytest`: **220 passed**.
- `ruff check .`: **All checks passed!**
- `ruff format --check .`: **18 files already formatted**.
- `git status --short --branch`: `main...origin/main`, sem alterações pendentes.

Esses resultados foram fornecidos pelo usuário para o estado anterior à presente revisão. **Não foram executados novamente neste ambiente após as alterações desta branch.**

## Sequência recomendada

1. Integrar a auditoria à documentação principal e manter esta lista como referência de riscos.
2. Prosseguir com uma família de instruções Z80 coerente, ampliando testes e matriz de cobertura na mesma mudança.
3. Adicionar testes de diagnóstico confiáveis e legais e comparar os resultados com o núcleo.
4. Definir o modelo temporal antes de integrar VDP, PSG e I/O.
5. Integrar a primeira máquina SG-1000 mínima e testar CPU, memória e dispositivos em conjunto.
6. Adicionar CI e decidir a licença em mudanças separadas.

## Critérios para encerrar uma futura auditoria

- [ ] Cada opcode oficial e prefixo aplicável tem estado documentado.
- [ ] Cada instrução implementada tem testes de resultado, flags, PC/SP, acessos à memória e T-states nominais.
- [ ] Os casos de wraparound e ramos tomados/não tomados estão cobertos.
- [ ] Diagnósticos de referência passam ou têm divergências registradas.
- [ ] O mapa de memória e os efeitos de I/O correspondem à configuração de hardware documentada.
- [ ] A máquina integrada avança CPU e dispositivos de forma determinística.
- [ ] A precisão declarada corresponde aos testes efetivamente executados.
