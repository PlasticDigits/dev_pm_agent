import { useState, useEffect, useCallback } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { listCommands } from '../api/commands'
import { listMdFiles, type FileSearchMatch } from '../api/files'
import { useAuth } from '../contexts/AuthContext'
import { FileViewerModal } from '../components/FileViewerModal'
import type { Command } from '../types'

function folderName(path: string): string {
  const normalized = path.replace(/\/$/, '')
  if (normalized === '~/repos' || normalized.endsWith('/repos')) return '_'
  return normalized.split(/[/\\]/).pop() || path
}

interface FolderNode {
  name: string
  path: string
  files: FileSearchMatch[]
  children: Map<string, FolderNode>
}

/** Build nested folder tree. Root (.) stays flat; other dirs nest by path (e.g. packages/foo/plans). */
function buildFolderTree(matches: FileSearchMatch[]): { rootFiles: FileSearchMatch[]; tree: FolderNode[] } {
  const byDir = new Map<string, FileSearchMatch[]>()
  for (const m of matches) {
    const parts = m.path.split('/')
    const dir = parts.length > 1 ? parts.slice(0, -1).join('/') : '.'
    const list = byDir.get(dir) ?? []
    list.push(m)
    byDir.set(dir, list)
  }
  for (const list of byDir.values()) {
    list.sort((a, b) => (b.modified_at || '').localeCompare(a.modified_at || ''))
  }
  const rootFiles = byDir.get('.') ?? []

  const root = new Map<string, FolderNode>()
  for (const [dir, files] of byDir.entries()) {
    if (dir === '.') continue
    const parts = dir.split('/')
    let current = root
    let pathSoFar = ''
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      const isLeaf = i === parts.length - 1
      pathSoFar = pathSoFar ? `${pathSoFar}/${part}` : part
      if (!current.has(part)) {
        current.set(part, { name: part, path: pathSoFar, files: isLeaf ? files : [], children: new Map() })
      }
      const node = current.get(part)!
      if (isLeaf) {
        node.files = files
      }
      current = node.children
    }
  }

  const tree: FolderNode[] = [...root.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([, n]) => n)
  return { rootFiles, tree }
}

function countFilesInNode(node: FolderNode): number {
  return node.files.length + [...node.children.values()].reduce((s, c) => s + countFilesInNode(c), 0)
}

function FolderSection({
  node,
  onFileClick,
}: {
  node: FolderNode
  onFileClick: (path: string) => void
}) {
  const childList = [...node.children.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([, n]) => n)
  const hasContent = node.files.length > 0 || childList.length > 0

  return (
    <details className="group">
      <summary className="flex cursor-pointer list-none items-center gap-1 rounded px-1 py-0 text-sm font-semibold uppercase tracking-wide text-[var(--text-soft)] hover:bg-[var(--surface-muted)] [&::-webkit-details-marker]:hidden">
        <span className="transition-transform group-open:rotate-90" aria-hidden>▶</span>
        {node.name}
        <span className="text-xs font-normal normal-case">({countFilesInNode(node)})</span>
      </summary>
      {hasContent && (
        <div className="ml-1.5 mt-0 border-l border-[var(--border)] pl-1">
          {node.files.map((m) => (
            <button
              key={m.path}
              type="button"
              onClick={() => onFileClick(m.path)}
              className="block w-full rounded px-1 py-0 text-left text-sm hover:bg-[var(--surface-muted)] focus:outline-none focus:ring-1 focus:ring-[var(--border)]"
            >
              <span className="font-mono text-[var(--text)]">{m.path.split('/').pop()}</span>
              {m.modified_at && (
                <span className="ml-1.5 text-xs text-[var(--text-muted)]">{m.modified_at}</span>
              )}
            </button>
          ))}
          {childList.map((child) => (
            <FolderSection key={child.path} node={child} onFileClick={onFileClick} />
          ))}
        </div>
      )}
    </details>
  )
}

export default function ChatDocs() {
  const { chatId } = useParams<{ chatId: string }>()
  const { token } = useAuth()
  const navigate = useNavigate()
  const [commands, setCommands] = useState<Command[]>([])
  const [matches, setMatches] = useState<FileSearchMatch[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [viewingFile, setViewingFile] = useState<string | null>(null)
  const [repoPath, setRepoPath] = useState<string | null>(null)

  const byCursorChat = commands.filter((c) => c.cursor_chat_id === chatId)
  const byId = commands.filter((c) => c.id === chatId)
  const root =
    byCursorChat[0] ?? byId[0]

  const refreshCommands = useCallback(async () => {
    if (!token) return
    try {
      const list = await listCommands(token)
      setCommands(list)
    } catch (_) {}
  }, [token])

  useEffect(() => {
    if (!token) {
      navigate('/login')
      return
    }
    refreshCommands()
  }, [token, navigate, refreshCommands])

  useEffect(() => {
    if (!token || !root?.repo_path) {
      setRepoPath(null)
      setMatches([])
      setLoading(false)
      return
    }
    const rp = root.repo_path
    setRepoPath(rp)
    setLoading(true)
    setError('')
    listMdFiles(token, rp)
      .then((r) => setMatches(r.matches))
      .catch((e) => setError(e instanceof Error ? e.message : 'Failed to list docs'))
      .finally(() => setLoading(false))
  }, [token, root?.repo_path])

  if (!token) return null

  const { rootFiles, tree } = buildFolderTree(matches)
  const hasContent = rootFiles.length > 0 || tree.length > 0
  const handleFileClick = useCallback((path: string) => setViewingFile(path), [])

  return (
    <div className="chat-app">
      {viewingFile && repoPath && token && (
        <FileViewerModal
          filePath={viewingFile}
          repoPath={repoPath}
          token={token}
          onClose={() => setViewingFile(null)}
          pathKnown
          backLabel="Docs"
        />
      )}
      <header className="chat-header">
        <button
          type="button"
          onClick={() => navigate(`/chat/${chatId}`)}
          className="chat-back"
          aria-label="Back to chat"
        >
          ←
        </button>
        <div className="chat-header-info">
          <h1 className="chat-title">Docs</h1>
          <p className="chat-meta">
            {root
              ? `${folderName(root.repo_path || '~/repos')} • ${matches.length} .md files`
              : 'Loading…'}
          </p>
          {error && <p className="text-sm text-red-600">{error}</p>}
        </div>
      </header>

      <main className="chat-scroll">
        {!root?.repo_path ? (
          <p className="chat-empty">No repo context for this chat.</p>
        ) : loading ? (
          <p className="chat-empty">Loading docs…</p>
        ) : !hasContent ? (
          <p className="chat-empty">No .md files found in this repo.</p>
        ) : (
          <div className="space-y-px">
            {rootFiles.length > 0 && (
              <details className="group">
                <summary className="flex cursor-pointer list-none items-center gap-1 rounded px-1 py-0 text-sm font-semibold uppercase tracking-wide text-[var(--text-soft)] hover:bg-[var(--surface-muted)] [&::-webkit-details-marker]:hidden">
                  <span className="transition-transform group-open:rotate-90" aria-hidden>▶</span>
                  (root)
                  <span className="text-xs font-normal normal-case">({rootFiles.length})</span>
                </summary>
                <div className="ml-1.5 mt-0 border-l border-[var(--border)] pl-1">
                  {rootFiles.map((m) => (
                    <button
                      key={m.path}
                      type="button"
                      onClick={() => handleFileClick(m.path)}
                      className="block w-full rounded px-1 py-0 text-left text-sm hover:bg-[var(--surface-muted)] focus:outline-none focus:ring-1 focus:ring-[var(--border)]"
                    >
                      <span className="font-mono text-[var(--text)]">{m.path}</span>
                      {m.modified_at && (
                        <span className="ml-1.5 text-xs text-[var(--text-muted)]">{m.modified_at}</span>
                      )}
                    </button>
                  ))}
                </div>
              </details>
            )}
            {tree.map((node) => (
              <FolderSection key={node.path} node={node} onFileClick={handleFileClick} />
            ))}
          </div>
        )}
      </main>
    </div>
  )
}
