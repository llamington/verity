use std::fs;
use std::path::PathBuf;

use verity::pkm::{decrypt_45, encrypt_45};

fn dump_dirs() -> Vec<PathBuf> {
    let root = std::env::var("VERITY_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus"));
    let reports = root.join("reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return Vec::new();
    };
    let mut dirs: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.extension().is_some_and(|e| e == "pkm"))
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn crypto_matches_pkhex_dumps() {
    let dirs = dump_dirs();
    if dirs.is_empty() {
        eprintln!("skipping: no oracle dumps (run scripts/oracle-smoke.sh)");
        return;
    }
    let mut checked = 0;
    for dir in dirs {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "enc") {
                continue;
            }
            let enc = fs::read(&path).unwrap();
            let dec = ["pk4", "pk5"]
                .iter()
                .map(|x| path.with_extension(x))
                .find(|p| p.exists())
                .map(|p| fs::read(p).unwrap())
                .unwrap();

            let mut buf = enc.clone();
            decrypt_45(&mut buf);
            assert_eq!(buf, dec, "decrypt mismatch: {}", path.display());
            encrypt_45(&mut buf);
            assert_eq!(buf, enc, "encrypt mismatch: {}", path.display());
            checked += 1;
        }
    }
    assert!(checked > 0);
    eprintln!("checked {checked} pkm");
}
