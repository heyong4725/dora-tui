#![allow(dead_code)]

use std::{collections::HashMap, path::PathBuf};

use super::app::UserConfig;

#[derive(Debug, Clone)]
pub struct CliContext {
    pub working_directory: PathBuf,
    pub environment: HashMap<String, String>,
    pub user_config: UserConfig,
    pub cli_args: Vec<String>,
}

impl CliContext {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            environment: std::env::vars().collect(),
            user_config: UserConfig::default(),
            cli_args: std::env::args().collect(),
        }
    }
}

impl Default for CliContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub enum CommandMode {
    #[default]
    Normal,
}
