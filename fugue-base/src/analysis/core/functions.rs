use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use itertools::Itertools;

use crate::analysis::{AnalysisError, AnalysisPass};
use crate::lifter::{LiftedInsnTargetKind, LifterExt};
use crate::project::Project;
use crate::storage::StorageProvider;
use crate::types::Address;

pub struct ControlFlowRecoveryConfig {
    pub max_blocks: usize,
}

impl Default for ControlFlowRecoveryConfig {
    fn default() -> Self {
        ControlFlowRecoveryConfig { max_blocks: 65536 }
    }
}

pub struct ControlFlowRecovery {
    config: ControlFlowRecoveryConfig,
    candidates: VecDeque<Address>,
}

struct FunctionBuilder {
    candidates: VecDeque<Address>,
    local_targets: BTreeSet<Address>,
    global_targets: BTreeSet<Address>,
}

impl ControlFlowRecovery {
    pub fn new() -> Self {
        ControlFlowRecovery::new_with(ControlFlowRecoveryConfig::default())
    }

    pub fn new_with(config: ControlFlowRecoveryConfig) -> Self {
        ControlFlowRecovery {
            config,
            candidates: VecDeque::new(),
        }
    }

    pub fn add_candidate(&mut self, address: impl Into<Address>) {
        self.candidates.push_back(address.into());
    }

    pub fn add_candidates(&mut self, addresses: impl IntoIterator<Item = impl Into<Address>>) {
        self.candidates
            .extend(addresses.into_iter().map(|addr| addr.into()));
    }
}

impl AnalysisPass for ControlFlowRecovery {
    fn analyse(&mut self, project: &mut Project) -> Result<(), AnalysisError> {
        let mut builder = FunctionBuilder::new();

        if let Some(entry) = project.entry() {
            tracing::debug!("entry point: {entry}");
            self.add_candidate(entry);
        }

        for symbol in project.iter_local_symbols() {
            tracing::debug!(
                "local function: {} (name: {:?})",
                symbol.address(),
                symbol.symbol()
            );
            self.add_candidate(symbol.address());
        }

        while let Some(address) = self.candidates.pop_front() {
            if !project.storage.contains_segment(address) {
                tracing::trace!("skipping {address}: not mapped");
            }

            let _f = builder.analyse(project, address);

            self.add_candidates(builder.global_targets.iter().copied());
        }

        Ok(())
    }
}

impl FunctionBuilder {
    pub fn new() -> Self {
        FunctionBuilder {
            candidates: VecDeque::new(),
            local_targets: BTreeSet::new(),
            global_targets: BTreeSet::new(),
        }
    }

    pub fn add_candidate(&mut self, address: impl Into<Address>) {
        self.candidates.push_back(address.into());
    }

    pub fn add_candidates(&mut self, addresses: impl IntoIterator<Item = impl Into<Address>>) {
        self.candidates
            .extend(addresses.into_iter().map(|addr| addr.into()));
    }

    pub fn clear(&mut self) {
        self.candidates.clear();
        self.local_targets.clear();
        self.global_targets.clear();
    }

    pub fn analyse(&mut self, project: &mut Project, address: impl Into<Address>) {
        let candidate = address.into();

        tracing::debug!("exploring from {candidate}");

        self.candidates.clear();
        self.local_targets.clear();
        self.global_targets.clear();

        self.candidates.push_back(candidate);

        let mut insns = BTreeMap::<Address, _>::new();
        let mut bytes = [0u8; 32];

        'pass: loop {
            // This is the stage where we build blocks by collecting instructions and marking them.
            'outer: while let Some(block) = self.candidates.pop_front() {
                if !project.storage.contains_segment(block) {
                    tracing::trace!("skipping {block}: not mapped");
                    continue 'outer;
                }

                if !self.local_targets.insert(block) {
                    continue;
                }

                let mut offset = 0usize;

                '_inner: loop {
                    let address = block + offset;

                    tracing::debug!("lifting at {address}");

                    // If we've already disassembled this instruction select the next candidate,
                    // otherwise get the entry ready for update.
                    let Entry::Vacant(entry) = insns.entry(address) else {
                        continue 'outer;
                    };

                    let Ok(size) = project.storage.read_bytes(address, &mut bytes) else {
                        tracing::trace!("skipping {block}: not mapped");
                        continue;
                    };

                    tracing::debug!("lifting {address}: {:?} ({size})", bytes);

                    let bytes = &bytes[..size];

                    match project.lifter.lift_insn(address, bytes) {
                        Ok(insn) => {
                            let insn = entry.insert(insn);

                            tracing::trace!("{address}: {:?}", insn.properties());

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
                        Err(e) => {
                            // flows into bad data??
                            // self.local_targets.remove(&address);
                            tracing::debug!("skipping {address}; lifting failed: {e}");
                            continue 'outer;
                        }
                    }
                }
            }

            tracing::debug!("{:?}", self.local_targets);

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
                tracing::debug!("blk@{}", block[0].0);
                for (addr, insn) in block {
                    tracing::debug!("{addr}: {}", insn.display(project.language));
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
    use super::*;

    use crate::analysis::AnalysisPass;
    use crate::attributes;
    use crate::storage::MemoryMappedStorage;
    use crate::types::attributes::*;

    #[test]
    fn test_control_flow_recovery() -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .with_line_number(true)
            .with_file(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut project = Project::from_file_with::<MemoryMappedStorage>(
                "tests/ls.elf",
                attributes![
                    ATTRIBUTE_PROJECT_PATH => "/tmp/ls.fudb",
                ],
            )?;
            let mut cfr = ControlFlowRecovery::new();

            cfr.analyse(&mut project)?;

            Ok(())
        })
    }
}
