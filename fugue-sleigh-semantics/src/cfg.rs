use std::collections::hash_map::Entry;

use arrayvec::ArrayVec;
use ustr::{Ustr, UstrMap};

use crate::ast::{AstError, CodeBlock, Stmt};

pub struct CFG<'a> {
    blocks: Vec<&'a [Stmt]>,
    edges: Vec<ArrayVec<usize, 2>>,
    labels: Labels,
}

#[repr(transparent)]
pub struct Labels(UstrMap<usize>);

impl Labels {
    pub fn new() -> Self {
        Self(UstrMap::default())
    }

    #[inline]
    pub fn get(&self, label: Ustr) -> Option<usize> {
        self.0.get(&label).copied()
    }

    #[inline]
    pub fn insert(&mut self, label: Ustr, block: usize) -> Result<(), AstError> {
        match self.0.entry(label) {
            Entry::Vacant(entry) => {
                entry.insert(block);
                Ok(())
            }
            Entry::Occupied(_) => Err(AstError::DuplicateLabel(label)),
        }
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = (Ustr, usize)> + 'a {
        self.0.iter().map(|(n, i)| (*n, *i))
    }
}

impl<'a> CFG<'a> {
    pub fn new(block: &'a CodeBlock) -> Result<Self, AstError> {
        let mut blocks = Vec::new();
        let mut edges = Vec::new();
        let mut labels = Labels::new();

        // start of split
        let mut split = 0;
        // pending labels
        let mut pending = Vec::new();

        let stmts = block.statements();
        let stmts_len = stmts.len();

        // pass 1: create blocks and labels
        for (i, stmt) in stmts.iter().enumerate() {
            if let Some(label) = stmt.label() {
                if pending.is_empty() && i > split {
                    blocks.push(&stmts[split..i]);
                }
                pending.push(label);
                split = i + 1;
                continue;
            }

            if stmt.is_branch() {
                blocks.push(&stmts[split..i + 1]);
                split = i + 1;
            }

            for label in pending.drain(..) {
                labels.insert(label, blocks.len())?;
            }
        }

        // build final block
        if split < stmts_len {
            blocks.push(&stmts[split..stmts_len]);
        }

        // clear any remaining labels
        for label in pending.drain(..) {
            labels.insert(label, blocks.len())?;
        }

        // pass 2: create control-flow
        for (i, block) in blocks.iter().enumerate() {
            let last = block.last().expect("at least one statement");
            let mut links = ArrayVec::new();

            if last.has_fall() {
                links.push(i + 1);
            }

            if let Some(label) = last.branch_target() {
                links.push(labels.get(label).ok_or(AstError::UndefinedLabel(label))?);
            }

            edges.push(links);
        }

        Ok(Self {
            blocks,
            edges,
            labels,
        })
    }

    pub fn block(&self, n: usize) -> Option<&'a [Stmt]> {
        self.blocks.get(n).copied()
    }

    pub fn block_at(&self, label: Ustr) -> Option<(usize, &'a [Stmt])> {
        self.label(label)
            .and_then(|n| self.block(n).map(|b| (n, b)))
    }

    pub fn blocks(&self) -> &[&'a [Stmt]] {
        &self.blocks
    }

    pub fn edges(&self, n: usize) -> Option<&[usize]> {
        self.edges.get(n).map(|edges| edges.as_slice())
    }

    pub fn label(&self, label: Ustr) -> Option<usize> {
        self.labels.get(label)
    }

    pub fn labels(&self) -> &Labels {
        &self.labels
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::CodeBlock;

    #[test]
    fn test_cfg() -> Result<(), Box<dyn std::error::Error>> {
        let ast_ok = CodeBlock::parse(
            r#"
            <start>
                local v0:8 = 10;
                local v1:8 = 20;
                local counter:8 = 0;
            <loop>
            <loop1>
                v2 = v0 + v1;
            <loop2>
                if v2 < 100 goto <loop>;
            <done>
            "#,
        )?;

        let cfg = CFG::new(&ast_ok)?;

        assert_eq!(cfg.blocks.len(), 3);

        assert_eq!(cfg.edges.len(), 3);
        assert_eq!(cfg.edges[0].len(), 1);
        assert_eq!(cfg.edges[1].len(), 1);
        assert_eq!(cfg.edges[2].len(), 2);

        let ast_err = CodeBlock::parse(
            r#"
            <start>
                local v1:8 = 0;
            <loop>
                v1 = v1 + 1;
                if v1 < 10 goto <loop1>;
            <done>
            "#,
        )?;

        assert!(
            matches!(
                CFG::new(&ast_err),
                Err(AstError::UndefinedLabel(label)) if label == "loop1",
            ),
            "invalid label",
        );

        Ok(())
    }
}
