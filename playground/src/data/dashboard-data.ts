import type { z } from "zod"
import { schema } from "@/components/data-table"

export type Task = z.infer<typeof schema>

export const dashboardData: Task[] = [
  { id: 1, header: "Mobile test suite coverage", type: "Automated", status: "In Progress", target: "18 cases", limit: "2026-09-30", reviewer: "Chen Chen" },
  { id: 2, header: "Core API regression tests", type: "Automated", status: "Done", target: "42 endpoints", limit: "2026-09-12", reviewer: "Li Mu" },
  { id: 3, header: "Payment flow security audit", type: "Manual", status: "In Progress", target: "6 checks", limit: "2026-10-05", reviewer: "Wang Shuo" },
  { id: 4, header: "Performance benchmark v3", type: "Automated", status: "Queued", target: "8 metrics", limit: "2026-10-15", reviewer: "Zhao Yi" },
  { id: 5, header: "Multilingual UI walkthrough", type: "Manual", status: "Done", target: "5 locales", limit: "2026-08-28", reviewer: "Chen Chen" },
  { id: 6, header: "Offline sync boundary validation", type: "Automated", status: "In Progress", target: "12 cases", limit: "2026-10-02", reviewer: "Li Mu" },
  { id: 7, header: "Permission matrix verification", type: "Manual", status: "Queued", target: "24 pairs", limit: "2026-10-20", reviewer: "Wang Shuo" },
]
