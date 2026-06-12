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

// ── Raw item (union-aware grouping) ──────────────────────────────────────────

#[derive(Debug, Clone)]
enum RawItem {
    Field(RawField),
    UnionGroup(Vec<RawItem>),
    StructGroup(Vec<RawItem>),
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

    // Array size on the type itself: `CHAR[20] name` — same as `CHAR name[20]`
    let type_array_size: Option<usize> = if matches!(tokens.get(*pos), Some(Token::LBracket)) {
        *pos += 1;
        let n = match tokens.get(*pos) {
            Some(Token::Number(n)) => {
                let n = *n;
                *pos += 1;
                n
            }
            Some(Token::RBracket) => 0,
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

        // Array suffix on declarator, or fall back to type-level array size
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
            type_array_size
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

fn parse_raw_fields(tokens: &[Token], pos: &mut usize) -> Vec<RawItem> {
    let mut items = Vec::new();

    while *pos < tokens.len() {
        if matches!(tokens.get(*pos), Some(Token::RBrace)) {
            break;
        }
        if matches!(tokens.get(*pos), Some(Token::Semi)) {
            *pos += 1;
            continue;
        }

        let mut handled = false;

        // Anonymous embedded struct or union
        if let Some(Token::Ident(kw)) = tokens.get(*pos) {
            if is_struct_keyword(kw) {
                let is_nested_union = kw == "union";
                let saved = *pos;
                *pos += 1;
                skip_attribute(tokens, pos);

                // Optional tag name before '{'
                if let Some(Token::Ident(tag)) = tokens.get(*pos) {
                    if !is_reserved(tag) && !matches!(tokens.get(*pos + 1), Some(Token::LBrace)) {
                        // Named type reference, not an anonymous body
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
                    if is_nested_union {
                        items.push(RawItem::UnionGroup(nested));
                    } else {
                        items.push(RawItem::StructGroup(nested));
                    }
                    handled = true;
                } else {
                    *pos = saved;
                }
            }
        }

        if !handled {
            if let Some(new_fields) = try_parse_field_decl(tokens, pos) {
                items.extend(new_fields.into_iter().map(RawItem::Field));
            } else {
                *pos += 1;
            }
        }
    }

    items
}

// ── Struct definition parser ──────────────────────────────────────────────────

fn try_parse_struct_block(tokens: &[Token], pos: &mut usize) -> Option<Vec<StructDef>> {
    let start = *pos;

    // Optional `typedef`
    let is_typedef = matches!(tokens.get(*pos), Some(Token::Ident(s)) if s == "typedef");
    if is_typedef {
        *pos += 1;
    }

    // `struct` or `union` (enum treated as struct for layout)
    let is_union = match tokens.get(*pos) {
        Some(Token::Ident(s)) if is_struct_keyword(s) => {
            let u = s == "union";
            *pos += 1;
            u
        }
        _ => {
            *pos = start;
            return None;
        }
    };

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

    let raw_items = parse_raw_fields(tokens, pos);

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
        .map(|name| build_struct_def(name, &raw_items, is_union))
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

// Returns (fields, size_from_base, max_align).
// For unions all items start at `base`; for structs they are laid out sequentially.
fn layout_items(
    items: &[RawItem],
    is_union: bool,
    base: usize,
    ptr_size: usize,
) -> (Vec<Field>, usize, usize) {
    let mut fields = Vec::new();
    let mut offset = base;
    let mut max_align = 1usize;
    let mut max_size = 0usize;
    let mut bf_state: Option<(usize, u8, usize)> = None;

    for item in items {
        match item {
            RawItem::Field(raw) => {
                let ty = resolve_raw_type(raw);
                let field_size = ty.byte_size(ptr_size).max(1);
                let field_align = ty.align_of(ptr_size).max(1);
                max_align = max_align.max(field_align);

                let cur_base = if is_union { base } else { offset };

                if let Some(bits) = raw.bit_size {
                    let container_bits = (field_size * 8) as u8;
                    let (field_offset, bit_off) = if let Some((bf_off, bf_used, bf_csz)) = bf_state {
                        if bits <= container_bits - bf_used && bf_csz == field_size {
                            (bf_off, bf_used)
                        } else {
                            (align_up(cur_base, field_align), 0)
                        }
                    } else {
                        (align_up(cur_base, field_align), 0)
                    };

                    bf_state = Some((field_offset, bit_off + bits, field_size));
                    max_size = max_size.max(field_offset - base + field_size);
                    if !is_union {
                        offset = field_offset + field_size;
                    }

                    fields.push(Field {
                        name: raw.name.clone(),
                        ty,
                        offset: field_offset,
                        bit_offset: Some(bit_off),
                        bit_size: Some(bits),
                    });
                } else {
                    bf_state = None;
                    let field_offset = align_up(cur_base, field_align);

                    max_size = max_size.max(field_offset - base + field_size);
                    if !is_union {
                        offset = field_offset + field_size;
                    }

                    fields.push(Field {
                        name: raw.name.clone(),
                        ty,
                        offset: field_offset,
                        bit_offset: None,
                        bit_size: None,
                    });
                }
            }

            RawItem::UnionGroup(nested) => {
                bf_state = None;
                let group_base = if is_union { base } else { offset };
                let (nested_fields, group_size, group_align) =
                    layout_items(nested, true, group_base, ptr_size);
                max_align = max_align.max(group_align);
                fields.extend(nested_fields);
                max_size = max_size.max(group_size);
                if !is_union {
                    offset = group_base + group_size;
                }
            }

            RawItem::StructGroup(nested) => {
                bf_state = None;
                let group_base = if is_union { base } else { offset };
                let (nested_fields, group_size, group_align) =
                    layout_items(nested, false, group_base, ptr_size);
                max_align = max_align.max(group_align);
                fields.extend(nested_fields);
                max_size = max_size.max(group_size);
                if !is_union {
                    offset = group_base + group_size;
                }
            }
        }
    }

    let size = if is_union { max_size } else { offset - base };
    (fields, size, max_align)
}

fn build_struct_def(name: String, raw_items: &[RawItem], is_union: bool) -> StructDef {
    let (fields, size, max_align) = layout_items(raw_items, is_union, 0, PTR_SIZE);
    let total_size = align_up(size, max_align.max(1));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Registry {
        let mut reg = Registry::default();
        parse_header(src, &mut reg);
        reg
    }

    #[test]
    fn union_all_fields_at_offset_zero() {
        let reg = parse("union Foo { DWORD a; WORD b; BYTE c; };");
        let def = reg.get("Foo").unwrap();
        assert_eq!(def.fields[0].offset, 0, "a");
        assert_eq!(def.fields[1].offset, 0, "b");
        assert_eq!(def.fields[2].offset, 0, "c");
        assert_eq!(def.total_size, 4);
    }

    #[test]
    fn union_size_is_largest_member() {
        let reg = parse("union Bar { BYTE a; DWORD64 b; WORD c; };");
        let def = reg.get("Bar").unwrap();
        assert_eq!(def.total_size, 8);
    }

    #[test]
    fn anonymous_union_in_struct_shares_offset() {
        let reg = parse("struct Outer { DWORD x; union { DWORD y; WORD z; }; DWORD w; };");
        let def = reg.get("Outer").unwrap();
        let y = def.fields.iter().find(|f| f.name == "y").unwrap();
        let z = def.fields.iter().find(|f| f.name == "z").unwrap();
        let w = def.fields.iter().find(|f| f.name == "w").unwrap();
        assert_eq!(y.offset, 4, "y should follow x at offset 4");
        assert_eq!(z.offset, 4, "z should overlap y at offset 4");
        assert_eq!(w.offset, 8, "w should follow the union");
        assert_eq!(def.total_size, 12);
    }

    #[test]
    fn anonymous_struct_inside_union_is_sequential() {
        // Each arm of the union overlaps; within each struct arm fields are sequential.
        let reg = parse("union U { struct { WORD lo; WORD hi; }; DWORD full; };");
        let def = reg.get("U").unwrap();
        let lo = def.fields.iter().find(|f| f.name == "lo").unwrap();
        let hi = def.fields.iter().find(|f| f.name == "hi").unwrap();
        let full = def.fields.iter().find(|f| f.name == "full").unwrap();
        assert_eq!(lo.offset, 0);
        assert_eq!(hi.offset, 2);
        assert_eq!(full.offset, 0);
        assert_eq!(def.total_size, 4);
    }

    #[test]
    fn type_level_array_syntax() {
        // CHAR[20] array; is equivalent to CHAR array[20];
        let reg = parse("struct S { CHAR[20] name; DWORD x; };");
        let def = reg.get("S").unwrap();
        let name_f = def.fields.iter().find(|f| f.name == "name").unwrap();
        assert!(matches!(name_f.ty, FieldType::Arr(_, 20)));
        assert_eq!(name_f.offset, 0);
        let x_f = def.fields.iter().find(|f| f.name == "x").unwrap();
        assert_eq!(x_f.offset, 20);
    }

    #[test]
    fn struct_layout_unchanged() {
        let reg = parse("struct S { BYTE a; DWORD b; WORD c; };");
        let def = reg.get("S").unwrap();
        assert_eq!(def.fields[0].offset, 0); // a
        assert_eq!(def.fields[1].offset, 4); // b (aligned to 4)
        assert_eq!(def.fields[2].offset, 8); // c
        assert_eq!(def.total_size, 12);
    }
}