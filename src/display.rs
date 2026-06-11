use anyhow::Result;
use dbgeng::client::DebugClient;
use dbgeng::dlogln;
use dbgeng::windows::core::{Interface, IUnknown, PCSTR};
use dbgeng::windows::Win32::System::Diagnostics::Debug::Extensions::{
    IDebugControl, DEBUG_OUTCTL_AMBIENT_DML, DEBUG_OUTPUT_NORMAL,
};

use crate::types::*;

pub fn display_struct(
    dbg: &DebugClient,
    unk: &IUnknown,
    registry: &Registry,
    def: &StructDef,
    addr: u64,
    indent: usize,
    ptr_size: usize,
) -> Result<()> {
    let pad = "   ".repeat(indent);

    for field in &def.fields {
        let field_addr = addr + field.offset as u64;

        // Inline nested named struct (non-bitfield)
        if field.bit_offset.is_none() {
            if let FieldType::Named(nested_name) = &field.ty {
                if let Some(nested_def) = registry.get(nested_name) {
                    let nested_name = nested_def.name.clone();
                    dlogln!(dbg, "{}   +{:#06x} {:<20} : {}", pad, field.offset, field.name, nested_name)?;
                    let nested = registry.get(&nested_name).unwrap();
                    display_struct(dbg, unk, registry, nested, field_addr, indent + 1, ptr_size)?;
                    continue;
                }
            }
        }

        // Pointer to a known struct → DML clickable link
        if field.bit_offset.is_none() {
            if let FieldType::Ptr(inner) = &field.ty {
                if let FieldType::Named(target_name) = inner.as_ref() {
                    if registry.get(target_name).is_some() {
                        let mut buf = vec![0u8; ptr_size];
                        let ptr_val = if dbg.read_virtual_exact(field_addr, &mut buf).is_ok() {
                            bytes_to_u64(&buf)
                        } else {
                            0
                        };
                        let line = format!(
                            "{}   +{:#06x} {:<20} : ",
                            pad, field.offset, field.name
                        );
                        dml_output(
                            unk,
                            &format!(
                                "{}<link cmd=\"!dt {} 0x{:016x}\">0x{:016x}</link> ({}*)\n",
                                line, target_name, ptr_val, ptr_val, target_name
                            ),
                        )?;
                        continue;
                    }
                }
            }
        }

        let value_str = read_field_value(
            dbg, registry, &field.ty, field_addr, ptr_size, field.bit_offset, field.bit_size,
        )
        .unwrap_or_else(|_| "<unreadable>".to_string());

        match (field.bit_offset, field.bit_size) {
            (Some(bo), Some(bs)) => {
                dlogln!(
                    dbg,
                    "{}   +{:#06x} {:<20} : {} [Pos {}, {} Bit{}]",
                    pad, field.offset, field.name, value_str,
                    bo, bs, if bs == 1 { "" } else { "s" }
                )?;
            }
            _ => {
                dlogln!(dbg, "{}   +{:#06x} {:<20} : {}", pad, field.offset, field.name, value_str)?;
            }
        }
    }

    Ok(())
}

fn dml_output(unk: &IUnknown, text: &str) -> Result<()> {
    let ctrl: IDebugControl = unk.cast()?;
    let cstr = std::ffi::CString::new(text.replace('%', "%%"))?;
    unsafe {
        ctrl.ControlledOutput(
            DEBUG_OUTCTL_AMBIENT_DML,
            DEBUG_OUTPUT_NORMAL,
            PCSTR(cstr.as_ptr() as *const u8),
        )?;
    }
    Ok(())
}

fn read_field_value(
    dbg: &DebugClient,
    registry: &Registry,
    ty: &FieldType,
    addr: u64,
    ptr_size: usize,
    bit_offset: Option<u8>,
    bit_size: Option<u8>,
) -> Result<String> {
    match ty {
        FieldType::Prim(p) => {
            let size = p.size();
            if size == 0 {
                return Ok("(void)".to_string());
            }
            let mut buf = vec![0u8; size];
            dbg.read_virtual_exact(addr, &mut buf)?;
            let raw = bytes_to_u64(&buf);
            let val = match (bit_offset, bit_size) {
                (Some(bo), Some(bs)) => {
                    let mask = if bs >= 64 { u64::MAX } else { (1u64 << bs) - 1 };
                    (raw >> bo) & mask
                }
                _ => raw,
            };
            Ok(format_prim(p, val))
        }

        FieldType::Ptr(inner) => {
            let mut buf = vec![0u8; ptr_size];
            dbg.read_virtual_exact(addr, &mut buf)?;
            let ptr_val = bytes_to_u64(&buf);
            Ok(format!("0x{:016x} ({}*)", ptr_val, inner.display_name()))
        }

        FieldType::Arr(inner, n) => {
            let elem_size = registry.resolve_size(inner, ptr_size).max(1);
            let show_inline = matches!(inner.as_ref(), FieldType::Prim(_)) && *n <= 8;
            if show_inline {
                let mut parts = Vec::with_capacity(*n);
                for i in 0..*n {
                    let v = read_field_value(
                        dbg, registry, inner,
                        addr + (i * elem_size) as u64,
                        ptr_size, None, None,
                    )
                    .unwrap_or_else(|_| "?".to_string());
                    parts.push(v);
                }
                Ok(format!("[{}]{{ {} }}", n, parts.join(", ")))
            } else {
                Ok(format!("[{}] {} @ 0x{:016x}", n, inner.display_name(), addr))
            }
        }

        FieldType::Named(name) => {
            if let Some(nested) = registry.get(name) {
                Ok(format!("{} (size={})", nested.name, nested.total_size))
            } else {
                Ok(format!("(unknown: {})", name))
            }
        }
    }
}

fn bytes_to_u64(buf: &[u8]) -> u64 {
    let mut val = 0u64;
    for (i, &b) in buf.iter().enumerate().take(8) {
        val |= (b as u64) << (i * 8);
    }
    val
}

fn format_prim(ty: &PrimType, val: u64) -> String {
    match ty {
        PrimType::U8 => format!("{:#04x} ({}u)", val as u8, val as u8),
        PrimType::U16 => format!("{:#06x} ({}u)", val as u16, val as u16),
        PrimType::U32 => format!("{:#010x} ({}u)", val as u32, val as u32),
        PrimType::U64 => format!("{:#018x} ({}u)", val, val),
        PrimType::I8 => format!("{:#04x} ({})", val as u8, val as i8),
        PrimType::I16 => format!("{:#06x} ({})", val as u16, val as i16),
        PrimType::I32 => format!("{:#010x} ({})", val as u32, val as i32),
        PrimType::I64 => format!("{:#018x} ({})", val, val as i64),
        PrimType::F32 => format!("{}", f32::from_bits(val as u32)),
        PrimType::F64 => format!("{}", f64::from_bits(val)),
        PrimType::Bool => if val != 0 { "TRUE".to_string() } else { "FALSE".to_string() },
        PrimType::Char => {
            let c = val as u8;
            format!("{:#04x} '{}'", c, if c.is_ascii_graphic() { c as char } else { '.' })
        }
        PrimType::WChar => format!("{:#06x}", val as u16),
        PrimType::Void => "(void)".to_string(),
    }
}
