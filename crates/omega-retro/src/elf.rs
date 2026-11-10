//! Symbols a 64-bit little-endian ELF library names in its full symbol table, `.symtab`, which
//! holds the ones it keeps to itself as well as those it exports.

/// Each of `names` the ELF file `data` names in its `.symtab`, as `(name, value, size)`, the
/// value its address with the library loaded at 0: none where `data` is no such ELF or has no
/// `.symtab`, as a stripped library has not.
pub fn symbols<'a>(data: &[u8], names: &[&'a str]) -> Option<Vec<(&'a str, u64, u64)>> {
    if data.get(..6)? != b"\x7fELF\x02\x01" {
        return None;
    }
    let u16_at = |o: usize| Some(u16::from_le_bytes(data.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
    let u64_at = |o: usize| Some(u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?));
    let shoff = u64_at(0x28)? as usize;
    let (shentsize, shnum) = (u16_at(0x3A)? as usize, u16_at(0x3C)? as usize);
    // each section's type, offset, size, link and entry size
    let section = |i: usize| {
        let o = shoff.checked_add(i.checked_mul(shentsize)?)?;
        Some((
            u32_at(o + 4)?,
            u64_at(o + 0x18)? as usize,
            u64_at(o + 0x20)? as usize,
            u32_at(o + 0x28)? as usize,
            u64_at(o + 0x38)? as usize,
        ))
    };
    const SHT_SYMTAB: u32 = 2;
    let (_, off, size, link, entsize) =
        (0..shnum).find_map(|i| section(i).filter(|s| s.0 == SHT_SYMTAB))?;
    let (_, str_off, str_size, _, _) = section(link)?;
    let strtab = data.get(str_off..str_off.checked_add(str_size)?)?;
    if entsize < 24 {
        return None;
    }
    let mut found = Vec::new();
    for j in 0..size / entsize {
        let o = off + j * entsize;
        let name_at = u32_at(o)? as usize;
        let rest = strtab.get(name_at..)?;
        let name = &rest[..rest.iter().position(|b| *b == 0)?];
        if let Some(&wanted) = names.iter().find(|n| n.as_bytes() == name) {
            found.push((wanted, u64_at(o + 8)?, u64_at(o + 16)?));
        }
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A symbol this test's own binary names in its table, as a library names its own.
    #[no_mangle]
    #[used]
    static OMEGA_ELF_TEST_MARK: [u8; 24] = [7; 24];

    #[test]
    fn a_binary_names_a_static_of_its_own_with_its_size() {
        let exe = std::env::current_exe().unwrap();
        let data = std::fs::read(exe).unwrap();
        let found = symbols(&data, &["OMEGA_ELF_TEST_MARK", "no_such_symbol"])
            .expect("the test binary has no symbol table");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "OMEGA_ELF_TEST_MARK");
        assert_eq!(found[0].2, 24);
    }

    #[test]
    fn what_is_no_elf_has_no_symbols() {
        assert_eq!(symbols(b"not an elf at all", &["x"]), None);
    }
}
