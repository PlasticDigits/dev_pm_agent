import ReactMarkdown from 'react-markdown'
import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react'
import remarkGfm from 'remark-gfm'

/** Heuristic: does this look like a file path (e.g. README.md, plans/PLAN_GAP_1.md)? */
function looksLikeFilePath(text: string): boolean {
  const t = text.trim()
  if (t.length < 3 || t.length > 120) return false
  if (/^[#$]/.test(t)) return false // shell/cmd
  if (t.includes('(') || t.includes(')')) return false // fn call
  if (t.includes(' ')) return false
  if (t.includes('/')) return true // path with segment
  return /\.(md|mdx|txt|json|ts|tsx|js|jsx|py|rs|toml|yaml|yml|sql)$/i.test(t)
}

/** Is this link href a file path (not http/mailto/etc)? */
function isFileHref(href: string): boolean {
  if (!href || typeof href !== 'string') return false
  const h = href.trim()
  if (h.startsWith('http:') || h.startsWith('https:') || h.startsWith('mailto:') || h.startsWith('#')) return false
  return h.length > 0 && h.length < 200 && !h.includes(' ')
}

interface MarkdownOutputProps {
  content: string
  failed?: boolean
  className?: string
  /** When false, no internal scroll; parent handles overflow. Default true. */
  scrollable?: boolean
  /** When true with scrollable, use full available height (e.g. in modal) instead of max-h-[52vh]. */
  fillHeight?: boolean
  /** When set with onFileClick, file paths become clickable to open in modal. */
  repoPath?: string | null
  onFileClick?: (filePath: string) => void
}

function nodeText(node: React.ReactNode): string {
  if (typeof node === 'string') return node
  if (typeof node === 'number') return String(node)
  if (!node) return ''
  if (Array.isArray(node)) return node.map(nodeText).join('')
  if (typeof node === 'object' && 'props' in node) {
    const maybeProps = (node as { props?: { children?: React.ReactNode } }).props
    return nodeText(maybeProps?.children)
  }
  return ''
}

function isLikelyAsciiDiagram(text: string): boolean {
  const lines = text
    .split('\n')
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0)
  if (lines.length < 3) return false

  const maxLen = lines.reduce((max, line) => Math.max(max, line.length), 0)
  if (maxLen < 26) return false

  const sample = lines.slice(0, 16).join('\n')
  const frameChars = (
    sample.match(/[|+\-_=<>[\]()\\/┌┐└┘├┤┬┴┼│─━┃┏┓┗┛╋╔╗╚╝║═╬]/g) || []
  ).length
  const alphaChars = (sample.match(/[a-zA-Z]/g) || []).length
  const connectorHits = (
    sample.match(/->|<-|=>|<=|→|←|⇒|⇐|↔|↕|►|◄|▶|◀|⟶|⟵|⟹|⟸|➜|➝|➤|➔/g) || []
  ).length

  return frameChars >= 8 && (connectorHits > 0 || frameChars > alphaChars * 0.42)
}

function AutoFitAsciiPre({ text }: { text: string }) {
  const BASE_REM = 0.81
  const [mode, setMode] = useState<'full' | 'original'>('full')
  const containerRef = useRef<HTMLPreElement | null>(null)
  const measureRef = useRef<HTMLSpanElement | null>(null)
  const naturalWidthRef = useRef(0)
  const [scale, setScale] = useState(1)

  // Shared recalc: measure natural width from hidden span, then compute scale.
  // Called from useLayoutEffect (before paint) so the user never sees the wrong size.
  const recalc = useCallback(() => {
    const container = containerRef.current
    const measure = measureRef.current
    if (!container || !measure) return

    // Update hidden measurement element to match current text
    measure.textContent = text
    measure.style.fontSize = `${BASE_REM}rem`
    naturalWidthRef.current = measure.scrollWidth

    if (mode !== 'full') {
      setScale(1)
      return
    }
    const containerWidth = container.clientWidth
    const naturalWidth = naturalWidthRef.current
    if (containerWidth <= 0 || naturalWidth <= 0) return

    const next = Math.min(1, Math.max(0.28, containerWidth / naturalWidth))
    setScale((current) => (Math.abs(next - current) > 0.01 ? next : current))
  }, [text, mode])

  // Run before paint whenever text or mode changes — no visible flash.
  useLayoutEffect(() => {
    recalc()
  }, [recalc])

  // Debounced window resize listener
  useLayoutEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null
    const onResize = () => {
      if (timer) clearTimeout(timer)
      timer = setTimeout(recalc, 150)
    }
    window.addEventListener('resize', onResize)
    window.addEventListener('orientationchange', recalc)

    return () => {
      if (timer) clearTimeout(timer)
      window.removeEventListener('resize', onResize)
      window.removeEventListener('orientationchange', recalc)
    }
  }, [recalc])

  const size = useMemo(
    () => `${BASE_REM * (mode === 'full' ? scale : 1)}rem`,
    [mode, scale]
  )

  return (
    <div className="ascii-diagram-wrap">
      <span
        ref={measureRef}
        aria-hidden="true"
        className="ascii-measure"
        style={{ fontSize: `${BASE_REM}rem` }}
      />
      <div className="ascii-toggle">
        <button
          type="button"
          className={`ascii-toggle-btn ${mode === 'original' ? 'is-active' : ''}`}
          onClick={() => setMode('original')}
          aria-label="Show original size with horizontal scroll"
        >
          100%
        </button>
        <span className="ascii-toggle-sep">|</span>
        <button
          type="button"
          className={`ascii-toggle-btn ${mode === 'full' ? 'is-active' : ''}`}
          onClick={() => setMode('full')}
          aria-label="Auto scale diagram to fit width"
        >
          Full
        </button>
      </div>
      <pre
        ref={containerRef}
        className={`md-pre md-pre-ascii ${mode === 'full' ? 'md-pre-ascii-fit' : 'md-pre-ascii-original'}`}
      >
      <code className="md-code md-code-ascii" style={{ fontSize: size }}>
        {text}
      </code>
      </pre>
    </div>
  )
}

function TableWithMode({ children }: { children?: React.ReactNode }) {
  const [mode, setMode] = useState<'full' | 'original'>('full')

  return (
    <div className="md-table-wrap">
      <div className="ascii-toggle">
        <button
          type="button"
          className={`ascii-toggle-btn ${mode === 'original' ? 'is-active' : ''}`}
          onClick={() => setMode('original')}
          aria-label="Show original table size with horizontal scroll"
        >
          100%
        </button>
        <span className="ascii-toggle-sep">|</span>
        <button
          type="button"
          className={`ascii-toggle-btn ${mode === 'full' ? 'is-active' : ''}`}
          onClick={() => setMode('full')}
          aria-label="Fill table to available width"
        >
          Full
        </button>
      </div>
      <div className={`md-table-scroll ${mode === 'full' ? 'md-table-scroll-full' : 'md-table-scroll-original'}`}>
        <table className={`md-table ${mode === 'full' ? 'md-table-full' : 'md-table-original'}`}>{children}</table>
      </div>
    </div>
  )
}

function buildMarkdownComponents(
  repoPath: string | null | undefined,
  onFileClick: ((filePath: string) => void) | undefined
) {
  const canClickFile = repoPath && onFileClick
  const handleFile = (filePath: string) => {
    const normalized = filePath.replace(/^\.\//, '').trim()
    if (normalized && canClickFile) onFileClick(normalized)
  }

  return {
    p: ({ children }: { children?: React.ReactNode }) => <p className="my-1.5 leading-6">{children}</p>,
    ul: ({ children }: { children?: React.ReactNode }) => <ul className="list-inside list-disc">{children}</ul>,
    ol: ({ children }: { children?: React.ReactNode }) => <ol className="list-inside list-decimal">{children}</ol>,
    li: ({ children }: { children?: React.ReactNode }) => <li>{children}</li>,
    strong: ({ children }: { children?: React.ReactNode }) => <strong className="font-bold">{children}</strong>,
    a: ({
      href,
      children,
      ...props
    }: {
      href?: string
      children?: React.ReactNode
    }) => {
      if (canClickFile && href && isFileHref(href)) {
        return (
          <button
            type="button"
            className="cursor-pointer border-none bg-transparent p-0 font-inherit text-inherit underline text-[var(--link-text)] hover:text-[var(--link-text-hover)]"
            onClick={(e) => {
              e.preventDefault()
              handleFile(href)
            }}
          >
            {children}
          </button>
        )
      }
      return (
        <a href={href} className="text-[var(--link-text)] hover:text-[var(--link-text-hover)]" {...props}>
          {children}
        </a>
      )
    },
    code: ({
      className,
      children,
      ...props
    }: {
      className?: string
      children?: React.ReactNode
    }) => {
      const raw = nodeText(children)
      if (canClickFile && !className && looksLikeFilePath(raw)) {
        return (
          <button
            type="button"
            className="md-inline-code cursor-pointer border-none bg-transparent font-mono text-inherit underline decoration-[var(--link-text)] hover:decoration-[var(--link-text-hover)]"
            onClick={() => handleFile(raw)}
          >
            {children}
          </button>
        )
      }
      return className ? (
        <code className={`md-code ${className}`} {...props}>
          {children}
        </code>
      ) : (
        <code className="md-inline-code" {...props}>
          {children}
        </code>
      )
    },
    pre: ({ children }: { children?: React.ReactNode }) => {
      const raw = nodeText(children)
      if (isLikelyAsciiDiagram(raw)) {
        return <AutoFitAsciiPre text={raw} />
      }
      return <pre className="md-pre">{children}</pre>
    },
    table: ({ children }: { children?: React.ReactNode }) => <TableWithMode>{children}</TableWithMode>,
  }
}

export function MarkdownOutput({
  content,
  failed = false,
  className = '',
  scrollable = true,
  fillHeight = false,
  repoPath,
  onFileClick,
}: MarkdownOutputProps) {
  const components = useMemo(
    () => buildMarkdownComponents(repoPath, onFileClick),
    [repoPath, onFileClick]
  )
  const scrollClass = scrollable
    ? fillHeight
      ? 'h-full min-h-0 overflow-auto'
      : 'max-h-[52vh] overflow-auto'
    : ''
  return (
    <div
      className={`prose prose-invert prose-sm max-w-none markdown-output ${scrollClass} ${
        failed ? '[&_*]:!text-[var(--danger)] !text-[var(--danger)]' : '!text-[var(--text)]'
      } ${className}`}
    >
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {content}
      </ReactMarkdown>
    </div>
  )
}
