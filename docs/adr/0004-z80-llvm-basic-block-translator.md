# ADR 0004 — Tradutor incremental de blocos Z80 para LLVM ORC

- **Status:** Em implementação experimental.
- **Data:** 2026-10-25

## Contexto

A infraestrutura LLVM ORC existente valida a criação do LLJIT, a geração de IR
e a execução de código nativo, mas ainda não traduz instruções Z80. O núcleo
Python continua sendo a referência funcional e o núcleo Rust interpretativo
permanece um backend separado.

## Decisão

1. Implementar um tradutor de blocos básicos independente do interpretador Rust;
   reutilizar os testes diferenciais e o contrato de estado, não transcrever
   mecanicamente o código-fonte.
2. Começar por instruções sem acesso externo ao barramento: NOP, LD r,n
   para registradores e LD r,r sem (HL), mais HALT como terminador.
3. Preservar os T-states nominais conhecidos para esse subconjunto. A ausência
   de precisão de barramento ciclo a ciclo é uma limitação explícita, não uma
   licença para eliminar efeitos observáveis.
4. Recusar blocos que comecem com opcode não suportado, operando truncado ou
   operação de memória. Não executar parcialmente um bloco e depois pedir
   fallback a partir do PC original.
5. Não integrar ao runtime SG-1000 nem ativar por padrão até que o contrato de
   estado, side exits, invalidação de cache e testes diferenciais estejam
   definidos.

## Por que não compilar diretamente o código Rust existente?

O interpretador contém despacho dinâmico e chamadas a barramento. Compilá-lo
como um todo não equivale a traduzir os opcodes Z80 para blocos nativos. O JIT
precisa gerar código para as semânticas de instruções e preservar os efeitos de
memória, I/O, interrupções e sincronização. O LLVM ORC é a camada de compilação
e ligação dinâmica que permite executar esse código gerado.

## Etapas seguintes

- Testar o subset em Windows x64 com o workflow LLVM.
- Acrescentar flags aritméticas e instruções de controle de fluxo com limites
  de bloco explícitos.
- Definir um ABI de estado/barramento que possa ser compartilhado com Rust sem
  atravessar Python por instrução.
- Implementar side exits, detecção/invalidação de código auto-modificável e
  comparação diferencial contra Python e Rust.
- Medir cold-start, custo de compilação e throughput aquecido separadamente.

## Critério de desempenho

Não declarar ganho antes de benchmark reproduzível com workload idêntico,
aquecimento separado, assinaturas equivalentes e custo de compilação reportado.
