//! Chat mode prompt & inline block parsing: chat mode drops the end-of-turn report while keeping wisp blocks
//! independently toggleable and splicing inline `wisp` blocks right between chat prose.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use wisp_runtime_core::blocks::parse_chat;
use wisp_runtime_core::prompt::system::{SystemOpts, system_prompt};

#[test]
fn chat_mode_prompt_drops_report_and_keeps_blocks_independent() {
    let with_blocks = system_prompt(&SystemOpts { chat_mode: true, blocks: true, ..SystemOpts::default() });
    assert!(with_blocks.contains("Chat mode"));
    assert!(with_blocks.contains("Your responses in Chat mode"));
    assert!(with_blocks.contains("### Blocks"));
    assert!(!with_blocks.contains("will NOT\nread your transcript"));
    assert!(!with_blocks.contains("Text outside the block is hidden"));

    let plain = system_prompt(&SystemOpts { chat_mode: true, blocks: false, ..SystemOpts::default() });
    assert!(plain.contains("Chat mode"));
    assert!(plain.contains("no `wisp` block"));
    assert!(!plain.contains("### Blocks"));
}

#[test]
fn parse_chat_preserves_prose_and_splices_inline_blocks_in_order() {
    let plain = parse_chat("All 12 tests pass after updating `login.ts`.");
    assert!(!plain.fallback);
    assert_eq!(plain.blocks.len(), 1);
    assert_eq!(plain.blocks[0]["type"], "markdown");
    assert_eq!(plain.blocks[0]["text"], "All 12 tests pass after updating `login.ts`.");

    let mixed = "I fixed the timeout in `login.ts`:\n\n```wisp\n{\"blocks\":[{\"type\":\"diff\",\"file\":\"src/login.ts\"}]}\n```\n\nTests are green now.";
    let parsed = parse_chat(mixed);
    assert!(!parsed.fallback);
    assert_eq!(parsed.blocks.len(), 3);
    assert_eq!(parsed.blocks[0]["type"], "markdown");
    assert_eq!(parsed.blocks[0]["text"], "I fixed the timeout in `login.ts`:");
    assert_eq!(parsed.blocks[1]["type"], "diff");
    assert_eq!(parsed.blocks[1]["file"], "src/login.ts");
    assert_eq!(parsed.blocks[2]["type"], "markdown");
    assert_eq!(parsed.blocks[2]["text"], "Tests are green now.");
}

#[test]
fn parse_chat_unclosed_wisp_fence_keeps_the_prose_and_the_next_fence() {
    // render audit doc28a: an unclosed ```wisp block took the next unrelated ``` as its close, dropping the prose
    let text =
        "```wisp\n{\"blocks\":[{\"type\":\"markdown\",\"text\":\"hi\"}]}\n\nProse that should show.\n\n```ts\nconst x = 1;\n```\nafter";
    let parsed = parse_chat(text);
    assert_eq!(parsed.blocks.len(), 2);
    assert_eq!(parsed.blocks[0]["text"], "hi");
    let rest = parsed.blocks[1]["text"].as_str().unwrap_or_default();
    assert!(rest.contains("Prose that should show."), "{rest}");
    assert!(rest.contains("```ts\nconst x = 1;\n```"), "{rest}");
    assert!(rest.ends_with("after"), "{rest}");

    // a closed block still drops its fence
    let closed = parse_chat("```wisp\n{\"blocks\":[{\"type\":\"markdown\",\"text\":\"hi\"}]}\n```\ntail");
    assert_eq!(closed.blocks.len(), 2);
    assert_eq!(closed.blocks[1]["text"], "tail");
}
