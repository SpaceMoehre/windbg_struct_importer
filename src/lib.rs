mod display;
mod parser;
mod pdb;
mod types;

use std::ffi::{c_void, CStr};
use std::sync::{LazyLock, Mutex};

use dbgeng::client::DebugClient;
use dbgeng::windows::core::{Interface, IUnknown, HRESULT, PCSTR};
use dbgeng::windows::Win32::Foundation::{E_ABORT, S_OK};

use types::Registry;

type RawClient = *mut c_void;

static REGISTRY: LazyLock<Mutex<Registry>> =
    LazyLock::new(|| Mutex::new(Registry::default()));

// ── Required WinDbg extension exports ────────────────────────────────────────

#[unsafe(no_mangle)]
extern "C" fn DebugExtensionInitialize(_version: *mut u32, _flags: *mut u32) -> HRESULT {
    S_OK
}

#[unsafe(no_mangle)]
extern "C" fn DebugExtensionUninitialize() {}

// ── Commands ──────────────────────────────────────────────────────────────────

/// !importhdr <path>
/// Parse a C header and register all struct definitions. Also generates a PDB
/// and loads it as a synthetic module so the native `dt` command works.
#[unsafe(no_mangle)]
extern "C" fn importhdr(raw_client: RawClient, args: PCSTR) -> HRESULT {
    wrap(raw_client, args, |dbg, _unk, args| {
        let path = args.trim();
        if path.is_empty() {
            dbg.log("Usage: !importhdr <path_to_header.h>\n")?;
            return Ok(());
        }

        let source = std::fs::read_to_string(path)?;
        let mut reg = REGISTRY.lock().unwrap();
        let count = parser::parse_header(&source, &mut reg);
        dbg.log(format!("Imported {} struct definition(s) from \"{}\"\n", count, path))?;

        if count > 0 {
            register_synthetic_types(dbg, &reg)?;
        }

        Ok(())
    })
}

/// !dt <StructName> <address>
/// Display a struct's fields at the given address.
#[unsafe(no_mangle)]
extern "C" fn dt(raw_client: RawClient, args: PCSTR) -> HRESULT {
    wrap(raw_client, args, |dbg, unk, args| {
        let mut parts = args.split_whitespace();
        let struct_name = match parts.next() {
            Some(s) => s,
            None => {
                dbg.log("Usage: !dt <StructName> <address>\n")?;
                return Ok(());
            }
        };
        let addr_str = match parts.next() {
            Some(s) => s,
            None => {
                dbg.log("Usage: !dt <StructName> <address>\n")?;
                return Ok(());
            }
        };

        let addr = parse_address(dbg, addr_str)?;
        let reg = REGISTRY.lock().unwrap();

        match reg.get(struct_name).cloned() {
            Some(def) => {
                dbg.log(format!("{} @ 0x{:016x}  (size = 0x{:x})\n", def.name, addr, def.total_size))?;
                display::display_struct(dbg, unk, &reg, &def, addr, 0, 8)?;
            }
            None => {
                dbg.exec(format!("dt {}", args).as_str())?;
            }
        }
        Ok(())
    })
}

/// !liststructs
#[unsafe(no_mangle)]
extern "C" fn liststructs(raw_client: RawClient, _args: PCSTR) -> HRESULT {
    wrap(raw_client, PCSTR(std::ptr::null()), |dbg, _unk, _| {
        let reg = REGISTRY.lock().unwrap();
        let names = reg.list_names();
        if names.is_empty() {
            dbg.log("No structs loaded. Use !importhdr <header.h>.\n")?;
        } else {
            dbg.log(format!("Loaded structs ({}):\n", names.len()))?;
            for name in names {
                dbg.log(format!("  {}\n", name))?;
            }
        }
        Ok(())
    })
}

/// !clearstructs
#[unsafe(no_mangle)]
extern "C" fn clearstructs(raw_client: RawClient, _args: PCSTR) -> HRESULT {
    wrap(raw_client, PCSTR(std::ptr::null()), |dbg, _unk, _| {
        *REGISTRY.lock().unwrap() = Registry::default();
        dbg.log("All struct definitions cleared.\n")?;
        Ok(())
    })
}

// ── PDB + synthetic module registration ──────────────────────────────────────

fn register_synthetic_types(dbg: &DebugClient, reg: &Registry) -> anyhow::Result<()> {
    let defs: Vec<&types::StructDef> = reg.list_names()
        .iter()
        .filter_map(|name| reg.get(name))
        .collect();

    // Write PDB to %TEMP%\struct_importer.pdb
    let pdb_path = std::env::temp_dir().join("struct_importer.pdb");
    // Fake image path in same dir so WinDbg finds the PDB alongside it
    let image_path = std::env::temp_dir().join("struct_importer.dll");

    pdb::write_pdb(&defs, &pdb_path)?;

    // Add the temp dir to WinDbg's symbol path so it can find our PDB
    let temp_dir = std::env::temp_dir();
    let sympath_cmd = format!(".sympath+ {}", temp_dir.display());
    let _ = dbg.exec(&*sympath_cmd);

    // Add synthetic module so WinDbg loads struct_importer.pdb
    // Base address: use a fixed unused address in kernel space gap (0x1_0000_0000)
    // Remove any previous synthetic module first (ignore errors)
    let _ = dbg.remove_synthetic_module_by_name("struct_importer");

    let image_path_str = image_path.to_string_lossy();
    dbg.add_synthetic_module("0x100000000", "struct_importer", image_path_str.as_ref().into())?;

    // Force symbol reload for the synthetic module
    let _ = dbg.exec(".reload /f struct_importer");

    dbg.log("Types registered — use: dt struct_importer!StructName 0xaddr  (or dt StructName 0xaddr)\n")?;
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn wrap(
    raw_client: RawClient,
    args: PCSTR,
    callback: impl FnOnce(&DebugClient, &IUnknown, &str) -> anyhow::Result<()>,
) -> HRESULT {
    let Some(unk) = (unsafe { IUnknown::from_raw_borrowed(&raw_client) }) else {
        return E_ABORT;
    };
    let Ok(dbg) = DebugClient::new(unk) else {
        return E_ABORT;
    };
    let args_str = if args.0.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(args.0 as *const i8) }
            .to_string_lossy()
            .into_owned()
    };
    match callback(&dbg, unk, &args_str) {
        Ok(()) => S_OK,
        Err(e) => {
            let _ = dbg.log(format!("Error: {:#}\n", e));
            E_ABORT
        }
    }
}

fn parse_address(dbg: &DebugClient, s: &str) -> anyhow::Result<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return Ok(u64::from_str_radix(hex, 16)?);
    }
    if s.len() > 8 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(u64::from_str_radix(s, 16)?);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return Ok(s.parse()?);
    }
    Ok(dbg.eval64(s)?)
}
