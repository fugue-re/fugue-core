use std::collections::{BTreeSet, VecDeque};

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

    pub fn analyse(&mut self, project: &mut Project) {
        let mut builder = FunctionBuilder::new();

        while let Some(address) = self.candidates.pop_front() {
            if !project.storage.contains_segment(address) {
                tracing::trace!("skipping {address}: not mapped");
            }

            let _f = builder.analyse(project, address);

            todo!()
        }
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
        let address = address.into();
        todo!()
    }
}
