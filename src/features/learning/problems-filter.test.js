import { describe, expect, test } from "bun:test"

import { filterCatalog, pageWindow, topicCounts } from "./problems-filter"

const problem = (id, overrides) => ({
  id,
  problemId: id,
  title: `Problem ${id}`,
  statement: "",
  topics: [],
  difficulty: "easy",
  dialect: "postgresql",
  packId: "hr",
  ...overrides,
})

const problems = [
  problem("a", { topics: ["Joins", "Aggregation"] }),
  problem("b", { topics: ["Joins"], difficulty: "hard", packId: "shop" }),
  problem("c", { topics: ["CTEs"], title: "Recursive chain" }),
]
const statuses = new Map([
  ["a", { status: "solved" }],
  ["b", { status: "attempted" }],
])
const all = { query: "", topic: "all", packId: "all", difficulty: "all", status: "all" }

describe("problems filter", () => {
  test("counts topics in first-seen order", () => {
    expect(topicCounts(problems)).toEqual([
      { topic: "Joins", count: 2 },
      { topic: "Aggregation", count: 1 },
      { topic: "CTEs", count: 1 },
    ])
  })

  test("combines topic, pack, difficulty and search filters", () => {
    const ids = (filters) =>
      filterCatalog(problems, { ...all, ...filters }, statuses).map((p) => p.id)
    expect(ids({ topic: "Joins" })).toEqual(["a", "b"])
    expect(ids({ topic: "Joins", packId: "shop" })).toEqual(["b"])
    expect(ids({ difficulty: "hard" })).toEqual(["b"])
    expect(ids({ query: "recursive" })).toEqual(["c"])
  })

  test("filters by solved, attempted and not-started status", () => {
    const ids = (status) =>
      filterCatalog(problems, { ...all, status }, statuses).map((p) => p.id)
    expect(ids("solved")).toEqual(["a"])
    expect(ids("attempted")).toEqual(["b"])
    expect(ids("todo")).toEqual(["c"])
  })

  test("shows at most five page numbers around the current page", () => {
    expect(pageWindow(0, 3)).toEqual([0, 1, 2])
    expect(pageWindow(0, 20)).toEqual([0, 1, 2, 3, 4])
    expect(pageWindow(10, 20)).toEqual([8, 9, 10, 11, 12])
    expect(pageWindow(19, 20)).toEqual([15, 16, 17, 18, 19])
  })
})
