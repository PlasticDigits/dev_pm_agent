/**
 * Hardcoded command presets (PLAN §8).
 * Templates influence translation/execution behavior via context_mode.
 */

export interface Template {
  id: string
  label: string
  contextMode: string | null
  description?: string
}

/** Default: no template, free-form user intent. */
export const FREEFORM: Template = {
  id: 'freeform',
  label: 'Freeform',
  contextMode: null,
  description: 'Natural language task, no preset',
}

/** Plan flows (create → interactive review/update). */
export const PLAN_TEMPLATES: Template[] = [
  {
    id: 'monorepo_init',
    label: 'Monorepo init',
    contextMode: 'monorepo_init',
    description: 'Create plans/ folder and PLAN_INITIAL.md, then interactive review',
  },
  {
    id: 'gap_analysis',
    label: 'Gap analysis',
    contextMode: 'gap_analysis',
    description: 'Write PLAN_GAP_{x}.md, interactive review',
  },
  {
    id: 'security_review',
    label: 'Security review',
    contextMode: 'security_review',
    description: 'Write PLAN_SECURITY_{x}.md, interactive review',
  },
  {
    id: 'feature_plan',
    label: 'Feature plan',
    contextMode: 'feature_plan',
    description: 'Write PLAN_FEAT_{x}.md, interactive review',
  },
]

/** Sprints (implementation). */
export const SPRINT_TEMPLATES: Template[] = [
  {
    id: 'sprint',
    label: 'Sprint',
    contextMode: 'sprint',
    description: 'Select plan, create/continue sprint doc, implement',
  },
]

/** Commit (stage and commit changes from conversation). */
export const COMMIT_TEMPLATE: Template = {
  id: 'commit',
  label: 'Commit',
  contextMode: 'commit',
  description: 'Stage and commit changes from the conversation; pre-commit may run long. NEVER use --no-verify.',
}

/** All templates for discovery (freeform first, then plan flows, sprints, commit). */
export const TEMPLATES: Template[] = [
  FREEFORM,
  ...PLAN_TEMPLATES,
  ...SPRINT_TEMPLATES,
  COMMIT_TEMPLATE,
]

export function getTemplateById(id: string): Template | undefined {
  return TEMPLATES.find((t) => t.id === id)
}

export function getDefaultTemplate(): Template {
  return FREEFORM
}
