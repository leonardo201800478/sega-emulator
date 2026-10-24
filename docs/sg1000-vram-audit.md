# Auditoria técnica da VRAM e do caminho CPU–VDP (SG-1000)

**Status:** auditoria inicial; não é uma declaração de conformidade completa com o hardware.  
**Escopo:** implementação publicada de `src/sega_emulator/video/tms9918.py` na branch `main`, observada em 2026-10-24.  
**Objetivo:** registrar o que já existe, as lacunas confirmadas e um plano de validação incremental antes de corrigir comportamentos ou otimizar o caminho crítico.

## 1. Resumo executivo

O VDP atual possui uma VRAM de 16 KiB representada por `bytearray(0x4000)`, registradores, portas de dados/controle, buffer de leitura, renderização de modos gráficos e avaliação de sprites. Isso constitui uma base funcional, mas não demonstra que todos os detalhes do TMS9918A estejam corretos.

A implementação declara explicitamente que o avanço temporal é feito em T-states nominais do Z80 e que ainda não modela acessos à VRAM por pixel, contenção ou temporização elétrica do barramento. A imagem é composta como quadro inteiro, e os sprites são avaliados durante essa composição. Portanto, não se deve afirmar precisão por ciclo ou equivalência integral ao hardware.

O relato de que o fundo parece correto enquanto os sprites móveis ficam desorganizados direciona a investigação inicialmente para atributos/padrões de sprites, integração CPU–VDP e temporização. Não prova que a VRAM seja a causa, nem exclui erros de endereçamento ou protocolo.

## 2. Modelo atual observado no código

Arquivo principal: `src/sega_emulator/video/tms9918.py`.

### 2.1 Memória e registradores

- VRAM: `bytearray(0x4000)` (16 KiB).
- Registradores: `bytearray(8)`.
- Endereço de VRAM limitado a 14 bits com máscara `0x3FFF`.
- Porta de dados: `0xBE`.
- Porta de controle/status: `0xBF`.
- Buffer de leitura: `read_buffer`.
- Latch de primeiro byte do comando: `_control_first`.

### 2.2 Operações implementadas

- Escrita na porta de dados escreve no endereço atual e incrementa o endereço, com retorno ao espaço de 14 bits.
- Leitura na porta de dados devolve o valor do buffer, carrega o próximo byte do endereço atual e incrementa o endereço.
- A escrita de dois bytes na porta de controle seleciona escrita/leitura de VRAM ou escrita em registrador.
- A leitura de status retorna o status, repõe o valor para `0x1F` e limpa o latch de comando.
- O avanço temporal conta 228 T-states por linha e 262 linhas por quadro, usando esses valores nominais no modelo atual.
- A renderização de quadro lê a VRAM para compor modos gráficos e sprites.

Esses itens descrevem o código presente; a sua presença não prova que todas as sequências estejam conformes à documentação do chip.

## 3. Estado por área

| Área | Estado observado | O que falta provar |
|---|---|---|
| Capacidade da VRAM | Implementada como 16 KiB | Se o acesso e os efeitos observáveis correspondem à revisão de hardware pretendida |
| Incremento e wrap de endereço | Máscara de 14 bits presente | Sequências de fronteira e comandos alternados |
| Protocolo de leitura | Buffer de leitura antecipada implementado | Ordem exata dos bytes em todas as sequências de acesso |
| Protocolo de escrita | Escreve no endereço e incrementa | Efeito da escrita sobre o buffer de leitura, conforme documentação primária |
| Registradores | Oito bytes; comando de escrita implementado | Máscaras, bits não implementados e efeitos colaterais por registrador |
| Tabelas gráficas | Endereçamento calculado em cada modo | Casos de fronteira, sobreposição e bits de endereço por modo |
| Sprites | Atributos, padrões, transparência, colisão e overflow básicos | Prioridade, limites, 16×16, escala, status e comportamento por scanline |
| Temporização | Avanço em T-states nominais | Eventos no instante correto e sincronização com as operações de barramento |
| Desempenho | Renderização por pixels com criação repetida de objetos RGB | Perfil real antes de escolher otimizações |
| Determinismo | Estado e renderização em memória | Testes repetidos e regressões de estado após leituras/escritas |

## 4. Ponto específico a investigar: buffer de leitura após escrita

No código atual, `write_port(0xBE, value)` grava em `vram[address]`, incrementa `address` e limpa `_control_first`, mas não altera explicitamente `read_buffer`.

Isso é um **ponto de investigação**, não uma conclusão definitiva de bug. Antes de alterar o comportamento, conferir os manuais técnicos primários fornecidos ao projeto e definir a sequência exata esperada para:
1. configurar um endereço de leitura;
2. ler o primeiro byte após o ciclo de pré-carregamento;
3. escrever em um endereço;
4. iniciar outra leitura;
5. alternar comandos de controle e operações de dados.

A correção só deve ser feita depois de registrar o comportamento documentado e escrever um teste que falhe na implementação atual e passe com a correção.

## 5. Relação com os sprites móveis

O fundo aparentemente correto é evidência útil, mas limitada: mostra que parte do endereçamento de tabelas e da conversão de padrões em pixels funciona para o conteúdo testado. Não valida todos os modos, limites de endereço ou sequências de acesso.

O renderizador atual avalia os sprites em `_render_sprites()` ao compor o quadro. Nesse método, são lidos os atributos e padrões diretamente da VRAM, calculados tamanho e escala, contados sprites por linha e registrados colisão/overflow. Esse processamento não está distribuído temporalmente pelo instante de cada scanline.

Hipóteses a testar, sem escolher prematuramente uma causa:
1. bytes de posição, padrão e cor da tabela de atributos interpretados incorretamente;
2. cálculo de padrão para sprites 8×8 ou 16×16 incorreto;
3. escritas da CPU na VRAM/registradores em ordem ou instante incorretos;
4. status, interrupções ou eventos de VBlank não sincronizados com o software;
5. ausência de avaliação por scanline causando divergência quando o jogo altera atributos durante o quadro;
6. falhas funcionais do Z80 que alterem os dados escritos pelo jogo;
7. lentidão do renderizador dificultando testes, mas sem assumir que desempenho baixo sozinho causa coordenadas lógicas erradas.

## 6. Plano de testes

### Fase A — testes curtos e determinísticos de VRAM

Adicionar testes unitários isolados para:
- escrita e leitura de um byte;
- leitura com buffer e comportamento do primeiro byte;
- escrita/leitura em sequência de vários bytes;
- incremento de endereço;
- wrap de `0x3FFF` para `0x0000`;
- configuração de endereço de leitura e escrita via dois bytes de controle;
- alternância entre comandos de registrador e de VRAM;
- leitura de status limpando os bits previstos e o latch;
- entradas inválidas, se a API pública exigir validação.

Os valores esperados devem vir do manual técnico do TMS9918A ou de testes de referência independentes. Não codificar expectativas baseadas apenas no comportamento atual.

### Fase B — testes de endereçamento gráfico

Criar casos sintéticos que posicionem padrões e tabelas em endereços controlados e verifiquem os pixels resultantes. Cobrir:
- Graphics I;
- Graphics II, incluindo as três regiões verticais;
- Text;
- Multicolor;
- sprites 8×8;
- sprites 16×16, incluindo alinhamento/seleção de padrões;
- transparência, prioridade, colisão e overflow;
- limites da VRAM e registradores de base.

Comparar resultados com documentação e, quando disponível, imagens de referência geradas por um emulador confiável ou teste de hardware. Registrar a origem da referência.

### Fase C — integração CPU–VDP

- Inspecionar a rotina de execução da CPU e o ponto onde os T-states avançam o VDP.
- Confirmar que cada instrução e operação de E/S avança o tempo uma única vez, sem omissões nem dupla contagem.
- Testar escrita em registradores e VRAM em sequências reproduzíveis.
- Testar a sinalização de VBlank e a leitura de status.
- Criar uma regressão reproduzível para sprites móveis, com entradas e estado inicial fixos.
- Comparar o conteúdo da tabela de atributos antes e depois dos quadros para distinguir dados errados de interpretação errada.

### Fase D — temporização mais precisa

Depois de as operações funcionais estarem cobertas:
- definir o modelo temporal de scanline e os eventos do VDP;
- determinar quais mudanças em VRAM/registradores precisam ter efeito no quadro atual e em qual instante;
- modelar apenas as restrições de acesso que forem documentadas ou demonstradas por testes;
- evitar afirmar precisão de ciclo sem testes específicos para essa propriedade.

### Fase E — desempenho

Medir separadamente:
- tempo de execução da CPU;
- custo de `advance()`;
- custo de `render_frame()`;
- custo de renderização do fundo;
- custo de renderização dos sprites;
- custo de conversão RGB e apresentação no frontend.

A implementação atual chama `_put_pixel()` muitas vezes, e essa função cria `bytes(PALETTE[color & 0x0F])` por pixel. Isso é um candidato a profiling, não prova de gargalo. Só otimizar depois de medir, mantendo testes de equivalência de framebuffer e determinismo.

## 7. Critérios de aceitação

Não declarar a VRAM completamente validada até que:
- os testes de protocolo cubram leitura, escrita, buffer, incremento e wrap;
- os resultados esperados estejam ligados a uma fonte técnica ou referência independente;
- os testes de endereçamento cubram os modos e casos de fronteira relevantes;
- as regressões de sprites sejam reproduzíveis e passem;
- a integração CPU–VDP tenha uma contabilidade de ciclos testada;
- testes determinísticos demonstrem resultados iguais para o mesmo estado e sequência de operações;
- os testes automatizados relevantes tenham sido realmente executados e seus resultados registrados.

Não declarar precisão por ciclo até existir um conjunto explícito de testes temporais e evidências que a sustentem.

## 8. Separação entre fatos, hipóteses e pendências

**Fatos observados no código:** tamanho da VRAM, portas, buffer de leitura, máscara de endereço, cálculo nominal de linhas e renderização de sprites no fim do quadro.

**Hipóteses ainda não confirmadas:** efeito exato da escrita sobre o buffer de leitura; causa dos sprites desorganizados; impacto de cada custo de renderização; necessidade e granularidade de modelagem de contenção para os casos-alvo.

**Pendências:** consultar os manuais técnicos anexados e referenciar as páginas relevantes; criar e executar os testes da Fase A; inspecionar a integração CPU–VDP; obter uma reprodução determinística do problema de sprites; medir desempenho antes de otimizar.

## 9. Próxima etapa recomendada

Começar pela **Fase A**, em uma alteração pequena e isolada: testes unitários para o protocolo de portas e VRAM, acompanhados de referências documentais. Não modificar ainda o renderizador inteiro, não introduzir um tilemap de sprites e não declarar o problema de sprites resolvido antes de uma reprodução e de uma regressão que demonstrem a correção.

## Referência de implementação

- Código auditado: [`src/sega_emulator/video/tms9918.py`](../src/sega_emulator/video/tms9918.py)
- Este documento registra a auditoria inicial e deve ser atualizado conforme testes, evidências e correções forem confirmados.
