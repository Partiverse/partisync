"use client"

import * as React from "react"
import { Button } from "@/components/ui/button"
import { Slider } from "@/components/ui/slider"
import { Label } from "@/components/ui/label"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { Separator } from "@/components/ui/separator"
import { cn } from "cn"
import { PaletteIcon, CheckIcon } from "lucide-react"

// shadcn official accent color options
const ACCENT_COLORS = [
  { name: "Neutral", value: "oklch(0.205 0 0)", foreground: "oklch(0.985 0 0)", className: "bg-neutral" },
  { name: "Slate", value: "oklch(0.39 0.025 250)", foreground: "oklch(0.985 0 0)", className: "bg-slate-500" },
  { name: "Stone", value: "oklch(0.397 0.013 58.5)", foreground: "oklch(0.985 0 0)", className: "bg-stone-500" },
  { name: "Gray", value: "oklch(0.507 0.023 264.3)", foreground: "oklch(0.985 0 0)", className: "bg-gray-500" },
  { name: "Zinc", value: "oklch(0.452 0.016 255)", foreground: "oklch(0.985 0 0)", className: "bg-zinc-500" },
  { name: "Red", value: "oklch(0.577 0.245 27.325)", foreground: "oklch(0.985 0 0)", className: "bg-red-500" },
  { name: "Orange", value: "oklch(0.655 0.214 41.116)", foreground: "oklch(0.985 0 0)", className: "bg-orange-500" },
  { name: "Amber", value: "oklch(0.769 0.188 70.08)", foreground: "oklch(0.145 0 0)", className: "bg-amber-500" },
  { name: "Yellow", value: "oklch(0.795 0.184 86.05)", foreground: "oklch(0.145 0 0)", className: "bg-yellow-500" },
  { name: "Lime", value: "oklch(0.723 0.219 142.5)", foreground: "oklch(0.145 0 0)", className: "bg-lime-500" },
  { name: "Green", value: "oklch(0.627 0.194 149.9)", foreground: "oklch(0.985 0 0)", className: "bg-green-500" },
  { name: "Emerald", value: "oklch(0.645 0.145 155.5)", foreground: "oklch(0.985 0 0)", className: "bg-emerald-500" },
  { name: "Teal", value: "oklch(0.6 0.127 168.4)", foreground: "oklch(0.985 0 0)", className: "bg-teal-500" },
  { name: "Cyan", value: "oklch(0.646 0.186 221.5)", foreground: "oklch(0.145 0 0)", className: "bg-cyan-500" },
  { name: "Sky", value: "oklch(0.646 0.196 214.4)", foreground: "oklch(0.985 0 0)", className: "bg-sky-500" },
  { name: "Blue", value: "oklch(0.546 0.245 255)", foreground: "oklch(0.985 0 0)", className: "bg-blue-500" },
  { name: "Indigo", value: "oklch(0.549 0.245 277.1)", foreground: "oklch(0.985 0 0)", className: "bg-indigo-500" },
  { name: "Violet", value: "oklch(0.558 0.238 293.1)", foreground: "oklch(0.985 0 0)", className: "bg-violet-500" },
  { name: "Purple", value: "oklch(0.549 0.249 302.3)", foreground: "oklch(0.985 0 0)", className: "bg-purple-500" },
  { name: "Fuchsia", value: "oklch(0.627 0.257 322.2)", foreground: "oklch(0.985 0 0)", className: "bg-fuchsia-500" },
  { name: "Pink", value: "oklch(0.657 0.224 349.7)", foreground: "oklch(0.985 0 0)", className: "bg-pink-500" },
  { name: "Rose", value: "oklch(0.657 0.22 16.25)", foreground: "oklch(0.985 0 0)", className: "bg-rose-500" },
]

const RADIUS_OPTIONS = [
  { label: "None", value: 0 },
  { label: "Sm", value: 0.3 },
  { label: "Md", value: 0.5 },
  { label: "Lg", value: 0.75 },
  { label: "Xl", value: 1.0 },
]

interface ThemeCustomizerState {
  accent: string
  radius: number
}

export function ThemeCustomizer() {
  const [state, setState] = React.useState<ThemeCustomizerState>({
    accent: ACCENT_COLORS[15].value, // Blue default
    radius: 0.625,
  })

  // Apply CSS variable changes
  // Use !important so custom accent survives Dark mode CSS cascade
  // (both :root and .dark define --primary at equal specificity)
  React.useEffect(() => {
    const root = document.documentElement
    root.style.setProperty("--primary", state.accent, "important")
    // Compute foreground: light colors need dark text
    const isLight = parseFloat(state.accent.match(/oklch\(([\d.]+)/)?.[1] ?? "0") > 0.6
    const foreground = isLight ? "oklch(0.145 0 0)" : "oklch(0.985 0 0)"
    root.style.setProperty("--primary-foreground", foreground, "important")
    root.style.setProperty("--radius", `${state.radius}rem`)
  }, [state])

  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button variant="ghost" size="icon" className="size-8" aria-label="Customize theme">
            <PaletteIcon className="size-4" />
          </Button>
        }
      />
      <PopoverContent className="w-80 p-4" align="end">
        <div className="space-y-6">
          {/* Accent Color */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <Label className="text-sm font-medium">Accent Color</Label>
              <span className="text-xs text-muted-foreground">
                {ACCENT_COLORS.find((c) => c.value === state.accent)?.name ?? "Custom"}
              </span>
            </div>
            <div className="grid grid-cols-11 gap-1.5">
              {ACCENT_COLORS.map((color) => (
                <button
                  key={color.value}
                  className={cn(
                    "h-6 w-6 rounded-full border-2 transition-all",
                    state.accent === color.value
                      ? "scale-125 border-foreground"
                      : "border-transparent hover:scale-110"
                  )}
                  style={{ backgroundColor: color.value }}
                  onClick={() => setState((s) => ({ ...s, accent: color.value }))}
                  aria-label={color.name}
                >
                  {state.accent === color.value && (
                    <CheckIcon className="size-3 mx-auto" style={{ color: color.foreground }} />
                  )}
                </button>
              ))}
            </div>
          </div>

          <Separator />

          {/* Border Radius */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <Label className="text-sm font-medium">Border Radius</Label>
              <span className="text-xs text-muted-foreground">
                {state.radius === 0 ? "None" : `${state.radius}rem`}
              </span>
            </div>
            <div className="flex items-center gap-3">
              {RADIUS_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  className={cn(
                    "flex-1 h-8 rounded-md border text-xs font-medium transition-all",
                    state.radius === opt.value
                      ? "border-foreground bg-muted"
                      : "border-border bg-background hover:bg-muted"
                  )}
                  onClick={() => setState((s) => ({ ...s, radius: opt.value }))}
                >
                  {opt.label}
                </button>
              ))}
            </div>
            <Slider
              value={([state.radius] as const) as number[]}
              onValueChange={(v) => setState((s) => ({ ...s, radius: (v as readonly number[])[0] }))}
              min={0}
              max={1.5}
              step={0.05}
              className="w-full"
            />
          </div>

          <Separator />

          {/* Preview */}
          <div className="space-y-2">
            <Label className="text-sm font-medium">Preview</Label>
            <div
              className="flex items-center gap-2 rounded-lg border p-3"
              style={{ borderRadius: `${state.radius}rem` }}
            >
              <div
                className="h-8 w-8 rounded-md"
                style={{ backgroundColor: state.accent }}
              />
              <div className="space-y-0.5">
                <p className="text-sm font-medium">Primary</p>
                <p className="text-xs text-muted-foreground">
                  {ACCENT_COLORS.find((c) => c.value === state.accent)?.name ?? "Custom"}
                </p>
              </div>
            </div>
            <Button
              className="w-full"
              style={{ borderRadius: `${state.radius}rem` }}
            >
              Button
            </Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}
