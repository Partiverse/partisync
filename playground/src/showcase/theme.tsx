import { Moon, Monitor, Sun } from "lucide-react"
import { useTheme } from "next-themes"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Section } from "./shared"

type Theme = "light" | "dark" | "system"

const themeOptions: { value: Theme; label: string; icon: typeof Sun }[] = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Monitor },
]

const colorTokens: { name: string; variable: string }[] = [
  { name: "Background", variable: "--background" },
  { name: "Foreground", variable: "--foreground" },
  { name: "Card", variable: "--card" },
  { name: "Card Foreground", variable: "--card-foreground" },
  { name: "Popover", variable: "--popover" },
  { name: "Popover Foreground", variable: "--popover-foreground" },
  { name: "Primary", variable: "--primary" },
  { name: "Primary Foreground", variable: "--primary-foreground" },
  { name: "Secondary", variable: "--secondary" },
  { name: "Secondary Foreground", variable: "--secondary-foreground" },
  { name: "Muted", variable: "--muted" },
  { name: "Muted Foreground", variable: "--muted-foreground" },
  { name: "Accent", variable: "--accent" },
  { name: "Accent Foreground", variable: "--accent-foreground" },
  { name: "Destructive", variable: "--destructive" },
  { name: "Destructive Foreground", variable: "--destructive-foreground" },
  { name: "Border", variable: "--border" },
  { name: "Input", variable: "--input" },
  { name: "Ring", variable: "--ring" },
]

export default function ThemeSection() {
  return (
    <Section
      id="theme"
      title="Theme"
      description="Dark mode support powered by next-themes and CSS theme tokens. The switcher follows the official DropdownMenu pattern (Light / Dark / System), and token swatches reference existing index.css variables."
    >
      <div className="xl:col-span-2 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Theme Switcher</CardTitle>
            <CardDescription>
              Official ModeToggle pattern: inline toggle between Light / Dark / System using lucide Sun / Moon / Monitor icons.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <ThemeSwitcher />
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Radius & Typography</CardTitle>
            <CardDescription>
              Radius variables and Geist font. Typography class names follow the official typography specifications.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-1.5">
              <p className="text-sm font-medium">Radius</p>
              <p className="text-lg font-semibold tracking-tight">var(--radius) = 0.625rem</p>
              <p className="text-xs text-muted-foreground">Derived from --radius-sm / --radius-md / --radius-lg / --radius-xl</p>
            </div>
            <div className="space-y-1.5">
              <p className="text-sm font-medium">Geist Font</p>
              <p className="text-2xl font-semibold tracking-tight">The quick brown fox</p>
              <p className="text-sm text-muted-foreground">--font-sans: "Geist Variable", official Typography utility</p>
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="mt-4">
        <div className="mb-4">
          <h3 className="text-lg font-semibold tracking-tight">Theme Tokens</h3>
          <p className="mt-1 text-sm text-muted-foreground">The color swatches below are populated from index.css theme variables and automatically adapt to light/dark mode.</p>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {colorTokens.map((token) => (
            <Card key={token.variable} size="sm">
              <CardContent className="space-y-3">
                <div className="h-10 w-full rounded-md ring-1 ring-foreground/10" style={{ backgroundColor: `var(${token.variable})` }} />
                <div>
                  <p className="text-sm font-medium">{token.name}</p>
                  <p className="font-mono text-xs text-muted-foreground">{token.variable}</p>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    </Section>
  )
}

function ThemeSwitcher() {
  const { theme, setTheme } = useTheme()
  const ThemeIcon = themeOptions.find((t) => t.value === theme)?.icon ?? Sun

  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>
        <ThemeIcon className="size-4" />
        <span className="capitalize">{theme}</span>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {themeOptions.map(({ value, label, icon: Icon }) => (
          <DropdownMenuItem key={value} onClick={() => setTheme(value)}>
            <Icon className="size-4" />
            {label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
