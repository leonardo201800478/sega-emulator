# Base de literatura técnica do Z80

**Projeto:** Sega Emulator — Emulação de Alta Precisão  
**Objetivo:** catálogo curado de fontes primárias, literatura técnica, pesquisa de engenharia reversa e suítes de validação para orientar a implementação do Z80.  
**Atualização:** 23/10/2026.

## 1. Fonte adicionada: Technical literature — redcode/Z80

- **Página de referência:** https://github.com/redcode/Z80/wiki/Technical-literature
- **Repositório associado:** https://github.com/redcode/Z80
- **Tipo:** índice bibliográfico comunitário com artigos, manuais, especificações, patentes e discussões técnicas.
- **Responsável pela página:** Manuel Sainz de Baranda y Goñi; a página indica 85 revisões e atualização em 03/03/2026.
- **Licença indicada na página:** GNU Free Documentation License 1.3 para o conteúdo da wiki. Isso não significa que todos os documentos externos ali listados tenham essa licença; verificar cada documento individualmente.
- **Uso no projeto:** fonte de descoberta e rastreamento bibliográfico. Não tratar todas as entradas como igualmente autoritativas ou como evidência experimental validada.

O projeto redcode/Z80 é também uma implementação de referência em C que declara suporte a comportamentos documentados e não documentados, MEMPTR, Q, reset especial e modo de interrupção IM 0. Seu README descreve testes históricos e comparações com resultados de hardware. Isso torna o projeto relevante para comparação comportamental, mas não elimina a necessidade de verificar variante, configuração de compilação, proveniência dos testes e licença.

## 2. Materiais prioritários do catálogo

### 2.1 Documentação oficial e especificações históricas

Consultar primeiro para estabelecer o comportamento documentado e a temporização da variante de referência:

- Zilog, *Z80 CPU User Manual* — diversas revisões, incluindo 2001, 2002, 2005, 2014 e 2016.
- Zilog, *Z80-CPU Technical Manual* (1976) e *Z80-CPU Z80A-CPU Technical Manual* (1977).
- Zilog, *Z80-CPU Z80A-CPU Product Specification* (1978).
- Zilog, *The Z80 Family Program Interrupt Structure* (1978).
- Zilog, *Z80 CPU Peripherals User Manual* (2001), para periféricos da família quando forem relevantes.
- Manuais e databooks de Mostek, Toshiba, Sharp e SGS-Thomson como fontes comparativas de variantes/clones, não como substitutos automáticos da documentação Zilog.

A lista é um índice: conferir edição, data, revisão, paginação, fabricante e se o PDF corresponde ao título antes de citar uma regra concreta.

### 2.2 Instruções e opcodes

Recursos listados na página:
- Cristian Dinu, *Decoding Z80 Opcodes*, incluindo revisões de 2004–2018.
- Jonathan Graham Harston, *Full Z80 Opcode List Including Undocumented Opcodes*.
- Jonathan Graham Harston, *Z80 Microprocessor Undocumented Instructions*.
- Jonathan Graham Harston, *Z80 Opcode Map*.
- Sean Young, *Z80 Undocumented Features (In Software Behaviour)* e *Undocumented Z80 Documented*.
- *Zilog Z80 Instructions Sorted by Opcode*.

**Uso:** cruzar o inventário de opcodes e prefixos com docs/z80-opcode-matrix.md e docs/z80-coverage.md. Esses documentos auxiliares não substituem o manual oficial nem a execução de testes independentes. Registrar revisão exata, URL e data de consulta para cada regra adotada.

### 2.3 Flags, registradores internos e comportamentos pouco documentados

Artigos e documentos destacados no índice:
- Ken Shirriff, *Down to the silicon: How the Z80's Registers Are Implemented*.
- Ken Shirriff, *The Z-80 Has a 4-Bit ALU. Here's How It Works*.
- Ken Shirriff, *The Z-80's 16-Bit Increment/Decrement Circuit Reverse Engineered*.
- Ken Shirriff, *Reverse-Engineering the Z-80: The Silicon for Two Interesting Gates Explained*.
- Ken Shirriff, *Why the Z-80's Data Pins Are Scrambled*.
- Achim Flammenkamp, *Interrupt Behaviour of the Z80 CPU*.
- David Banks, *Undocumented Z80 Flags*.
- *MEMPTR, Esoteric Register of the Zilog Z80 CPU*.
- *Z80 Special Reset*, de Tony Brewer, incluindo a observação de que há uma versão posterior do artigo.
- Discussões e testes sobre SCF/CCF, flags de bloco, MEMPTR, modo de interrupção 2 e diferenças de chips Z80 em plataformas Sega de 8 bits.

**Uso:** priorizar a pesquisa de flags X/Y, SCF/CCF, Q, MEMPTR/WZ, operações de bloco, IM 0/2, RETI/RETN, reset especial e diferenças NMOS/CMOS. Separar afirmações derivadas de análise de silício, resultados em hardware real, observações de software e hipóteses do autor. Não generalizar automaticamente um resultado de uma revisão para todos os chips.

### 2.4 Suítes e exercitadores citados pelo projeto redcode/Z80

O README do repositório redcode/Z80 menciona, entre outros:
- **Zilog Z80 CPU Test Suite**, de Patrik Rak: programas para verificar registradores, flags documentadas e não documentadas, SCF/CCF e MEMPTR.
- **Z80 Test Suite**, de Mark Woodmass: testes de MEMPTR e subconjuntos de opcodes CB/DD/FD.
- **Z80 Instruction Set Exerciser**, de Frank D. Cringle, incluindo variantes conhecidas como ZEXDOC/ZEXALL e adaptações posteriores.

Esses nomes são pontos de partida para pesquisa e não uma afirmação de que os arquivos de teste estejam incorporados ao nosso repositório ou que sua execução já tenha ocorrido. Antes de baixar, executar ou redistribuir qualquer suíte:
1. identificar a origem canônica e a revisão exata;
2. ler licença, avisos e condições de redistribuição;
3. registrar máquina/variante usada para produzir resultados de referência;
4. manter ROMs e imagens de teste fora do Git caso a licença/proveniência não permita redistribuição;
5. capturar saída, configuração, versão do emulador e primeiro estado divergente.

### 2.5 Patentes e dispositivos periféricos

A página indexa patentes como US4332008 (refresh e arquitetura), US4486827 (special reset) e US6502181B1 (CPU eZ80), além de especificações de Z80 CTC, DMA e SIO.

Usar patentes como fonte histórica e explicativa, sem presumir que o conteúdo descreva integralmente o comportamento observável de toda revisão do Z80. CTC/DMA/SIO pertencem ao escopo futuro de periféricos e não devem ampliar prematuramente o primeiro marco SG-1000.

## 3. Como usar esta base nas decisões de implementação

Para cada questão técnica:
1. **Formular a questão observável**, por exemplo: qual é o valor de F depois de SCF quando o opcode anterior alterou flags?
2. **Consultar a fonte primária** e registrar a revisão/página.
3. **Consultar a literatura de engenharia reversa** no índice redcode/Z80 e abrir o artigo ou documento original.
4. **Comparar implementações de referência** sem presumir independência entre projetos ou suites.
5. **Criar um teste discriminante** que separe as hipóteses concorrentes.
6. **Executar localmente testes curtos**; encaminhar baterias longas para o ambiente de execução remota já usado no projeto.
7. **Registrar o resultado**, incluindo variante-alvo, configuração, estado inicial, resultado esperado, resultado observado e artefato/log.
8. **Só então** decidir se o comportamento entra no núcleo.

Não transformar uma nota bibliográfica em requisito implementado sem evidência e teste.

## 4. Registro de evidências recomendado

Para cada achado, registrar no mínimo:

| Campo | Conteúdo |
|---|---|
| ID | Identificador estável, por exemplo Z80-LIT-001 |
| Questão | Comportamento específico investigado |
| Fonte | URL e título exato |
| Versão | Revisão, data ou commit consultado |
| Localização | Página, seção, artigo ou caso de teste |
| Tipo de evidência | Manual, artigo de silício, teste em hardware, teste de software, implementação ou inferência |
| Variante | Zilog NMOS/CMOS, clone, ou ainda desconhecida |
| Hipótese | Comportamento que a fonte sugere |
| Teste | Nome do teste e condições de entrada |
| Resultado | Aprovado, reprovado, divergente ou não executado |
| Decisão | Implementar, investigar mais, ou manter em aberto |
| Licença | Licença e limites de uso do material/teste |

## 5. Licenças e reutilização

A página de literatura agrega materiais de autores e titulares diferentes. A licença da wiki não se estende automaticamente a PDFs, artigos, patentes, ROMs de teste ou código externo.

O repositório redcode/Z80 declara licença **GNU Lesser General Public License (LGPL), versão 3 ou posterior**, com condições especiais de licenciamento mencionadas no README. Isso deve ser verificado no arquivo de licença e no estado atual do repositório antes de qualquer vinculação ou reutilização. Para o Sega Emulator, a abordagem padrão permanece reimplementar o comportamento a partir de requisitos e testes independentes; não copiar código C para o núcleo Python/Rust sem revisão de licença e aprovação explícita.

## 6. Relação com os documentos existentes

- docs/z80-zilog-manual-specification.md: base de requisitos derivados dos manuais ZiLOG já fornecidos.
- docs/z80-undocumented-behavior-research.md: análise de implementações e comportamentos pouco documentados; usar a página redcode/Z80 como índice complementar de fontes.
- docs/z80-opcode-matrix.md: classificação de opcodes e prefixos.
- docs/z80-coverage.md: estado de cobertura e validação.
- docs/z80-code-audit-2026-10-22.md: lacunas concretas do núcleo atual.
- docs/z80-mame-reference.md: referência de implementação independente para comparação arquitetural.

## 7. Referências externas

1. [redcode/Z80 — Technical literature](https://github.com/redcode/Z80/wiki/Technical-literature) — catálogo bibliográfico, artigos, manuais, patentes e discussões.
2. [redcode/Z80 — repositório do emulador](https://github.com/redcode/Z80) — implementação C, documentação, testes citados e informações de licença.
3. [README do redcode/Z80](https://github.com/redcode/Z80/blob/master/README) — descrição da arquitetura, opções de build/teste e suites mencionadas. Como master pode mudar, registrar um commit fixo ao fazer uma auditoria formal.

**Limitação desta entrada:** o catálogo foi inspecionado para identificar categorias e prioridades. Nem todos os documentos externos foram baixados, lidos integralmente ou verificados em hardware. A página deve ser usada como índice de consulta, e não como evidência automática para cada afirmação.
