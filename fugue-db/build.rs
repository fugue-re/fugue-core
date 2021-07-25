use std::path::Path;
use flatc_rust as flatc;

fn main() {
    flatc::run(flatc::Args {
        inputs: &[Path::new("schema/fugue.fbs")],
        out_dir: Path::new("target/flatbuffers/"),
        ..Default::default()
    }).expect("schema compiled successfully");
}
