function folderName(path: string): string {
  const normalized = path.replace(/\/$/, '')
  if (normalized === '~/repos' || normalized.endsWith('/repos')) return '_'
  return normalized.split(/[/\\]/).pop() || path
}

export interface Repo {
  id: string
  path: string
  name?: string
}

export interface TaskConfigSelectorProps {
  repos: Repo[]
  models: string[]
  repoPath: string
  translatorModel: string
  workloadModel: string
  cursorChatId?: string | null
  onRepoPathChange?: (path: string) => void
  onTranslatorModelChange?: (model: string) => void
  onWorkloadModelChange?: (model: string) => void
  readOnly?: boolean
  showRepoHint?: boolean
}

const ROOT_REPO = '~/repos'

export function TaskConfigSelector({
  repos,
  models,
  repoPath,
  translatorModel,
  workloadModel,
  cursorChatId,
  onRepoPathChange,
  onTranslatorModelChange,
  onWorkloadModelChange,
  readOnly = false,
  showRepoHint = false,
}: TaskConfigSelectorProps) {
  const modelOptions = models.length ? models : ['composer-1.5']

  if (readOnly) {
    return (
      <div className="space-y-2.5">
        <div className="grid grid-cols-1 gap-2 text-sm sm:grid-cols-3">
          <div>
            <span className="field-label">Repo</span>
            <p className="panel-muted mt-1 py-1.5">
              {folderName(repoPath || ROOT_REPO)}
            </p>
          </div>
          <div>
            <span className="field-label">Translator</span>
            <p className="panel-muted mt-1 py-1.5">
              {translatorModel || '—'}
            </p>
          </div>
          <div>
            <span className="field-label">Workload</span>
            <p className="panel-muted mt-1 py-1.5">
              {workloadModel || '—'}
            </p>
          </div>
        </div>
        {cursorChatId && (
          <p className="text-xs text-muted">
            Cursor chat ID:{' '}
            <code className="code-block inline-block py-0.5">
              {cursorChatId}
            </code>
          </p>
        )}
      </div>
    )
  }

  return (
    <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
      <div>
        <label htmlFor="task-repo" className="field-label">
          Repo
        </label>
        {showRepoHint && repos.length === 0 && (
          <p className="mb-1 text-xs warn-text">
            No repos. Run the executor on the dev machine to sync ~/repos/
          </p>
        )}
        <select
          id="task-repo"
          value={repoPath}
          onChange={(e) => onRepoPathChange?.(e.target.value)}
          className="select-control"
        >
          <option value={ROOT_REPO}>_</option>
          {repos.map((r) => (
            <option key={r.id} value={r.path}>
              {r.name || folderName(r.path)}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label htmlFor="task-translator" className="field-label">
          Translator model
        </label>
        <select
          id="task-translator"
          value={translatorModel}
          onChange={(e) => onTranslatorModelChange?.(e.target.value)}
          className="select-control"
        >
          {modelOptions.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label htmlFor="task-workload" className="field-label">
          Workload model
        </label>
        <select
          id="task-workload"
          value={workloadModel}
          onChange={(e) => onWorkloadModelChange?.(e.target.value)}
          className="select-control"
        >
          {modelOptions.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>
    </div>
  )
}
