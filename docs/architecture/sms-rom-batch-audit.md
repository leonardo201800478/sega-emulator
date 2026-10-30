# Auditoria em lote de ROMs Sega Master System

## Objetivo

A ferramenta Qt permite selecionar uma pasta local e executar, sequencialmente,
cada ROM SMS encontrada. O objetivo é separar evidência de execução real do core
de verificações estáticas de nome, extensão ou cabeçalho.

## Fluxo

1. Descobrir arquivos `.sms`, `.rom`, `.bin` e arquivos ZIP, com opção de
   percorrer subpastas.
2. Para ZIPs, enumerar todos os membros suportados. Ler o membro diretamente,
   sem extração persistente e sem alterar os arquivos originais.
3. Criar uma nova `SMSMachine` por ROM através da fábrica fornecida pelo
   frontend. A fábrica usa o runtime Rust/PyO3 e a política regional/BIOS
   configurada na aplicação.
4. Executar frames reais por `SMSMachine.run_frame`, em modo sem pacing do host,
   com limite de frames, timeout entre frames e limite de instruções por frame.
5. Registrar PC, instruções, T-states, frames avançados, SHA-256 da ROM e do
   framebuffer, quantidade de cores RGB, tempo e erro.
6. Exportar JSON local em `logs/sms_rom_audit/`. Nenhum byte de ROM ou BIOS é
   gravado no relatório, no repositório ou em artefatos de CI.

## Estados do relatório

- `BOOT_CONFIRMED`: o contador de frames avançou até o alvo e o framebuffer
  tem mais de uma cor RGB.
- `EXECUTED_NO_VISUAL_VARIATION`: CPU/VDP executaram, mas não há evidência
  visual suficiente. É inconclusivo, não falha automática.
- `TIMEOUT`: limite de tempo por ROM atingido entre frames.
- `FRAME_NOT_ADVANCING`: `run_frame` terminou sem avançar o contador do VDP.
- `INVALID_FRAMEBUFFER`: tamanho do framebuffer não corresponde à altura
  informada pelo VDP.
- `READ_OR_EXTRACTION_FAILED`: arquivo, ZIP ou membro não pôde ser lido.
- `MACHINE_CREATION_FAILED`: a ROM foi lida, mas a máquina não pôde ser criada.
- `BOOT_FAILED`: erro durante a execução do runtime.
- `CANCELLED`: cancelamento solicitado pelo usuário.

Um framebuffer uniforme pode ser um estado legítimo de um jogo; por isso,
não deve ser rotulado como incompatibilidade sem inspeção adicional. A ferramenta
não declara compatibilidade de gameplay, áudio, controles, mapper ou fidelidade
temporal apenas por confirmar a inicialização.

## Limites e segurança

- O runtime de hardware permanece exclusivamente em Rust.
- O worker Qt executa fora da thread da interface e cria uma máquina isolada por
  item; não interfere na ROM atualmente aberta.
- O cancelamento é observado entre frames. A chamada nativa em andamento termina
  sob o limite `max_steps_per_frame`.
- O limite atual de ROM é 1 MiB, igual ao carregador SMS existente.
- ZIPs não são extraídos, evitando path traversal e resíduos em disco.
- O JSON contém caminhos locais e hashes; deve permanecer fora do Git.
- A primeira versão audita somente Master System, não Game Gear.

## Critérios de aceitação

- [ ] Varredura recursiva e não recursiva, incluindo ROMs soltas e múltiplos
      membros SMS em ZIP.
- [ ] Uma instância Rust independente por item.
- [ ] Execução limitada por frames, passos por frame e timeout.
- [ ] Cancelamento sem bloquear a interface.
- [ ] Relatório JSON sem conteúdo binário de ROM/BIOS.
- [ ] Testes unitários do scanner, ZIP, resultados e estados inconclusivos.
- [ ] Testes Qt headless e CI no SHA candidato.
- [ ] Validação manual no Windows com biblioteca real do usuário.
