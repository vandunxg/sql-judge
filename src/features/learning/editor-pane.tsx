import { useEffect, useMemo, useRef } from "react"
import Editor, { type OnMount } from "@monaco-editor/react"
import { Code2, RotateCcw } from "lucide-react"
import type { Position, editor } from "monaco-editor"

import { useIsDark } from "@/components/use-is-dark"
import { useAppSettings } from "@/lib/app-settings"
import { Button } from "@/components/ui/button"

import type { LearningProblem } from "./catalog"
import { dialectLabels } from "./problem-pane"
import { paneClass } from "./solve-pane"
import { sqlCompletionItems } from "./sql-completions"
import type { JudgeMode } from "./use-judge-run"

type EditorPaneProps = {
  problem: LearningProblem | null
  value: string
  readOnly: boolean
  onChange: (value: string) => void
  onReset: () => void
  onRun: (mode: JudgeMode) => void
}

export function EditorPane({ problem, value, readOnly, onChange, onReset, onRun }: EditorPaneProps) {
  const dark = useIsDark()
  const { editorFontSize } = useAppSettings()
  const completionItems = useMemo(
    () => (problem ? sqlCompletionItems(problem.dialect, problem.schema) : []),
    [problem]
  )
  const completionItemsRef = useRef(completionItems)
  const onRunRef = useRef(onRun)
  const providerRef = useRef<{ dispose(): void } | null>(null)

  useEffect(() => {
    completionItemsRef.current = completionItems
    onRunRef.current = onRun
  })

  useEffect(() => () => providerRef.current?.dispose(), [])

  const handleMount: OnMount = (editor, monaco) => {
    const { CompletionItemKind, CompletionItemInsertTextRule } = monaco.languages
    const { KeyCode, KeyMod } = monaco
    const kinds = {
      keyword: CompletionItemKind.Keyword,
      snippet: CompletionItemKind.Snippet,
      table: CompletionItemKind.Struct,
      column: CompletionItemKind.Field,
    }
    providerRef.current?.dispose()
    providerRef.current = monaco.languages.registerCompletionItemProvider("sql", {
      provideCompletionItems: (model: editor.ITextModel, position: Position) => {
        const word = model.getWordUntilPosition(position)
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn,
        }
        return {
          suggestions: completionItemsRef.current.map((item) => ({
            label: item.label,
            detail: item.detail,
            insertText: item.insertText,
            insertTextRules:
              item.kind === "snippet" ? CompletionItemInsertTextRule.InsertAsSnippet : undefined,
            kind: kinds[item.kind],
            range,
          })),
        }
      },
    })
    editor.addCommand(KeyMod.CtrlCmd | KeyCode.Enter, () => onRunRef.current("run"))
    editor.addCommand(KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.Enter, () => onRunRef.current("submit"))
  }

  return (
    <section className={`${paneClass} min-h-[360px] flex-1`} aria-label="SQL editor">
      <div className="flex h-[42px] shrink-0 items-center justify-between border-b pr-1.5 pl-3">
        <div className="flex items-center gap-2.5 text-[13px]">
          <span className="inline-flex items-center gap-1.5 font-medium">
            <Code2 className="size-3.5 text-ring" /> Code
          </span>
          {problem ? <span className="text-muted-foreground">{dialectLabels[problem.dialect]}</span> : null}
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Reset to starter code"
          title="Reset to starter code"
          disabled={readOnly || !problem}
          onClick={onReset}
        >
          <RotateCcw />
        </Button>
      </div>
      <div className="relative min-h-0 flex-1">
        <div className="absolute inset-x-0 inset-y-2.5">
          <Editor
            height="100%"
            language="sql"
            theme={dark ? "vs-dark" : "light"}
            value={value}
            onChange={(next) => onChange(next ?? "")}
            onMount={handleMount}
            options={{
              ariaLabel: problem ? `SQL editor for ${problem.title}` : "SQL editor",
              automaticLayout: true,
              fontFamily: "'Geist Mono', ui-monospace, SFMono-Regular, Menlo, monospace",
              fontSize: editorFontSize,
              lineHeight: Math.round(editorFontSize * 1.7),
              minimap: { enabled: false },
              readOnly,
              scrollBeyondLastLine: false,
              tabSize: 2,
              wordWrap: "on",
            }}
          />
        </div>
      </div>
      <div className="flex justify-between gap-3 border-t px-3.5 py-1.5 text-xs text-muted-foreground">
        <span>Read-only · one statement</span>
        <span>{value.length.toLocaleString()} chars</span>
      </div>
    </section>
  )
}
