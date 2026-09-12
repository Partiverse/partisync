import * as React from "react"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Field, FieldContent, FieldDescription, FieldError, FieldGroup, FieldLabel, FieldLegend, FieldSeparator, FieldSet, FieldTitle } from "@/components/ui/field"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Checkbox } from "@/components/ui/checkbox"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Switch } from "@/components/ui/switch"
import { Slider } from "@/components/ui/slider"
import { Button } from "@/components/ui/button"
import { Demo, Section } from "../shared"

export function FieldSection() {
  const [select, setSelect] = React.useState("")
  const [slider, setSlider] = React.useState(50)
  const [radio, setRadio] = React.useState("default")

  return (
<Section
        id="field"
        title="Field"
        group="installation"
        description="Combine labels, controls, and help text to compose accessible form fields and grouped inputs."
        fileKey="forms"
      >
      {/* ============ Field · Official 17 H2 demos ============ */}
      <Demo
        title="Field · Composition"
        description="Fields are composed from a label, control, and description."      >
        <FieldGroup>
          <Field>
            <FieldLabel htmlFor="f-comp-name">Name</FieldLabel>
            <Input id="f-comp-name" placeholder="Enter your name" />
            <FieldDescription>This is your public display name.</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor="f-comp-email">Email</FieldLabel>
            <Input id="f-comp-email" type="email" placeholder="you@example.com" />
          </Field>
        </FieldGroup>
      </Demo>

      <Demo
        title="Field · Anatomy"
        description="Field anatomy with FieldTitle and FieldContent."      >
        <Field orientation="vertical">
          <FieldTitle>Name</FieldTitle>
          <FieldContent>
            <Input placeholder="Jane Doe" />
            <FieldDescription>We will never share your name.</FieldDescription>
          </FieldContent>
        </Field>
      </Demo>

      <Demo
        title="Field · Form"
        description="Fields compose into a full form with a submit button."      >
        <form className="w-full space-y-4" onSubmit={(e) => e.preventDefault()}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="f-form-name">Full name</FieldLabel>
              <Input id="f-form-name" placeholder="Jane Doe" required />
            </Field>
            <Field>
              <FieldLabel htmlFor="f-form-email">Email</FieldLabel>
              <Input id="f-form-email" type="email" placeholder="you@example.com" required />
            </Field>
            <Field orientation="horizontal">
              <Switch id="f-form-newsletter" />
              <FieldLabel htmlFor="f-form-newsletter">Subscribe to newsletter</FieldLabel>
            </Field>
            <Button type="submit">Save</Button>
          </FieldGroup>
        </form>
      </Demo>

      <Demo
        title="Field · Input"
        description="A field wrapping a text input."      >
        <Field>
          <FieldLabel htmlFor="f-input">Username</FieldLabel>
          <Input id="f-input" placeholder="shadcn" />
        </Field>
      </Demo>

      <Demo
        title="Field · Textarea"
        description="A field wrapping a textarea with a description."      >
        <Field>
          <FieldLabel htmlFor="f-textarea">Bio</FieldLabel>
          <Textarea id="f-textarea" placeholder="Tell us about yourself..." rows={3} />
          <FieldDescription>Max 280 characters.</FieldDescription>
        </Field>
      </Demo>

      <Demo
        title="Field · Select"
        description="A field wrapping a select control."      >
        <Field>
          <FieldLabel htmlFor="f-select">Priority</FieldLabel>
          <Select value={select} onValueChange={(v) => setSelect(v ?? "medium")}>
            <SelectTrigger id="f-select" className="w-full">
              <SelectValue placeholder="Select..." />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="low">Low</SelectItem>
              <SelectItem value="medium">Medium</SelectItem>
              <SelectItem value="high">High</SelectItem>
            </SelectContent>
          </Select>
        </Field>
      </Demo>

      <Demo
        title="Field · Slider"
        description="A field wrapping a slider with live value feedback."      >
        <Field>
          <FieldLabel>Volume</FieldLabel>
          <Slider value={[slider]} onValueChange={(v) => setSlider(Array.isArray(v) ? v[0] : (v as number))} />
          <FieldDescription>Current: {slider}%</FieldDescription>
        </Field>
      </Demo>

      <Demo
        title="Field · Fieldset"
        description="FieldSet groups related fields under a legend."      >
        <FieldSet>
          <FieldLegend>Profile</FieldLegend>
          <FieldDescription>This information will be displayed publicly.</FieldDescription>
          <Field>
            <FieldLabel htmlFor="f-fs-name">Display name</FieldLabel>
            <Input id="f-fs-name" placeholder="johndoe" />
          </Field>
          <Field>
            <FieldLabel htmlFor="f-fs-url">Website</FieldLabel>
            <Input id="f-fs-url" placeholder="https://example.com" />
          </Field>
        </FieldSet>
      </Demo>

      <Demo
        title="Field · Checkbox"
        description="A horizontal field with a checkbox and a rich label."      >
        <Field orientation="horizontal">
          <Checkbox id="f-cb" />
          <FieldLabel htmlFor="f-cb">
            <FieldTitle>Email notifications</FieldTitle>
            <FieldDescription>Receive emails about account activity.</FieldDescription>
          </FieldLabel>
        </Field>
      </Demo>

      <Demo
        title="Field · Radio"
        description="A fieldset with a radio group for single selection."      >
        <FieldSet>
          <FieldLegend variant="label">Notify me about</FieldLegend>
          <RadioGroup value={radio} onValueChange={(v) => setRadio(v as string)}>
            {[
              { v: "all", t: "All new messages" },
              { v: "mentions", t: "Direct mentions and replies" },
              { v: "none", t: "Nothing" },
            ].map(({ v, t }) => (
              <Field key={v} orientation="horizontal">
                <RadioGroupItem value={v} id={`f-radio-${v}`} />
                <FieldLabel htmlFor={`f-radio-${v}`}>{t}</FieldLabel>
              </Field>
            ))}
          </RadioGroup>
        </FieldSet>
      </Demo>

      <Demo
        title="Field · Switch"
        description="A horizontal field with a switch and a rich label."      >
        <Field orientation="horizontal">
          <Switch id="f-sw" defaultChecked />
          <FieldLabel htmlFor="f-sw">
            <FieldTitle>Two-factor authentication</FieldTitle>
            <FieldDescription>Enable 2FA for extra security.</FieldDescription>
          </FieldLabel>
        </Field>
      </Demo>

      <Demo
        title="Field · Choice Card"
        description="Radio items rendered as choice cards."      >
        <RadioGroup defaultValue="card-2" onValueChange={(v) => setRadio(v as string)}>
          <FieldLabel>
            <Field orientation="horizontal">
              <RadioGroupItem value="card-1" />
              <FieldContent>
                <FieldTitle>Free plan</FieldTitle>
                <FieldDescription>$0/month · 1 project</FieldDescription>
              </FieldContent>
            </Field>
          </FieldLabel>
          <FieldLabel>
            <Field orientation="horizontal">
              <RadioGroupItem value="card-2" />
              <FieldContent>
                <FieldTitle>Pro plan</FieldTitle>
                <FieldDescription>$20/month · unlimited projects</FieldDescription>
              </FieldContent>
            </Field>
          </FieldLabel>
        </RadioGroup>
      </Demo>

      <Demo
        title="Field · Field Group"
        description="FieldGroup arranges related fields with a separator."      >
        <FieldGroup>
          <FieldLegend>Personal information</FieldLegend>
          <Field>
            <FieldLabel htmlFor="f-fg-first">First name</FieldLabel>
            <Input id="f-fg-first" placeholder="John" />
          </Field>
          <Field>
            <FieldLabel htmlFor="f-fg-last">Last name</FieldLabel>
            <Input id="f-fg-last" placeholder="Doe" />
          </Field>
          <FieldSeparator>Or</FieldSeparator>
          <Field>
            <FieldLabel htmlFor="f-fg-social">Social security number</FieldLabel>
            <Input id="f-fg-social" placeholder="•••-••-••••" />
          </Field>
        </FieldGroup>
      </Demo>

      <Demo
        title="Field · Responsive Layout"
        description="The responsive orientation adapts to the viewport width."      >
        <Field orientation="responsive">
          <FieldLabel htmlFor="f-resp">Email</FieldLabel>
          <Input id="f-resp" type="email" placeholder="you@example.com" />
        </Field>
        <Field orientation="responsive" className="mt-4">
          <FieldLabel htmlFor="f-resp2">Password</FieldLabel>
          <Input id="f-resp2" type="password" placeholder="••••••••" />
        </Field>
      </Demo>

      <Demo
        title="Field · Validation and Errors"
        description="Fields expose invalid state with an error message."      >
        <Field data-invalid="true">
          <FieldLabel htmlFor="f-err">Email</FieldLabel>
          <Input id="f-err" aria-invalid defaultValue="not-an-email" />
          <FieldError>The email address is invalid.</FieldError>
        </Field>
      </Demo>

      <Demo
        title="Field · Accessibility (sr-only label)"
        description="Use sr-only labels for controls without visible text."      >
        <Field>
          <FieldLabel htmlFor="f-a11y" className="sr-only">
            Search
          </FieldLabel>
          <Input id="f-a11y" placeholder="Search..." />
        </Field>
      </Demo>

      </Section>
  )
}
