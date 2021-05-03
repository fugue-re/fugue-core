use criterion::{criterion_group, criterion_main, Criterion};

use fugue_ir::LanguageDB;
use fugue_ir::processor::Specification;

fn criterion_pspec_x86(c: &mut Criterion) {
    c.bench_function("Specification::from_file(\"x86.pspec\")]",
                     |b| b.iter(|| Specification::from_file("../data/x86.pspec")));
}

fn criterion_trans_x86(c: &mut Criterion) {
    c.bench_function("LanguageDB::from_file(\"x86.ldefs\")]",
                     |b| b.iter(|| -> Result<(), fugue_lift::error::Error> {
                         let ldef = LanguageDB::from_file("./data/x86/x86.ldefs")?;
                         for builder in ldef.iter() {
                             builder.build()?;
                         }
                         Ok(())
                     }));
}

criterion_group!(benches, criterion_pspec_x86, criterion_trans_x86);
criterion_main!(benches);
