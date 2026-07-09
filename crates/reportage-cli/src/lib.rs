//! Library surface for `reportage-cli`, kept minimal.
//!
//! Only `docs` is exposed: `src/bin/gen_ai_reading_order.rs` (issue #142) needs the same
//! `DOCUMENTS` table the `reportage` binary uses for `reportage docs`, and a lib target is the
//! only way for another binary in this crate to reach it without duplicating the table.

pub mod docs;
