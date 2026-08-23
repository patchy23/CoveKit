import { EditorView } from '@codemirror/view'

const shared = {
  '.cm-scroller': { overflow: 'auto' },
  '.cm-line': { padding: '0 4px 0 0' },
  '.cm-foldGutter': { minWidth: '14px' },
  '.cm-foldGutter .cm-gutterElement': {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    cursor: 'pointer',
  },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 6px 0 2px', minWidth: '26px' },
  '.cm-run-statement-gutter': { minWidth: '24px' },
  '.cm-run-statement-gutter .cm-gutterElement': {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    minWidth: '24px',
    padding: '0 2px',
  },
  '.cm-run-statement-btn': {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '20px',
    height: '20px',
    margin: '0',
    padding: '0',
    border: 'none',
    borderRadius: '4px',
    background: 'transparent',
    cursor: 'pointer',
    opacity: '0',
    transition: 'opacity 0.12s, background-color 0.12s',
  },
  '&.cm-editor:hover .cm-run-statement-btn': { opacity: '0.55' },
  '&.cm-editor:hover .cm-run-statement-btn:hover': { opacity: '1' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
}

export const lightTheme = EditorView.theme({
  ...shared,
  '&': {
    backgroundColor: 'var(--color-surface-muted)',
    color: 'var(--color-primary)',
    fontSize: '13px',
    height: '100%',
  },
  '.cm-content': {
    fontFamily: 'var(--font-mono)',
    caretColor: 'var(--color-tertiary)',
    padding: '11px 12px 11px 0',
    lineHeight: '1.65',
  },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    color: 'var(--color-text-muted)',
    border: 'none',
    paddingLeft: '8px',
  },
  '.cm-activeLine': { backgroundColor: 'var(--color-border)' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-tertiary-strong)' },
  '.cm-fold-marker': {
    display: 'inline-flex',
    color: 'var(--color-text-muted)',
    opacity: '0.55',
    transition: 'opacity 0.12s',
  },
  '.cm-fold-marker:hover': { opacity: '1', color: 'var(--color-secondary)' },
  '.cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft)' },
  '&.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft)' },
  '.cm-cursor': { borderLeftColor: 'var(--color-tertiary-strong)' },
  '.cm-tooltip': {
    backgroundColor: 'var(--color-surface)',
    border: '1px solid var(--color-border-strong)',
    color: 'var(--color-primary)',
    borderRadius: '6px',
    boxShadow: '0 4px 12px rgba(0,0,0,0.12)',
  },
  '.cm-run-statement-btn': {
    ...shared['.cm-run-statement-btn'],
    color: 'var(--color-success-strong)',
  },
  '.cm-run-statement-btn:hover': { background: 'var(--color-success-soft)', opacity: '1' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
  },
  '.cm-searchMatch': { backgroundColor: 'var(--color-tertiary-soft)' },
  '.cm-selectionMatch': { backgroundColor: 'var(--color-tertiary-soft)' },
})

export const darkTheme = EditorView.theme(
  {
    ...shared,
    '&': {
      backgroundColor: 'var(--color-surface-muted-dark)',
      color: 'var(--color-primary-dark)',
      fontSize: '13px',
      height: '100%',
    },
    '.cm-content': {
      fontFamily: 'var(--font-mono)',
      caretColor: 'var(--color-tertiary-dark)',
      padding: '11px 12px 11px 0',
      lineHeight: '1.65',
    },
    '.cm-gutters': {
      backgroundColor: 'transparent',
      color: 'var(--color-text-muted-dark)',
      border: 'none',
      paddingLeft: '8px',
    },
    '.cm-activeLine': { backgroundColor: 'var(--color-border-dark)' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-tertiary-dark)' },
    '.cm-fold-marker': {
      display: 'inline-flex',
      color: 'var(--color-text-muted-dark)',
      opacity: '0.55',
      transition: 'opacity 0.12s',
    },
    '.cm-fold-marker:hover': { opacity: '1', color: 'var(--color-secondary-dark)' },
    '.cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
    '&.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
    '.cm-cursor': { borderLeftColor: 'var(--color-tertiary-dark)' },
    '.cm-tooltip': {
      backgroundColor: 'var(--color-surface-dark)',
      border: '1px solid var(--color-border-strong-dark)',
      color: 'var(--color-primary-dark)',
      borderRadius: '6px',
      boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
    },
    '.cm-run-statement-btn': {
      ...shared['.cm-run-statement-btn'],
      color: 'var(--color-success-dark)',
    },
    '.cm-run-statement-btn:hover': { background: 'var(--color-success-soft-dark)', opacity: '1' },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: 'var(--color-tertiary-soft-dark)',
      color: 'var(--color-tertiary-dark)',
    },
    '.cm-searchMatch': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
    '.cm-selectionMatch': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
  },
  { dark: true }
)
