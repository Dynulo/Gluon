use serde::{Serialize, Deserialize};

use serde_json;

use std::fs::File;
use std::io::Error;

use std::io::Write;

use crate::error::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct Repo {
    pub l: Vec<Layer>
}
impl Repo {
    pub fn new() -> Self {
        Repo {
            l: Vec::new()
        }
    }
    pub fn save(&self, dir: &String) -> Result<(), Error> {
        let mut out = File::create(format!("{}/repo.cbor", dir))?;
        serde_cbor::to_writer(&mut out, &self).unwrap_or_print();
        let mut out = File::create(format!("{}/repo.json", dir))?;
        out.write_fmt(format_args!("{}", serde_json::to_string_pretty(&self)?));
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Layer {
    pub n: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub f: Vec<ModFile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub l: Vec<Layer>,
}
impl Layer {
    pub fn new(name: String) -> Self {
        Layer {
            n: name,
            f: Vec::new(),
            l: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModFile {
    pub n: String,
    pub h: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub p: Vec<ModPart>,
}
impl ModFile {
    pub fn new(name: String, hash: String) -> Self {
        ModFile {
            n: name,
            h: hash,
            p: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModPart {
    pub n: String,
    pub h: String,
    pub l: usize,
    pub s: usize,
}
impl ModPart {
    pub fn new(name: String, hash: String, size: usize, start: usize) -> Self {
        ModPart {
            n: name,
            h: hash,
            l: size,
            s: start,
        }
    }
}
