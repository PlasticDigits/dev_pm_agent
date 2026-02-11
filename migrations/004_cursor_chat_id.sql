-- Migration 004: Cursor CLI chat session support
-- Store chat ID from `agent create-chat` for resumable sessions via `--resume [chatId]`

ALTER TABLE commands ADD COLUMN cursor_chat_id TEXT;
