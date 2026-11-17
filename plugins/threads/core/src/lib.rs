//! wisp-threads-core (ADR-0079): what keeps a task's agent on track after hundreds of turns, compactions and
//! restarts, decided without IO.
//!
//! - `notes`: the sections of a task's Notes and how one is rewritten (a list section's items, one a line; the same
//!   item twice is one; caps), and the Notes as markdown.
//! - `note`: what goes in front of a follow-up: the Notes as they are now, and a nudge when your message changes
//!   something said earlier (supersession).
//! - `health`: how worn the thread is, from what wisp already has (no model call).
//! - `offload`: long Bash output into a file: the command's rewrite, and what the agent sees of it.

pub mod health;
pub mod note;
pub mod notes;
pub mod offload;
