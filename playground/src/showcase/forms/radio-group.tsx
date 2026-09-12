import * as React from "react"
import { Field, FieldContent, FieldDescription, FieldLabel, FieldLegend, FieldSet, FieldTitle } from "@/components/ui/field"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Demo, Section } from "../shared"

export function RadioGroupSection() {
  const [, setRadio] = React.useState("default")

  return (
<Section
        id="radio-group"
        title="Radio Group"
        group="examples"
        description="A set of checkable buttons—known as radio buttons—where no more than one of the buttons can be checked at a time."
        fileKey="forms"
      >
      {/* ============ RadioGroup · Official H2 demos ============ */}
      <Demo
        title="RadioGroup · Composition"
        description="RadioGroup renders a list of radio items."      >
        <RadioGroup defaultValue="comfort" onValueChange={(v) => setRadio(v as string)}>
          {[
            { v: "compact", t: "Compact" },
            { v: "comfort", t: "Comfortable" },
            { v: "spacious", t: "Spacious" },
          ].map(({ v, t }) => (
            <label key={v} className="flex items-center gap-2 text-sm">
              <RadioGroupItem value={v} /> {t}
            </label>
          ))}
        </RadioGroup>
      </Demo>

      <Demo
        title="RadioGroup · Description"
        description="FieldLabel adds a title and description to each option."      >
        {[
          { v: "all", t: "All messages", d: "Get notified for every message." },
          { v: "mentions", t: "Mentions only", d: "Only when someone @mentions you." },
        ].map(({ v, t, d }) => (
          <Field key={v} orientation="horizontal">
            <RadioGroupItem value={v} id={`rg-d-${v}`} />
            <FieldLabel htmlFor={`rg-d-${v}`}>
              <FieldTitle>{t}</FieldTitle>
              <FieldDescription>{d}</FieldDescription>
            </FieldLabel>
          </Field>
        ))}
      </Demo>

      <Demo
        title="RadioGroup · Choice Card"
        description="Radio items rendered as choice cards with details."      >
        <RadioGroup defaultValue="pro" onValueChange={(v) => setRadio(v as string)}>
          {[
            { v: "free", t: "Free", d: "$0/month · 1 project" },
            { v: "pro", t: "Pro", d: "$20/month · unlimited" },
          ].map(({ v, t, d }) => (
            <FieldLabel key={v}>
              <Field orientation="horizontal">
                <RadioGroupItem value={v} />
                <FieldContent>
                  <FieldTitle>{t}</FieldTitle>
                  <FieldDescription>{d}</FieldDescription>
                </FieldContent>
              </Field>
            </FieldLabel>
          ))}
        </RadioGroup>
      </Demo>

      <Demo
        title="RadioGroup · Fieldset"
        description="FieldSet groups a radio group under a legend."      >
        <FieldSet>
          <FieldLegend variant="label">Choose a plan</FieldLegend>
          <RadioGroup defaultValue="pro">
            <Field orientation="horizontal">
              <RadioGroupItem value="free" id="rg-fs-free" />
              <FieldLabel htmlFor="rg-fs-free">Free</FieldLabel>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="pro" id="rg-fs-pro" />
              <FieldLabel htmlFor="rg-fs-pro">Pro</FieldLabel>
            </Field>
          </RadioGroup>
        </FieldSet>
      </Demo>

      <Demo
        title="RadioGroup · Disabled"
        description="Disabled options are non-interactive and dimmed."      >
        <div className="flex flex-col gap-3">
          <label className="flex items-center gap-2 text-sm opacity-50">
            <RadioGroupItem value="a" disabled /> Disabled option
          </label>
        </div>
      </Demo>

      <Demo
        title="RadioGroup · Invalid"
        description="Use aria-invalid to mark a radio group as invalid."      >
        <RadioGroup defaultValue="">
          <label className="flex items-center gap-2 text-sm text-destructive">
            <RadioGroupItem aria-invalid value="x" /> Pick one of the options
          </label>
        </RadioGroup>
      </Demo>

      </Section>
  )
}
