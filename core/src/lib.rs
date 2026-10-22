#![forbid(unsafe_code)]

//! Core logic for Live Meeting Assistant.

pub mod assist;
pub mod capture;
pub mod llm;
pub mod screenshot;
pub mod settings;
pub mod stt;
pub mod summarize;
pub mod triggers;
pub mod vault;

#[cfg(test)]
mod test_http;
