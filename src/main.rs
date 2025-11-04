mod app;
mod config;
mod database;
mod error;
mod handlers;
mod models;
mod services;
mod startup;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::bootstrap().await
}
