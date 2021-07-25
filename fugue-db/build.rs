use std::path::Path;

fn main() {
    flatc_rust::Flatc::from_path(flatc::flatc()).run(flatc_rust::Args {
        inputs: &[Path::new("schema/fugue.fbs")],
        out_dir: Path::new("target/flatbuffers/"),
        ..Default::default()
    }).expect("schema compiled successfully");
}
