import { Input } from "@/components/ui/input"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupText } from "@/components/ui/input-group"
import { Badge } from "@/components/ui/badge"
import { Demo, Section } from "../shared"

export function InputSection() {
  return (
<Section
        id="input"
        title="Input"
        group="examples"
        description="A text input component for forms and user data entry with built-in styling and accessibility features."
        fileKey="forms"
      >
      {/* ============ Input · Official H2 demos ============ */}
      <Demo
        title="Input · Basic"
        description="A basic input field."      >
        <Input placeholder="Type something..." />
      </Demo>

      <Demo
        title="Input · Field (Field Wrapper)"
        description="Inputs pair with Field for labels and descriptions."      >
        <Field>
          <FieldLabel htmlFor="i-field">Email</FieldLabel>
          <Input id="i-field" type="email" placeholder="you@example.com" />
        </Field>
      </Demo>

      <Demo
        title="Input · Field Group"
        description="Fields group related inputs together."      >
        <FieldGroup>
          <Field>
            <FieldLabel htmlFor="i-fg1">First name</FieldLabel>
            <Input id="i-fg1" placeholder="John" />
          </Field>
          <Field>
            <FieldLabel htmlFor="i-fg2">Last name</FieldLabel>
            <Input id="i-fg2" placeholder="Doe" />
          </Field>
        </FieldGroup>
      </Demo>

      <Demo
        title="Input · Disabled"
        description="Use the disabled attribute to prevent interaction."      >
        <Input disabled placeholder="Disabled input" />
      </Demo>

      <Demo
        title="Input · Invalid"
        description="Use aria-invalid to mark an input as invalid."      >
        <Input aria-invalid defaultValue="not an email" />
      </Demo>

      <Demo
        title="Input · File"
        description="A native file input styled with the Input component."      >
        <Input type="file" />
      </Demo>

      <Demo
        title="Input · Inline (Prefix)"
        description="InputGroupAddon renders a prefix inside the input."      >
        <InputGroup>
          <InputGroupAddon align="inline-start">
            <InputGroupText>https://</InputGroupText>
          </InputGroupAddon>
          <InputGroupInput placeholder="example.com" />
        </InputGroup>
      </Demo>

      <Demo
        title="Input · Grid"
        description="Inputs can be laid out in a grid."      >
        <div className="grid w-full grid-cols-2 gap-2">
          <Input placeholder="First name" />
          <Input placeholder="Last name" />
          <Input placeholder="City" />
          <Input placeholder="ZIP" />
        </div>
      </Demo>

      <Demo
        title="Input · Required"
        description="Mark required inputs with an asterisk and the required attribute."      >
        <Field>
          <FieldLabel htmlFor="i-req">
            Username <span className="text-destructive">*</span>
          </FieldLabel>
          <Input id="i-req" required placeholder="shadcn" />
        </Field>
      </Demo>

      <Demo
        title="Input · Badge"
        description="InputGroupAddon can render a badge as a suffix."      >
        <InputGroup>
          <InputGroupInput placeholder="Enter your handle..." />
          <InputGroupAddon align="inline-end">
            <Badge variant="secondary">@shadcn</Badge>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      </Section>
  )
}
