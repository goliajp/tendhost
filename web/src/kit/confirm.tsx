import { useState } from 'react'
import { Button, Field, Input } from './ui'

/** A destructive action that takes a second, explicit press. */
export function ConfirmButton({
  label,
  question,
  confirmLabel,
  busy,
  onConfirm,
}: {
  label: string
  question: string
  confirmLabel: string
  busy?: boolean
  onConfirm: () => void
}) {
  const [asking, setAsking] = useState(false)
  if (!asking) {
    return (
      <Button tone="danger" onClick={() => setAsking(true)}>
        {label}
      </Button>
    )
  }
  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <span className="text-[0.93rem]">{question}</span>
      <Button tone="danger" disabled={busy} onClick={onConfirm}>
        {confirmLabel}
      </Button>
      <Button onClick={() => setAsking(false)}>Keep</Button>
    </span>
  )
}

/** For what cannot be undone and takes data with it: the name has to be
 *  typed before the button works. */
export function TypedConfirm({
  expected,
  prompt,
  confirmLabel,
  busy,
  onConfirm,
  onCancel,
}: {
  expected: string
  prompt: string
  confirmLabel: string
  busy?: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  const [typed, setTyped] = useState('')
  return (
    <form
      className="space-y-3"
      onSubmit={(event) => {
        event.preventDefault()
        if (typed === expected) onConfirm()
      }}
    >
      <Field label={prompt}>
        {(control) => (
          <Input
            {...control}
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            autoFocus
          />
        )}
      </Field>
      <div className="flex flex-wrap gap-2">
        <Button type="submit" tone="danger" disabled={busy || typed !== expected}>
          {confirmLabel}
        </Button>
        <Button onClick={onCancel}>Keep it</Button>
      </div>
    </form>
  )
}
