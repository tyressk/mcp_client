use std::env;

use rig::{
    client::{CompletionClient, EmbeddingsClient},
    embeddings::EmbeddingsBuilder,
    providers::{openai},
    vector_store::in_memory_store::InMemoryVectorStore,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
pub mod chat;
pub mod config;
pub mod mcp_adaptor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "logs",
        format!("{}.log", env!("CARGO_CRATE_NAME")),
    );
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(file_appender)
        .with_file(false)
        .with_ansi(false)
        .init();

    let config = config::Config::retrieve("config.toml").await?;
    dotenvy::dotenv().ok();
    let secret = env::var("OPENAI_KEY").expect("OPENAI_KEY not found");
    let openai_client = openai::Client::new(&secret);
    let mcp_manager = config.mcp.create_manager().await?;
    tracing::info!(
        "MCP Manager created, {} servers started",
        mcp_manager.clients.len()
    );
    let tool_set = mcp_manager.get_tool_set().await?;
    let embedding_model =
        openai_client.embedding_model(openai::embedding::TEXT_EMBEDDING_3_SMALL);
    let embeddings = EmbeddingsBuilder::new(embedding_model.clone())
        .documents(tool_set.schemas()?)?
        .build()
        .await?;
    let store = InMemoryVectorStore::from_documents_with_id_f(embeddings, |f| {
       tracing::info!("store tool {}", f.name);
        f.name.clone()
    });
    let index = store.index(embedding_model);
    let openai = openai_client
        .agent(openai::GPT_4O)
        .dynamic_tools(4, index, tool_set)
        .build();

    chat::cli_chatbot(openai).await?;

    Ok(())
}