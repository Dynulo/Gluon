use armake2::io::*;
use armake2::pbo::{PBO, PBOHeader};
use linked_hash_map::{LinkedHashMap};
use rayon::prelude::*;
use reqwest;
use reqwest::header::{HeaderValue, RANGE};

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io::{Cursor, Error, Read};
use std::path::PathBuf;

use crate::error::*;
use crate::files::*;

type QueueVec = VecDeque<(ModFile, PathBuf, String)>;

pub fn process(dir: PathBuf, config: String) -> Result<(), Error> {
    let mut body = reqwest::get(&format!("{}/repo.cbor", &config)).unwrap_or_print();
    let repo = crate::files::Repo::open(&mut body)?;
    let mut queue: QueueVec  = VecDeque::new();
    for moddir in &repo.l {
        println!("Mod: {}", moddir.n);
        process_layer(&dir, &config, &String::from("."), moddir, &mut queue).unwrap_or_print();
    }
    queue.par_iter().for_each(|entry| {
        let (file, pbuf, path) = entry;
        println!("Downloading {:?} to {}", pbuf, format!("{}/{}", dir.display(), &path));
        crate::server::send(format!("B {:?}", pbuf));
        fs::create_dir_all(format!("{}/{}", dir.display(), &path)).unwrap_or_print();
        let mut urlpath = path.clone();
        urlpath.remove(0);
        urlpath.remove(0);
        let url = format!("{}/{}/{}", &config, &urlpath, &file.n);
        if pbuf.exists() {
            if let Some(a) = pbuf.extension() {
                if a == "pbo" {
                    pbo(pbuf, file, url).unwrap_or_print();
                } else {
                    let mut out = File::create(pbuf).unwrap_or_print();
                    crate::download::download(&url, &mut out, None).unwrap_or_print();
                }
            } else {
                let mut out = File::create(pbuf).unwrap_or_print();
                crate::download::download(&url, &mut out, None).unwrap_or_print();
            }
        } else {
            let mut out = File::create(pbuf).unwrap_or_print();
            crate::download::download(&url, &mut out, None).unwrap_or_print();
        }
        crate::server::send(format!("E {:?}", pbuf));
    });
    crate::server::send(format!("Done"));
    Ok(())
}

pub fn process_layer(dir: &PathBuf, root: &String, lpath: &String, layer: &Layer, queue: &mut QueueVec) -> Result<(), Error> {
    let path = format!("{}/{}", &lpath, &layer.n);
    println!("layer: {}", path);
    if PathBuf::from(format!("{}/{}", dir.display(), &path)).exists() {
        for e in fs::read_dir(format!("{}/{}", dir.display(), &path))? {
            let epath = e?.path();
            let name = epath.file_name().unwrap().to_str().unwrap().to_owned();
            if epath.is_dir() {
                if !layer.l.iter().any(|x| x.n == name) {
                    fs::remove_dir_all(format!("{}/{}/{}", dir.display(), &path, name))?;
                }
            } else {
                if !layer.f.iter().any(|x| x.n == name) {
                    fs::remove_file(format!("{}/{}/{}", dir.display(), &path, name))?;
                }
            }
        }
    }

    for slayer in &layer.l {
        process_layer(&dir, &root, &path, slayer, queue)?;
    }
    for file in layer.f.clone() {
        let pbuf = PathBuf::from(&format!("{}/{}/{}", dir.display(), &path, &file.n));
        if pbuf.exists() {
            if crate::hash::hash_file(&pbuf)? != file.h {
                queue.push_back((file, pbuf, path.clone()));
            }
        } else {
            queue.push_back((file, pbuf, path.clone()));
        }
    }
    Ok(())
}

fn pbo(pbuf: &PathBuf, file: &ModFile, url: String) -> Result<(), Error> {
    // https://www.youtube.com/watch?v=KAHLwAxS7FI
    let pbofile = PBO::read(&mut File::open(&pbuf).unwrap_or_print()).unwrap_or_print();
    let client = reqwest::Client::new();
    let mut response = client.get(&url)
        .header(RANGE,
            HeaderValue::from_str(&format!("bytes={}-{}", 0, file.p[0].s - 1)).unwrap_or_print()
        ).send().unwrap_or_print();
    let mut headers: Vec<PBOHeader> = Vec::new();
    let mut first = true;
    let mut header_extensions: HashMap<String, String> = HashMap::new();
    loop {
        let header = PBOHeader::read(&mut response)?;
        if header.packing_method == 0x56657273 {
            if !first { unreachable!(); }
            loop {
                let s = response.read_cstring()?;
                if s.len() == 0 { break; }
                header_extensions.insert(s, response.read_cstring()?);
            }
        } else if header.filename == "" {
            break;
        } else {
            headers.push(header);
        }
        first = false;
    }
    let mut files: LinkedHashMap<String, Cursor<Box<[u8]>>> = LinkedHashMap::new();
    for header in &headers {
        let newfile = file.p.iter().filter(|&x| x.n == header.filename).collect::<Vec<_>>()[0];
        if !pbofile.headers.iter().any(|x| x.filename == header.filename) {
            println!("new file: {}", header.filename);
            let client = reqwest::Client::new();
            let mut response = client.get(&url)
                .header(RANGE,
                    HeaderValue::from_str(&format!("bytes={}-{}", newfile.s, newfile.s + newfile.l)).unwrap_or_print()
                ).send().unwrap_or_print();
            let mut buffer: Box<[u8]> = vec![0; newfile.l as usize].into_boxed_slice();
            response.read_exact(&mut buffer)?;
            files.insert(header.filename.clone(), Cursor::new(buffer));
        } else {
            if crate::hash::hash_cursor(pbofile.files.get(&header.filename).unwrap().clone())? != newfile.h {
                println!("hash part mismatch: {}", header.filename);
                let client = reqwest::Client::new();
                let mut response = client.get(&url)
                    .header(RANGE,
                        HeaderValue::from_str(&format!("bytes={}-{}", newfile.s, newfile.s + newfile.l)).unwrap_or_print()
                    ).send().unwrap_or_print();
                let mut buffer: Box<[u8]> = vec![0; newfile.l as usize].into_boxed_slice();
                response.read_exact(&mut buffer)?;
                files.insert(header.filename.clone(), Cursor::new(buffer));
            } else {
                files.insert(header.filename.clone(), pbofile.files.get(&header.filename).unwrap().clone());
            }
        }
    }
    let newpbo = PBO {
        files: files,
        header_extensions: header_extensions,
        headers: headers,
        checksum: None,
    };
    let mut outfile = File::create(&pbuf)?;
    newpbo.write(&mut outfile)?;
    Ok(())
}
