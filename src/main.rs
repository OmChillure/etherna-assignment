use anyhow::Result;
use clap::{Parser, ValueEnum};
use matcher::{api, engine};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Role {
    Api,
    Matcher,
}

#[derive(Parser, Debug)]
#[command(name = "matcher")]
struct Args {
    #[arg(long, value_enum)]
    role: Role,

    #[arg(long, default_value = "0.0.0.0:8080")]
    bind: String,

    #[arg(long, default_value = "redis://127.0.0.1:6379", env = "REDIS_URL")]
    redis: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    match args.role {
        Role::Api => api::run(args.bind, args.redis).await,
        Role::Matcher => engine::run(args.redis).await,
    }
}
