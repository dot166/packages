use anyhow::Result;
use brotli::Decompressor;
use std::fs::File;
use std::io;
use std::path::Path;

pub fn decompress_brotli(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    let input = File::open(source)?;
    let mut decoder = Decompressor::new(input, 4096);

    let mut output = File::create(destination)?;

    io::copy(&mut decoder, &mut output)?;

    Ok(())
}
