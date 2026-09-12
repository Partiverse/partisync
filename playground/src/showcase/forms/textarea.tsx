import { Textarea } from "@/components/ui/textarea"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupText } from "@/components/ui/input-group"
import { Demo, Section } from "../shared"

export function TextareaSection() {
  return (
<Section
        id="textarea"
        title="Textarea"
        group="examples"
        description="Displays a form textarea or a component that looks like a textarea."
        fileKey="forms"
      >
      {/* ============ Textarea · Official H2 demos ============ */}
      <Demo
        title="Textarea · Field"
        description="Textarea pairs with Field for labels."      >
        <Field>
          <FieldLabel htmlFor="ta-f">Comment</FieldLabel>
          <Textarea id="ta-f" placeholder="Write your comment..." rows={3} />
        </Field>
      </Demo>

      <Demo
        title="Textarea · Disabled"
        description="Use the disabled attribute to prevent interaction."      >
        <Textarea disabled placeholder="Disabled textarea" rows={2} />
      </Demo>

      <Demo
        title="Textarea · Invalid"
        description="Use aria-invalid to mark a textarea as invalid."      >
        <Textarea aria-invalid placeholder="Invalid input" rows={2} />
      </Demo>

      <Demo
        title="Textarea · Button"
        description="InputGroupAddon renders a footer with a post button."      >
        <InputGroup>
          <Textarea placeholder="Write a comment..." rows={3} className="border-0 focus-visible:ring-0" />
          <InputGroupAddon align="block-end">
            <InputGroupText className="text-muted-foreground">0 / 280</InputGroupText>
            <InputGroupButton variant="default">Post</InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      </Section>
  )
}
