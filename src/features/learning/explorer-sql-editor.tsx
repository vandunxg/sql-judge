import { useEffect, useRef } from "react"
import Editor, { type OnMount } from "@monaco-editor/react"

import { useIsDark } from "@/components/use-is-dark"
import { useAppSettings } from "@/lib/app-settings"

type SqlEditorProps = {
  value: string
  label: string
  onChange: (value: string) => void
  onRun: () => void
}

export function ExplorerSqlEditor({ value, label, onChange, onRun }: SqlEditorProps) {
  const dark = useIsDark()
  const { editorFontSize } = useAppSettings()
  const onRunRef = useRef(onRun)

  useEffect(() => {
    onRunRef.current = onRun
  })

  const handleMount: OnMount = (editor, monaco) => {
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => onRunRef.current())
  }

  return (
    <div className="relative h-[168px] shrink-0 border-b py-3.5">
      <Editor
        height="100%"
        language="sql"
        theme={dark ? "vs-dark" : "light"}
        value={value}
        onChange={(next) => onChange(next ?? "")}
        onMount={handleMount}
        options={{
          ariaLabel: label,
          automaticLayout: true,
          fontFamily: "'Geist Mono', ui-monospace, SFMono-Regular, Menlo, monospace",
          fontSize: editorFontSize,
          lineHeight: 23,
          minimap: { enabled: false },
          scrollBeyondLastLine: false,
          tabSize: 2,
          wordWrap: "on",
        }}
      />
    </div>
  )
}
