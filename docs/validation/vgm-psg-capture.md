# Captura VGM do PSG SG-1000

## Objetivo

Gravar os comandos enviados ao SN76489 para investigar a qualidade sonora separadamente da saída PCM do emulador. O VGM resultante pode ser aberto em ferramentas compatíveis e comparado com uma gravação equivalente do Gearsystem.

## Como usar

1. Compile/instale a versão do emulador que contém esta funcionalidade e abra uma ROM.
2. Em **Diagnóstico → Iniciar gravação PSG em VGM…**, escolha o destino `.vgm`.
3. Reproduza no jogo o trecho que deseja analisar.
4. Em **Diagnóstico → Parar gravação VGM**, finalize o arquivo.
5. Registre também a ROM, o SHA-256, o backend selecionado, o trecho do jogo e o comportamento auditivo observado.

O arquivo contém comandos SN76489 (comando VGM `0x50`) e esperas derivadas dos timestamps em T-states convertidos para a base de 44.100 amostras/s. Ele não contém a forma de onda PCM do emulador.

## Interpretação

- VGM igual ou equivalente entre emuladores ajuda a separar divergências de escrita/temporização dos problemas do gerador de som.
- Se os comandos diferirem, investigar o caminho CPU → I/O → PSG e a temporização dos writes antes de ajustar filtros.
- Se os comandos forem equivalentes, mas o som diferir, comparar o modelo digital, a mistura de canais, a conversão de taxa, níveis e a saída de áudio.
- VGM não mede clipping na saída PCM, não representa o circuito analógico do console e não prova fidelidade ciclo a ciclo.
- Os timestamps são os expostos pelo modelo de emulação. Não se deve inferir precisão sub-instrução quando ela não está modelada.

## Referência externa e licença

O Gearsystem (https://github.com/drhelius/Gearsystem) é usado como referência de comportamento e fluxo de trabalho, não como fonte para copiar implementação. O projeto declara GPL-3.0; esta funcionalidade foi escrita independentemente.
