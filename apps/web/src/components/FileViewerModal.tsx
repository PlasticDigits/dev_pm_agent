import { useCallback, useEffect, useState } from 'react'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism'
import { readFile, searchFiles, type FileSearchMatch } from '../api/files'
import { MarkdownOutput } from './MarkdownOutput'

const EXT_TO_LANG: Record<string, string> = {
  rs: 'rust',
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  mjs: 'javascript',
  cjs: 'javascript',
  json: 'json',
  sol: 'solidity',
  py: 'python',
  go: 'go',
  sh: 'bash',
  bash: 'bash',
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'toml',
  css: 'css',
  scss: 'scss',
  html: 'html',
  xml: 'xml',
  sql: 'sql',
  md: 'markdown',
}

function languageFromPath(path: string): string {
  const ext = path.split('.').pop()?.toLowerCase()
  return ext ? EXT_TO_LANG[ext] ?? 'text' : 'text'
}

function fileNameFromPath(path: string): string {
  const s = path.replace(/^\.\//, '').trim()
  const last = s.split(/[/\\]/).pop()
  return last || s
}

interface FileViewerModalProps {
  filePath: string
  repoPath: string
  token: string
  onClose: () => void
  /** When true, skip search and read file directly (e.g. when path comes from docs list). */
  pathKnown?: boolean
  /** Label for back/close when opened from docs (e.g. "Back to docs"). */
  backLabel?: string
}

type Phase = 'search' | 'picker' | 'reading' | 'content' | 'error'

export function FileViewerModal({ filePath, repoPath, token, onClose, pathKnown, backLabel }: FileViewerModalProps) {
  const [phase, setPhase] = useState<Phase>(pathKnown ? 'reading' : 'search')
  const [matches, setMatches] = useState<FileSearchMatch[]>([])
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [content, setContent] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const fileName = fileNameFromPath(filePath)

  const doRead = useCallback(
    (pathToRead: string) => {
      setPhase('reading')
      setError(null)
      setContent(null)
      readFile(token, repoPath, pathToRead)
        .then((r) => {
          setContent(r.content)
          setPhase('content')
        })
        .catch((e) => {
          setError(e instanceof Error ? e.message : 'Failed to read file')
          setPhase('error')
        })
    },
    [token, repoPath]
  )

  useEffect(() => {
    if (pathKnown) {
      doRead(filePath)
      return
    }
    let cancelled = false
    setPhase('search')
    setError(null)
    setMatches([])
    setSelectedPath(null)
    setContent(null)
    searchFiles(token, repoPath, fileName)
      .then((r) => {
        if (cancelled) return
        if (r.matches.length === 0) {
          setError(`No files named "${fileName}" found in repository`)
          setPhase('error')
        } else if (r.matches.length === 1) {
          doRead(r.matches[0].path)
        } else {
          setMatches(r.matches)
          setPhase('picker')
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : 'Search failed')
          setPhase('error')
        }
      })
    return () => {
      cancelled = true
    }
  }, [token, repoPath, fileName, doRead, pathKnown, filePath])

  const handleSelectMatch = (m: FileSearchMatch) => {
    setSelectedPath(m.path)
    doRead(m.path)
  }

  const displayPath = selectedPath ?? filePath
  const isMarkdown = /\.(md|markdown)$/i.test(displayPath)

  return (
    <div
      className="fixed inset-0 z-50 flex h-screen w-screen items-center justify-center bg-black/60"
      style={{ width: '100vw', height: '100vh' }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="file-viewer-title"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="flex h-full w-full flex-col border border-[var(--border)] bg-[var(--surface)]"
        style={{ maxWidth: '100vw', maxHeight: '100vh' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--border)] px-4 py-3">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            {(phase === 'content' && matches.length > 1) ? (
              <button
                type="button"
                onClick={() => {
                  setPhase('picker')
                  setContent(null)
                  setSelectedPath(null)
                }}
                className="shrink-0 rounded p-1.5 text-[var(--text-soft)] hover:bg-[var(--surface-muted)] hover:text-[var(--text)]"
                aria-label="Back to file list"
                title="Select a different file"
              >
                ←
              </button>
            ) : backLabel ? (
              <button
                type="button"
                onClick={onClose}
                className="shrink-0 rounded p-1.5 text-[var(--text-soft)] hover:bg-[var(--surface-muted)] hover:text-[var(--text)]"
                aria-label={backLabel}
                title={backLabel}
              >
                ← {backLabel}
              </button>
            ) : null}
            <h2 id="file-viewer-title" className="min-w-0 truncate font-medium text-[var(--text)]">
              {displayPath}
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="shrink-0 rounded p-1.5 text-[var(--text-soft)] hover:bg-[var(--surface-muted)] hover:text-[var(--text)]"
            aria-label="Close"
          >
            ×
          </button>
        </div>
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden p-4">
          {phase === 'search' && (
            <p className="text-[var(--text-muted)]">Searching for {fileName}…</p>
          )}
          {phase === 'picker' && (
            <div className="min-h-0 flex-1 overflow-auto">
              <div className="space-y-1">
              <p className="mb-3 text-sm text-[var(--text-muted)]">
                Multiple matches for {fileName}. Select one to open:
              </p>
              <ul className="space-y-1">
                {matches.map((m) => (
                  <li key={m.path}>
                    <button
                      type="button"
                      onClick={() => handleSelectMatch(m)}
                      className="w-full rounded px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)] focus:outline-none focus:ring-1 focus:ring-[var(--border)]"
                    >
                      <span className="font-mono text-[var(--text)]">{m.path}</span>
                      {m.modified_at && (
                        <span className="ml-2 text-xs text-[var(--text-muted)]">
                          {m.modified_at}
                        </span>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
              </div>
            </div>
          )}
          {phase === 'reading' && (
            <p className="text-[var(--text-muted)]">Loading…</p>
          )}
          {phase === 'error' && (
            <p className="text-[var(--danger)]">{error}</p>
          )}
          {phase === 'content' && content !== null && (
            isMarkdown ? (
              <div className="flex min-h-0 flex-1 flex-col overflow-auto">
                <MarkdownOutput content={content} scrollable={true} fillHeight />
              </div>
            ) : (
              <div className="min-h-0 flex-1 overflow-auto text-sm">
                <SyntaxHighlighter
                  language={languageFromPath(displayPath)}
                  style={oneDark}
                  wrapLongLines
                  customStyle={{
                    margin: 0,
                    padding: '0.75rem 1rem',
                    background: 'var(--surface-muted)',
                    fontSize: 'inherit',
                    borderRadius: '0.375rem',
                  }}
                  codeTagProps={{ style: { fontFamily: 'ui-monospace, monospace' } }}
                >
                  {content}
                </SyntaxHighlighter>
              </div>
            )
          )}
        </div>
      </div>
    </div>
  )
}
