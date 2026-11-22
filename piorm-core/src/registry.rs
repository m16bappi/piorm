use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use once_cell::sync::OnceCell;

use crate::config::DatabaseConfig;
use crate::error::PiormError;
use crate::pool::PiormPool;

static REGISTRY: OnceCell<Arc<RwLock<DatabaseRegistry>>> = OnceCell::new();

pub struct DatabaseRegistry {
    pub pools: HashMap<String, PiormPool>,
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

    pub fn get_pool(&self, alias: &str) -> crate::Result<&PiormPool> {
        self.pools
            .get(alias)
            .ok_or_else(|| PiormError::UnknownDatabase(alias.to_string()))
    }

    pub fn get_config(&self, alias: &str) -> crate::Result<&DatabaseConfig> {
        self.configs
            .get(alias)
            .ok_or_else(|| PiormError::UnknownDatabase(alias.to_string()))
    }
}

/// Initialize all database connections from a list of configs concurrently.
pub async fn initialize(configs: Vec<DatabaseConfig>) -> crate::Result<()> {
    // Connect all pools concurrently.
    let pool_futs: Vec<_> = configs.iter().map(|c| PiormPool::connect(c)).collect();
    let pools_vec = futures::future::try_join_all(pool_futs).await?;

    let global = DatabaseRegistry::global();
    let mut guard = global.write().unwrap();
    guard.pools.clear();
    guard.configs.clear();
    for (config, pool) in configs.into_iter().zip(pools_vec) {
        guard.pools.insert(config.alias.clone(), pool);
        guard.configs.insert(config.alias.clone(), config);
    }
    Ok(())
}
