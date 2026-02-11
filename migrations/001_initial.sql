-- Admin account (single row; single-user design)
CREATE TABLE IF NOT EXISTS admin (
  id              TEXT PRIMARY KEY,
  username        TEXT UNIQUE NOT NULL,
  password_hash    TEXT NOT NULL,
  totp_secret     TEXT NOT NULL,
  mfa_enabled     INTEGER DEFAULT 0,
  executor_registered INTEGER DEFAULT 0,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- Devices: executor (1) + controllers (N)
CREATE TABLE IF NOT EXISTS devices (
  id              TEXT PRIMARY KEY,
  admin_id        TEXT NOT NULL REFERENCES admin(id) ON DELETE CASCADE,
  device_id       TEXT NOT NULL,
  name            TEXT,
  role            TEXT NOT NULL CHECK (role IN ('executor', 'controller')),
  token_hash      TEXT,
  registered_at   TEXT NOT NULL,
  last_seen_at    TEXT NOT NULL,
  UNIQUE(admin_id, device_id)
);

-- Only one executor per admin
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_executor ON devices(admin_id) WHERE role = 'executor';

-- Pairing codes (short-lived, for controller registration)
CREATE TABLE IF NOT EXISTS pairing_codes (
  id              TEXT PRIMARY KEY,
  code            TEXT UNIQUE NOT NULL,
  executor_id     TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  used            INTEGER DEFAULT 0,
  expires_at      TEXT NOT NULL,
  created_at      TEXT NOT NULL
);

-- Commands (relay messages)
CREATE TABLE IF NOT EXISTS commands (
  id              TEXT PRIMARY KEY,
  device_id       TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  input           TEXT NOT NULL,
  status          TEXT NOT NULL DEFAULT 'pending',
  output          TEXT,
  summary         TEXT,
  repo_path       TEXT,
  context_mode    TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- Known repos (optional; for repo selector in UI)
CREATE TABLE IF NOT EXISTS repos (
  id              TEXT PRIMARY KEY,
  admin_id        TEXT NOT NULL REFERENCES admin(id) ON DELETE CASCADE,
  path            TEXT NOT NULL,
  name            TEXT,
  created_at      TEXT NOT NULL
);
