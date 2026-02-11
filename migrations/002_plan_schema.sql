-- Migration 002: Align schema with PLAN.md (device registration, command models)
-- Prereq: 001_initial.sql applied
-- Note: admin.mfa_enabled and admin.executor_registered are deprecated; app ignores them.

-- Replace pairing_codes with device_registration_codes
DROP TABLE IF EXISTS pairing_codes;

CREATE TABLE IF NOT EXISTS device_registration_codes (
  id                  TEXT PRIMARY KEY,
  code                TEXT UNIQUE NOT NULL,
  created_by_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  used                INTEGER DEFAULT 0,
  expires_at          TEXT NOT NULL,
  created_at          TEXT NOT NULL
);

-- Add model columns to commands (run once; migration runner must track applied migrations)
ALTER TABLE commands ADD COLUMN translator_model TEXT;
ALTER TABLE commands ADD COLUMN workload_model TEXT;
