//! Port of `tdf.py` (table definition file reading and dumping).

#![allow(clippy::used_underscore_items)]

use crate::pystr;

/// Port of `tdf.py::TdfField`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdfField {
    pub r#type: i64,
    pub read_only: bool,
    pub name: String,
    pub aliases: Vec<String>,
    pub processing: String,
    pub units: String,
    pub description: String,
    pub begin_index: i64,
    pub dimension: i64,
    pub subdims: Vec<i64>,
}

impl Default for TdfField {
    fn default() -> Self {
        Self {
            r#type: 0,
            read_only: false,
            name: String::new(),
            aliases: Vec::new(),
            processing: String::new(),
            units: String::new(),
            description: String::new(),
            begin_index: 1,
            dimension: 1,
            subdims: Vec::new(),
        }
    }
}

/// Port of `tdf.py::TdfTable`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TdfTable {
    pub name: String,
    pub size: i64,
    pub time_type: i64,
    pub time_into: (i64, i64),
    pub interval: (i64, i64),
    pub fields: Vec<TdfField>,
}

/// Port of `tdf.py::_cstr`.
pub fn _cstr(data: &[u8], pos: usize) -> Result<(String, usize), String> {
    let rel = data
        .get(pos..)
        .and_then(|slice| slice.iter().position(|&b| b == 0))
        .ok_or_else(|| format!("unterminated cstr at {pos}"))?;
    let end = pos + rel;
    Ok((pystr::decode_latin1(&data[pos..end]), end + 1))
}

fn read_u32_be(data: &[u8], pos: usize) -> Result<i64, String> {
    let bytes: [u8; 4] = data
        .get(pos..pos + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| format!("unexpected EOF reading u32 at {pos}"))?;
    Ok(i64::from(u32::from_be_bytes(bytes)))
}

fn read_i32_be(data: &[u8], pos: usize) -> Result<i64, String> {
    let bytes: [u8; 4] = data
        .get(pos..pos + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| format!("unexpected EOF reading i32 at {pos}"))?;
    Ok(i64::from(i32::from_be_bytes(bytes)))
}

/// Port of `tdf.py::parse`.
pub fn parse(data: &[u8]) -> Result<(i64, Vec<TdfTable>), String> {
    let version = i64::from(*data.first().ok_or("empty tdf data")?);
    let mut pos = 1;
    let mut tables = Vec::new();
    while pos < data.len() {
        let name;
        (name, pos) = _cstr(data, pos)?;
        let size = read_u32_be(data, pos)?;
        let time_type = i64::from(
            *data
                .get(pos + 4)
                .ok_or_else(|| format!("unexpected EOF reading time_type at {}", pos + 4))?,
        );
        pos += 5;
        let into = (read_i32_be(data, pos)?, read_i32_be(data, pos + 4)?);
        let interval = (read_i32_be(data, pos + 8)?, read_i32_be(data, pos + 12)?);
        pos += 16;
        let mut table = TdfTable {
            name,
            size,
            time_type,
            time_into: into,
            interval,
            fields: Vec::new(),
        };
        while *data
            .get(pos)
            .ok_or_else(|| format!("unexpected EOF reading field terminator at {pos}"))?
            != 0
        {
            let ftype = data[pos];
            pos += 1;
            let fname;
            (fname, pos) = _cstr(data, pos)?;
            let mut aliases = Vec::new();
            loop {
                let alias;
                (alias, pos) = _cstr(data, pos)?;
                if alias.is_empty() {
                    break;
                }
                aliases.push(alias);
            }
            let processing;
            (processing, pos) = _cstr(data, pos)?;
            let units;
            (units, pos) = _cstr(data, pos)?;
            let description;
            (description, pos) = _cstr(data, pos)?;
            let begin = read_u32_be(data, pos)?;
            let dim = read_u32_be(data, pos + 4)?;
            pos += 8;
            let mut subdims = Vec::new();
            loop {
                let sd = read_u32_be(data, pos)?;
                pos += 4;
                if sd == 0 {
                    break;
                }
                subdims.push(sd);
            }
            table.fields.push(TdfField {
                r#type: i64::from(ftype & 0x7F),
                read_only: (ftype & 0x80) != 0,
                name: fname,
                aliases,
                processing,
                units,
                description,
                begin_index: begin,
                dimension: dim,
                subdims,
            });
        }
        pos += 1;
        tables.append(&mut vec![table]);
    }
    Ok((version, tables))
}

/// Port of `tdf.py::dump`.
pub fn dump(data: &[u8]) -> Result<String, String> {
    let (version, tables) = parse(data)?;
    let mut lines = vec![format!("version {version}")];
    for t in tables {
        lines.push(format!(
            "table {} size={} timetype={} into=({}, {}) interval=({}, {})",
            t.name, t.size, t.time_type, t.time_into.0, t.time_into.1, t.interval.0, t.interval.1
        ));
        for f in t.fields {
            let ro = if f.read_only { " ro" } else { "" };
            let mut extra = Vec::new();
            if !f.aliases.is_empty() {
                let rendered: Vec<String> = f.aliases.iter().map(|a| pystr::repr_str(a)).collect();
                extra.push(format!("aliases=[{}]", rendered.join(", ")));
            }
            for (key, val) in [
                ("processing", &f.processing),
                ("units", &f.units),
                ("description", &f.description),
            ] {
                if !val.is_empty() {
                    extra.push(format!("{key}={}", pystr::repr_str(val)));
                }
            }
            let subdims: Vec<String> = f.subdims.iter().map(i64::to_string).collect();
            let line = format!(
                "  {} type={}{ro} begin={} dim={} subdims=[{}] {}",
                f.name,
                f.r#type,
                f.begin_index,
                f.dimension,
                subdims.join(", "),
                extra.join(" ")
            );
            lines.push(pystr::rstrip(&line).to_string());
        }
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_dump_match_python() {
        // Construct a minimal TDF payload:
        // version=1, table="TestTbl", size=100, time_type=1, into=(0, 0), interval=(5, 0)
        // field: ftype=0x84 (type=4, read_only=true), name="BattV", aliases=["BV"],
        // processing="Smp", units="Volts", description="Battery", begin=1, dim=1, subdims=[]
        let mut raw = vec![1u8];
        raw.extend_from_slice(b"TestTbl\0");
        raw.extend_from_slice(&100u32.to_be_bytes());
        raw.push(1);
        raw.extend_from_slice(&0i32.to_be_bytes());
        raw.extend_from_slice(&0i32.to_be_bytes());
        raw.extend_from_slice(&5i32.to_be_bytes());
        raw.extend_from_slice(&0i32.to_be_bytes());
        raw.push(0x84);
        raw.extend_from_slice(b"BattV\0");
        raw.extend_from_slice(b"BV\0\0");
        raw.extend_from_slice(b"Smp\0");
        raw.extend_from_slice(b"Volts\0");
        raw.extend_from_slice(b"Battery\0");
        raw.extend_from_slice(&1u32.to_be_bytes());
        raw.extend_from_slice(&1u32.to_be_bytes());
        raw.extend_from_slice(&0u32.to_be_bytes());
        raw.push(0);

        let dumped = dump(&raw).expect("valid tdf");
        assert_eq!(
            dumped,
            concat!(
                "version 1\n",
                "table TestTbl size=100 timetype=1 into=(0, 0) interval=(5, 0)\n",
                "  BattV type=4 ro begin=1 dim=1 subdims=[] aliases=['BV'] processing='Smp' units='Volts' description='Battery'",
            )
        );
    }

    #[test]
    fn tdf_field_defaults() {
        let f = TdfField::default();
        assert_eq!(f.begin_index, 1);
        assert_eq!(f.dimension, 1);
        assert!(!f.read_only);
    }
}
