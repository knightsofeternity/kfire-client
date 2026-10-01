//! Reading the KFire addon's SavedVariables file, as World of Warcraft writes
//! it: `KFirePlayed = { ["v"] = 1, ["chars"] = { ["eu/Realm/Name"] = { ... } } }`.
//!
//! A deliberately small Lua reader: tables, bracketed and bare keys, strings
//! with escapes, numbers, booleans, nil, and `--` / `--[[ ]]` comments. Anything
//! else makes the file unreadable rather than half-read: a wrong /played is
//! worse than none.

use std::collections::BTreeMap;

/// A Lua value, as far as this file needs.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
    Table(BTreeMap<String, Value>),
}

impl Value {
    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Table(t) => t.get(key),
            _ => None,
        }
    }
    fn str(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
    fn num(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"--[[") {
                match find(&self.s[self.i + 4..], b"]]") {
                    Some(end) => self.i += 4 + end + 2,
                    None => self.i = self.s.len(),
                }
            } else if self.s[self.i..].starts_with(b"--") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
            } else {
                return;
            }
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Option<String> {
        self.skip();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
        {
            self.i += 1;
        }
        if self.i == start || self.s[start].is_ascii_digit() {
            self.i = start;
            return None;
        }
        Some(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
    }

    fn string(&mut self) -> Option<String> {
        self.skip();
        let quote = *self.s.get(self.i)?;
        if quote != b'"' && quote != b'\'' {
            return None;
        }
        self.i += 1;
        let mut out = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            if c == quote {
                return String::from_utf8(out).ok();
            }
            if c != b'\\' {
                out.push(c);
                continue;
            }
            let e = *self.s.get(self.i)?;
            self.i += 1;
            match e {
                b'n' => out.push(b'\n'),
                b't' => out.push(b'\t'),
                b'r' => out.push(b'\r'),
                b'\\' | b'"' | b'\'' => out.push(e),
                b'\n' => out.push(b'\n'),
                b'0'..=b'9' => {
                    // \ddd: up to three decimal digits, one byte.
                    let mut n = (e - b'0') as u32;
                    for _ in 0..2 {
                        match self.s.get(self.i) {
                            Some(d) if d.is_ascii_digit() => {
                                n = n * 10 + (d - b'0') as u32;
                                self.i += 1;
                            }
                            _ => break,
                        }
                    }
                    out.push(u8::try_from(n).ok()?);
                }
                _ => return None,
            }
        }
        None
    }

    fn number(&mut self) -> Option<f64> {
        self.skip();
        let start = self.i;
        while self.i < self.s.len()
            && matches!(
                self.s[self.i],
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E'
            )
        {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .parse()
            .ok()
    }

    fn value(&mut self) -> Option<Value> {
        self.skip();
        match *self.s.get(self.i)? {
            b'{' => self.table(),
            b'"' | b'\'' => self.string().map(Value::Str),
            b'-' | b'0'..=b'9' | b'.' => self.number().map(Value::Num),
            _ => match self.ident()?.as_str() {
                "true" => Some(Value::Bool(true)),
                "false" => Some(Value::Bool(false)),
                "nil" => Some(Value::Nil),
                _ => None,
            },
        }
    }

    fn table(&mut self) -> Option<Value> {
        if !self.eat(b'{') {
            return None;
        }
        let mut t = BTreeMap::new();
        let mut index = 1u64;
        loop {
            if self.eat(b'}') {
                return Some(Value::Table(t));
            }
            let key = if self.eat(b'[') {
                let k = match self.value()? {
                    Value::Str(s) => s,
                    Value::Num(n) => format!("{n}"),
                    _ => return None,
                };
                if !self.eat(b']') || !self.eat(b'=') {
                    return None;
                }
                k
            } else {
                let save = self.i;
                match self.ident() {
                    Some(name) if self.eat(b'=') => name,
                    _ => {
                        // A positional entry.
                        self.i = save;
                        let k = index.to_string();
                        index += 1;
                        k
                    }
                }
            };
            let v = self.value()?;
            t.insert(key, v);
            if !self.eat(b',') && !self.eat(b';') {
                return if self.eat(b'}') {
                    Some(Value::Table(t))
                } else {
                    None
                };
            }
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Reads `name = <value>` assignments and returns the one called `name`.
pub fn global(src: &str, name: &str) -> Option<Value> {
    let mut p = Parser {
        s: src.as_bytes(),
        i: 0,
    };
    loop {
        p.skip();
        if p.i >= p.s.len() {
            return None;
        }
        let id = p.ident()?;
        if !p.eat(b'=') {
            return None;
        }
        let v = p.value()?;
        if id == name {
            return Some(v);
        }
    }
}

/// One character as the addon recorded it.
#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub region: String,
    pub realm: String,
    pub realm_norm: String,
    pub name: String,
    pub played_seconds: i64,
    pub level: Option<i64>,
    pub class: Option<String>,
    /// Unix seconds, from the game's clock.
    pub at: i64,
}

/// The characters of a KFire.lua file, or None when it is unreadable or of
/// an unknown version.
pub fn characters(src: &str) -> Option<Vec<Character>> {
    let root = global(src, "KFirePlayed")?;
    if root.num("v")? as i64 != 1 {
        return None;
    }
    let Value::Table(chars) = root.get("chars")? else {
        return None;
    };
    let mut out = Vec::new();
    for c in chars.values() {
        let (Some(name), Some(realm_norm), Some(played), Some(at)) = (
            c.str("name"),
            c.str("realmNorm"),
            c.num("played"),
            c.num("at"),
        ) else {
            continue;
        };
        if played < 0.0 || name.is_empty() || realm_norm.is_empty() {
            continue;
        }
        out.push(Character {
            region: c.str("region").unwrap_or_else(|| "unknown".into()),
            realm: c.str("realm").unwrap_or_else(|| realm_norm.clone()),
            realm_norm,
            name,
            played_seconds: played as i64,
            level: c.num("level").map(|l| l as i64),
            class: c.str("class"),
            at: at as i64,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like a real file: WoW writes bracketed keys, a trailing comma on
    /// every entry, and may add other addons' variables in other files only.
    const REAL: &str = r#"
KFirePlayed = {
["v"] = 1,
["chars"] = {
["eu/Cho'gall/Ouranos"] = {
["realm"] = "Cho'gall",
["played"] = 17587583,
["region"] = "eu",
["at"] = 1790870000,
["class"] = "PRIEST",
["level"] = 80,
["name"] = "Ouranos",
["realmNorm"] = "Cho'gall",
},
["eu/ConfrérieduThorium/Jaïna"] = {
["realm"] = "Confrérie du Thorium",
["played"] = 3600.5,
["region"] = "eu",
["at"] = 1790860000,
["class"] = "MAGE",
["level"] = 70,
["name"] = "Jaïna",
["realmNorm"] = "ConfrérieduThorium",
},
},
}
"#;

    #[test]
    fn reads_a_real_file() {
        let mut c = characters(REAL).expect("readable");
        c.sort_by(|a, b| b.played_seconds.cmp(&a.played_seconds));
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].name, "Ouranos");
        assert_eq!(c[0].played_seconds, 17_587_583);
        assert_eq!(c[0].realm, "Cho'gall");
        assert_eq!(c[1].name, "Jaïna");
        assert_eq!(c[1].played_seconds, 3600);
        assert_eq!(c[1].class.as_deref(), Some("MAGE"));
    }

    #[test]
    fn escapes_comments_and_bare_keys() {
        let src = "-- written by WoW\nKFirePlayed = { v = 1; chars = { [\"x\"] = { name = \"A\\\"b\", realmNorm = \"R\", played = 5, at = 1, }, }, } --[[ end ]]";
        let c = characters(src).unwrap();
        assert_eq!(c[0].name, "A\"b");
    }

    #[test]
    fn unknown_version_truncated_or_foreign_is_refused() {
        assert!(characters("KFirePlayed = { [\"v\"] = 2, [\"chars\"] = {} }").is_none());
        assert!(characters(&REAL[..REAL.len() / 2]).is_none());
        assert!(characters("SomethingElse = { }").is_none());
        assert!(characters("").is_none());
    }

    #[test]
    fn a_character_missing_its_played_is_skipped_not_the_file() {
        let src = "KFirePlayed = { v = 1, chars = { a = { name = \"A\", realmNorm = \"R\", at = 1 }, b = { name = \"B\", realmNorm = \"R\", played = 7, at = 1 } } }";
        let c = characters(src).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].name, "B");
    }
}
