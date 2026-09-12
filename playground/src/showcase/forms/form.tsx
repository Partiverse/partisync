import * as React from "react"
import { Form, FormControl, FormDescription, FormField, FormLabel, FormMessage, FormSubmit } from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Demo, Section } from "../shared"
import { CheckIcon, AlertCircleIcon } from "lucide-react"

function BasicFormDemo() {
  const [submitted, setSubmitted] = React.useState(false)

  return (
    <Form
      onFormSubmit={(values) => {
        console.log("Form submitted:", values)
        setSubmitted(true)
        setTimeout(() => setSubmitted(false), 3000)
      }}
      className="w-full max-w-sm space-y-4"
    >
      <FormField>
        <FormLabel htmlFor="f-name">Full Name</FormLabel>
        <FormControl>
          <Input id="f-name" name="fullName" placeholder="Jane Doe" />
        </FormControl>
      </FormField>

      <FormField>
        <FormLabel htmlFor="f-email">Email</FormLabel>
        <FormControl>
          <Input id="f-email" name="email" type="email" placeholder="jane@example.com" />
        </FormControl>
        <FormDescription>We'll never share your email.</FormDescription>
      </FormField>

      <FormField>
        <FormLabel htmlFor="f-password">Password</FormLabel>
        <FormControl>
          <Input id="f-password" name="password" type="password" placeholder="••••••••" />
        </FormControl>
      </FormField>

      <FormSubmit render={<Button type="submit" className="w-full" />}>
        {submitted ? <><CheckIcon className="size-4" /> Submitted!</> : "Sign In"}
      </FormSubmit>
    </Form>
  )
}

function ValidatedFormDemo() {
  const [errors, setErrors] = React.useState<Record<string, string>>({})

  return (
    <Form
      errors={errors}
      onFormSubmit={(values) => {
        console.log("Validated form:", values)
        setErrors({})
      }}
      className="w-full max-w-sm space-y-4"
    >
      <FormField>
        <FormLabel htmlFor="fv-name">Full Name</FormLabel>
        <FormControl>
          <Input
            id="fv-name"
            name="fullName"
            placeholder="Jane Doe"
            aria-invalid={!!errors.fullName}
          />
        </FormControl>
        {errors.fullName && (
          <FormMessage><AlertCircleIcon className="size-3 inline" /> {errors.fullName}</FormMessage>
        )}
      </FormField>

      <FormField>
        <FormLabel htmlFor="fv-email">Email</FormLabel>
        <FormControl>
          <Input
            id="fv-email"
            name="email"
            type="email"
            placeholder="you@example.com"
            aria-invalid={!!errors.email}
          />
        </FormControl>
        {errors.email && (
          <FormMessage><AlertCircleIcon className="size-3 inline" /> {errors.email}</FormMessage>
        )}
      </FormField>

      <FormField>
        <FormLabel htmlFor="fv-username">Username</FormLabel>
        <FormControl>
          <Input
            id="fv-username"
            name="username"
            placeholder="janedoe"
            aria-invalid={!!errors.username}
          />
        </FormControl>
        {errors.username && (
          <FormMessage><AlertCircleIcon className="size-3 inline" /> {errors.username}</FormMessage>
        )}
      </FormField>

      <FormSubmit render={<Button type="submit" className="w-full" />}>
        Create Account
      </FormSubmit>
    </Form>
  )
}

export function FormSection() {
  return (
    <Section
      id="form"
      title="Form"
      group="installation"
      description="A native form element with consolidated error handling using Base UI."
      fileKey="forms"
    >
      <Demo
        title="Form · Basic"
        description="A basic form with fields and a submit button."
        center
      >
        <BasicFormDemo />
      </Demo>

      <Demo
        title="Form · Validation"
        description="A form demonstrating field-level validation with error messages."
        center
      >
        <ValidatedFormDemo />
      </Demo>
    </Section>
  )
}
