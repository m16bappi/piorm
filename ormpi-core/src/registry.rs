use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use once_cell::sync::OnceCell;

use crate::config::DatabaseConfig;
use crate::error::OrmpiError;
use crate::pool::OrmpiPool;

static REGISTRY: OnceCell<Arc<RwLock<DatabaseRegistry>>> = OnceCell::new();

pub struct DatabaseRegistry {
    pub pools: HashMap<String, OrmpiPool>,
    pub configs: HashMap<String, DatabaseConfig>,
}

impl DatabaseRegistry {
    pub fn global() -> Arc<RwLock<DatabaseRegistry>> {
        REGISTRY
            .get_or_init(|| {
                Arc::new(RwLock::new(DatabaseRegistry {
                    pools: HashMap::new(),
                    configs: HashMap::new(),
                }))
            })
            .clone()
    }

    pub fn get_pool(&self, alias: &str) -> crate::Result<&OrmpiPool> {
        self.pools
            .get(alias)
            .ok_or_else(|| OrmpiError::UnknownDatabase(alias.to_string()))
    }

    pub fn get_config(&self, alias: &str) -> crate::Result<&DatabaseConfig> {
        self.configs
            .get(alias)
            .ok_or_else(|| OrmpiError::UnknownDatabase(alias.to_string()))
    }
}

/// Initialize all database connections from a list of configs, replacing any previous registry.
pub async fn initialize(configs: Vec<DatabaseConfig>) -> crate::Result<()> {
    let mut pools = HashMap::new();
    let mut cfgs = HashMap::new();

    for config in configs {
        let pool = OrmpiPool::connect(&config).await?;
        pools.insert(config.alias.clone(), pool);
        cfgs.insert(config.alias.clone(), config);
    }

    let global = DatabaseRegistry::global();
    let mut guard = global.write().unwrap();
    guard.pools = pools;
    guard.configs = cfgs;
    Ok(())
}
