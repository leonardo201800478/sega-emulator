; Eric R. Quinn
; July 22, 2000
; PFR_Detect.asm
;1234567890123456789012345678901234567890123456789012345678901234567890123456789
;
; Limitations:
; ------------
;
;	This test requires that ALL of ROM Page 0 can be mapped into Frame 0
;	(either by default at power-on/reset, or because it was explictily
;	 mapped at the beginning of the program).
;
;	Also, the starting VRAM address for the font tiles is hard-coded.
;	This is slightly dangerous if the included font file changes.
;	
;	See the Release Notes section for more information about PFR_Detect.
;
;
; Version 2.00: March 20, 2001
; ------------------------------
;
;	+ Added error message if required initial conditions are not met.
;	+ Added Backup-RAM page test result for systems that allow
;	  (unconditional) writes to Frame 2.  This is only important when
;	  the test is run on emulators.
;	+ Added Backup-RAM page uniqueness test
;	+ Incorporated font from SMS BIOS 1.3
;	+ Added SDSC ROM tag, title, and release notes
;	+ Completely re-structured test mechanism.  Now any permutation
;	  of fixed/paging capabilities is supported (previous versions
;	  had bugs for certain combinations).
;	= Changed VDP Register 0 value to 0x06, and Register 1 to 0x80
;	= Changed to WLA-DX syntax
;	- Due to font space limitations, all on-screen text is now upper-case
;
;
; Version 1.00: January 9, 2001
; -----------------------------
;
;	+ Added Backup RAM tests (Page 0 and Page 1)
;	= Fixed bug that causes hang if Page Frame 1 is not pageable
;	= Cleaned source code and integrated SMSMAX macro library into test
;
;
; Version 0.01: July 22, 2000
; ---------------------------
;
;	+ Initial version
;


; Standard SMS definitions
; ------------------------

; Definitions for sizes (in bytes) of various memory structures
;
.DEFINE SMS_FRAME_SIZE			$4000
.DEFINE SMS_PAGE_SIZE			SMS_FRAME_SIZE

.DEFINE	SMS_RAM_SIZE			$2000

; "BASES" represent physical memory addresses
;
.DEFINE SMS_BASE_FRAME_0		$0000
.DEFINE SMS_BASE_FRAME_1		SMS_BASE_FRAME_0 + SMS_FRAME_SIZE
.DEFINE SMS_BASE_FRAME_2		SMS_BASE_FRAME_1 + SMS_FRAME_SIZE

.DEFINE SMS_BASE_RAM			$C000
.DEFINE SMS_BASE_RAM_MIRROR		SMS_BASE_RAM + SMS_RAM_SIZE

.DEFINE	SMS_BASE_PFR_BACKUPRAM		$FFFC
.DEFINE	SMS_BASE_PFR_0			$FFFD
.DEFINE	SMS_BASE_PFR_1			$FFFE
.DEFINE	SMS_BASE_PFR_2			$FFFF

; "OFFSETS" are relative to the beginning of a ROM page or a Page Frame
;
.DEFINE SMS_OFFSET_SDSC_TAG		$3FE0
.DEFINE SMS_OFFSET_SEGA_TAG		$3FF0

; "VECTORS" represent physical addresses of standard execution entry-points
;
.DEFINE SMS_VECTOR_RESET		$0000
.DEFINE SMS_VECTOR_INT			$0038
.DEFINE SMS_VECTOR_NMI			$0066

; VDP Definitions
;
.DEFINE	SMS_VDP_REGISTER_COUNT		$0B

.DEFINE	SMS_VDP_COMMAND_ACCESS_VRAM	$40
.DEFINE	SMS_VDP_COMMAND_ACCESS_REG00	$80
.DEFINE	SMS_VDP_COMMAND_ACCESS_REG01	$81
.DEFINE	SMS_VDP_COMMAND_ACCESS_PALETTE	$C0

.DEFINE	SMS_VDP_REG01_ENABLEDISPLAY	$C0
.DEFINE	SMS_VDP_REG01_DISABLEDISPLAY	$80

.DEFINE	SMS_VDP_PALETTE_BLACK		$00
.DEFINE	SMS_VDP_PALETTE_WHITE		$3F

; I/O Port Definitions
;
.DEFINE	SMS_INPORT_CURRENT_SCANLINE	$07E
.DEFINE	SMS_INPORT_VDP_STATUS		$0BF

.DEFINE	SMS_OUTPORT_VDP_DATA		$0BE
.DEFINE	SMS_OUTPORT_VDP_COMMAND		$0BF

; Backup-RAM Definitions
;
.DEFINE	SMS_BACKUPRAM_MAP_PAGE0		$08
.DEFINE	SMS_BACKUPRAM_MAP_PAGE1		$0C
.DEFINE	SMS_BACKUPRAM_DISABLE		$00


; Sizes (in pages) of standard SMS ROM components
;
.DEFINE SMS_ROMSIZE_CARD		$02
.DEFINE SMS_ROMSIZE_1MEGA		$08
.DEFINE	SMS_ROMSIZE_2MEGA		$10
.DEFINE	SMS_ROMSIZE_4MEGA		$20

; Definitions for SLOTS required by WLA-DX MEMORYMAP
;
.DEFINE SMS_FRAME_0			$0
.DEFINE SMS_FRAME_1			$1
.DEFINE SMS_FRAME_2			$2
.DEFINE SMS_RAM				$3
.DEFINE SMS_RAM_MIRROR			$4


; Program-specific definitions
; ----------------------------

; ROM Page definitions
;
.DEFINE	CODE_PAGE			$00
.DEFINE	TEST_DATA_PAGE			$01


; Generic offsets (not page-specific)
;
.DEFINE	COMPARE_DATA_03F0		$03F0
.DEFINE	COMPARE_DATA_0400		$0400

; CODE_PAGE specific offsets
;
.DEFINE	CODE_PAGE_RELOCATE_START	$0800

; TEST_DATA_PAGE specific offsets
;
.DEFINE	TEST_DATA_PAGE_TITLE		$0500
.DEFINE	TEST_DATA_PAGE_RELEASE_NOTES	$0600

; RAM (run-time variables) offsets
;
.DEFINE	RAM_PAGE0_TEST_RESULT		$0000
.DEFINE	RAM_FRAME0_03F0_RESULT		$0010
.DEFINE	RAM_FRAME0_0400_RESULT		$0011
.DEFINE	RAM_FRAME1_RESULT		$0012
.DEFINE	RAM_FRAME2_RESULT		$0013
.DEFINE	RAM_BACKUPRAM_PAGE0_RESULT	$0014
.DEFINE	RAM_BACKUPRAM_PAGE1_RESULT	$0015

.DEFINE	RAM_COMPARE_TABLE_1		$0020
.DEFINE	RAM_COMPARE_TABLE_2		$0030


; Result codes
;
.DEFINE	RESULT_NOT_TESTED		$00

.DEFINE	RESULT_PASS			$01
.DEFINE	RESULT_FAIL			$FF

.DEFINE	RESULT_PAGEABLE			$01
.DEFINE	RESULT_FIXED			$FF

.DEFINE	RESULT_EXISTS			$01
.DEFINE RESULT_SAMEASPAGE0		$02
.DEFINE RESULT_WROTETOROM		$FE
.DEFINE	RESULT_DOESNTEXIST		$FF


; Data group definitions
; (using macro because WLA-DX doesn't support DEFINE-ing groups of data
;
.DEFINE	GROUP_SIZE			$10

.MACRO	M_CONTROLGROUP_DATA
	.DB	$0F, $1E, $2D, $3C, $4B, $5A, $69, $78
	.DB	$87, $96, $A5, $B4, $C3, $D2, $E1, $F0
.ENDM

.MACRO	M_TESTGROUP_DATA
	.DB	$F0, $E1, $D2, $C3, $B4, $A5, $96, $87
	.DB	$78, $69, $5A, $4B, $3C, $2D, $1E, $0F
.ENDM



.COMPUTESMSCHECKSUM

.MEMORYMAP

	DEFAULTSLOT	SMS_FRAME_0

	SLOTSIZE	SMS_FRAME_SIZE
	SLOT		SMS_FRAME_0	SMS_BASE_FRAME_0
	SLOT		SMS_FRAME_1	SMS_BASE_FRAME_1
	SLOT		SMS_FRAME_2	SMS_BASE_FRAME_2
	
	SLOTSIZE	SMS_RAM_SIZE
	SLOT		SMS_RAM		SMS_BASE_RAM
	SLOT		SMS_RAM_MIRROR	SMS_BASE_RAM_MIRROR

.ENDME

.ROMBANKSIZE SMS_PAGE_SIZE

.ROMBANKS SMS_ROMSIZE_CARD


.BANK CODE_PAGE

.ORG 	SMS_VECTOR_RESET
.SECTION CODE_PAGE_Start FORCE

; Note the use of the .SECTION directive.  By setting .ORG and using FORCE
; WLA-DX will attempt to force this code section into the space reserved for
; it.  If there's not enough room WLA-DX will generate an error during 
; assembly.  The .SECTION directive is used liberally throughout the code
; as a check that certain code sections are small enough to fit in the space
; reserved for them.  In this case, the .SECTION is used to ensure that the
; start up code does not collide with the interrupt vector.

CODE_PAGE_Start:
	DI
	IM	1

	LD	SP, $0DFF0

	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_DISABLE

	LD	HL, SMS_BASE_PFR_0
	LD	(HL), CODE_PAGE

	JP	SMS_BASE_FRAME_0 + CODE_PAGE_Main

.ENDS	; CODE_PAGE_Startup

.ORG	SMS_VECTOR_INT
.SECTION CODE_PAGE_INTHandler FORCE

	RETI

.ENDS	; CODE_PAGE_INTHandler


.ORG	SMS_VECTOR_NMI
.SECTION CODE_PAGE_NMIHandler FORCE

	RETN

.ENDS	; CODE_PAGE_NMIHandler


.SECTION CODE_PAGE_Main FORCE
CODE_PAGE_Main:

	; Clear VDP status
	;
	IN	A, (SMS_INPORT_VDP_STATUS)

	; Load default values into VDP registers
	;
	LD	HL, SMS_BASE_FRAME_0 + CODE_PAGE_VDPRegisterTable
	LD	B, SMS_VDP_REGISTER_COUNT
	LD	C, SMS_VDP_COMMAND_ACCESS_REG00

CODE_PAGE_Initialize_VDP_Registers_Loop:

	LD	A, (HL)
	INC	HL
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, C
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	INC	C
	DJNZ	CODE_PAGE_Initialize_VDP_Registers_Loop

	; Set up Color Palette
	;
	XOR	A	; Clear A register
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, SMS_VDP_COMMAND_ACCESS_PALETTE
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, SMS_VDP_PALETTE_BLACK
	OUT	(SMS_OUTPORT_VDP_DATA), A
	LD	A, SMS_VDP_PALETTE_WHITE
	OUT	(SMS_OUTPORT_VDP_DATA), A	

	; Clear VRAM
	;
	XOR	A	; Clear A register
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, SMS_VDP_COMMAND_ACCESS_VRAM
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	BC, $4000

CODE_PAGE_Clear_VRAM_Loop:
	
	XOR	A	; Clear A register
	OUT	(SMS_OUTPORT_VDP_DATA), A
	DEC	BC
	LD	A, B
	OR	C
	JR	NZ, CODE_PAGE_Clear_VRAM_Loop

	; Load font table into VRAM
	; This table is a sub-set of the ASCII table
	; In order to use the tiles according to their ASCII
	; values the table needs to be loaded into VRAM based
	; on the first character in the table, the comma.
	; The comma's ASCII value is: 0x27.  So, the starting
	; VRAM address is 0x27*0x20 = 0x04E0.

	; Set up access to VRAM
	;
	LD	BC, $44E0
	LD	A, C
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, B
	OUT	(SMS_OUTPORT_VDP_COMMAND), A

	LD	HL, CODE_PAGE_FontTableStart
	LD	BC, CODE_PAGE_FontTableEnd-CODE_PAGE_FontTableStart

CODE_PAGE_Load_Font_Loop:

	LD	A, (HL)
	OUT	(SMS_OUTPORT_VDP_DATA), A
	XOR	A	; Clear A register
	OUT	(SMS_OUTPORT_VDP_DATA), A
	OUT	(SMS_OUTPORT_VDP_DATA), A
	OUT	(SMS_OUTPORT_VDP_DATA), A
	INC	HL
	DEC	BC
	LD	A, B
	OR	C
	JR	NZ, CODE_PAGE_Load_Font_Loop

CODE_PAGE_Page0_Test:

	; Assume Page0Test will fail
	;
	LD	HL, SMS_BASE_RAM + RAM_PAGE0_TEST_RESULT
	LD	(HL), RESULT_FAIL

	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_FRAME_0 + COMPARE_DATA_03F0
	LD	HL, SMS_BASE_FRAME_0 + COMPARE_DATA_0400

CODE_PAGE_Page0_Test_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	NZ, SMS_BASE_FRAME_0 + CODE_PAGE_Page0_Test_Done
	JP	PE, SMS_BASE_FRAME_0 + CODE_PAGE_Page0_Test_Loop

	; Set variable indicating pass of page 0 test
	;
	LD	HL, SMS_BASE_RAM + RAM_PAGE0_TEST_RESULT
	LD	(HL), RESULT_PASS


CODE_PAGE_Page0_Test_Done:

	; Determine if required start conditions have been met
	;
	LD	HL, SMS_BASE_RAM + RAM_PAGE0_TEST_RESULT
	LD	A, (HL)
	CP	RESULT_PASS
	JP	Z, SMS_BASE_FRAME_0 + CODE_PAGE_Start_Detection
	
CODE_PAGE_Page0_Test_Fail:

	; Print screen messages
	;	
	LD	BC, CODE_PAGE_FailStringStart
	LD	DE, CODE_PAGE_FailStringEnd-CODE_PAGE_FailStringStart
	LD	HL, $0800
	CALL	SMS_BASE_FRAME_0 + PrintString

	;Turn on the screen
	LD	A, SMS_VDP_REG01_ENABLEDISPLAY
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, SMS_VDP_COMMAND_ACCESS_REG01
	OUT	(SMS_OUTPORT_VDP_COMMAND), A


CODE_PAGE_Page0_Test_Fail_Loop:
	JP	SMS_BASE_FRAME_0 + CODE_PAGE_Page0_Test_Fail_Loop


; -------------------------------------------------------------------------
; PrintString
;
; This procedure prints a string to a specific screen location
; The character data must already be loaded into VRAM
;
; This procedure is relocatable
;
; INPUTS -	BC: Pointer to string
;		DE: String length
;		H: Row number, L: Column number
;
; OUTPUT - String printed at desired screen location
;
; Valid row numbers are 0...31, the row number must never exceed 31.
; Valid column numbers are 0...27, the column number must never exceed 27.
; -------------------------------------------------------------------------
;
; PROC
PrintString:

	PUSH	AF
	PUSH	HL

	LD	A, L
	LD	L, $00

	; Multiply value row value by 32
	SRL	H
	RR	L
	SRL	H
	RR	L
	SRL	H
	RR	L

	; Add column offset
	ADD	A, L
	LD	L, A

	; Multiply by 2
	SLA	L
	RL	H

	; Add Screen display offset
	PUSH	DE
	LD	DE, $7800
	ADD	HL, DE
	POP	DE

	LD	A, L
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, H
	OUT	(SMS_OUTPORT_VDP_COMMAND), A

	; Move string pointer into HL
	LD	L, C
	LD	H, B

	; HL is pointing to the string
	; DE has the string length.

PrintString_CharacterMove:

	LD	A, (HL)
	INC	HL
	OUT	(SMS_OUTPORT_VDP_DATA), A
	SUB	A
	OUT	(SMS_OUTPORT_VDP_DATA), A
	DEC	DE
	LD	A, D
	OR 	E
	JR	NZ, PrintString_CharacterMove

	POP	HL
	POP	AF

	RET

; ENDP

CODE_PAGE_VDPRegisterTable:
	.DB	$06, $80, $FF, $FF, $FF, $FF, $FF, $00, $00, $00, $00

CODE_PAGE_FailStringStart:
	.DB " THIS TEST REQUIRES THAT EITHER "
	.DB "   ROM PAGE 0 IS MAPPED INTO    "
	.DB "  FRAME 0 AT POWER-ON/RESET OR  "
	.DB "   THAT FRAME 0 IS PAGEABLE.    "
	.DB "  ONE OF THESE CONDITIONS WAS   "
	.DB "            NOT MET.            "
	.DB "                                "
	.DB "      THE TEST HAS HALTED.      "
CODE_PAGE_FailStringEnd:

CODE_PAGE_FontTableStart: 
.INCLUDE "smsfont.inc"
CODE_PAGE_FontTableEnd:

.ENDS	; CODE_PAGE_Main


.ORG	COMPARE_DATA_03F0
.SECTION CODE_PAGE_Compare_Data_03F0 FORCE

	M_CONTROLGROUP_DATA

.ENDS	; CODE_PAGE_Compare_Data_03F0


.ORG	COMPARE_DATA_0400
.SECTION CODE_PAGE_Compare_Data_0400 FORCE

	M_CONTROLGROUP_DATA

.ENDS	; CODE_PAGE_Compare_Data_0400


CODE_PAGE_Start_Detection:

	LD	A, RESULT_NOT_TESTED
	LD	HL, SMS_BASE_RAM + RAM_FRAME0_03F0_RESULT
	LD	(HL), A
	LD	HL, SMS_BASE_RAM + RAM_FRAME0_0400_RESULT
	LD	(HL), A
	LD	HL, SMS_BASE_RAM + RAM_FRAME1_RESULT
	LD	(HL), A
	LD	HL, SMS_BASE_RAM + RAM_FRAME2_RESULT
	LD	(HL), A
	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE0_RESULT
	LD	(HL), A
	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE1_RESULT
	LD	(HL), A

	; Copy the actual page frame testing code and necessary
	; data into RAM for execution.
	;
	LD	BC, CODE_PAGE_Relocate_End - CODE_PAGE_Relocate_Start
	LD	HL, CODE_PAGE_RELOCATE_START
	LD	DE, SMS_BASE_RAM + CODE_PAGE_RELOCATE_START
	LDIR

	; Jump to RAM and execute tests from there.
	;
	JP	SMS_BASE_RAM + CODE_PAGE_Frame0_Test_Begin



.ORG CODE_PAGE_RELOCATE_START
.SECTION CODE_PAGE_Relocate_Start FORCE

CODE_PAGE_Relocate_Start:

;
; The relocate section contains all the code and data necessary to perform
; the page frame tests.  The code consists of the actual tests, as
; well as the PrintString routine required to write test output
; to the screen.  The data consists of the test output strings
; written to memory.  The compare data is located in a different
; page that is mapped into different page frames at run time.
;

CODE_PAGE_Frame0_Test_Begin:

	; Frame0_Test Algorithm
	; ---------------------
	;
	; ASSUME: CODE_PAGE (ROM Page 0) is mapped into Frame 0
	;	  If this isn't true, test would already have halted.
	;
	; Read data from Frame 0 offset COMPARE_DATA_03F0, and save in RAM
	; Read data from Frame 0 offset COMPARE_DATA_0400, and save in RAM
	; Map TEST_DATA_PAGE into Frame 0;
	; IF (saved data matches Frame 0 data at COMPARE_DATA_03F0)
	; {
	; 	Page Frame 0 0x0000-0x0400 region is fixed.
	; }
	; ELSE (saved data differs from Frame 0 data at COMPARE_DATA_03F0)
	; {
	; 	Page Frame 0 0x0000-0x0400 region is pageable.
	; }
	;
	; IF (saved data matches Frame 0 data at COMPARE_DATA_0400)
	; {
	; 	Page Frame 0 is fixed.
	; }
	; ELSE (saved data differs from Frame 0 data at COMPARE_DATA_0400)
	; {
	; 	Page Frame 0 is pageable.
	; }
	; Goto Frame 1 test
	;

	; Read the compare data from offset 0x03F0 into RAM
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_0 + COMPARE_DATA_03F0
	LDIR

	; Read the compare data from offset 0x0400 into RAM
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_2
	LD	HL, SMS_BASE_FRAME_0 + COMPARE_DATA_0400
	LDIR

	; Map COMPAREDATA_PAGE into Page Frame 0
	;
	LD	HL, SMS_BASE_PFR_0
	LD	(HL), TEST_DATA_PAGE


CODE_PAGE_Frame0_03F0_Test:

	; Compare new data with original data (saved in RAM).
	; Every byte must be different.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_0 + COMPARE_DATA_03F0

CODE_PAGE_Frame0_03F0_Test_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	Z,  SMS_BASE_RAM + CODE_PAGE_Frame0_03F0_Test_Fixed
	JP	PE, SMS_BASE_RAM + CODE_PAGE_Frame0_03F0_Test_Loop
	JP	SMS_BASE_RAM + CODE_PAGE_Frame0_03F0_Test_Pageable

CODE_PAGE_Frame0_03F0_Test_Fixed:

	; Store FIXED result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME0_03F0_RESULT
	LD	(HL), RESULT_FIXED

	JP	SMS_BASE_RAM + CODE_PAGE_Frame0_0400_Test

CODE_PAGE_Frame0_03F0_Test_Pageable:

	; Store PAGEABLE result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME0_03F0_RESULT
	LD	(HL), RESULT_PAGEABLE


CODE_PAGE_Frame0_0400_Test:

	; Compare new data with original data (saved in RAM).
	; Every byte must be different.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_2
	LD	HL, SMS_BASE_FRAME_0 + COMPARE_DATA_0400

CODE_PAGE_Frame0_0400_Test_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	Z,  SMS_BASE_RAM + CODE_PAGE_Frame0_0400_Test_Fixed
	JP	PE, SMS_BASE_RAM + CODE_PAGE_Frame0_0400_Test_Loop
	JP	SMS_BASE_RAM + CODE_PAGE_Frame0_0400_Test_Pageable

CODE_PAGE_Frame0_0400_Test_Fixed:

	; Store FIXED result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME0_0400_RESULT
	LD	(HL), RESULT_FIXED

	JP	SMS_BASE_RAM + CODE_PAGE_Frame1_Test_Begin

CODE_PAGE_Frame0_0400_Test_Pageable:

	; Store PAGEABLE result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME0_0400_RESULT
	LD	(HL), RESULT_PAGEABLE


CODE_PAGE_Frame1_Test_Begin:

	; Restore CODE_PAGE in Frame 0
	;
	LD	HL, SMS_BASE_PFR_0
	LD	(HL), CODE_PAGE


	; Frame1_Test Algorithm
	; ---------------------
	;
	; ASSUME: Nothing.  Need to test both ROM pages.
	;
	; Read data from Frame 1 offset COMPARE_DATA_0400, and save in RAM
	; Map CODE_PAGE into Frame 1;
	; IF (saved data matches Frame 1 data at COMPARE_DATA_0400)
	; {
	; 	Map TEST_DATA_PAGE into Frame 1;
	; 	IF (saved data matches Frame 1 data at COMPARE_DATA_0400)
	; 	{
	; 		Page Frame 1 is fixed.
	; 	}
	;	ELSE (saved data differs from Frame 1 data at COMPARE_DATA_0400)
	;	{
	; 		Page Frame 1 is pageable.
	;	}
	; }
	; ELSE (saved data differs from Frame 1 data at COMPARE_DATA_0400)
	; {
	; 	Page Frame 1 is pageable.
	; }
	; Goto Frame 2 test(s)

	; Read the compare data from offset COMPARE_DATA_0400 into RAM
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_1 + COMPARE_DATA_0400
	LDIR

	; Map CODE_PAGE into Page Frame 1
	;
	LD	HL, SMS_BASE_PFR_1
	LD	(HL), CODE_PAGE


CODE_PAGE_Frame1_FirstTest:

	; Compare new data with original data (saved in RAM).
	; Every byte must be different.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_1 + COMPARE_DATA_0400

CODE_PAGE_Frame1_FirstTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	Z,  SMS_BASE_RAM + CODE_PAGE_Frame1_SecondTest
	JP	PE, SMS_BASE_RAM + CODE_PAGE_Frame1_FirstTest_Loop
	JP	SMS_BASE_RAM + CODE_PAGE_Frame1_Test_Pageable

CODE_PAGE_Frame1_SecondTest:

	; Map TEST_DATA_PAGE into Page Frame 1
	;
	LD	HL, SMS_BASE_PFR_1
	LD	(HL), TEST_DATA_PAGE

	; Compare new data with original data (saved in RAM).
	; Every byte must be different.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_1 + COMPARE_DATA_0400

CODE_PAGE_Frame1_SecondTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	Z,  SMS_BASE_RAM + CODE_PAGE_Frame1_Test_Fixed
	JP	PE, SMS_BASE_RAM + CODE_PAGE_Frame1_SecondTest_Loop
	JP	SMS_BASE_RAM + CODE_PAGE_Frame1_Test_Pageable

CODE_PAGE_Frame1_Test_Fixed:

	; Store FIXED result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME1_RESULT
	LD	(HL), RESULT_FIXED

	JP	SMS_BASE_RAM + CODE_PAGE_Frame2_Test_Begin

CODE_PAGE_Frame1_Test_Pageable:

	; Store PAGEABLE result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME1_RESULT
	LD	(HL), RESULT_PAGEABLE



CODE_PAGE_Frame2_Test_Begin:

	; Frame2_Test Algorithm
	; ---------------------
	;
	; ASSUME: Nothing.  There's no specification for what should be loaded
	;	  into Frame 2 at power-on/reset.  Need to test both ROM pages.
	;
	; Disable Backup-RAM
	; Read data from Frame 2 offset COMPARE_DATA_0400, and save in RAM
	; Map CODE_PAGE into Frame 2;
	; IF (saved data matches Frame 2 data at COMPARE_DATA_0400)
	; {
	; 	Map TEST_DATA_PAGE into Frame 2;
	; 	IF (saved data matches Frame 2 data at COMPARE_DATA_0400)
	; 	{
	; 		Page Frame 2 is fixed.
	; 	}
	;	ELSE (saved data differs from Frame 2 data at COMPARE_DATA_0400)
	;	{
	; 		Page Frame 2 is pageable.
	;	}
	; }
	; ELSE (saved data differs from Frame 2 data at COMPARE_DATA_0400)
	; {
	; 	Page Frame 2 is pageable.
	; }
	; Goto Page Backup-RAM test(s)

	; Disable Backup-RAM
	;
	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_DISABLE

	; Read the compare data from offset COMPARE_DATA_0400 into RAM
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400
	LDIR

	; Map CODE_PAGE into Page Frame 2
	;
	LD	HL, SMS_BASE_PFR_2
	LD	(HL), CODE_PAGE


CODE_PAGE_Frame2_FirstTest:

	; Compare new data with original data (saved in RAM).
	; Every byte must be different.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400

CODE_PAGE_Frame2_FirstTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	Z,  SMS_BASE_RAM + CODE_PAGE_Frame2_SecondTest
	JP	PE, SMS_BASE_RAM + CODE_PAGE_Frame2_FirstTest_Loop
	JP	SMS_BASE_RAM + CODE_PAGE_Frame2_Test_Pageable

CODE_PAGE_Frame2_SecondTest:

	; Map TEST_DATA_PAGE into Page Frame 2
	;
	LD	HL, SMS_BASE_PFR_2
	LD	(HL), TEST_DATA_PAGE

	; Compare new data with original data (saved in RAM).
	; Every byte must be different.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400

CODE_PAGE_Frame2_SecondTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	Z,  SMS_BASE_RAM + CODE_PAGE_Frame2_Test_Fixed
	JP	PE, SMS_BASE_RAM + CODE_PAGE_Frame2_SecondTest_Loop
	JP	SMS_BASE_RAM + CODE_PAGE_Frame2_Test_Pageable

CODE_PAGE_Frame2_Test_Fixed:

	; Store FIXED result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME2_RESULT
	LD	(HL), RESULT_FIXED

	JP	SMS_BASE_RAM + CODE_PAGE_BackupRAM_Test_Begin

CODE_PAGE_Frame2_Test_Pageable:

	; Store PAGEABLE result in RAM
	;
	LD	HL, SMS_BASE_RAM + RAM_FRAME2_RESULT
	LD	(HL), RESULT_PAGEABLE


CODE_PAGE_BackupRAM_Test_Begin:

	; BackupRAM_Test Algorithm
	; ------------------------
	;
	; ASSUME: Nothing.
	;
	; Disable Backup-RAM
	; Read data from Frame 2 offset COMPARE_DATA_0400,
	;  and save in RAM_COMPARE_TABLE_1
	; Write inverted data to RAM_COMPARE_TABLE_2
	; Map Backup-RAM Page 0
	; Write RAM_COMPARE_TABLE_2 data to Frame 2 offset COMPARE_DATA_0400
	; IF (RAM_COMPARE_TABLE_2 data matches Frame 2 data at
	;     COMPARE_DATA_0400)
	; {
	; 	Disable Backup-RAM
	; 	IF (RAM_COMPARE_TABLE_1 data matches Frame 2 data at
	;           COMPARE_DATA_0400)
	; 	{
	; 		Back-up RAM Page 0 exists.
	; 	}
	;	ELSE (RAM_COMPARE_TABLE_1 data differs from Frame 2 data at
	;	      COMPARE_DATA_0400)
	;	{
	; 		System allows writing to any page in Frame 2.
	;	}
	; }
	; ELSE (RAM_COMPARE_TABLE_2 data differs from Frame 2 data at
	;	COMPARE_DATA_0400)
	; {
	; 	Backup-RAM Page 0 does not exist.
	; }
	;
	; Repeat above for Backup-RAM Page 1
	;
	; IF(Backup-RAM Page 0 exists AND Backup-RAM Page 1 exists)
	; {
	;	Know that: RAM_COMPARE_TABLE_2 is inverted RAM_COMPARE_TABLE_1
	;	Map Backup-RAM Page 0
	;	Write RAM_COMPARE_TABLE_1 data to Frame 2
	;	 offset COMPARE_DATA_0400
	;	Map Backup-RAM Page 1
	;	Write RAM_COMPARE_TABLE_2 data to Frame 2
	;	 offset COMPARE_DATA_0400
	;	Map Backup-RAM Page 0
	;	IF(RAM_COMPARE_TABLE_1 data matches Frame 2 data at
	;	   COMPARE_DATA_0400)
	;	{
	;		Backup-RAM Page 1 is different from Backup-RAM Page 0.
	;		System supports two backup pages.
	;	}
	;	ELSE(RAM_COMPARE_TABLE_1 data differs from Frame 2 data at
	;	     COMPARE_DATA_0400)
	;	{
	;		Backup-RAM Page 1 is the same as Backup-RAM Page 0.
	;	}
	; }
	;

	; Initialize B to loop count, C to value to write to Backup-RAM paging
	; register, DE to address to store result, and HL to address of
	; Backup-RAM paging register.
	;
	LD	B, $02
	LD	C, SMS_BACKUPRAM_MAP_PAGE0
	LD	DE, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE0_RESULT
	LD	HL, SMS_BASE_PFR_BACKUPRAM

CODE_PAGE_BackupRAM_PageTest_Begin:

	; Save register values in alternate registers
	EXX

	; Disable Backup-RAM
	;
	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_DISABLE

	; Read data from Frame 2 offset COMPARE_DATA_0400,
	; and save in RAM_COMPARE_TABLE_1
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400
	LDIR

	; Write inverted data to RAM_COMPARE_TABLE_2
	;
	LD	B, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_2
	LD	HL, SMS_BASE_RAM + RAM_COMPARE_TABLE_1

CODE_PAGE_BackupRAM_PageTest_Invert_Loop:

	LD	A, (HL)
	CPL
	LD	(DE), A
	INC	DE
	INC	HL
	DJNZ	CODE_PAGE_BackupRAM_PageTest_Invert_Loop

	; Map Backup-RAM Page
	; HL' contains SMS_BASE_PFR_BACKUPRAM, C contains data
	;
	EXX
	LD	A, C
	LD	(HL), A
	EXX
	
	; Write RAM_COMPARE_TABLE_2 data to Frame 2 offset COMPARE_DATA_0400
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_FRAME_2 + COMPARE_DATA_0400
	LD	HL, SMS_BASE_RAM + RAM_COMPARE_TABLE_2
	LDIR

CODE_PAGE_BackupRAM_PageTest_FirstTest:

	; Compare RAM_COMPARE_TABLE_2 data with Frame 2 data at
	; COMPARE_DATA_0400
	; Every byte must be the same.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_2
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400

CODE_PAGE_BackupRAM_PageTest_FirstTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	NZ,  SMS_BASE_RAM + CODE_PAGE_BackupRAM_PageTest_DoesntExist
	JP	PE, SMS_BASE_RAM + CODE_PAGE_BackupRAM_PageTest_FirstTest_Loop

CODE_PAGE_BackupRAM_PageTest_SecondTest:

	; Disable Backup-RAM
	;
	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_DISABLE
	
	; Compare RAM_COMPARE_TABLE_1 data with Frame 2 data at
	; COMPARE_DATA_0400
	; Every byte must be the same.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400

CODE_PAGE_BackupRAM_PageTest_SecondTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	NZ,  SMS_BASE_RAM + CODE_PAGE_BackupRAM_PageTest_WroteToROM
	JP	PE, SMS_BASE_RAM + CODE_PAGE_BackupRAM_PageTest_SecondTest_Loop

CODE_PAGE_BackupRAM_PageTest_Exists:

	; Store EXISTS result in RAM
	; DE' contains address for result
	;
	EXX
	EX	DE, HL
	LD	(HL), RESULT_EXISTS

	JP	SMS_BASE_RAM + CODE_PAGE_BackupRAM_PageTest_Done

CODE_PAGE_BackupRAM_PageTest_WroteToROM:

	; Store WROTETOROM result in RAM
	; DE' contains address for result
	;
	EXX
	EX	DE, HL
	LD	(HL), RESULT_WROTETOROM

	JP	SMS_BASE_RAM + CODE_PAGE_BackupRAM_PageTest_Done

CODE_PAGE_BackupRAM_PageTest_DoesntExist:

	; Store PAGEABLE result in RAM
	; DE' contains address for result
	;
	EXX
	EX	DE, HL
	LD	(HL), RESULT_DOESNTEXIST

CODE_PAGE_BackupRAM_PageTest_Done:

	EX	DE, HL
	LD	C, SMS_BACKUPRAM_MAP_PAGE0
	LD	DE, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE1_RESULT
	DJNZ	CODE_PAGE_BackupRAM_PageTest_Begin


CODE_PAGE_BackupRAM_UniquenessTest_Begin:

	; Test if Backup-RAM Page 0 exists. If not, then test is done.
	;
	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE0_RESULT
	LD	A, (HL)
	CP	RESULT_EXISTS
	JP	NZ, CODE_PAGE_BackupRAM_Test_Done

	; Test if Backup-RAM Page 1 exists.  If not, then test is done.
	;
	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE1_RESULT
	LD	A, (HL)
	CP	RESULT_EXISTS
	JP	NZ, CODE_PAGE_BackupRAM_Test_Done

	; Map Backup-RAM Page 0
	;
	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_MAP_PAGE0

	; Write RAM_COMPARE_TABLE_1 data to Frame 2 offset COMPARE_DATA_0400
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_FRAME_2 + COMPARE_DATA_0400
	LD	HL, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LDIR

	; Map Backup-RAM Page 1
	;
	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_MAP_PAGE1

	; Write RAM_COMPARE_TABLE_2 data to Frame 2 offset COMPARE_DATA_0400
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_FRAME_2 + COMPARE_DATA_0400
	LD	HL, SMS_BASE_RAM + RAM_COMPARE_TABLE_2
	LDIR

	; Map Backup-RAM Page 0
	;
	LD	HL, SMS_BASE_PFR_BACKUPRAM
	LD	(HL), SMS_BACKUPRAM_MAP_PAGE0

	; Compare RAM_COMPARE_TABLE_1 data (original data written to
	; Backup-RAM Page 0) with Frame 2 data at COMPARE_DATA_0400
	; Every byte must be the same.
	;
	LD	BC, GROUP_SIZE
	LD	DE, SMS_BASE_RAM + RAM_COMPARE_TABLE_1
	LD	HL, SMS_BASE_FRAME_2 + COMPARE_DATA_0400

CODE_PAGE_BackupRAM_UniquenessTest_Loop:

	LD	A, (DE)
	INC	DE
	CPI
	JP	NZ, SMS_BASE_RAM + CODE_PAGE_BackupRAM_UniquenessTest_PagesSame
	JP	PE, SMS_BASE_RAM + CODE_PAGE_BackupRAM_UniquenessTest_Loop
	JP	CODE_PAGE_BackupRAM_Test_Done

CODE_PAGE_BackupRAM_UniquenessTest_PagesSame:
	
	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE1_RESULT
	LD	(HL), RESULT_SAMEASPAGE0


CODE_PAGE_BackupRAM_Test_Done:

	JP	SMS_BASE_FRAME_0 + CODE_PAGE_Print_Results


CODE_PAGE_Relocate_End:
.ENDS

; Putting this ORG statement here will ensure that WLA-DX will issue a
; an error if the Relocation section is too large to fit in RAM.
;
.ORG	SMS_RAM_SIZE

CODE_PAGE_Print_Results:

	; Print screen messages
	;	
	LD	BC, CODE_PAGE_HeaderStringStart
	LD	DE, CODE_PAGE_HeaderStringEnd-CODE_PAGE_HeaderStringStart
	LD	HL, $0000
	CALL	SMS_BASE_FRAME_0 + PrintString

	LD	BC, CODE_PAGE_SeparatorStringStart
	LD	DE, CODE_PAGE_SeparatorStringEnd-CODE_PAGE_SeparatorStringStart
	LD	HL, $0900
	CALL	SMS_BASE_FRAME_0 + PrintString

	LD	BC, CODE_PAGE_TestMessagesStringStart
	LD	DE, CODE_PAGE_TestMessagesStringEnd-CODE_PAGE_TestMessagesStringStart
	LD	HL, $0A00
	CALL	SMS_BASE_FRAME_0 + PrintString

	LD	BC, CODE_PAGE_SeparatorStringStart
	LD	DE, CODE_PAGE_SeparatorStringEnd-CODE_PAGE_SeparatorStringStart
	LD	HL, $1600
	CALL	SMS_BASE_FRAME_0 + PrintString

	LD	BC, CODE_PAGE_VersionStringStart
	LD	DE, CODE_PAGE_VersionStringEnd-CODE_PAGE_VersionStringStart
	LD	HL, $1700
	CALL	SMS_BASE_FRAME_0 + PrintString

	LD	BC, CODE_PAGE_TestRunningStringStart
	LD	DE, CODE_PAGE_TestRunningStringEnd-CODE_PAGE_TestRunningStringStart
	LD	HL, $1712
	CALL	SMS_BASE_FRAME_0 + PrintString

	
CODE_PAGE_Frame0_03F0_Result:

	LD	HL, SMS_BASE_RAM + RAM_FRAME0_03F0_RESULT
	LD	A, (HL)
	CP	RESULT_PAGEABLE
	JP	Z, CODE_PAGE_Frame0_03F0_Pageable
	CP	RESULT_FIXED
	JP	Z, CODE_PAGE_Frame0_03F0_Fixed
	JP	CODE_PAGE_Frame0_0400_Result
	
CODE_PAGE_Frame0_03F0_Pageable:

	LD	BC, CODE_PAGE_PageableStringStart
	LD	DE, CODE_PAGE_PageableStringEnd-CODE_PAGE_PageableStringStart
	LD	HL, $0C12
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_Frame0_0400_Result

CODE_PAGE_Frame0_03F0_Fixed:

	LD	BC, CODE_PAGE_FixedStringStart
	LD	DE, CODE_PAGE_FixedStringEnd-CODE_PAGE_FixedStringStart
	LD	HL, $0C12
	CALL	SMS_BASE_FRAME_0 + PrintString


CODE_PAGE_Frame0_0400_Result:

	LD	HL, SMS_BASE_RAM + RAM_FRAME0_0400_RESULT
	LD	A, (HL)
	CP	RESULT_PAGEABLE
	JP	Z, CODE_PAGE_Frame0_0400_Pageable
	CP	RESULT_FIXED
	JP	Z, CODE_PAGE_Frame0_0400_Fixed
	JP	CODE_PAGE_Frame1_Result
	
CODE_PAGE_Frame0_0400_Pageable:

	LD	BC, CODE_PAGE_PageableStringStart
	LD	DE, CODE_PAGE_PageableStringEnd-CODE_PAGE_PageableStringStart
	LD	HL, $0E12
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_Frame1_Result

CODE_PAGE_Frame0_0400_Fixed:

	LD	BC, CODE_PAGE_FixedStringStart
	LD	DE, CODE_PAGE_FixedStringEnd-CODE_PAGE_FixedStringStart
	LD	HL, $0E12
	CALL	SMS_BASE_FRAME_0 + PrintString


CODE_PAGE_Frame1_Result:

	LD	HL, SMS_BASE_RAM + RAM_FRAME1_RESULT
	LD	A, (HL)
	CP	RESULT_PAGEABLE
	JP	Z, CODE_PAGE_Frame1_Pageable
	CP	RESULT_FIXED
	JP	Z, CODE_PAGE_Frame1_Fixed
	JP	CODE_PAGE_Frame2_Result
	
CODE_PAGE_Frame1_Pageable:

	LD	BC, CODE_PAGE_PageableStringStart
	LD	DE, CODE_PAGE_PageableStringEnd-CODE_PAGE_PageableStringStart
	LD	HL, $1012
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_Frame2_Result

CODE_PAGE_Frame1_Fixed:

	LD	BC, CODE_PAGE_FixedStringStart
	LD	DE, CODE_PAGE_FixedStringEnd-CODE_PAGE_FixedStringStart
	LD	HL, $1012
	CALL	SMS_BASE_FRAME_0 + PrintString


CODE_PAGE_Frame2_Result:

	LD	HL, SMS_BASE_RAM + RAM_FRAME2_RESULT
	LD	A, (HL)
	CP	RESULT_PAGEABLE
	JP	Z, CODE_PAGE_Frame2_Pageable
	CP	RESULT_FIXED
	JP	Z, CODE_PAGE_Frame2_Fixed
	JP	CODE_PAGE_BackupRAM_Page0_Result
	
CODE_PAGE_Frame2_Pageable:

	LD	BC, CODE_PAGE_PageableStringStart
	LD	DE, CODE_PAGE_PageableStringEnd-CODE_PAGE_PageableStringStart
	LD	HL, $1212
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_BackupRAM_Page0_Result

CODE_PAGE_Frame2_Fixed:

	LD	BC, CODE_PAGE_FixedStringStart
	LD	DE, CODE_PAGE_FixedStringEnd-CODE_PAGE_FixedStringStart
	LD	HL, $1212
	CALL	SMS_BASE_FRAME_0 + PrintString

CODE_PAGE_BackupRAM_Page0_Result:

	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE0_RESULT
	LD	A, (HL)
	CP	RESULT_EXISTS
	JP	Z, CODE_PAGE_BackupRAM_Page0_Exists
	CP	RESULT_WROTETOROM
	JP	Z, CODE_PAGE_BackupRAM_Page0_WroteToROM
	CP	RESULT_DOESNTEXIST
	JP	Z, CODE_PAGE_BackupRAM_Page0_DoesntExist
	JP	CODE_PAGE_BackupRAM_Page1_Result
	
CODE_PAGE_BackupRAM_Page0_Exists:

	LD	BC, CODE_PAGE_ExistsStringStart
	LD	DE, CODE_PAGE_ExistsStringEnd-CODE_PAGE_ExistsStringStart
	LD	HL, $1412
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_BackupRAM_Page1_Result

CODE_PAGE_BackupRAM_Page0_WroteToROM:

	LD	BC, CODE_PAGE_WroteToROMStringStart
	LD	DE, CODE_PAGE_WroteToROMStringEnd-CODE_PAGE_WroteToROMStringStart
	LD	HL, $1412
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_BackupRAM_Page1_Result

CODE_PAGE_BackupRAM_Page0_DoesntExist:

	LD	BC, CODE_PAGE_DoesntExistStringStart
	LD	DE, CODE_PAGE_DoesntExistStringEnd-CODE_PAGE_DoesntExistStringStart
	LD	HL, $1412
	CALL	SMS_BASE_FRAME_0 + PrintString


CODE_PAGE_BackupRAM_Page1_Result:

	LD	HL, SMS_BASE_RAM + RAM_BACKUPRAM_PAGE1_RESULT
	LD	A, (HL)
	CP	RESULT_EXISTS
	JP	Z, CODE_PAGE_BackupRAM_Page1_Exists
	CP	RESULT_SAMEASPAGE0
	JP	Z, CODE_PAGE_BackupRAM_Page1_SameAsPage0
	CP	RESULT_WROTETOROM
	JP	Z, CODE_PAGE_BackupRAM_Page1_WroteToROM
	CP	RESULT_DOESNTEXIST
	JP	Z, CODE_PAGE_BackupRAM_Page1_DoesntExist
	JP	CODE_PAGE_Test_Complete
	
CODE_PAGE_BackupRAM_Page1_Exists:

	LD	BC, CODE_PAGE_ExistsStringStart
	LD	DE, CODE_PAGE_ExistsStringEnd-CODE_PAGE_ExistsStringStart
	LD	HL, $1512
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_Test_Complete

CODE_PAGE_BackupRAM_Page1_SameAsPage0:

	LD	BC, CODE_PAGE_SameAsPage0StringStart
	LD	DE, CODE_PAGE_SameAsPage0StringEnd-CODE_PAGE_SameAsPage0StringStart
	LD	HL, $1512
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_Test_Complete

CODE_PAGE_BackupRAM_Page1_WroteToROM:

	LD	BC, CODE_PAGE_WroteToROMStringStart
	LD	DE, CODE_PAGE_WroteToROMStringEnd-CODE_PAGE_WroteToROMStringStart
	LD	HL, $1512
	CALL	SMS_BASE_FRAME_0 + PrintString
	JP	CODE_PAGE_Test_Complete

CODE_PAGE_BackupRAM_Page1_DoesntExist:

	LD	BC, CODE_PAGE_DoesntExistStringStart
	LD	DE, CODE_PAGE_DoesntExistStringEnd-CODE_PAGE_DoesntExistStringStart
	LD	HL, $1512
	CALL	SMS_BASE_FRAME_0 + PrintString


CODE_PAGE_Test_Complete:

	LD	BC, CODE_PAGE_TestCompleteStringStart
	LD	DE, CODE_PAGE_TestCompleteStringEnd-CODE_PAGE_TestCompleteStringStart
	LD	HL, $1712
	CALL	SMS_BASE_FRAME_0 + PrintString

	;Turn on the screen
	;
	LD	A, SMS_VDP_REG01_ENABLEDISPLAY
	OUT	(SMS_OUTPORT_VDP_COMMAND), A
	LD	A, SMS_VDP_COMMAND_ACCESS_REG01
	OUT	(SMS_OUTPORT_VDP_COMMAND), A

CODE_PAGE_End:
	JR	CODE_PAGE_End


CODE_PAGE_HeaderStringStart:
	.DB "   THIS TEST DETERMINES THE     "
	.DB " SYSTEM'S CURRENTLY AVAILABLE   "
	.DB " PAGING CAPABILITIES, (ASSUMING "
	.DB " THE STANDARD SEGA-STYLE SMS/GG "
	.DB " PAGING SCHEME).                "
	.DB "   IF 'TEST COMPLETE' IS NOT    "
	.DB " DISPLAYED AT THE BOTTOM OF THE "
	.DB " SCREEN THEN A FATAL TEST ERROR "
	.DB " HAS OCCURED.                   "
CODE_PAGE_HeaderStringEnd:

CODE_PAGE_SeparatorStringStart:
	.DB "--------------------------------"
CODE_PAGE_SeparatorStringEnd:

CODE_PAGE_TestMessagesStringStart:
	.DB " PAGING REGION:                 "
	.DB "                                "
	.DB "  0X0000-0X03FF:     NOT TESTED "
	.DB "                                "
	.DB "  0X0400-0X3FFF:     NOT TESTED "
	.DB "                                "
	.DB "  0X4000-0X7FFF:     NOT TESTED "
	.DB "                                "
	.DB "  0X8000-0XBFFF:     NOT TESTED "
	.DB "                                "
	.DB "  BACKUP PAGE 0:     NOT TESTED "
	.DB "         PAGE 1:     NOT TESTED "
CODE_PAGE_TestMessagesStringEnd:

CODE_PAGE_VersionStringStart:
	.DB " VER. 2.00"
CODE_PAGE_VersionStringEnd:

CODE_PAGE_TestRunningStringStart:
	.DB " TEST RUNNING"
CODE_PAGE_TestRunningStringEnd:

CODE_PAGE_TestCompleteStringStart:
	.DB "TEST COMPLETE"
CODE_PAGE_TestCompleteStringEnd:

CODE_PAGE_FixedStringStart:
	.DB "        FIXED"
CODE_PAGE_FixedStringEnd:

CODE_PAGE_PageableStringStart:
	.DB "     PAGEABLE"
CODE_PAGE_PageableStringEnd:

CODE_PAGE_ExistsStringStart:
	.DB "       EXISTS"
CODE_PAGE_ExistsStringEnd:

CODE_PAGE_SameAsPage0StringStart:
	.DB "SAME AS PAGE0"
CODE_PAGE_SameAsPage0StringEnd:

CODE_PAGE_WroteToROMStringStart:
	.DB " WROTE TO ROM"
CODE_PAGE_WroteToROMStringEnd:

CODE_PAGE_DoesntExistStringStart:
	.DB "DOESN'T EXIST"
CODE_PAGE_DoesntExistStringEnd:

; END BANK CODE_PAGE


.BANK TEST_DATA_PAGE

.ORG	COMPARE_DATA_03F0
.SECTION TEST_DATA_PAGE_Compare_Data_03F0 FORCE

	M_TESTGROUP_DATA

.ENDS	; TEST_DATA_PAGE_Compare_Data_03F0


.ORG	COMPARE_DATA_0400
.SECTION TEST_DATA_PAGE_Compare_Data_0400 FORCE

	M_TESTGROUP_DATA

.ENDS	; TEST_DATA_PAGE_Compare_Data_0400


.ORG	TEST_DATA_PAGE_TITLE
.SECTION TEST_DATA_PAGE_Title FORCE
	.DB "PFR_Detect"
	.DB $00
.ENDS

.ORG	TEST_DATA_PAGE_RELEASE_NOTES
.SECTION TEST_DATA_PAGE_Release_Notes FORCE
	.DB "Author: Eric R. Quinn"
	.DB $0D,$0D

	.DB "This program determines the system's currently available paging "
	.DB "capabilities, (assuming the standard SEGA-style SMS/GG paging "
	.DB "scheme)."
	.DB $0D, $0D

	.DB "The program requires that all of ROM page 0 is available before "
	.DB "the detection routines can be executed.  ROM page 0 is made "
	.DB "available in one of two ways: Either ROM page 0 is already mapped "
	.DB "into Frame 0 at power-on/reset, or Frame 0 is pageable which "
	.DB "allows the program to map the page itself.  If neither of these "
	.DB "conditions is met the test will display an error message and halt."
	.DB $0D, $0D

	.DB "If the above condition is satisfied, the program will attempt to "
	.DB "determine the paging capabilities of Frame 0, 1 and 2, including "
	.DB "testing for paging of the first 1KByte region in Frame 0."
	.DB $0D, $0D

	.DB "The program will also attempt to detect the presence of up to two "
	.DB "Backup-RAM pages.  The test will attempt to determine if the "
	.DB "pages exist, or if the system simply allows writing to any page "
	.DB " in Frame 2. If two backup-RAM pages do exist, the program will "
	.DB "attempt to determine if the two pages are unique.  The results "
	.DB "for the Backup-RAM page tests are explained below:"
	.DB $0D, $0D

	.DB "DOESN'T EXIST: The Backup-RAM page could not be mapped into "	
	.DB "Frame 2."
	.DB $0D, $0D

	.DB "WROTE TO ROM: The system allows writing to any page mapped into "
	.DB "Frame 2.  Because of this, the program was unable to determine if "
	.DB "a unique Backup-RAM page exists."
	.DB $0D, $0D

	.DB "EXISTS: The Backup-RAM page exists."
	.DB $0D, $0D

	.DB "SAME AS PAGE 0: The program determined that only one Backup-RAM "
	.DB "page exists, regardless of the page number written to the "
	.DB "Backup-RAM paging register (0xFFFC)."
	.DB $0D, $0D

	.DB $00
.ENDS

.ORG	SMS_OFFSET_SDSC_TAG
.SECTION TEST_DATA_PAGE_SDSC_Tag FORCE

	.DB	"SDSC"		; SDSC Tag
	.DB	$02		; Major revision
	.DB	$00		; Minor revision
	.DB	$20		; Day
	.DB	$03		; Month
	.DW	$2001		; Year
	.DW	$0000		; RESERVED
	.DW	SMS_BASE_FRAME_1 + TEST_DATA_PAGE_TITLE
	.DW	SMS_BASE_FRAME_1 + TEST_DATA_PAGE_RELEASE_NOTES

.ENDS	; TEST_DATA_PAGE_SDSC_Tag


.ORG	SMS_OFFSET_SEGA_TAG
.SECTION TEST_DATA_PAGE_Sega_Tag FORCE

	.DB	"TMR SEGA"	; Trademark
	.DW	$0020		; Year
	.DW	$0000		; Checksum not correct 
	.DW	$0000		; Part Num not correct
	.DW	$2207		; Day / Month

.ENDS	; TEST_DATA_PAGE_Sega_Tag

; END BANK TEST_DATA_PAGE

; END PFR_Detect