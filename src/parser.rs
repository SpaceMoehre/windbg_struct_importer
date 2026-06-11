use crate::types::*;

pub const PTR_SIZE: usize = 8;

// ── Tokenizer ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(usize),
    LBrace,
    RBrace,
    Semi,
    Star,
    LBracket,
    RBracket,
    Colon,
    Comma,
    LParen,
    RParen,
}

fn tokenize(src: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;

    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Line comment
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        // Preprocessor directive — skip to end of logical line
        if b[i] == b'#' {
            while i < b.len() {
                if b[i] == b'\\' {
                    i += 1;
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                } else if b[i] == b'\n' {
                    break;
                }
                i += 1;
            }
            continue;
        }

        match b[i] {
            b'{' => { tokens.push(Token::LBrace); i += 1; }
            b'}' => { tokens.push(Token::RBrace); i += 1; }
            b';' => { tokens.push(Token::Semi); i += 1; }
            b'*' => { tokens.push(Token::Star); i += 1; }
            b'[' => { tokens.push(Token::LBracket); i += 1; }
            b']' => { tokens.push(Token::RBracket); i += 1; }
            b',' => { tokens.push(Token::Comma); i += 1; }
            b'(' => { tokens.push(Token::LParen); i += 1; }
            b')' => { tokens.push(Token::RParen); i += 1; }
            b':' => {
                if i + 1 < b.len() && b[i + 1] == b':' {
                    i += 2; // skip C++ ::
                } else {
                    tokens.push(Token::Colon);
                    i += 1;
                }
            }
            c if c.is_ascii_digit() => {
                let start = i;
                if i + 1 < b.len() && b[i] == b'0' && (b[i + 1] == b'x' || b[i + 1] == b'X') {
                    i += 2;
                    while i < b.len() && b[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                    let n = usize::from_str_radix(&src[start + 2..i], 16).unwrap_or(0);
                    tokens.push(Token::Number(n));
                } else {
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    let n = src[start..i].parse().unwrap_or(0);
                    tokens.push(Token::Number(n));
                }
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                tokens.push(Token::Ident(src[start..i].to_string()));
            }
            _ => { i += 1; }
        }
    }

    tokens
}

// ── Type mapping ─────────────────────────────────────────────────────────────

fn ident_to_field_type(name: &str) -> Option<FieldType> {
    let ty = match name {
        // 1-byte unsigned
        "BYTE" | "UCHAR" | "uint8_t" | "UINT8" | "BOOLEAN" => FieldType::Prim(PrimType::U8),
        // 1-byte signed
        "INT8" | "int8_t" => FieldType::Prim(PrimType::I8),
        "char" => FieldType::Prim(PrimType::Char),
        "CHAR" => FieldType::Prim(PrimType::Char),
        "bool" => FieldType::Prim(PrimType::Bool),

        // 2-byte
        "WORD" | "USHORT" | "uint16_t" | "UINT16" => FieldType::Prim(PrimType::U16),
        "WCHAR" => FieldType::Prim(PrimType::WChar),
        "SHORT" | "int16_t" | "INT16" => FieldType::Prim(PrimType::I16),

        // 4-byte unsigned
        "DWORD" | "ULONG" | "UINT" | "UINT32" | "uint32_t" | "ULONG32" | "DWORD32" => {
            FieldType::Prim(PrimType::U32)
        }
        // 4-byte signed
        "LONG" | "INT" | "INT32" | "int32_t" | "LONG32" | "NTSTATUS" | "HRESULT"
        | "LSTATUS" | "BOOL" => FieldType::Prim(PrimType::I32),
        "int" => FieldType::Prim(PrimType::I32),
        "float" => FieldType::Prim(PrimType::F32),

        // 8-byte unsigned
        "DWORD64" | "QWORD" | "ULONGLONG" | "ULONG64" | "uint64_t" | "UINT64" => {
            FieldType::Prim(PrimType::U64)
        }
        // 8-byte signed
        "LONGLONG" | "LONG64" | "int64_t" | "INT64" | "__int64" => FieldType::Prim(PrimType::I64),
        "double" => FieldType::Prim(PrimType::F64),

        // Pointer-sized opaque handles / pointer typedefs
        "PVOID" | "LPVOID" | "HANDLE" | "HMODULE" | "HINSTANCE" | "HWND" | "HMENU"
        | "HDC" | "HPEN" | "HBRUSH" | "HFONT" | "HBITMAP" | "HGDIOBJ" | "HRGN"
        | "HACCEL" | "HCURSOR" | "HICON" | "HKEY" | "HLOCAL" | "HGLOBAL" | "HFILE"
        | "HDESK" | "HWINSTA" | "HMONITOR" | "HTASK" | "HRSRC" | "HHOOK"
        | "HCOLORSPACE" | "HPALETTE" | "HMETAFILE" | "HENHMETAFILE"
        | "SC_HANDLE" | "SERVICE_STATUS_HANDLE"
        | "LPSTR" | "LPCSTR" | "LPWSTR" | "LPCWSTR" | "PWSTR" | "PCWSTR" | "PSTR" | "PCSTR"
        | "PUCHAR" | "PCHAR" | "PBYTE" | "PWORD" | "PDWORD" | "PULONG" | "PULONG64"
        | "SIZE_T" | "ULONG_PTR" | "LONG_PTR" | "DWORD_PTR" | "UINT_PTR" | "INT_PTR"
        | "POINTER_64" | "POINTER_32"
        | "FARPROC" | "PROC" | "NEARPROC"
        => FieldType::Ptr(Box::new(FieldType::Prim(PrimType::Void))),

        "void" => FieldType::Prim(PrimType::Void),

        _ => return None,
    };
    Some(ty)
}

fn multiword_to_field_type(words: &[&str]) -> FieldType {
    let s: String = words.join(" ");
    match s.as_str() {
        "unsigned char" => FieldType::Prim(PrimType::U8),
        "signed char" => FieldType::Prim(PrimType::I8),
        "unsigned short" | "unsigned short int" => FieldType::Prim(PrimType::U16),
        "short int" | "signed short" | "signed short int" => FieldType::Prim(PrimType::I16),
        "unsigned int" | "unsigned" => FieldType::Prim(PrimType::U32),
        "signed int" | "signed" => FieldType::Prim(PrimType::I32),
        "unsigned long" | "unsigned long int" => FieldType::Prim(PrimType::U32),
        "long" | "long int" | "signed long" | "signed long int" => FieldType::Prim(PrimType::I32),
        "unsigned long long" | "unsigned long long int" => FieldType::Prim(PrimType::U64),
        "long long" | "long long int" | "signed long long" | "signed long long int" => {
            FieldType::Prim(PrimType::I64)
        }
        "long double" => FieldType::Prim(PrimType::F64),
        _ => FieldType::Named(s),
    }
}

// ── Raw field (before layout computation) ────────────────────────────────────

#[derive(Debug, Clone)]
struct RawField {
    name: String,
    base_type: String,
    ptr_depth: usize,
    array_size: Option<usize>,
    bit_size: Option<u8>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_struct_keyword(s: &str) -> bool {
    matches!(s, "struct" | "union" | "enum")
}

fn is_qualifier(s: &str) -> bool {
    matches!(
        s,
        "const" | "volatile" | "unsigned" | "signed" | "static" | "extern" | "inline"
            | "__inline" | "__forceinline" | "FORCEINLINE" | "FAR" | "NEAR"
            | "UNALIGNED" | "_UNALIGNED" | "__unaligned" | "RESTRICTED_POINTER"
            | "__restrict" | "WINAPI" | "NTAPI" | "CALLBACK" | "APIENTRY"
            | "__cdecl" | "__stdcall" | "__fastcall" | "__thiscall"
    )
}

fn is_prim_keyword(s: &str) -> bool {
    matches!(
        s,
        "void" | "int" | "char" | "float" | "double" | "long" | "short"
    )
}

fn is_reserved(s: &str) -> bool {
    is_struct_keyword(s)
        || is_qualifier(s)
        || matches!(s, "typedef" | "register" | "auto" | "mutable" | "virtual")
}

fn skip_attribute(tokens: &[Token], pos: &mut usize) {
    loop {
        match tokens.get(*pos) {
            Some(Token::Ident(s))
                if matches!(
                    s.as_str(),
                    "__declspec"
                        | "__attribute__"
                        | "DECLSPEC_ALIGN"
                        | "DECLSPEC_CACHEALIGN"
                        | "DECLSPEC_NORETURN"
                        | "DECLSPEC_NOTHROW"
                ) =>
            {
                *pos += 1;
                if matches!(tokens.get(*pos), Some(Token::LParen)) {
                    let mut depth = 1usize;
                    *pos += 1;
                    while *pos < tokens.len() && depth > 0 {
                        match &tokens[*pos] {
                            Token::LParen => depth += 1,
                            Token::RParen => depth -= 1,
                            _ => {}
                        }
                        *pos += 1;
                    }
                }
            }
            _ => break,
        }
    }
}

fn align_up(offset: usize, align: usize) -> usize {
    if align <= 1 {
        return offset;
    }
    (offset + align - 1) & !(align - 1)
}

// ── Field declaration parser ──────────────────────────────────────────────────

fn try_parse_field_decl(tokens: &[Token], pos: &mut usize) -> Option<Vec<RawField>> {
    let start = *pos;

    // Skip qualifiers and attributes before the type
    loop {
        match tokens.get(*pos) {
            Some(Token::Ident(s)) if is_qualifier(s) => {
                *pos += 1;
            }
            _ => break,
        }
    }
    skip_attribute(tokens, pos);

    // --- Determine base type ---
    let base_type: String = match tokens.get(*pos) {
        // struct/union/enum tag reference
        Some(Token::Ident(s)) if is_struct_keyword(s) => {
            *pos += 1;
            skip_attribute(tokens, pos);
            match tokens.get(*pos) {
                Some(Token::Ident(tag)) if !is_reserved(tag) => {
                    let t = tag.clone();
                    *pos += 1;
                    t
                }
                _ => {
                    *pos = start;
                    return None;
                }
            }
        }

        // Multi-word primitive: long long, unsigned int, etc.
        Some(Token::Ident(s)) if is_prim_keyword(s) => {
            let mut words: Vec<String> = Vec::new();
            while let Some(Token::Ident(w)) = tokens.get(*pos) {
                if is_prim_keyword(w) || is_qualifier(w) {
                    words.push(w.clone());
                    *pos += 1;
                } else {
                    break;
                }
            }
            let slices: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
            multiword_to_field_type(&slices).display_name()
        }

        // Single identifier type (typedef name)
        Some(Token::Ident(s)) if !is_reserved(s) => {
            let t = s.clone();
            *pos += 1;
            t
        }

        _ => {
            *pos = start;
            return None;
        }
    };

    skip_attribute(tokens, pos);

    let mut result = Vec::new();

    // --- Parse declarators (comma-separated) ---
    loop {
        // Count pointer stars
        let mut ptr_depth = 0usize;
        while matches!(tokens.get(*pos), Some(Token::Star)) {
            ptr_depth += 1;
            *pos += 1;
        }

        // Qualifiers between * and name (e.g., `const`)
        while let Some(Token::Ident(s)) = tokens.get(*pos) {
            if is_qualifier(s) {
                *pos += 1;
            } else {
                break;
            }
        }

        // Field name
        let name = match tokens.get(*pos) {
            Some(Token::Ident(n)) if !is_reserved(n) => {
                let n = n.clone();
                *pos += 1;
                n
            }
            // Terminator without a declarator — bail out
            Some(Token::Semi) | Some(Token::RBrace) => break,
            _ => {
                *pos = start;
                return None;
            }
        };

        // Array suffix
        let array_size = if matches!(tokens.get(*pos), Some(Token::LBracket)) {
            *pos += 1;
            let n = match tokens.get(*pos) {
                Some(Token::Number(n)) => {
                    let n = *n;
                    *pos += 1;
                    n
                }
                Some(Token::RBracket) => 0, // flexible []
                _ => {
                    *pos = start;
                    return None;
                }
            };
            match tokens.get(*pos) {
                Some(Token::RBracket) => *pos += 1,
                _ => {
                    *pos = start;
                    return None;
                }
            }
            Some(n)
        } else {
            None
        };

        // Bitfield suffix
        let bit_size = if matches!(tokens.get(*pos), Some(Token::Colon)) {
            *pos += 1;
            match tokens.get(*pos) {
                Some(Token::Number(n)) => {
                    let n = *n as u8;
                    *pos += 1;
                    Some(n)
                }
                _ => {
                    *pos = start;
                    return None;
                }
            }
        } else {
            None
        };

        result.push(RawField {
            name,
            base_type: base_type.clone(),
            ptr_depth,
            array_size,
            bit_size,
        });

        if matches!(tokens.get(*pos), Some(Token::Comma)) {
            *pos += 1;
        } else {
            break;
        }
    }

    // Consume trailing semicolon
    if matches!(tokens.get(*pos), Some(Token::Semi)) {
        *pos += 1;
    }

    if result.is_empty() {
        *pos = start;
        None
    } else {
        Some(result)
    }
}

// ── Field body parser ─────────────────────────────────────────────────────────

fn parse_raw_fields(tokens: &[Token], pos: &mut usize) -> Vec<RawField> {
    let mut fields = Vec::new();

    while *pos < tokens.len() {
        if matches!(tokens.get(*pos), Some(Token::RBrace)) {
            break;
        }
        if matches!(tokens.get(*pos), Some(Token::Semi)) {
            *pos += 1;
            continue;
        }

        // Anonymous embedded struct or union — flatten its fields
        if let Some(Token::Ident(kw)) = tokens.get(*pos) {
            if is_struct_keyword(kw) {
                let saved = *pos;
                *pos += 1;
                skip_attribute(tokens, pos);

                // Optional tag name before '{'
                if let Some(Token::Ident(tag)) = tokens.get(*pos) {
                    if !is_reserved(tag) && !matches!(tokens.get(*pos + 1), Some(Token::LBrace)) {
                        // It's a named struct field reference, not an anonymous body
                        *pos = saved;
                    }
                }

                if matches!(tokens.get(*pos), Some(Token::LBrace)) {
                    *pos += 1;
                    let nested = parse_raw_fields(tokens, pos);
                    if matches!(tokens.get(*pos), Some(Token::RBrace)) {
                        *pos += 1;
                    }
                    // Optional variable name after '}'
                    if let Some(Token::Ident(n)) = tokens.get(*pos) {
                        if !is_reserved(n) {
                            *pos += 1;
                        }
                    }
                    if matches!(tokens.get(*pos), Some(Token::Semi)) {
                        *pos += 1;
                    }
                    fields.extend(nested);
                    continue;
                } else {
                    *pos = saved;
                }
            }
        }

        if let Some(new_fields) = try_parse_field_decl(tokens, pos) {
            fields.extend(new_fields);
        } else {
            *pos += 1;
        }
    }

    fields
}

// ── Struct definition parser ──────────────────────────────────────────────────

fn try_parse_struct_block(tokens: &[Token], pos: &mut usize) -> Option<Vec<StructDef>> {
    let start = *pos;

    // Optional `typedef`
    let is_typedef = matches!(tokens.get(*pos), Some(Token::Ident(s)) if s == "typedef");
    if is_typedef {
        *pos += 1;
    }

    // `struct` or `union`
    match tokens.get(*pos) {
        Some(Token::Ident(s)) if is_struct_keyword(s) => *pos += 1,
        _ => {
            *pos = start;
            return None;
        }
    }

    skip_attribute(tokens, pos);

    // Optional tag name
    let tag_name = match tokens.get(*pos) {
        Some(Token::Ident(s)) if !is_reserved(s) && tokens.get(*pos + 1) != Some(&Token::LParen) => {
            let n = s.clone();
            *pos += 1;
            Some(n)
        }
        _ => None,
    };

    // Must have a body `{`
    if !matches!(tokens.get(*pos), Some(Token::LBrace)) {
        *pos = start;
        return None;
    }
    *pos += 1;

    let raw_fields = parse_raw_fields(tokens, pos);

    if !matches!(tokens.get(*pos), Some(Token::RBrace)) {
        *pos = start;
        return None;
    }
    *pos += 1;

    // Collect names after `}` (typedef aliases and pointer aliases)
    let mut aliases: Vec<String> = Vec::new();
    loop {
        match tokens.get(*pos) {
            Some(Token::Ident(s)) if !is_reserved(s) => {
                aliases.push(s.clone());
                *pos += 1;
            }
            Some(Token::Star) => {
                *pos += 1; // pointer alias — skip but continue so we consume the name
            }
            Some(Token::Comma) => {
                *pos += 1;
            }
            _ => break,
        }
    }

    if matches!(tokens.get(*pos), Some(Token::Semi)) {
        *pos += 1;
    }

    // Collect all unique names for this struct
    let mut all_names: Vec<String> = Vec::new();
    if let Some(tag) = tag_name {
        all_names.push(tag);
    }
    for alias in aliases {
        if !all_names.contains(&alias) {
            all_names.push(alias);
        }
    }

    if all_names.is_empty() {
        return None;
    }

    let results = all_names
        .into_iter()
        .map(|name| build_struct_def(name, &raw_fields))
        .collect();

    Some(results)
}

// ── Layout computation ────────────────────────────────────────────────────────

fn resolve_raw_type(raw: &RawField) -> FieldType {
    let base = if let Some(ty) = ident_to_field_type(&raw.base_type) {
        ty
    } else {
        FieldType::Named(raw.base_type.clone())
    };

    let mut ty = base;
    for _ in 0..raw.ptr_depth {
        ty = FieldType::Ptr(Box::new(ty));
    }
    if let Some(n) = raw.array_size {
        if n > 0 {
            ty = FieldType::Arr(Box::new(ty), n);
        }
    }
    ty
}

fn build_struct_def(name: String, raw_fields: &[RawField]) -> StructDef {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    let mut max_align = 1usize;

    // Bitfield state: (container_byte_offset, bits_consumed)
    let mut bf_state: Option<(usize, u8, usize)> = None; // (offset, bits_used, container_size)

    for raw in raw_fields {
        let ty = resolve_raw_type(raw);
        let field_size = ty.byte_size(PTR_SIZE).max(1);
        let field_align = ty.align_of(PTR_SIZE).max(1);

        if let Some(bits) = raw.bit_size {
            let container_bits = (field_size * 8) as u8;

            let (field_offset, bit_off) = if let Some((bf_off, bf_used, bf_csz)) = bf_state {
                if bits <= container_bits - bf_used && bf_csz == field_size {
                    // Fits in current container
                    (bf_off, bf_used)
                } else {
                    // New container
                    let new_off = align_up(offset, field_align);
                    (new_off, 0)
                }
            } else {
                let new_off = align_up(offset, field_align);
                (new_off, 0)
            };

            bf_state = Some((field_offset, bit_off + bits, field_size));
            offset = field_offset + field_size;
            max_align = max_align.max(field_align);

            fields.push(Field {
                name: raw.name.clone(),
                ty,
                offset: field_offset,
                bit_offset: Some(bit_off),
                bit_size: Some(bits),
            });
        } else {
            bf_state = None;

            let field_offset = align_up(offset, field_align);
            max_align = max_align.max(field_align);

            fields.push(Field {
                name: raw.name.clone(),
                ty,
                offset: field_offset,
                bit_offset: None,
                bit_size: None,
            });

            offset = field_offset + field_size;
        }
    }

    let total_size = align_up(offset, max_align);

    StructDef { name, fields, total_size }
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn parse_header(source: &str, registry: &mut Registry) -> usize {
    let tokens = tokenize(source);
    let mut pos = 0;
    let mut count = 0;

    while pos < tokens.len() {
        if let Some(defs) = try_parse_struct_block(&tokens, &mut pos) {
            count += defs.len();
            for def in defs {
                registry.insert(def);
            }
        } else {
            pos += 1;
        }
    }

    count
}
