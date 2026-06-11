// Minimal PDB (MSF7) writer.
// Emits TPI type records for parsed C structs so WinDbg's native `dt` can use them.

use std::collections::HashMap;
use std::path::Path;

use crate::types::*;

// ── Public entry point ────────────────────────────────────────────────────────

pub fn write_pdb(defs: &[&StructDef], path: &Path) -> anyhow::Result<()> {
    let ctx = TypeCtx::new(defs);
    let tpi_bytes = ctx.emit(defs);

    let stream0: &[u8] = &[];
    let stream1 = pdb_info_stream();
    let stream2 = tpi_stream(&tpi_bytes);
    let stream3 = dbi_stream();
    let stream4 = empty_tpi_stream(); // IPI

    write_msf(path, &[stream0, &stream1, &stream2, &stream3, &stream4])
}

// ── CodeView type index constants ─────────────────────────────────────────────

const T_NOTYPE: u32 = 0x0000;
const T_VOID: u32 = 0x0003;
const T_CHAR: u32 = 0x0070;
const T_WCHAR: u32 = 0x0071;
const T_UCHAR: u32 = 0x0020;
const T_USHORT: u32 = 0x0021;
const T_ULONG: u32 = 0x0022;
const T_UQUAD: u32 = 0x0023;
const T_INT1: u32 = 0x0068;
const T_INT2: u32 = 0x0072;
const T_INT4: u32 = 0x0074;
const T_INT8: u32 = 0x0076;
const T_BOOL08: u32 = 0x0030;
const T_REAL32: u32 = 0x0040;
const T_REAL64: u32 = 0x0041;
const T_64PVOID: u32 = 0x0603; // 64-bit pointer to void

fn prim_cv(p: &PrimType) -> u32 {
    match p {
        PrimType::U8 => T_UCHAR,
        PrimType::U16 => T_USHORT,
        PrimType::U32 => T_ULONG,
        PrimType::U64 => T_UQUAD,
        PrimType::I8 => T_INT1,
        PrimType::I16 => T_INT2,
        PrimType::I32 => T_INT4,
        PrimType::I64 => T_INT8,
        PrimType::F32 => T_REAL32,
        PrimType::F64 => T_REAL64,
        PrimType::Bool => T_BOOL08,
        PrimType::Char => T_CHAR,
        PrimType::WChar => T_WCHAR,
        PrimType::Void => T_VOID,
    }
}

// ── Type context: assigns indices and emits records ───────────────────────────

struct TypeCtx {
    // struct name → (fieldlist_idx, struct_idx)
    structs: HashMap<String, (u32, u32)>,
    // (elem_cv_idx, array_byte_size) → array_idx
    arrays: HashMap<(u32, usize), u32>,
    // pointee_cv_idx → pointer_idx  (for LF_POINTER records)
    pointers: HashMap<u32, u32>,
    next: u32,
}

impl TypeCtx {
    fn new(defs: &[&StructDef]) -> Self {
        let mut ctx = Self {
            structs: HashMap::new(),
            arrays: HashMap::new(),
            pointers: HashMap::new(),
            next: 0x1000,
        };
        // Phase 1: assign indices for all structs
        for def in defs {
            let fl = ctx.next;
            ctx.next += 1;
            let st = ctx.next;
            ctx.next += 1;
            ctx.structs.insert(def.name.clone(), (fl, st));
        }
        // Phase 2: assign indices for arrays and struct-pointers
        for def in defs {
            for field in &def.fields {
                ctx.preallocate_field(&field.ty, defs);
            }
        }
        ctx
    }

    fn preallocate_field(&mut self, ty: &FieldType, defs: &[&StructDef]) {
        match ty {
            FieldType::Arr(inner, n) => {
                let elem_cv = self.field_cv_no_alloc(inner);
                let byte_size = Self::calc_field_byte_size(inner, defs, &self.structs);
                let key = (elem_cv, byte_size * n);
                if !self.arrays.contains_key(&key) {
                    let idx = self.next;
                    self.next += 1;
                    self.arrays.insert(key, idx);
                }
                self.preallocate_field(inner, defs);
            }
            FieldType::Ptr(inner) => {
                if let FieldType::Named(name) = inner.as_ref() {
                    if let Some(&(_, st_idx)) = self.structs.get(name) {
                        if !self.pointers.contains_key(&st_idx) {
                            let idx = self.next;
                            self.next += 1;
                            self.pointers.insert(st_idx, idx);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Returns CV type index without allocating new types (used during preallocate)
    fn field_cv_no_alloc(&self, ty: &FieldType) -> u32 {
        match ty {
            FieldType::Prim(p) => prim_cv(p),
            FieldType::Ptr(inner) => match inner.as_ref() {
                FieldType::Prim(PrimType::Void) => T_64PVOID,
                FieldType::Prim(p) => (6u32 << 8) | prim_cv(p),
                FieldType::Named(name) => {
                    self.structs.get(name).map(|&(_, si)| {
                        self.pointers.get(&si).copied().unwrap_or(T_64PVOID)
                    }).unwrap_or(T_64PVOID)
                }
                _ => T_64PVOID,
            },
            FieldType::Arr(inner, n) => {
                let elem_cv = self.field_cv_no_alloc(inner);
                // look up array index
                // (byte size unknown here, so use 0 as placeholder - will be fixed in emit)
                let _ = (elem_cv, n);
                T_NOTYPE // placeholder; actual idx looked up in emit
            }
            FieldType::Named(name) => {
                self.structs.get(name).map(|&(_, si)| si).unwrap_or(T_VOID)
            }
        }
    }

    fn field_cv(&self, ty: &FieldType, defs: &[&StructDef]) -> u32 {
        match ty {
            FieldType::Prim(p) => prim_cv(p),
            FieldType::Ptr(inner) => match inner.as_ref() {
                FieldType::Prim(PrimType::Void) => T_64PVOID,
                FieldType::Prim(p) => (6u32 << 8) | prim_cv(p),
                FieldType::Named(name) => {
                    self.structs.get(name).and_then(|&(_, si)| {
                        self.pointers.get(&si).copied()
                    }).unwrap_or(T_64PVOID)
                }
                _ => T_64PVOID,
            },
            FieldType::Arr(inner, n) => {
                let elem_cv = self.field_cv(inner, defs);
                let byte_size = Self::calc_field_byte_size(inner, defs, &self.structs);
                let key = (elem_cv, byte_size * n);
                self.arrays.get(&key).copied().unwrap_or(T_VOID)
            }
            FieldType::Named(name) => {
                self.structs.get(name).map(|&(_, si)| si).unwrap_or(T_VOID)
            }
        }
    }

    fn calc_field_byte_size(
        ty: &FieldType,
        defs: &[&StructDef],
        struct_map: &HashMap<String, (u32, u32)>,
    ) -> usize {
        match ty {
            FieldType::Prim(p) => p.size(),
            FieldType::Ptr(_) => 8,
            FieldType::Arr(inner, n) => {
                Self::calc_field_byte_size(inner, defs, struct_map) * n
            }
            FieldType::Named(name) => {
                defs.iter().find(|d| d.name == *name).map(|d| d.total_size).unwrap_or(8)
            }
        }
    }

    fn emit(&self, defs: &[&StructDef]) -> Vec<u8> {
        // Collect all records keyed by type index
        let mut records: Vec<(u32, Vec<u8>)> = Vec::new();

        // Struct fieldlists and struct records
        for def in defs {
            if let Some(&(fl_idx, st_idx)) = self.structs.get(&def.name) {
                records.push((fl_idx, self.emit_fieldlist(def, defs)));
                records.push((st_idx, emit_structure(def, fl_idx)));
            }
        }

        // Array records
        for (&(elem_cv, total_bytes), &arr_idx) in &self.arrays {
            records.push((arr_idx, emit_array(elem_cv, total_bytes)));
        }

        // Pointer records (to struct types)
        for (&pointee_idx, &ptr_idx) in &self.pointers {
            records.push((ptr_idx, emit_pointer64(pointee_idx)));
        }

        // Sort by index and concatenate with length prefixes
        records.sort_unstable_by_key(|(idx, _)| *idx);

        let mut out = Vec::new();
        for (_, rec) in records {
            let len = rec.len() as u16;
            push_u16(&mut out, len);
            out.extend_from_slice(&rec);
        }
        out
    }

    fn emit_fieldlist(&self, def: &StructDef, defs: &[&StructDef]) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        push_u16(&mut body, 0x1203); // LF_FIELDLIST

        for field in &def.fields {
            let member_start = body.len();
            push_u16(&mut body, 0x150D); // LF_MEMBER
            push_u16(&mut body, 0x0003); // attr: public
            push_u32(&mut body, self.field_cv(&field.ty, defs));
            emit_numeric(&mut body, field.offset as u64);
            body.extend_from_slice(field.name.as_bytes());
            body.push(0); // null terminator
            // Pad to 4-byte alignment within the fieldlist body
            let member_len = body.len() - member_start;
            let pad = (4 - (member_len & 3)) & 3;
            for i in 0..pad {
                body.push(0xF0 + (pad - i) as u8);
            }
        }

        body
    }
}

fn emit_structure(def: &StructDef, fieldlist_idx: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u16(&mut body, 0x1505); // LF_STRUCTURE
    push_u16(&mut body, def.fields.len() as u16); // count
    push_u16(&mut body, 0x0000); // property
    push_u32(&mut body, fieldlist_idx); // field type index
    push_u32(&mut body, 0); // derived
    push_u32(&mut body, 0); // vshape
    emit_numeric(&mut body, def.total_size as u64);
    body.extend_from_slice(def.name.as_bytes());
    body.push(0);
    // unique name (mangled) — use same as name
    body.extend_from_slice(def.name.as_bytes());
    body.push(0);
    pad4(&mut body);
    body
}

fn emit_array(elem_cv: u32, total_bytes: usize) -> Vec<u8> {
    let mut body = Vec::new();
    push_u16(&mut body, 0x1503); // LF_ARRAY
    push_u32(&mut body, elem_cv); // element type
    push_u32(&mut body, T_UQUAD); // index type (size_t = uint64)
    emit_numeric(&mut body, total_bytes as u64);
    body.push(0); // empty name
    pad4(&mut body);
    body
}

fn emit_pointer64(pointee_cv: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u16(&mut body, 0x1002); // LF_POINTER
    push_u32(&mut body, pointee_cv); // referent type
    // attr: PointerKind=Near64(0xC), PointerMode=Pointer(0), no flags
    // attr = kind | (mode << 5) | (size << 13)
    // Near64 = 0xC, mode 0, size = 8 → (8 << 13) | 0 | 0xC = 0x1000C
    push_u32(&mut body, 0x0000_100C);
    body
}

// ── Leaf numeric encoding ─────────────────────────────────────────────────────

fn emit_numeric(out: &mut Vec<u8>, val: u64) {
    if val < 0x8000 {
        push_u16(out, val as u16);
    } else if val <= 0xFFFF {
        push_u16(out, 0x8002); // LF_USHORT
        push_u16(out, val as u16);
    } else if val <= 0xFFFF_FFFF {
        push_u16(out, 0x8004); // LF_ULONG
        push_u32(out, val as u32);
    } else {
        push_u16(out, 0x8006); // LF_UQUADWORD
        push_u64(out, val);
    }
}

fn pad4(body: &mut Vec<u8>) {
    let pad = (4 - (body.len() & 3)) & 3;
    for i in 0..pad {
        body.push(0xF0 + (pad - i) as u8);
    }
}

// ── Stream builders ───────────────────────────────────────────────────────────

fn pdb_info_stream() -> Vec<u8> {
    let mut s = Vec::new();
    push_u32(&mut s, 20000404u32); // Version VC70
    push_u32(&mut s, 1u32);         // Signature (timestamp)
    push_u32(&mut s, 1u32);         // Age
    // GUID (16 bytes, fixed non-zero value)
    s.extend_from_slice(&[
        0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67,
        0x89, 0xAB, 0xCD, 0xEF, 0x00, 0x11, 0x22, 0x33,
    ]);
    // Named stream map: empty
    push_u32(&mut s, 0); // string buffer size
    push_u32(&mut s, 0); // hash capacity
    push_u32(&mut s, 0); // hash size
    // no present bitvec words, no deleted bitvec words, no entries
    // Feature codes
    push_u32(&mut s, 20140508u32); // PdbFeatureSignature::VC140
    s
}

fn tpi_header(num_records: u32, record_bytes: u32) -> Vec<u8> {
    let mut h = Vec::new();
    push_u32(&mut h, 0x8000_0006u32); // Version V80
    push_u32(&mut h, 56u32);           // HeaderSize
    push_u32(&mut h, 0x1000u32);        // TypeIndexBegin
    push_u32(&mut h, 0x1000u32 + num_records); // TypeIndexEnd
    push_u32(&mut h, record_bytes);    // TypeRecordBytes
    push_u16(&mut h, 0xFFFFu16);       // HashStreamIndex (none)
    push_u16(&mut h, 0xFFFFu16);       // HashAuxStreamIndex
    push_u32(&mut h, 4u32);             // HashKeySize
    push_u32(&mut h, 0x0003_FFFFu32);  // NumHashBuckets
    push_i32(&mut h, 0);               // HashValueBufferOffset
    push_u32(&mut h, 0);               // HashValueBufferLength
    push_i32(&mut h, 0);               // IndexOffsetBufferOffset
    push_u32(&mut h, 0);               // IndexOffsetBufferLength
    push_i32(&mut h, 0);               // HashAdjBufferOffset
    push_u32(&mut h, 0);               // HashAdjBufferLength
    h
}

fn tpi_stream(records: &[u8]) -> Vec<u8> {
    let num_records = count_records(records);
    let mut s = tpi_header(num_records, records.len() as u32);
    s.extend_from_slice(records);
    s
}

fn empty_tpi_stream() -> Vec<u8> {
    tpi_stream(&[])
}

fn count_records(data: &[u8]) -> u32 {
    let mut pos = 0;
    let mut count = 0u32;
    while pos + 2 <= data.len() {
        let len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2 + len;
        count += 1;
    }
    count
}

fn dbi_stream() -> Vec<u8> {
    let mut s = Vec::new();
    push_i32(&mut s, -1i32);         // VersionSignature
    push_u32(&mut s, 19990903u32);   // VersionHeader V70
    push_u32(&mut s, 1u32);           // Age
    push_i16(&mut s, -1i16);          // GlobalStreamIndex
    push_u16(&mut s, 0x8000u16);      // BuildNumber (EC flag)
    push_i16(&mut s, -1i16);          // PublicsStreamIndex
    push_u16(&mut s, 0u16);           // PdbDllVersion
    push_i16(&mut s, -1i16);          // SymRecordStream
    push_u16(&mut s, 0u16);           // PdbDllRbld
    push_i32(&mut s, 0i32);           // ModInfoSize
    push_i32(&mut s, 0i32);           // SectionContributionSize
    push_i32(&mut s, 0i32);           // SectionMapSize
    push_i32(&mut s, 0i32);           // SourceInfoSize
    push_i32(&mut s, 0i32);           // TypeServerMapSize
    push_u32(&mut s, 0u32);           // MFCTypeServerIndex
    push_i32(&mut s, 0i32);           // OptionalDbgHeaderSize
    push_i32(&mut s, 0i32);           // ECSubstreamSize
    push_u16(&mut s, 0u16);           // Flags
    push_u16(&mut s, 0x8664u16);      // Machine AMD64
    push_u32(&mut s, 0u32);           // Padding
    s
}

// ── MSF (Multi-Stream File) writer ────────────────────────────────────────────

const BLOCK_SIZE: usize = 4096;

fn write_msf(path: &Path, streams: &[&[u8]]) -> anyhow::Result<()> {
    // Lay out all stream blocks.
    // Blocks 0=superblock, 1=FPM1, 2=FPM2, then stream data, then directory, then block map.

    let mut blocks: Vec<Vec<u8>> = Vec::new();
    blocks.push(vec![0u8; BLOCK_SIZE]); // 0: superblock (filled last)
    blocks.push(vec![0xFFu8; BLOCK_SIZE]); // 1: FPM1
    blocks.push(vec![0xFFu8; BLOCK_SIZE]); // 2: FPM2

    let mut stream_block_lists: Vec<Vec<u32>> = Vec::new();

    for data in streams {
        let mut list = Vec::new();
        if data.is_empty() {
            // Empty stream: one block but 0 bytes, actually no blocks needed.
            // We still push an empty list.
        } else {
            let mut offset = 0;
            while offset < data.len() {
                let end = (offset + BLOCK_SIZE).min(data.len());
                let mut block = vec![0u8; BLOCK_SIZE];
                block[..end - offset].copy_from_slice(&data[offset..end]);
                list.push(blocks.len() as u32);
                blocks.push(block);
                offset += BLOCK_SIZE;
            }
        }
        stream_block_lists.push(list);
    }

    // Build directory bytes
    let dir_bytes = build_directory(streams, &stream_block_lists);

    // Allocate blocks for directory
    let mut dir_block_list: Vec<u32> = Vec::new();
    {
        let mut offset = 0;
        while offset < dir_bytes.len() {
            let end = (offset + BLOCK_SIZE).min(dir_bytes.len());
            let mut block = vec![0u8; BLOCK_SIZE];
            block[..end - offset].copy_from_slice(&dir_bytes[offset..end]);
            dir_block_list.push(blocks.len() as u32);
            blocks.push(block);
            offset += BLOCK_SIZE;
        }
        if dir_bytes.is_empty() {
            let idx = blocks.len() as u32;
            blocks.push(vec![0u8; BLOCK_SIZE]);
            dir_block_list.push(idx);
        }
    }

    // Block map block: contains the list of directory block indices
    let block_map_idx = blocks.len() as u32;
    {
        let mut block = vec![0u8; BLOCK_SIZE];
        for (i, &bi) in dir_block_list.iter().enumerate() {
            let off = i * 4;
            block[off..off + 4].copy_from_slice(&bi.to_le_bytes());
        }
        blocks.push(block);
    }

    // Fill superblock
    let superblock = build_superblock(
        blocks.len() as u32,
        dir_bytes.len() as u32,
        block_map_idx,
    );
    blocks[0][..superblock.len()].copy_from_slice(&superblock);

    // Write file
    let mut file = Vec::with_capacity(blocks.len() * BLOCK_SIZE);
    for block in &blocks {
        file.extend_from_slice(block);
    }
    std::fs::write(path, &file)?;
    Ok(())
}

fn build_directory(streams: &[&[u8]], block_lists: &[Vec<u32>]) -> Vec<u8> {
    let mut d = Vec::new();
    push_u32(&mut d, streams.len() as u32);
    for stream in streams {
        push_u32(&mut d, stream.len() as u32);
    }
    for list in block_lists {
        for &bi in list {
            push_u32(&mut d, bi);
        }
    }
    d
}

fn build_superblock(num_blocks: u32, dir_size: u32, block_map_addr: u32) -> Vec<u8> {
    let mut s: Vec<u8> = Vec::with_capacity(BLOCK_SIZE);
    s.extend_from_slice(b"Microsoft C/C++ MSF 7.00\r\n\x1a\x44\x53\x00\x00\x00");
    push_u32(&mut s, BLOCK_SIZE as u32); // BlockSize
    push_u32(&mut s, 1u32);               // FreeBlockMapBlock
    push_u32(&mut s, num_blocks);          // NumBlocks
    push_u32(&mut s, dir_size);            // NumDirectoryBytes
    push_u32(&mut s, 0u32);               // Unknown
    push_u32(&mut s, block_map_addr);      // BlockMapAddr
    s.resize(BLOCK_SIZE, 0);
    s
}

// ── Byte helpers ──────────────────────────────────────────────────────────────

fn push_u16(v: &mut Vec<u8>, n: u16) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_u32(v: &mut Vec<u8>, n: u32) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_u64(v: &mut Vec<u8>, n: u64) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_i16(v: &mut Vec<u8>, n: i16) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_i32(v: &mut Vec<u8>, n: i32) { v.extend_from_slice(&n.to_le_bytes()); }
