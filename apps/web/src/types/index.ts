/**
 * Shared types for web app — align with PLAN.md API design
 */

export type CommandStatus = 'pending' | 'running' | 'done' | 'failed' | 'cancelled';

export interface Command {
  id: string;
  device_id: string;
  input: string;
  status: CommandStatus;
  output?: string;
  summary?: string;
  repo_path?: string;
  context_mode?: string;
  translator_model?: string;
  workload_model?: string;
  cursor_chat_id?: string;
  created_at: string;
  updated_at: string;
}
