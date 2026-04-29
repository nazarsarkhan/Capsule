use anyhow::Result;

use crate::core::detector::Project;

pub mod node;
pub mod python;
pub mod typescript;

pub trait Adapter {
    fn scan(&self, project: &Project) -> Result<Vec<String>>;
    fn lock(&self, project: &Project, yes: bool, no_install: bool, verbose: bool) -> Result<()>;
    fn run(&self, project: &Project, yes: bool, no_install: bool, verbose: bool) -> Result<()>;
}
