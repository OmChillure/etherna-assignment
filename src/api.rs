use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::broadcast;

use crate::redis_io::{Redis, CHANNEL_FILLS};
use crate::types::{NewOrder, Order};

#[derive(Clone)]
struct AppState {
    redis: Redis,
    fills_tx: broadcast::Sender<String>,
}

pub async fn run(bind: String, redis_url: String) -> Result<()> {
    let redis = Redis::connect(&redis_url).await?;
    let (fills_tx, _) = broadcast::channel::<String>(1024);

    tokio::spawn(forward_fills(redis.url().to_string(), fills_tx.clone()));

    let state = AppState { redis, fills_tx };

    let app = Router::new()
        .route("/orders", post(post_order))
        .route("/orderbook", get(get_orderbook))
        .route("/ws", get(handle_ws))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn post_order(
    State(s): State<AppState>,
    Json(req): Json<NewOrder>,
) -> Result<Json<Value>, ApiError> {
    if req.qty == 0 || req.price == 0 {
        return Err(ApiError::BadRequest("price and qty must be > 0"));
    }
    let id = s.redis.next_order_id().await?;
    let order = Order {
        id,
        side: req.side,
        price: req.price,
        qty: req.qty,
    };
    s.redis.xadd_order(&order).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn get_orderbook(State(s): State<AppState>) -> Result<Response, ApiError> {
    let body = s.redis.get_snapshot().await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "application/json")], body).into_response())
}

async fn handle_ws(State(s): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| serve_client(socket, s.fills_tx.subscribe()))
}

async fn serve_client(socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    let (mut sink, mut stream) = socket.split();

    let send = async {
        while let Ok(msg) = rx.recv().await {
            if sink.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    };

    let recv = async {
        while let Some(Ok(msg)) = stream.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    };

    tokio::select! {
        _ = send => {},
        _ = recv => {},
    }
}

async fn forward_fills(redis_url: String, tx: broadcast::Sender<String>) {
    loop {
        if let Err(e) = forward_fills_attempt(&redis_url, &tx).await {
            tracing::warn!(error = %e, "fills forwarder error, reconnecting");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}

async fn forward_fills_attempt(redis_url: &str, tx: &broadcast::Sender<String>) -> Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe(CHANNEL_FILLS).await?;
    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let payload: String = msg.get_payload()?;
        let _ = tx.send(payload);
    }
    Ok(())
}

#[derive(Debug)]
enum ApiError {
    BadRequest(&'static str),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use axum::http::StatusCode;
        match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            ApiError::Internal(e) => {
                tracing::error!(error = %e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
}
