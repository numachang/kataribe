//! `OpenAI` 互換 Chat Completions API（LM Studio・Ollama・`KoboldCpp`・llama.cpp server・
//! `OpenRouter`・`OpenAI` など）へのストリーミングクライアント。
//!
//! 主な利用先は LM Studio。[`OpenAiCompatClient`] が HTTP 通信・再試行・SSE の解析・
//! 推論（`<think>` タグ）の分離を担う。[`ClaudeCodeModel`] は、API キーの代わりに
//! Claude Code（`claude -p`）で生成する。どちらも [`ChatModel`] トレイトを介して
//! `kataribe-engine` から使われる想定で、`testing` feature の
//! [`testing::ScriptedChatModel`] はその代わりに使えるテスト用の偽モデル。

mod chat_model;
mod chunk;
mod claude_code;
mod client;
mod collect;
mod config;
mod error;
mod event;
mod message;
mod model_info;
mod request;
mod sse;
mod think_filter;

#[cfg(feature = "testing")]
pub mod testing;

pub use chat_model::{ChatModel, ChatStream};
pub use claude_code::{ClaudeCodeConfig, ClaudeCodeModel};
pub use client::OpenAiCompatClient;
pub use collect::{Completion, collect};
pub use config::ClientConfig;
pub use error::LlmError;
pub use event::{ChatEvent, Finish, FinishReason, Usage};
pub use message::{Message, Role};
pub use model_info::ModelInfo;
pub use request::{ChatRequest, ResponseFormat, Sampling};
