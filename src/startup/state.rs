use crate::{
    config::Config,
    database::Database,
    services::{cloud_sync::CloudSync, edge_processor::EdgeProcessor},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub edge_processor: Arc<EdgeProcessor>,
    pub cloud_sync: Arc<CloudSync>,
    pub config: Arc<Config>,
}
