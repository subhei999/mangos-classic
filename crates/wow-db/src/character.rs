use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::{FromRow, MySql, Row, Transaction};
use wow_common::position::WorldPosition;

use crate::pool::DbError;

include!("character/types.rs");
include!("character/queries.rs");
include!("character/lifecycle.rs");
include!("character/creation.rs");
include!("character/state.rs");
include!("character/inventory.rs");
include!("character/auction.rs");
include!("character/mail.rs");
include!("character/progression.rs");
include!("character/starter.rs");
include!("character/tests.rs");
