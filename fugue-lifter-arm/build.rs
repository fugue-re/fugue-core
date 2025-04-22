#![allow(unused)]

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

fn build_lifter(arch: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let lifter = fugue_lifter_codegen::build("data/processors", arch)?;
    let output = PathBuf::from_iter([
        env::var("OUT_DIR").expect("OUT_DIR").as_ref(),
        output,
    ]);

    let mut writer = BufWriter::new(File::create(&output)?);
    writer.write_all(lifter.as_ref())?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=data/processors");

    #[cfg(feature = "arm-be")]
    build_lifter("ARM:BE:32:v8", "arm_be.rs")?;
    #[cfg(feature = "arm-le")]
    build_lifter("ARM:LE:32:v8", "arm_le.rs")?;

    Ok(())
}
