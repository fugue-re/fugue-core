use std::path::Path;
use std::env;

fn main() {
    flatc_rust::Flatc::from_path(flatc::flatc()).run(flatc_rust::Args {
        inputs: &[&env::current_dir().unwrap().join("schema/fugue.fbs")],
        out_dir: Path::new(&env::var("OUT_DIR").unwrap()),
        ..Default::default()
    }).expect("schema compiled successfully");
}
