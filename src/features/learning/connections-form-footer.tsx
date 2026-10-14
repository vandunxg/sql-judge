import { Button } from "@/components/ui/button"

type FooterProps = {
  testing: boolean
  canTest: boolean
  canDelete: boolean
  disabled: boolean
  onDelete: () => void
  onTest: () => void
  isNew: boolean
}

export function ConnectionsFormFooter({
  canTest,
  testing,
  canDelete,
  disabled,
  onDelete,
  onTest,
  isNew,
}: FooterProps) {
  return (
    <div className="flex flex-wrap items-center gap-3 border-t bg-sidebar px-6 py-4">
      <div className="flex-1" />
      {canDelete ? (
        <Button
          type="button"
          variant="ghost"
          className="text-err hover:text-err"
          onClick={onDelete}
          disabled={disabled}
        >
          Delete
        </Button>
      ) : null}
      <Button
        type="button"
        variant="outline"
        className="bg-card shadow-xs"
        onClick={onTest}
        disabled={!canTest || testing}
      >
        Test connection
      </Button>
      <Button type="submit" disabled={disabled}>
        {isNew ? "Save profile" : "Save changes"}
      </Button>
    </div>
  )
}
