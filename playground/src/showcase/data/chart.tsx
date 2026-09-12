import { Demo, Section } from "../shared"
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart"
import {
  AreaChart,
  Area,
  BarChart,
  Bar,
  LineChart,
  Line,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
} from "recharts"

// ─── Sample Data ────────────────────────────────────────────────────────────

const areaData = [
  { month: "Jan", revenue: 1860, expenses: 800 },
  { month: "Feb", revenue: 2200, expenses: 950 },
  { month: "Mar", revenue: 3100, expenses: 1200 },
  { month: "Apr", revenue: 2800, expenses: 1100 },
  { month: "May", revenue: 3600, expenses: 1400 },
  { month: "Jun", revenue: 4200, expenses: 1800 },
]

const barData = [
  { category: "Electronics", sales: 2750 },
  { category: "Clothing", sales: 2100 },
  { category: "Home", sales: 1800 },
  { category: "Books", sales: 1200 },
  { category: "Sports", sales: 950 },
]

const lineData = [
  { year: "2020", users: 400 },
  { year: "2021", users: 950 },
  { year: "2022", users: 1800 },
  { year: "2023", users: 2400 },
  { year: "2024", users: 3600 },
]

const pieData = [
  { browser: "Chrome", value: 48, fill: "oklch(0.646 0.222 41.116)" },
  { browser: "Safari", value: 28, fill: "oklch(0.627 0.194 149.9)" },
  { browser: "Firefox", value: 12, fill: "oklch(0.577 0.245 27.325)" },
  { browser: "Edge", value: 8, fill: "oklch(0.646 0.196 214.4)" },
  { browser: "Other", value: 4, fill: "oklch(0.507 0.023 264.3)" },
]

// ─── Demos ───────────────────────────────────────────────────────────────────

function AreaChartDemo() {
  return (
    <ChartContainer
      className="w-full"
      config={{
        revenue: { label: "Revenue (USD)", color: "oklch(0.646 0.222 41.116)" },
        expenses: { label: "Expenses (USD)", color: "oklch(0.577 0.245 27.325)" },
      }}
    >
      <AreaChart data={areaData} margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
        <CartesianGrid vertical={false} strokeDasharray="3 3" />
        <XAxis
          dataKey="month"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          tickFormatter={(v) => `$${v}`}
        />
        <ChartTooltip
          content={<ChartTooltipContent indicator="dot" />}
        />
        <Area
          dataKey="revenue"
          type="monotone"
          fill="oklch(0.646 0.222 41.116 / 0.2)"
          stroke="oklch(0.646 0.222 41.116)"
          strokeWidth={2}
        />
        <Area
          dataKey="expenses"
          type="monotone"
          fill="oklch(0.577 0.245 27.325 / 0.2)"
          stroke="oklch(0.577 0.245 27.325)"
          strokeWidth={2}
        />
      </AreaChart>
    </ChartContainer>
  )
}

function BarChartDemo() {
  return (
    <ChartContainer
      className="w-full"
      config={{
        sales: { label: "Sales (USD)", color: "oklch(0.627 0.194 149.9)" },
      }}
    >
      <BarChart data={barData} margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
        <CartesianGrid vertical={false} strokeDasharray="3 3" />
        <XAxis
          dataKey="category"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          tickFormatter={(v) => `$${v}`}
        />
        <ChartTooltip
          content={<ChartTooltipContent indicator="dot" />}
        />
        <Bar dataKey="sales" fill="oklch(0.627 0.194 149.9)" radius={4} />
      </BarChart>
    </ChartContainer>
  )
}

function LineChartDemo() {
  return (
    <ChartContainer
      className="w-full"
      config={{
        users: { label: "Users", color: "oklch(0.646 0.245 255)" },
      }}
    >
      <LineChart data={lineData} margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
        <CartesianGrid vertical={false} strokeDasharray="3 3" />
        <XAxis
          dataKey="year"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          tickMargin={8}
        />
        <ChartTooltip
          content={<ChartTooltipContent indicator="dot" />}
        />
        <Line
          dataKey="users"
          type="monotone"
          stroke="oklch(0.646 0.245 255)"
          strokeWidth={2}
          dot={{ fill: "oklch(0.646 0.245 255)", r: 4 }}
          activeDot={{ r: 6 }}
        />
      </LineChart>
    </ChartContainer>
  )
}

function PieChartDemo() {
  return (
    <div className="flex items-center gap-8">
      <ChartContainer
        className="w-48 h-48"
        config={{
          Chrome: { label: "Chrome", color: pieData[0].fill },
          Safari: { label: "Safari", color: pieData[1].fill },
          Firefox: { label: "Firefox", color: pieData[2].fill },
          Edge: { label: "Edge", color: pieData[3].fill },
          Other: { label: "Other", color: pieData[4].fill },
        }}
      >
        <PieChart>
          <Pie
            data={pieData}
            dataKey="value"
            nameKey="browser"
            cx="50%"
            cy="50%"
            innerRadius={48}
            outerRadius={72}
          >
            {pieData.map((entry) => (
              <Cell key={entry.browser} fill={entry.fill} />
            ))}
          </Pie>
          <ChartTooltip content={<ChartTooltipContent hideIndicator />} />
        </PieChart>
      </ChartContainer>
      <div className="flex flex-col gap-2">
        {pieData.map((item) => (
          <div key={item.browser} className="flex items-center gap-2">
            <span
              className="size-2 rounded-full shrink-0"
              style={{ backgroundColor: item.fill }}
            />
            <span className="text-sm">
              {item.browser}{" "}
              <span className="text-muted-foreground">{item.value}%</span>
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

// ─── Section ─────────────────────────────────────────────────────────────────

export function ChartSection() {
  return (
    <Section
      id="chart"
      title="Chart"
      group="examples"
      description="Interactive charts built with Recharts."
      fileKey="data"
    >
      <Demo
        title="Chart · Area"
        description="An area chart showing revenue and expenses over time."
        center
      >
        <AreaChartDemo />
      </Demo>

      <Demo
        title="Chart · Bar"
        description="A bar chart showing sales by category."
        center
      >
        <BarChartDemo />
      </Demo>

      <Demo
        title="Chart · Line"
        description="A line chart showing user growth over time."
        center
      >
        <LineChartDemo />
      </Demo>

      <Demo
        title="Chart · Pie"
        description="A pie chart showing browser market share."
        center
      >
        <PieChartDemo />
      </Demo>
    </Section>
  )
}
