-- Migration 003: Bootstrap devices (pre-admin device registration for first-run setup)
-- Prereq: 001, 002 applied
-- Used when no admin exists: device key is generated via CLI, registered here,
-- then claimed during web setup when user creates account.

CREATE TABLE IF NOT EXISTS bootstrap_devices (
  token_hash    TEXT PRIMARY KEY,
  created_at   TEXT NOT NULL
);
