# Jogos de SMS que exigem mapper especial, e como tratá-los no emulador

**Fontes:** SMS Power (Development/Mappers) para a especificação dos registradores, e o banco de dados de CRCs e as implementações de mapper do Gearsystem (`game_db.h`, `*MemoryRule.cpp`) para a lista de jogos. Para saves, as listas de cartuchos com bateria da consolemods e da Sega Retro.

**Limites desta lista**
- Não é exaustiva. É a lista de jogos que o Gearsystem precisa identificar por CRC para escolher o mapper. Dumps alterados, hacks e traduções podem ter CRC diferente.
- O banco inclui entradas de Game Gear e protótipos. Marquei [GG] e [Proto] onde aparecem.
- O código Rust no fim é um esboço de arquitetura. Não foi compilado nem testado.

---

## 1. Visão geral: quem precisa de quê

| Categoria | Mapper | Quantos jogos conhecidos | Precisa de CRC? |
|---|---|---:|---|
| ROM ≤ 48 KB | nenhum (ROM fixa) | maioria dos cartuchos de 32 KB | não |
| ROM > 48 KB | Sega (registradores em $FFFC–$FFFF) | ~99% do catálogo | não |
| Sega + RAM no cartucho | Sega com bit 3 de $FFFC | 8 com bateria, mais alguns sem | não, usa o registrador |
| Codemasters | registradores em $0000/$4000/$8000 | poucos títulos (tabela 3) | sim |
| Coreanos simples | $A000, MSX/Nemesis, Janggun | 14 títulos | sim |
| Multicarts coreanos | 12 variantes diferentes | 38 entradas no banco | sim |
| Outros | Jumbo/DahJee, 4 PAK, Iratahack | 14 + 1 + 1 | sim |

Em ROMs maiores que 48 KB, o BIOS assume o mapper Sega. Por isso o mapper Sega deve ser o padrão. Os outros só entram quando o CRC está na tabela.

## 2. Mapper Sega (padrão)

Quatro slots de 16 KB. Registradores só de escrita em $FFFC–$FFFF, espelhados na RAM do console.

| Endereço | Função |
|---|---|
| $FFFC | bit 7 ROM write, bit 4 RAM em $C000–$FFFF, bit 3 RAM em $8000–$BFFF, bit 2 banco de RAM, bits 1–0 deslocamento de banco |
| $FFFD | banco do slot 0 ($0000–$3FFF) |
| $FFFE | banco do slot 1 ($4000–$7FFF) |
| $FFFF | banco do slot 2 ($8000–$BFFF) |

Pontos que costumam dar bug:
- No slot 0, os primeiros 1 KB ($0000–$03FF) não são afetados pelo banco, para preservar os vetores de interrupção.
- Escritas em $FFFC–$FFFF vão para o registrador e também para a RAM do sistema, por isso o último valor pode ser lido de volta. O inverso não vale: escrever no espelho da RAM em outro endereço não altera o mapeamento.
- Bancos acima do tamanho da ROM devem espelhar (máscara por potência de dois).
- O deslocamento de banco (bits 1–0 de $FFFC) existe só em um chip de 512 KB e nenhum software conhecido usa.
- Reset: o chip 315-5235 volta a FFFC=00, FFFD=00, FFFE=01, FFFF=02. Em outros, os registradores ficam indefinidos após reset.

**Aztec Adventure:** o Gearsystem o lista com o mapper padrão (a entrada só marca YM2413). Portanto o problema de scroll que você relatou não vem do mapper.

## 3. Jogos com RAM no cartucho (mapper Sega, bit 3 de $FFFC)

**Com bateria** (devem persistir o save em arquivo):
Golden Axe Warrior, Golfamania, Miracle Warriors: Seal of the Dark Lord, Monopoly, Penguin Land, Phantasy Star, Ultima IV: Quest of the Avatar, Ys: The Vanished Omens.

**RAM sem bateria** (Gearsystem marca como "RAM without battery"): The Flash, Desert Speedtrap.

A maioria desses cartuchos tem 8 KB de SRAM espelhados para preencher os dois bancos de 16 KB. Quando o bit 3 está ligado, a RAM sobrepõe o slot 2 e o banco escolhido em $FFFF é ignorado.

## 4. Mapper Codemasters

Registradores nos três endereços base, e o primeiro 1 KB do slot 0 deixa de ser protegido. Inicializa com bancos 0, 1 e 0 nos slots 0, 1 e 2. Não há espelho na RAM, então não dá para ler de volta o último valor escrito.

| Escrita em | Efeito |
|---|---|
| $0000 | banco do slot 0 |
| $4000 | banco do slot 1; bit 7 liga RAM de 8 KB em $A000–$BFFF (só jogos com RAM) |
| $8000 | banco do slot 2 |

Jogos de SMS no banco do Gearsystem: Cosmic Spacehead, CJ Elephant Fugitive, Fantastic Dizzy, Micro Machines, mais as entradas `Excellent Dizzy Collection [Proto]`, `Excellent Dizzy Collection [SMS-GG]`, `Fantastic Dizzy [SMS-GG]` e `Jang Pung 2 [SMS-GG]`.
Entradas do banco sem versão comercial de SMS que eu tenha confirmado (Game Gear ou cancelados): Dropzone, Micro Machines 2: Turbo Tournament, Ernie Els Golf (único com 8 KB de RAM), Pete Sampras Tennis, Dinobasher, Man Overboard!.

## 5. Mappers coreanos simples

| Mapper | Registradores | Jogos |
|---|---|---|
| Korean ($A000) | slots 0 e 1 fixos nos bancos 0 e 1; $A000 escolhe o banco do slot 2 | Dallyeora Pigu-Wang, Sangokushi 3, Jang Pung II, Jang Pung 3 |
| MSX | $0000–$0003: bancos de 8 KB (ver tabela abaixo); 16 KB iniciais fixos | Penguin Adventure, Street Master, Won-Si-In, F-1 Spirit, Cyborg Z, Knightmare II, Super Boy 3, Nemesis 2 (o Gearsystem também usa este mapper para Nemesis) |
| Nemesis | igual ao MSX, mas $0000–$1FFF sempre mapeia o último banco de 8 KB | Nemesis (única variante conhecida; confira o comportamento dos 8 KB iniciais, pois o Gearsystem usa o mapper MSX para ele) |
| Janggun | $4000/$6000/$8000/$A000 (8 KB cada); $FFFE/$FFFF (16 KB); bit 6 do valor inverte os bits lidos | Janggun-ui Adeul |

Registradores do MSX/Nemesis: $0000 → slot $8000–$9FFF; $0001 → $A000–$BFFF; $0002 → $4000–$5FFF; $0003 → $6000–$7FFF.

Nota de implementação do Gearsystem: no Janggun, as escritas em $FFFE/$FFFF também vão para a RAM do sistema, e `value & 0x3F` seleciona o banco de 8 KB.

## 6. Multicarts coreanos e outros (cada um com registrador próprio)

Cada variante usa um endereço de registrador e uma lógica de bancos diferentes. O Gearsystem as nomeia pelo endereço:

| Variante | Registrador | Jogos (compilações) |
|---|---|---|
| Korean MSX SMS 8000 | $8000 | Zemina Best 25, Best 39, Best 88 |
| Korean 0000 XOR FF | $0000 | Zemina 4-in-1 |
| Korean SMS 32KB 2000 | $2000 | 11 Hap Gam-Boy |
| Korean MSX 32KB 2000 | $2000 | 2 Hap in 1 (Moai-ui bomul, David-2) |
| Korean 2000 XOR 1F | $2000–$3FFF (espelho em $A000–$BFFF) | 128 Hap, Game Mo-eumjip 188 Hap (v0, v1) |
| Korean MSX 8KB 0300 | $0000–$0300 (por byte alto) | Game Chongjiphap 200, Super Game 270 Hap, Super Game World 260/75/30 Hap, Super Multi Game 75 in 1 e 125 in 1 |
| Korean FFFF HiCom | $FFFF | Hi-Com 3-in-1 (A–F), Hi-Com 8-in-1 (A–C) |
| Korean FFFE | $FFFE | Super Game 45 |
| Korean BFFC | $BFFC | Super Game 180, Super Game 200, Super Game 52 Hap |
| Korean FFF3 FFFC | $FFF3 e $FFFC | Super Game 150, Super Game 270 |
| Korean MD FFF5 | $FFF5 | Jaemiissneun Game Mo-eumjip 42 Hap e 65 Hap, Mega Mode Super Game 138, Pigu-Wang 7 Hap |
| Korean MD FFF0 | $FFF0 | Mega Mode Super Game 30 |

Outros:

| Mapper | Característica | Jogos |
|---|---|---|
| Jumbo / DahJee | escrita em $2000–$3FFF vai para RAM do cartucho; RAM do sistema só 1 KB espelhado em $C000 | Adventure Island, Bomberman Special, The Goonies, Legend of Kage, Rally-X, Star Soldier, Tank Battalion, Yie Ar Kung-Fu II, Zanac (versões DahJee); Knightmare, Pippols, Road Fighter, TwinBee (versões Jumbo); King's Valley |
| 4 PAK All Action | registradores em $3FFE, $7FFF, $BFFF | 4 PAK All Action (Taiwan) |
| Iratahack | emula flash (sequências em $5555/$2AAA, apagamento de setor de 16 KB) | Wee Knight's Escape Trilogy (título do banco) |

Os detalhes de cada variante são específicos e pouco documentados. Para a lógica exata, a referência mais confiável que encontrei é o código de cada `*MemoryRule.cpp` do Gearsystem (GPLv3). Se você for portar, verifique a licença antes de copiar.

## 7. Outros casos que parecem "mapper" mas não são

- Marcados como SG-1000 no banco (o Gearsystem usa um mapper específico para eles): The Castle, King's Valley, Othello, Safari Hunting, Loretta no Shouzou.
- EEPROM 93C46 (só Game Gear): Majors Pro Baseball, Pro Yakyuu GG League, World Series Baseball.
- Paddle (periférico, não mapper): BMX Trial – Alex Kidd, Galactic Protector, Megumi Rescue, Woody Pop.
- YM2413 problemático: Wanted, American Pro Football, Walter Payton Football quebram com FM ligado.

---

## 8. Como resolver no seu emulador

O `bus.rs` indica que o mapa de memória e a decodificação de portas ficam na máquina SMS. O ponto natural é um módulo `cartridge` que a máquina consulta em toda leitura e escrita de $0000–$FFFF. Não tenho o arquivo da máquina, então o esboço abaixo é genérico.

**Arquitetura sugerida**

1. `Cartridge { rom: Vec<u8>, mapper: Mapper, cart_ram: Vec<u8> }`.
2. `Mapper` como `enum` (despacho estático, sem alocação por acesso). Cada variante guarda seus registradores e bancos.
3. Seleção ao carregar a ROM:
   1. calcular CRC32 da ROM, sem o cabeçalho de 512 bytes se houver;
   2. procurar numa tabela estática `crc → MapperKind`;
   3. se não achar: ROM ≤ 48 KB → `RomOnly`, senão `Sega`.
4. Opção na UI para forçar o mapper, como o Gearsystem faz. Isso resolve ROMs alteradas e a descoberta de um mapper novo.
5. A máquina trata RAM do sistema ($C000–$DFFF, espelhada em $E000–$FFFF) e chama `cartridge.write(addr, value)` em toda escrita, inclusive em $C000–$FFFF, porque o mapper Sega observa $FFFC–$FFFF. A escrita continua indo para a RAM do sistema.

**Esboço**

~~~rust
pub enum Mapper {
    RomOnly,
    Sega { regs: [u8; 4], ram_bank_sel: bool },
    Codemasters { bank: [u8; 3], ram_on: bool },
    Korean { slot2: u8 },
    Msx { bank8k: [u8; 4], nemesis: bool },
}

impl Cartridge {
    pub fn read(&self, addr: u16) -> Option<u8> {
        let a = addr as usize;
        match &self.mapper {
            Mapper::RomOnly => (a < 0xC000).then(|| self.rom[a % self.rom.len()]),
            Mapper::Sega { regs, .. } => {
                if a >= 0xC000 { return None; }
                let n16 = (self.rom.len() / 0x4000).max(1);
                let (bank, off) = match a >> 14 {
                    0 if a < 0x0400 => (0, a),
                    0 => (regs[1] as usize % n16, a),
                    1 => (regs[2] as usize % n16, a & 0x3FFF),
                    _ => (regs[3] as usize % n16, a & 0x3FFF),
                };
                Some(self.rom[bank * 0x4000 + off])
            }
            Mapper::Codemasters { bank, .. } => { /* slots por (a>>14) */ todo!() }
            _ => todo!(),
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match &mut self.mapper {
            Mapper::Sega { regs, .. } if addr >= 0xFFFC => regs[(addr - 0xFFFC) as usize] = value,
            Mapper::Codemasters { bank, ram_on } => match addr {
                0x0000 => bank[0] = value,
                0x4000 => { bank[1] = value; *ram_on = value & 0x80 != 0; }
                0x8000 => bank[2] = value,
                _ => {}
            },
            Mapper::Korean { slot2 } if addr == 0xA000 => *slot2 = value,
            Mapper::Msx { bank8k, .. } if addr < 4 => bank8k[addr as usize] = value,
            _ => {}
        }
    }
}
~~~

Em `Sega`, `regs[0]` é $FFFC e `regs[1..=3]` são os bancos dos slots 0 a 2 ($FFFD–$FFFF). Inicialize com `[0, 0, 1, 2]`. A RAM do cartucho (bit 3 de FFFC) precisa ser checada antes da ROM no slot 2; deixei de fora para encurtar.

**Ordem sugerida de implementação**
1. Sega completo (RAM de cartucho, vetores do slot 0, máscara por tamanho) + persistência de `cart_ram` em arquivo para os 8 jogos com bateria.
2. Codemasters (cobre Dizzy, Micro Machines, Cosmic Spacehead, CJ Elephant).
3. Tabela de CRC com Codemasters, Korean ($A000) e MSX/Nemesis.
4. Opção "forçar mapper" na UI.
5. As demais variantes coreanas, só se você precisar das compilações.

**Testes recomendados:** um teste por mapper com ROM sintética onde cada banco de 16 KB (ou 8 KB) é preenchido com o seu número, escreve no registrador e verifica o byte lido nos três slots, incluindo o 1 KB inicial do slot 0.

