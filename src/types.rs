use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum PrimType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Char,
    WChar,
    Void,
}

impl PrimType {
    pub fn size(&self) -> usize {
        match self {
            PrimType::U8 | PrimType::I8 | PrimType::Bool | PrimType::Char => 1,
            PrimType::U16 | PrimType::I16 | PrimType::WChar => 2,
            PrimType::U32 | PrimType::I32 | PrimType::F32 => 4,
            PrimType::U64 | PrimType::I64 | PrimType::F64 => 8,
            PrimType::Void => 0,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            PrimType::U8 => "Uint1B",
            PrimType::U16 => "Uint2B",
            PrimType::U32 => "Uint4B",
            PrimType::U64 => "Uint8B",
            PrimType::I8 => "Int1B",
            PrimType::I16 => "Int2B",
            PrimType::I32 => "Int4B",
            PrimType::I64 => "Int8B",
            PrimType::F32 => "float",
            PrimType::F64 => "double",
            PrimType::Bool => "Bool",
            PrimType::Char => "Char",
            PrimType::WChar => "Wchar",
            PrimType::Void => "Void",
        }
    }
}

#[derive(Debug, Clone)]
pub enum FieldType {
    Prim(PrimType),
    Ptr(Box<FieldType>),
    Arr(Box<FieldType>, usize),
    Named(String),
}

impl FieldType {
    pub fn byte_size(&self, ptr_size: usize) -> usize {
        match self {
            FieldType::Prim(p) => p.size(),
            FieldType::Ptr(_) => ptr_size,
            FieldType::Arr(inner, n) => inner.byte_size(ptr_size) * n,
            FieldType::Named(_) => 0,
        }
    }

    pub fn align_of(&self, ptr_size: usize) -> usize {
        match self {
            FieldType::Prim(p) => p.size().max(1),
            FieldType::Ptr(_) => ptr_size,
            FieldType::Arr(inner, _) => inner.align_of(ptr_size),
            FieldType::Named(_) => 1,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            FieldType::Prim(p) => p.display_name().to_string(),
            FieldType::Ptr(inner) => format!("Ptr64 {}", inner.display_name()),
            FieldType::Arr(inner, n) => format!("[{}] {}", n, inner.display_name()),
            FieldType::Named(name) => name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: FieldType,
    pub offset: usize,
    pub bit_offset: Option<u8>,
    pub bit_size: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
    pub total_size: usize,
    pub align: usize,
}

#[derive(Debug, Default)]
pub struct Registry {
    structs: HashMap<String, StructDef>,
}

impl Registry {
    pub fn insert(&mut self, def: StructDef) {
        self.structs.insert(def.name.clone(), def);
    }

    pub fn get(&self, name: &str) -> Option<&StructDef> {
        if let Some(s) = self.structs.get(name) {
            return Some(s);
        }
        let stripped = name.trim_start_matches('_');
        if stripped != name {
            if let Some(s) = self.structs.get(stripped) {
                return Some(s);
            }
        }
        self.structs.get(&format!("_{}", name))
    }

    pub fn resolve_size(&self, ty: &FieldType, ptr_size: usize) -> usize {
        match ty {
            FieldType::Named(name) => self.get(name).map(|s| s.total_size).unwrap_or(ptr_size),
            FieldType::Arr(inner, n) => self.resolve_size(inner, ptr_size) * n,
            other => other.byte_size(ptr_size),
        }
    }

    pub fn resolve_align(&self, ty: &FieldType, ptr_size: usize) -> usize {
        match ty {
            FieldType::Named(name) => self.get(name).map(|s| s.align).unwrap_or(1),
            FieldType::Arr(inner, _) => self.resolve_align(inner, ptr_size),
            other => other.align_of(ptr_size),
        }
    }

    pub fn list_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.structs.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }
}
