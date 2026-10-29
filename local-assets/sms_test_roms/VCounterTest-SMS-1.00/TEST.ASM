;----------------------------------------------------------------------------
; V Counter Test Program
;
; Bug fixes:
; - Result buffer is now cleared before results are written to it
; - The initial page of results is written with the screen turned off
;
;----------------------------------------------------------------------------

RESULT_BUFFER           =       $C000
LINE_BUFFER             =       $C800
JOY_OLD                 =       $D000
JOY_NEW                 =       $D001
MENU_CURSOR             =       $D002
EXIT_FLAG               =       $D003
VBLANK_ROUTINE          =       $D004
SCREEN_ROUTINE          =       $D008
RESULT_OFFSET           =       $D00C
PAGE_COUNT              =       $D010
FORMAT_MODE             =       $D011
REG0_VALUE              =       $D012
REG1_VALUE              =       $D014
SCAN_LENGTH             =       $D016
SCAN_COUNT              =       $D018
MENU_CURSOR_OLD         =       $D020
VBL_LINE                =       $D021
NTSC_LINES              =       262
PAL_LINES               =       312

                        .org    $0000
                        di
                        im      1
                        ld      sp, $DFF0
                        jp      main

                        .org    $0010
                        ld      a, l
                        out     ($BF), a
                        ld      a, h
                        out     ($BF), a
                        ret

                        .org    $0018
                        ld      a, l
                        out     ($BE), a
                        ld      a, h
                        out     ($BE), a
                        ret


                        .org    $0038
                        in      a, ($BF)
                        call    irq_routine
                        ei
                        reti

                        .org    $0066
                        retn

;----------------------------------------------------------------------------
main:
                        call    vdp_init
                        call    conio_init

                        ; Initialize variables
                        xor     a
                        ld      (JOY_NEW), a
                        ld      (JOY_OLD), a
                        ld      a, $01 ; pal def
                        ld      (SCAN_LENGTH), a
                        ld      a, $05
                        ld      (MENU_CURSOR), a
                        ld      (MENU_CURSOR_OLD), a
                        ld      hl, NTSC_LINES
                        ld      (SCAN_COUNT), hl

                        ; Install handlers
                        ld      hl, _do_menu
                        ld      (SCREEN_ROUTINE), hl
next_screen:
                        ; Screen off
                        ld      hl, $8100
                        rst     10h

                        ; Call screen handler
                        ld      hl, main_here
                        push    hl
                        ld      hl, (SCREEN_ROUTINE)
                        jp      (hl)
main_here:
                        ; Screen on
                        ld      hl, $81E2
                        rst     10h
                        ei

                        ; Run screen routine until the exit
                        ; flag is changed
                        xor     a
                        ld      (EXIT_FLAG), a
wait_exit:              ld      a, (EXIT_FLAG)
                        cp      $01
                        jr      nz, wait_exit

                        ; Do the next screen
                        di
                        jp      next_screen


;----------------------------------------------------------------------------

_do_menu:
                        ; Clear the screen
                        call    clear_screen

                        ; Print menu text
                        ld      hl, menu_text
                        ld      bc, $0101
                        call    printf

                        ; Select an option line 
                        ld      a, (MENU_CURSOR)
                        call    select_line

                        ; Update scan text
                        call    _update_scan_text

                        ; Install handlers
                        ld      hl, _vblank_menu_routine
                        ld      (VBLANK_ROUTINE), hl
                        ld      hl, _do_test
                        ld      (SCREEN_ROUTINE), hl

                        ret

;============================================================================

_do_test:
                        ; Clear screen
                        call    clear_screen

                        ; Print menu text
                        ld      hl, result_text
                        ld      bc, $0101
                        call    printf

                        ; Reset result offset and page count
                        ld      hl, RESULT_BUFFER
                        ld      (RESULT_OFFSET), hl
                        xor     a
                        ld      (PAGE_COUNT), a

                        ; Set registers for the test mode
                        ld      hl, mode_table
                        ld      a, (MENU_CURSOR)
                        sub     $05
                        and     $03
                        sla     a
                        sla     a
                        ld      l, a
                        push    hl
                        pop     ix
                        ld      l, (ix+$00)
                        ld      h, (ix+$01)
                        rst     10h
                        ld      a, l
                        ld      (REG0_VALUE), a
                        ld      l, (ix+$02)
                        ld      h, (ix+$03)
                        rst     10h
                        ld      a, l
                        ld      (REG1_VALUE), a

                        ; Scan V counter
                        call    scan_hvc

                        ; Restore display
                        ld      hl, $8006
                        rst     10h
                        ld      hl, $81A2
                        rst     10h

                        ; Print results screen
                        call    result_screen

                        ; Install handlers
                        ld      hl, _vblank_test_routine
                        ld      (VBLANK_ROUTINE), hl
                        ld      hl, _do_menu
                        ld      (SCREEN_ROUTINE), hl

                        ret

;============================================================================
;
;============================================================================

_vblank_test_routine:
                        ld      a, (JOY_OLD)
                        ld      b, a

                        bit     5, b
                        jr      z, test_no_c
                        ld      a, (PAGE_COUNT)
                        cp      $02
                        jr      z, test_exit
                        inc     a
                        ld      (PAGE_COUNT), a
                        ; inc results offset
                        ld      hl, (RESULT_OFFSET)
                        ld      bc, $0080
                        add     hl, bc
                        ld      (RESULT_OFFSET), hl
                        jp      test_update
test_no_c:
                        bit     4, b
                        jr      z, test_no_b
                        ld      a, (PAGE_COUNT)
                        cp      $00
                        jr      z, test_exit
                        dec     a
                        ld      (PAGE_COUNT), a
                        ; dec results offset
                        ld      hl, (RESULT_OFFSET)
                        ld      bc, $007F
                        scf
                        sbc     hl, bc
                        ld      (RESULT_OFFSET), hl
                        jp      test_update
test_no_b:
test_exit:
                        ret
test_update:
                        call    result_screen
                        jp      test_exit

;============================================================================


_vblank_menu_routine:
                        ld      a, (MENU_CURSOR)
                        ld      (MENU_CURSOR_OLD), a

                        ld      a, (JOY_OLD)
                        ld      b, a

                        bit     5, b
                        jr      z, menu_not_b
                        ld      a, $01
                        ld      (EXIT_FLAG), a
                        jp      menu_not_down
menu_not_b:
                        bit     4, b
                        jr      z, menu_not_c
                        ld      a, (SCAN_LENGTH)
                        xor     $01
                        ld      (SCAN_LENGTH), a
                        call    _update_scan_text
                        jp      menu_not_down
menu_not_c:
                        bit     0, b
                        jr      z, menu_not_up
                        ld      a, (MENU_CURSOR)
                        cp      $05
                        jr      z, menu_not_down
                        dec     a
                        ld      (MENU_CURSOR), a
                        jp      menu_update
menu_not_up:
                        bit     1, b
                        jr      z, menu_not_down
                        ld      a, (MENU_CURSOR)
                        cp      $07
                        jr      z, menu_not_down
                        inc     a
                        ld      (MENU_CURSOR), a
                        jp      menu_update
menu_not_down:
                        ret
menu_update:
                        ld      a, (MENU_CURSOR_OLD)
                        call    select_line
                        ld      a, (MENU_CURSOR)
                        call    select_line
                        jp      menu_not_down

;============================================================================

irq_routine:
                        ex      af, af'
                        exx

                        ; screen off
                        ld      hl, $81A2
                        rst     10h

                        ; read joy
                        ld      a, (JOY_NEW)
                        ld      b, a
                        ld      a, (JOY_OLD)
                        ld      c, a
                        in      a, ($DC)
                        cpl
                        ld      c, b
                        ld      b, a
                        xor     c
                        and     c
                        ld      d, a
                        ld      a, b
                        ld      (JOY_NEW), a
                        ld      a, c
                        ld      (JOY_OLD), a
                        ld      a, d
                        ld      (JOY_OLD), a

                        ; do vblank routine
                        ld      hl, vbl_here
                        push    hl
                        ld      hl, (VBLANK_ROUTINE)
                        jp      (hl)
vbl_here:

                        ; screen on
                        ld      hl, $81E2
                        rst     10h

                        exx
                        ex      af, af'

                        ret

_update_scan_text:
                        push    af
                        push    bc
                        push    hl

                        ld      hl, NTSC_LINES
                        ld      (SCAN_COUNT), hl
                        ld      hl, text262

                        ld      a, (SCAN_LENGTH)
                        cp      $01
                        jr      nz, less262

                        ld      hl, PAL_LINES
                        ld      (SCAN_COUNT), hl
                        ld      hl, text312
less262:        
                        ld      bc, $0F10
                        call    printf

                        pop     hl
                        pop     bc
                        pop     af

                        ret


;----------------------------------------------------------------------------

; scan v counter = fixed length and dest buf
scan_hvc:
                        push    af
                        push    bc
                        push    de
                        push    hl

                        xor     a
                        ld      (VBL_LINE), a

                        ; clear result buffer
                        ld      hl, RESULT_BUFFER
                        push    hl
                        pop     de
                        inc     de
                        ld      bc, 383
                        xor     a
                        ld      a, $00
                        ld      (hl), a
                        ldir

                        ld      bc, (SCAN_COUNT)
                        inc     bc      ; buffer
                        ld      hl, RESULT_BUFFER

                        ; wait for vblank line
wait_v_in:              in      a, ($BF)
                        bit     7, a
                        jr      nz, wait_v_in
                        ; wait for line ff
pFF:                    in      a, ($7E)
                        cp      $FF
                        jr      z, pFF
                        ; wait for line 00
x00:                    in      a, ($7E)
                        cp      $00
                        jr      nz, x00
                        in      a, ($BF)        ; clear irq
scan_next:        
                        in      a, ($7E)
                        ld      (hl), a
                        inc     hl

                        push    af
                        in      a, ($BF)
                        bit     7, a
                        jr      z, no_vbl
                        in      a, ($7E)
                        ld      (VBL_LINE), a
no_vbl:                 pop     af

                        ld      d, a
wait_next:              in      a, ($7E)
                        cp      d
                        jr      z, wait_next

                        dec     bc
                        ld      a, c
                        or      b
                        jr      nz, scan_next

                        pop     hl
                        pop     de
                        pop     bc
                        pop     af

                        ret


; print results table - need hilighting
result_screen:
                        push    af
                        push    bc
                        push    hl

                        ld      a, (PAGE_COUNT)
                        ld      bc, $0407
                        call    printhexd

                        ld      a, (REG0_VALUE)
                        ld      bc, $0411
                        call    printhexb

                        ld      a, (REG1_VALUE)
                        ld      bc, $0414
                        call    printhexb

                        ld      bc, $041D
                        ld      a, (VBL_LINE)
                        call    printhexb

                        ld      hl, (RESULT_OFFSET)
                        ld      c, $01  ; x
                        ld      d, $08
next_col:               ld      b, $06  ; y
                        ld      e, $10
next_row:               ld      a, (hl)
                        inc     hl

                        call    printhexb
                        inc     b             
                        dec     e
                        jr      nz, next_row
                        inc     c
                        inc     c
                        inc     c
                        dec     d
                        jr      nz, next_col

                        pop     hl
                        pop     bc
                        pop     af
                        ret

; clear name table
clear_screen:
                        push    af
                        push    bc
                        push    hl
                        ld      hl, $7800
                        rst     10h
                        ld      bc, 1536
clear_loop:             xor     a
                        out     ($BE), a
                        dec     bc
                        ld      a, c
                        or      b
                        jr      nz, clear_loop
                        pop     hl
                        pop     bc
                        pop     af
                        ret
; hilight a line
; a = line
select_line:
                        push    af
                        push    hl
                        push    bc

                        push    af
                        sla     a
                        sla     a
                        sla     a
                        sla     a
                        sla     a
                        sla     a
                        and     $C0     ; mask hi bits of L
                        ld      l, a
                        pop     af
                        sra     a
                        sra     a
                        and     $07     ; mask lo bits of H
                        or      $38     ; 3800
                        ld      h, a
                        rst     10h     ; set read addr
                                  
                        push    hl              ; read bytes
                        ld      bc, $40BE
                        ld      hl, LINE_BUFFER
read_line:              in      a, (c)
                        ld      (hl), a
                        inc     hl
                        djnz    read_line

                        pop     hl      ; set write addr
                        ld      a, h
                        or      $40
                        ld      h, a
                        rst     10h

                        ld      bc, $20BE       ; write words
                        ld      hl, LINE_BUFFER
write_line:             ld      a, (hl)
                        inc     hl
                        out     (c), a
                        ld      a, (hl)
                        inc     hl
                        xor     $08             ; pal=1
                        out     (c), a
                        djnz    write_line

                        pop     bc
                        pop     hl
                        pop     af

                        ret

;----------------------------------------------------------------------------

                        .include "vdp.inc"
                        .include "conio.inc"

result_text:            .db     1
                        .db     "Press 1 for the previous page,",1
                        .db     "Press 2 for the next page.",1
                        .db     "Page: -/2  Reg: -- --  VBL: --",0

menu_text:              .db     "V counter test program",1
                        .db     "by Charles MacDonald",1
                        .db     "E-mail: cgfm2@hotmail.com",1,1
                        .db     "0. 256x192 Mode 4",1
                        .db     "1. 256x224 Mode 4",1
                        .db     "2. 256x240 Mode 4",1,1,1
                        .db     "Press 2 to run a test.",1,1
                        .db     "Press 1 to toggle scanning",1
                        .db     "262 or 312 lines.",1,1
                        .db     "Lines to scan: ---", 0
text262:                .db     "262",0
text312:                .db     "312",0
sdsc_program_name:      .db     "V Counter Test Program",0
sdsc_release_notes:     .db     "Author: Charles MacDonald",0

                        .org    $2F00
mode_table:             .dw     $8066, $81E0    ; 256x192 (M4)
                        .dw     $8026, $8170    ; 256x224 (M4)
                        .dw     $8026, $81E8    ; 256x240 (M4)

                        ; SDSC data

                        .org    $7FE0
                        .db     "SDSC"
                        .db     $01, $00
                        .db     $30
                        .db     $07
                        .db     $01, $20
                        .db     $00, $00
                        .dw     sdsc_program_name
                        .dw     sdsc_release_notes

                        ; SEGA data

                        .org    $7FF0
                        .db     "TMR SEGA"
                        .dw     $FFFF
                        .db     $00, $00
                        .db     $00, $00
                        .db     $00
                        .db     $00

                        .end

