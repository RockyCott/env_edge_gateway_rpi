use crate::{
    config::Config,
    database::Database,
    services::{cloud_sync::CloudSync, edge_processor::EdgeProcessor},
};
use std::sync::Arc;
use tokio::{sync::Mutex};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub edge_processor: Arc<EdgeProcessor>,
    pub cloud_sync: Arc<Mutex<CloudSync>>,
    pub config: Arc<Config>,
}
