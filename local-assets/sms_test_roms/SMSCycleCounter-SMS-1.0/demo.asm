
.define V_Counter $7e
.define H_Counter $7f
.define VDP_Data $be
.define VDP_Ctrl $bf

;==============================================================
; WLA-DX banking setup
;==============================================================
.memorymap
defaultslot 0
slotsize $4000
slot 0 $0000
slot 1 $4000
slot 2 $8000
.endme


.rombankmap
bankstotal 2
banksize $4000
banks 2
.endro

; Memory map (for Phantasy Star tilemap decompression)
.enum $c000
TileMapData: dsb 32*24*2

.ende

;==============================================================
; SDSC tag and SMS rom header
;==============================================================
.sdsctag 1.00,"Demo program","Demonstrates usage of BMP2tile","Maxim"

.bank 0 slot 0
.org $0000


;==============================================================
; Boot section
;==============================================================
.org $0000
.section "Boot section" force
  di              ; disable interrupts
  im 1            ; Interrupt mode 1
  jp main         ; jump to main program
.ends

;==============================================================
; VBlank handler
;==============================================================
.org $0038
.section "VBlank handler" force
  ; Only acknowledge VBlank irq
  push af
  in a,$bf
  pop af
  ei
  reti
.ends

;==============================================================
; Pause button handler
;==============================================================
.org $0066
.section "Pause button handler" force
  ; Wait 229 cycles, NMI takes 11 cycles.

  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc

  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc

  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc

  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc

  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc

  nop							;4 cyc

  retn							;14
.ends

;==============================================================
; Main program
;==============================================================
.section "Main program" free
main:
  ld sp, $dff0

  ; Initialise VDP
  call DefaultInitialiseVDP
  call ClearVRAM

  call NoSprites ; they mess things up

  call ScreenOn

colorloop
  ld a,$3F						;7 cyc, White
  call SetBgColor				;17 + 65
  ld a,$00						;7 cyc, Black
  call SetBgColor				;17 + 65
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  nop							;4 cyc
  ld a,$4						;7 cyc
  ld a,$4						;7 cyc
  jr colorloop					;12 cyc
								;228 cycles total


.ends

;==============================================================
; Turn on screen, wait for button, turn off screen.
;==============================================================
; A=color, Clobbers A & d
;==============================================================
.section "Set Bg color" free
SetBgColor:
;set palette 0
  ld d,a						;4 cyc
  ld a,$00						;7 cyc
  out (VDP_Ctrl),a				;11 cyc
  ld a,$c0						;7 cyc
  out (VDP_Ctrl),a				;11 cyc

  ld a,d						;4 cyc
  out (VDP_Data),a				;11 cyc

  ret							;10 cyc
.ends							;65 cycles total

;==============================================================
; Set up VDP registers (default values)
;==============================================================
; Call DefaultInitialiseVDP to set up VDP to default values.
; Also defines NameTableAddress, SpriteTableAddress and SpriteSet
; which can be used after this code in the source file.
; To change the values used, copy and paste the modified data
; and code into the main source. Data is commented to help.
;==============================================================
.section "Initialise VDP to defaults" free
DefaultInitialiseVDP:
    push hl
    push bc
        ld hl,_Data
        ld b,_End-_Data
        ld c,VDP_Ctrl
        otir
    pop bc
    pop hl
    ret

.define SpriteSet           0       ; 0 for sprites to use tiles 0-255, 1 for 256+
.define NameTableAddress    $3800   ; must be a multiple of $800; usually $3800; fills $700 bytes (unstretched)
.define SpriteTableAddress  $3f00   ; must be a multiple of $100; usually $3f00; fills $100 bytes

_Data:
    .db %00000100,$80
    ;    |||||||`- Disable sync
    ;    ||||||`-- Enable extra height modes
    ;    |||||`--- SMS mode instead of SG
    ;    ||||`---- Shift sprites left 8 pixels
    ;    |||`----- Enable line interrupts
    ;    ||`------ Blank leftmost column for scrolling
    ;    |`------- Fix top 2 rows during horizontal scrolling
    ;    `-------- Fix right 8 columns during vertical scrolling
    .db %10000100,$81
    ;     |||| |`- Zoomed sprites -> 16x16 pixels
    ;     |||| `-- Doubled sprites -> 2 tiles per sprite, 8x16
    ;     |||`---- 30 row/240 line mode
    ;     ||`----- 28 row/224 line mode
    ;     |`------ Enable VBlank interrupts
    ;     `------- Enable display
    .db (NameTableAddress>>10) |%11110001,$82
    .db (SpriteTableAddress>>7)|%10000001,$85
    .db (SpriteSet<<2)         |%11111011,$86
    .db $f|$f0,$87
    ;    `-------- Border palette colour (sprite palette)
    .db $00,$88
    ;    ``------- Horizontal scroll
    .db $00,$89
    ;    ``------- Vertical scroll
    .db $ff,$8a
    ;    ``------- Line interrupt spacing ($ff to disable)
_End:
.ends

;==============================================================
; Clear VRAM
;==============================================================
; Sets all of VRAM to zero
;==============================================================
.section "Clear VRAM" free
ClearVRAM:
  push af
  push hl
    ld hl,$4000
    call VRAMToHL
    ; Output 16KB of zeroes
    ld hl, $4000     ; Counter for 16KB of VRAM
  -:ld a,$00         ; Value to write
    out (VDP_Data),a ; Output to VRAM address, which is auto-incremented after each write
    dec hl
    ld a,h
    or l
    jp nz,-
  pop hl
  pop af
  ret
.ends

;==============================================================
; VRAM to HL
;==============================================================
; Sets VRAM write address to hl
;==============================================================
.section "VRAM to HL" free
VRAMToHL:
  push af
    ld a,l
    out (VDP_Ctrl),a
    ld a,h
    out (VDP_Ctrl),a
  pop af
  ret
.ends

;==============================================================
; VRAM writer
;==============================================================
; Writes BC bytes from HL to VRAM
; Clobbers HL, BC, A
;==============================================================
.section "Raw VRAM writer" free
WriteToVRAM:
-:ld a,(hl)
  out (VDP_Data),a
  inc hl
  dec bc
  ld a,c
  or b
  jp nz,-
  ret
.ends

;==============================================================
; Sprite disabler
;==============================================================
; Sets sprite 1 to y=208
; Clobbers HL, A
;==============================================================
.section "No sprites" free
NoSprites:
  ld hl,SpriteTableAddress | $4000
  call VRAMToHL
  ld a,208
  out (VDP_Data),a
  ret
.ends

;==============================================================
; Turn on screen, wait for button, turn off screen.
;==============================================================
; Clobbers A
;==============================================================
.section "Screen Wait" free
ScreenWait:
  call ScreenOn
  call WaitForButton
  call ScreenOff
  ret
.ends

;==============================================================
; Turn on screen.
;==============================================================
; Clobbers A
;==============================================================
.section "Screen On" free
ScreenOn:
  ld a,$e4
  out (VDP_Ctrl),a
  ld a,$81
  out (VDP_Ctrl),a
  ret
.ends

;==============================================================
; Turn off screen.
;==============================================================
; Clobbers A
;==============================================================
.section "Screen Off" free
ScreenOff:
  ld a,$84
  out (VDP_Ctrl),a
  ld a,$81
  out (VDP_Ctrl),a
  ret
.ends

;==============================================================
; Wait for button press
;==============================================================
; Clobbers A
; Not very efficient, I'm aiming for simplicity here
;==============================================================
.section "Wait for button press" free
WaitForButton:
-:in a,$dc ; get input
  cpl      ; invert bits
  or a     ; test bits
  jr nz,-  ; wait for no button press
-:halt
  in a,$dc ; get input
  cpl      ; invert bits
  or a     ; see if any are set
  jr z,-
  ret
.ends

