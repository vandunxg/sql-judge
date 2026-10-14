export type CsvCell = string | number | boolean | null

// NULL becomes an empty unquoted field; empty text is quoted so the two stay distinguishable.
function csvField(value: CsvCell): string {
  if (value === null) {
    return ""
  }
  const text = String(value)
  return text === "" || /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

export function toCsv(columns: string[], rows: CsvCell[][]): string {
  return [columns, ...rows].map((row) => row.map(csvField).join(",")).join("\r\n")
}
