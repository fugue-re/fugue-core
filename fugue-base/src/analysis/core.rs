use std::collections::HashMap;

use thiserror::Error;

use crate::project::Project;

pub mod functions;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("analysis pass forms a cyclic dependency: {0} -> {1}")]
    CyclicDependency(String, String),
    #[error("analysis pass not found: {0}")]
    PassNotFound(String),
    #[error("analysis pass failed: {0}")]
    PassFailed(String, anyhow::Error),
}

pub struct AnalysisManager<S = ()> {
    passes: HashMap<String, Box<dyn AnalysisPass<S>>>,
}

impl<S> AnalysisManager<S> {
    pub fn new() -> Self {
        AnalysisManager {
            passes: HashMap::new(),
        }
    }

    pub fn add_pass(&mut self, name: impl Into<String>, pass: impl AnalysisPass<S> + 'static) {
        self.passes.insert(name.into(), Box::new(pass));
    }

    pub fn analyse_with(
        &mut self,
        project: &mut Project,
        pass_name: &str,
        state: &mut S,
    ) -> Result<(), AnalysisError> {
        if let Some(pass) = self.passes.get_mut(pass_name) {
            pass.analyse_with(project, state)
        } else {
            Err(AnalysisError::PassNotFound(pass_name.to_string()))
        }
    }
}

impl AnalysisManager {
    pub fn analyse(&mut self, project: &mut Project, pass_name: &str) -> Result<(), AnalysisError> {
        self.analyse_with(project, pass_name, &mut Default::default())
    }
}

pub trait AnalysisPass<S = ()> {
    fn analyse(&mut self, #[allow(unused)] project: &mut Project) -> Result<(), AnalysisError> {
        unimplemented!(
            "either `AnalysisPass::analyse` or `AnalysisPass::analyse_with` must be implemented"
        )
    }

    fn analyse_with(
        &mut self,
        project: &mut Project,
        #[allow(unused)] state: &mut S,
    ) -> Result<(), AnalysisError> {
        self.analyse(project)
    }
}

impl<S, F> AnalysisPass<S> for F
where
    F: FnMut(&mut Project, &mut S) -> Result<(), AnalysisError>,
{
    fn analyse_with(&mut self, project: &mut Project, state: &mut S) -> Result<(), AnalysisError> {
        self(project, state)
    }
}

pub trait AnalysisCondition {
    fn evaluate(&mut self) -> bool;
}

impl<F> AnalysisCondition for F
where
    F: FnMut() -> bool,
{
    fn evaluate(&mut self) -> bool {
        self()
    }
}

impl AnalysisCondition for usize {
    fn evaluate(&mut self) -> bool {
        if *self > 0 {
            *self -= 1;
            true
        } else {
            false
        }
    }
}

pub struct AnalysisGroup<S = ()> {
    passes: Vec<Box<dyn AnalysisPass<S>>>,
}

impl<S> AnalysisGroup<S> {
    pub fn new() -> Self {
        AnalysisGroup { passes: Vec::new() }
    }

    pub fn add_pass(&mut self, pass: impl AnalysisPass<S> + 'static) {
        self.passes.push(Box::new(pass));
    }

    pub fn add_passes(&mut self, passes: impl IntoIterator<Item = impl AnalysisPass<S> + 'static>) {
        self.passes.extend(
            passes
                .into_iter()
                .map(|pass| Box::new(pass) as Box<dyn AnalysisPass<S>>),
        );
    }
}

impl<S> AnalysisPass<S> for AnalysisGroup<S> {
    fn analyse_with(&mut self, project: &mut Project, state: &mut S) -> Result<(), AnalysisError> {
        for pass in self.passes.iter_mut() {
            pass.analyse_with(project, state)?;
        }
        Ok(())
    }
}

pub struct IteratedAnalysis<S = ()> {
    pass: Box<dyn AnalysisPass<S>>,
    condition: Box<dyn AnalysisCondition>,
}

impl<S> IteratedAnalysis<S> {
    pub fn new(
        pass: impl AnalysisPass<S> + 'static,
        condition: impl AnalysisCondition + 'static,
    ) -> Self {
        IteratedAnalysis {
            pass: Box::new(pass),
            condition: Box::new(condition),
        }
    }
}

impl<S> AnalysisPass<S> for IteratedAnalysis<S> {
    fn analyse_with(&mut self, project: &mut Project, state: &mut S) -> Result<(), AnalysisError> {
        while self.condition.evaluate() {
            self.pass.analyse_with(project, state)?;
        }
        Ok(())
    }
}

pub struct ConditionalAnalysis<S = ()> {
    pass: Box<dyn AnalysisPass<S>>,
    condition: Box<dyn AnalysisCondition>,
}

impl<S> ConditionalAnalysis<S> {
    pub fn new(
        pass: impl AnalysisPass<S> + 'static,
        condition: impl AnalysisCondition + 'static,
    ) -> Self {
        ConditionalAnalysis {
            pass: Box::new(pass),
            condition: Box::new(condition),
        }
    }
}

impl<S> AnalysisPass<S> for ConditionalAnalysis<S> {
    fn analyse_with(&mut self, project: &mut Project, state: &mut S) -> Result<(), AnalysisError> {
        if self.condition.evaluate() {
            self.pass.analyse_with(project, state)?;
        }
        Ok(())
    }
}

pub struct StatefulAnalysis<S> {
    pass: Box<dyn AnalysisPass<S>>,
    state: S,
}

impl<S> StatefulAnalysis<S> {
    pub fn new(pass: impl AnalysisPass<S> + 'static, state: S) -> Self {
        StatefulAnalysis {
            pass: Box::new(pass),
            state,
        }
    }
}

impl<S> AnalysisPass for StatefulAnalysis<S> {
    fn analyse(&mut self, project: &mut Project) -> Result<(), AnalysisError> {
        self.pass.analyse_with(project, &mut self.state)
    }
}

pub type NoState = ();

#[cfg(test)]
mod test {
    use crate::storage::InMemoryStorage;

    use super::*;

    #[test]
    fn test_analysis_passes() -> Result<(), Box<dyn std::error::Error>> {
        let mut analyses = AnalysisManager::new();
        let mut project = Project::<InMemoryStorage>::from_file("tests/ls.elf")?;

        analyses.add_pass(
            "hello-world",
            |_project: &mut Project, _state: &mut NoState| {
                println!("Hello, world!");
                Ok(())
            },
        );

        analyses.add_pass(
            "cond-hello-world",
            IteratedAnalysis::new(
                StatefulAnalysis::new(
                    |_project: &mut Project, state: &mut bool| {
                        println!("Hello, world; state is {state}!");
                        *state = !*state;
                        Ok(())
                    },
                    false,
                ),
                5,
            ),
        );

        analyses.analyse(&mut project, "hello-world")?;
        analyses.analyse(&mut project, "cond-hello-world")?;

        Ok(())
    }
}
