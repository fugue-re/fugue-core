use std::env;

fn main() {
    flatcc::build(env::current_dir().unwrap().join("schema/fugue.fbs"))
        .expect("schema compiled successfully");
}
