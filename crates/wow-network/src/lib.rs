//! Auth and worldserver runtime for a WoW 1.12.x Classic server emulator.
//!
//! This crate provides:
//! - [`AuthServer`]: SRP6 authentication / realm-list server (port 3724)
//! - [`WorldServer`]: world socket handling plus the current world simulation
//!   monolith (port 8085)

#[path = "auth/mod.rs"]
pub mod auth;
pub mod observability;
#[path = "world/mod.rs"]
pub mod world;

pub use auth::session::AuthSession;
pub use auth::AuthServer;
pub use world::WorldServer;
