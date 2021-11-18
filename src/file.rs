use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::errors::*;

pub fn read<P>(path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    read_(path.as_ref())
}

fn read_(path: &Path) -> Result<String> {
    let mut s = String::new();
    File::open(path)
        .chain_err(|| format!("couldn't open {}", path.display()))?
        .read_to_string(&mut s)
        .chain_err(|| format!("couldn't read {}", path.display()))?;
    Ok(s)
}
