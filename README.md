# Windbg Struct Importer

## Build

```
cargo build --release
```

## Usage

In WinDbg:

```
.load path\to\struct_importer.dll

!importhdr C:\program files (x86)\Windows Kits\10\Include\10.0.26100.0\shared\ntddkbd.h

!dt _KEYBOARD_INPUT_DATA ffff930a`f8e38b00
[dbgeng-rs] _KEYBOARD_INPUT_DATA @ 0xffff930af8e38b00  (size = 0xc)
[dbgeng-rs]    +0x0000 UnitId               : 0x0000 (0u)
[dbgeng-rs]    +0x0002 MakeCode             : 0x0036 (54u)
[dbgeng-rs]    +0x0004 Flags                : 0x0001 (1u)
[dbgeng-rs]    +0x0006 Reserved             : 0x0000 (0u)
[dbgeng-rs]    +0x0008 ExtraInformation     : 0x00000000 (0u)
```
