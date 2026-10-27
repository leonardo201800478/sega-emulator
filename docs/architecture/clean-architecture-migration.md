# Clean Architecture — plano de consolidação

## Objetivo

Tornar inequívoco qual implementação é executada no produto: hardware e runtime oficiais em Rust, frontend Qt/PySide6 em Python e implementação Python antiga isolada como referência de testes. A organização deve impedir que alterações sejam feitas em um módulo que não participa do build/instalação usados pela aplicação.

Este documento define o destino e os critérios de migração. Não declara que a migração já foi concluída.

## Regras arquiteturais

1. **Domain / hardware (Rust):** estado e comportamento de CPU, memória, barramentos, I/O, VDP, PSG, controladores, interrupções, scheduler e sincronização pertencem aos crates de máquina e aos componentes Rust compartilhados.
2. **Application / machine runtime (Rust):** compõe os dispositivos de uma máquina, avança a execução, coordena entrada e entrega saídas em blocos. Não depende de Qt, Python ou PyO3.
3. **Adapters (Rust/PyO3):** traduz comandos de alto nível e expõe buffers/estado observável. Não implementa regras de hardware e não chama Python por instrução, byte, pixel ou amostra.
4. **Presentation / tools (Python):** Qt/PySide6, configuração, seleção/carregamento de ROM, logs de aplicação, ferramentas e automação. Não contém um segundo runtime de hardware usado pelo produto.
5. **Legacy reference (Python):** cores antigos preservados sob `legacy/python_reference/` para oráculos diferenciais e investigação. Não fazem parte do pacote de runtime nem são importados pela aplicação.
6. **Dependency rule:** dependências apontam para dentro: frontend → API/adaptador → runtime da máquina → componentes de hardware. Crates de hardware não dependem do frontend, da camada de aplicação ou do adaptador PyO3.
7. **Single source of truth:** a configuração raiz do Cargo define explicitamente os crates do workspace; a extensão tem manifesto/build próprios, mas usa os mesmos crates oficiais. Os workflows e scripts devem referenciar os mesmos caminhos.
8. **No silent fallback:** se o runtime Rust requerido não estiver instalado ou não suportar uma operação, o produto informa o erro explicitamente; não troca silenciosamente para o core Python legado.
9. **Ownership and buffers:** APIs transferem comandos e resultados em blocos. Propriedade, mutabilidade, vida útil dos buffers e sincronização de áudio/vídeo precisam estar documentadas antes de introduzir buffers compartilhados ou threads.
10. **Evidence:** cada migração tem testes de regressão independentes, paridade diferencial quando aplicável e validação CI no SHA candidato.

## Estrutura-alvo em `native/`

```text
native/
  sg1000_core/       # runtime e hardware específicos do SG-1000
  sms_core/          # runtime e hardware específicos do Master System
  gg_core/           # runtime e hardware específicos do Game Gear
  md_core/           # runtime e hardware específicos do Mega Drive/Genesis
  scd_core/          # add-on Sega CD; integra-se ao md_core
  32x_core/          # add-on 32X; integra-se ao md_core
  sega_emu_core/     # componentes compartilhados, orquestração e adaptador nativo
    z80_alu/
    z80_bus/
    z80_core/        # somente a fronteira PyO3/build da extensão; sem regras de hardware exclusivas
```

Os nomes de diretórios acima são os sete diretórios de primeiro nível desejados. Os subcrates compartilhados permanecem agrupados dentro de `sega_emu_core/`. Os crates `scd_core` e `32x_core` são extensões da máquina Mega Drive, não substitutos dela.

## Destino do Python legado

O destino pretendido é `legacy/python_reference/sega_emulator_reference/`, organizado por domínio (`cpu`, `bus`, `audio`, `video`, `machine`). O namespace de referência deve ser distinto do pacote de produção para evitar que imports da aplicação resolvam acidentalmente para o legado.

Os testes de referência podem importar explicitamente esse namespace. O pacote instalado a partir de `src/sega_emulator/` não deve empacotar nem importar a implementação legada. Não apagar fontes até que o adaptador Rust e a superfície da UI substituam todos os contratos utilizados e os testes relevantes passem.

## Migração em fatias compiláveis

### Fase 0 — inventário e congelamento de contratos
- Registrar os caminhos e imports atuais, o grafo de crates, a API usada pela UI e os workflows que constroem/testam cada componente.
- Fixar testes de aceitação para a API da máquina, estado do VDP, áudio, entrada, reset, ROMs inválidas e seleção do backend.
- Marcar explicitamente quais testes são testes de referência Python e quais validam o runtime Rust.

### Fase 1 — layout nativo e workspace
- Mover `z80_alu`, `z80_bus` e o adaptador PyO3 para `native/sega_emu_core/`, preservando o histórico Git e os arquivos.
- Atualizar todos os caminhos de dependência, membros/exclusões do workspace, locks, workflows, scripts e documentação na mesma alteração.
- Garantir build/teste do workspace e build Release/Maturin do adaptador antes de seguir.

### Fase 2 — runtime Rust oficial do SG-1000
- Consolidar CPU, memória/cartucho, portas, VDP, PSG, scheduler e máquina em tipos Rust independentes de PyO3.
- Reduzir o adaptador a chamadas de alto nível e buffers de saída; nenhum callback Python no loop de execução.
- Executar regressões unitárias e diferenciais em checkpoints equivalentes. Não declarar precisão ciclo a ciclo sem modelo temporal de barramento e testes correspondentes.

### Fase 3 — retirar os cores Python do caminho de produção
- Mover os módulos antigos para `legacy/python_reference/sega_emulator_reference/`.
- Criar/adaptar uma API Python fina que construa e controle exclusivamente o runtime Rust para o produto Qt.
- Manter o legado disponível somente por imports explícitos em ferramentas/testes diferenciais. Não manter fallback de produção.
- Tratar o frontend Pygame separadamente: ou migrá-lo para a mesma API Rust, ou marcá-lo como ferramenta legada fora do runtime oficial.

### Fase 4 — expansão de plataformas
- Só após o marco SG-1000 cumprir aceitação, migrar Master System/Game Gear e depois Mega Drive, Sega CD e 32X.
- Compartilhar abstrações comprovadas, mas não forçar dispositivos incompatíveis a uma API universal falsa.

## Critérios de aceitação

- [ ] Os sete diretórios de primeiro nível existem em `native/`; nenhum crate compartilhado antigo permanece como irmão deles.
- [ ] `cargo metadata --locked --format-version 1` resolve o workspace e os caminhos locais.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` passam.
- [ ] Build Release/Maturin e testes de integração do adaptador passam no Windows.
- [ ] O pacote de produção não importa módulos de `legacy/`; o legado não é incluído na distribuição normal.
- [ ] Os testes diferenciais identificam a primeira divergência de CPU/flags/PC/SP/ciclos, memória/I/O, estado de VDP, framebuffer, PSG e amostras.
- [ ] Repetir o mesmo teste produz o mesmo estado e saídas.
- [ ] Um teste de integração prova que a aplicação importa a extensão recém-construída, não uma cópia antiga instalada em outro local.
- [ ] Os workflows cobrem todos os caminhos de código, manifests, locks e scripts que validam.
- [ ] PASS, FAIL, BLOCKED e NOT RUN são reportados separadamente; build bem-sucedido não é evidência de fidelidade funcional.

## Estado observado na criação deste plano

O branch ainda contém `native/z80_alu`, `native/z80_bus` e `native/z80_core` nos caminhos antigos. O `Cargo.toml` raiz não inclui os novos crates de plataforma. `native/sg1000_core` e `native/sms_core` são parciais; o caminho de execução Rust/PyO3 existente ainda é experimental e algumas APIs públicas expõem objetos de dispositivos para a UI. Por isso, mover os módulos Python para fora do pacote de produção antes de completar a API Rust quebraria consumidores existentes. A movimentação do legado deve ocorrer junto à substituição explícita desses contratos, e não como uma renomeação isolada.

Nenhum teste é declarado aprovado por este documento. A validação é feita no SHA candidato pelos workflows e pelos comandos acima.
