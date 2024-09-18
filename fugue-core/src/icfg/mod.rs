use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::marker::PhantomData;

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use itertools::Itertools as _;

use thiserror::Error;

use crate::language::Language;
use crate::lifter::LiftedInsnTargetKind;
use crate::lifter::{InsnLifter, Lifter};
use crate::project::{
    LoadedSegment, Project, ProjectRawView, ProjectRawViewError, ProjectRawViewReader,
};

#[derive(Debug, Error)]
pub enum ICFGBuilerError {
    #[error(transparent)]
    RawView(#[from] ProjectRawViewError),
}

struct ICFGLiftingContext<'a, R>
where
    R: ProjectRawView,
{
    language: &'a Language,
    lifter: Lifter<'a>,
    fast_lifter: Box<dyn InsnLifter>,
    view: R::Reader<'a>,
    _marker: PhantomData<&'a R>,
}

impl<'a, R> ICFGLiftingContext<'a, R>
where
    R: ProjectRawView,
{
    fn new(project: &'a Project<R>) -> Result<Self, ICFGBuilerError> {
        Ok(Self {
            language: project.language(),
            lifter: project.lifter(),
            fast_lifter: project.language().lifter_for_arch(),
            view: project.raw().reader()?,
            _marker: PhantomData,
        })
    }
}

pub struct ICFGBuilder<'a, R>
where
    R: ProjectRawView,
{
    config: ICFGBuilderConfig,
    context: ICFGLiftingContext<'a, R>,
    candidates: VecDeque<Address>,
    arena: IRBuilderArena,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ICFGBuilderConfig {
    pub arena_init_capacity: usize,
    pub arena_purge_threshold: usize,

    pub ignore_loader_entrypoint: bool,
}

impl Default for ICFGBuilderConfig {
    fn default() -> Self {
        Self {
            arena_init_capacity: 4_096,
            arena_purge_threshold: 1_000_000,

            ignore_loader_entrypoint: false,
        }
    }
}

impl<'a, R> ICFGBuilder<'a, R>
where
    R: ProjectRawView,
{
    pub fn new(project: &'a Project<R>) -> Result<Self, ICFGBuilerError> {
        Self::new_with(project, Default::default())
    }

    pub fn new_with(
        project: &'a Project<R>,
        config: ICFGBuilderConfig,
    ) -> Result<Self, ICFGBuilerError> {
        let mut slf = Self {
            arena: IRBuilderArena::with_capacity(config.arena_init_capacity),
            candidates: VecDeque::new(),
            config,
            context: ICFGLiftingContext::new(project)?,
        };

        if !config.ignore_loader_entrypoint {
            if let Some(entry) = project.entry() {
                slf.add_candidate(entry);
            }
        }

        Ok(slf)
    }

    pub fn add_candidate(&mut self, candidate: impl Into<Address>) {
        self.candidates.push_back(candidate.into());
    }

    pub fn add_candidates(&mut self, candidates: impl IntoIterator<Item = Address>) {
        self.candidates.extend(candidates);
    }

    pub fn explore(&mut self) {
        let mut fb = FunctionBuilder::new(
            &self.arena,
            &mut self.context.lifter,
            &mut self.context.fast_lifter,
        );

        // Pass: explore candidates
        while let Some(candidate) = self.candidates.pop_front() {
            let Ok(region) = self.context.view.find_region(candidate) else {
                // Unknown address -- continue
                continue;
            };

            fb.explore(candidate, region);
        }

        // Pass: explore gaps
    }
}

pub struct FunctionBuilder<'a, 'b> {
    arena: &'a IRBuilderArena,
    lifter: &'a mut Lifter<'b>,
    fast_lifter: &'a mut Box<dyn InsnLifter>,
    candidates: VecDeque<Address>,
    local_targets: BTreeSet<Address>,
    global_targets: BTreeSet<Address>,
}

impl<'a, 'b> FunctionBuilder<'a, 'b> {
    pub fn new(
        arena: &'a IRBuilderArena,
        lifter: &'a mut Lifter<'b>,
        fast_lifter: &'a mut Box<dyn InsnLifter>,
    ) -> Self {
        Self {
            arena,
            lifter,
            fast_lifter,
            candidates: VecDeque::new(),
            local_targets: BTreeSet::new(),
            global_targets: BTreeSet::new(),
        }
    }

    pub fn explore(&mut self, candidate: Address, region: LoadedSegment) {
        println!("exploring from {candidate}");

        self.candidates.clear();
        self.local_targets.clear();
        self.global_targets.clear();

        self.candidates.push_back(candidate);

        let view = region.data();
        let start = region.address();
        let bounds = start..start + view.len();

        let mut insns = BTreeMap::<Address, _>::new();

        'pass: loop {
            // This is the stage where we build blocks by collecting instructions and marking them.
            'outer: while let Some(block) = self.candidates.pop_front() {
                // If the requested block is not inside our region, then we mark it as global
                if !bounds.contains(&block) {
                    self.global_targets.insert(block);
                    continue;
                }

                if !self.local_targets.insert(block) {
                    continue;
                }

                let mut offset = usize::from(block - start);

                '_inner: loop {
                    let address = start + offset;

                    // If we've already disassembled this instruction select the next candidate,
                    // otherwise get the entry ready for update.
                    let Entry::Vacant(entry) = insns.entry(address) else {
                        continue 'outer;
                    };

                    match self.fast_lifter.properties(
                        self.lifter,
                        self.arena,
                        address,
                        &view[offset..],
                    ) {
                        Ok(insn) => {
                            let insn = entry.insert(insn);

                            // Explicit control-flow
                            if insn.is_flow() {
                                // We're done with this block; we schedule the next bit of work

                                // These targets are what we can statically compute by scanning
                                // the instruction's PCode branch operations--we will miss things
                                // like PC relative jumps.
                                for (kind, target) in insn.iter_targets() {
                                    match kind {
                                        LiftedInsnTargetKind::Local => {
                                            if !self.local_targets.contains(&target) {
                                                self.candidates.push_back(target);
                                            }
                                        }
                                        LiftedInsnTargetKind::Global => {
                                            self.global_targets.insert(target);
                                        }
                                    }
                                }
                            }

                            // Implicit control-flow (it is a halt, etc.)
                            if !insn.has_fall() {
                                // we're done with this block
                                continue 'outer;
                            }

                            offset += insn.len();
                        }
                        Err(_) => {
                            // flows into bad data
                            self.local_targets.remove(&address);
                            continue 'outer;
                        }
                    }
                }
            }

            println!("{:?}", self.local_targets);

            // Structure the blocks
            let iinsns = &mut itertools::put_back(insns.iter());
            let mut iblocks = self
                .local_targets
                .iter()
                .skip(1)
                .chain(std::iter::once(&Address::MAX));

            // Targets may contain invalid addresses...
            let mut blocks = Vec::new();

            while let Some(next_block_start) = iblocks.next() {
                blocks.push(
                    iinsns
                        .peeking_take_while(|(start, _)| *start < next_block_start)
                        .collect::<Vec<_>>(),
                );
            }

            for block in blocks {
                println!("blk@{}", block[0].0);
                for (addr, insn) in block {
                    println!("{addr}: {:#?}", insn.try_pcode());
                }
            }

            // In this stage we attempt to recover function control-flow and schedule more blocks
            // due to jump table resolution.
            break;
        }
    }
}

#[cfg(test)]
mod test {
    use crate::language::LanguageBuilder;
    use crate::loader::{Loadable, Object};
    use crate::project::{ProjectBuilder, ProjectRawViewMmaped};
    use crate::util::BytesOrMapping;

    use super::*;

    #[test]
    #[ignore]
    fn test_icfg_explore1() -> Result<(), Box<dyn std::error::Error>> {
        // Load the binary at tests/ls.elf into a mapping object
        let input = BytesOrMapping::from_file("tests/ls.elf")?;
        let object = Object::new(input)?;

        let language_builder = LanguageBuilder::new("data")?;
        let project_builder = ProjectBuilder::<ProjectRawViewMmaped>::new(language_builder);

        // Create the project from the mapping object
        let project = project_builder.build(&object)?;

        // Let's get some functions!
        let mut icfg_builder = ICFGBuilder::new(&project)?;

        icfg_builder.add_candidate(0x4060u32);
        icfg_builder.explore();

        Ok(())
    }

    #[test]
    fn test_icfg_explore2() -> Result<(), Box<dyn std::error::Error>> {
        // Load the binary at tests/ls.elf into a mapping object
        let input = BytesOrMapping::from_file("tests/big.elf")?;
        let object = Object::new(input)?;

        let language_builder = LanguageBuilder::new("data")?;
        let project_builder = ProjectBuilder::<ProjectRawViewMmaped>::new(language_builder);

        // Create the project from the mapping object
        let project = project_builder.build(&object)?;

        // Let's get some functions!
        let mut icfg_builder = ICFGBuilder::new_with(
            &project,
            ICFGBuilderConfig {
                ignore_loader_entrypoint: true,
                ..Default::default()
            },
        )?;

        icfg_builder.add_candidate(0xffffffff813d1cf0u64);
        icfg_builder.explore();

        Ok(())
    }
}
