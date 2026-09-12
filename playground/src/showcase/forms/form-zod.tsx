import * as React from "react"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { z } from "zod"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field"
import { Demo, Section } from "../shared"
import { CheckIcon, AlertCircleIcon } from "lucide-react"

// ─── Schema ────────────────────────────────────────────────────────────────

const formSchema = z.object({
  fullName: z
    .string()
    .min(2, "Name must be at least 2 characters")
    .max(50, "Name must be at most 50 characters"),
  email: z.string().email("Please enter a valid email address"),
  username: z
    .string()
    .min(3, "Username must be at least 3 characters")
    .max(20, "Username must be at most 20 characters")
    .regex(/^[a-zA-Z0-9_]+$/, "Only letters, numbers, and underscores"),
  age: z
    .string()
    .refine((v) => !isNaN(Number(v)), { message: "Age must be a number" })
    .refine((v) => Number(v) >= 13, { message: "You must be at least 13 years old" })
    .refine((v) => Number(v) <= 120, { message: "Please enter a valid age" }),
  website: z
    .string()
    .url("Please enter a valid URL")
    .optional()
    .or(z.literal("")),
  role: z.enum(["developer", "designer", "manager", "other"], {
    message: "Please select a role",
  }),
  bio: z
    .string()
    .max(160, "Bio must be at most 160 characters")
    .optional()
    .or(z.literal("")),
  notifications: z.boolean().optional(),
  marketingConsent: z.boolean().optional(),
})

type FormValues = z.infer<typeof formSchema>

// ─── Individual demos ─────────────────────────────────────────────────────

function FormWithValidation() {
  const [isSubmitting, setIsSubmitting] = React.useState(false)
  const [submitted, setSubmitted] = React.useState(false)

  const {
    register,
    handleSubmit,
    formState: { errors },
    reset,
  } = useForm<FormValues>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      notifications: true,
      marketingConsent: false,
    },
  })

  const onSubmit = async (data: FormValues) => {
    setIsSubmitting(true)
    await new Promise((r) => setTimeout(r, 1000))
    // Convert age string to number for submission
    const submittedData = { ...data, age: Number(data.age) }
    console.log("Form submitted:", submittedData)
    setIsSubmitting(false)
    setSubmitted(true)
    setTimeout(() => setSubmitted(false), 3000)
    reset()
  }

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="w-full max-w-sm space-y-5">
      {/* Full Name */}
      <Field>
        <FieldLabel htmlFor="fz-name">Full Name</FieldLabel>
        <Input
          id="fz-name"
          placeholder="Jane Doe"
          {...register("fullName")}
          aria-invalid={!!errors.fullName}
        />
        {errors.fullName && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.fullName.message}
          </FieldError>
        )}
      </Field>

      {/* Email */}
      <Field>
        <FieldLabel htmlFor="fz-email">Email</FieldLabel>
        <Input
          id="fz-email"
          type="email"
          placeholder="jane@example.com"
          {...register("email")}
          aria-invalid={!!errors.email}
        />
        {errors.email && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.email.message}
          </FieldError>
        )}
      </Field>

      {/* Username */}
      <Field>
        <FieldLabel htmlFor="fz-username">Username</FieldLabel>
        <Input
          id="fz-username"
          placeholder="jane_doe"
          {...register("username")}
          aria-invalid={!!errors.username}
        />
        {errors.username && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.username.message}
          </FieldError>
        )}
        <FieldDescription>Letters, numbers, and underscores only.</FieldDescription>
      </Field>

      {/* Age */}
      <Field>
        <FieldLabel htmlFor="fz-age">Age</FieldLabel>
        <Input
          id="fz-age"
          type="number"
          placeholder="25"
          {...register("age")}
          aria-invalid={!!errors.age}
        />
        {errors.age && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.age.message}
          </FieldError>
        )}
      </Field>

      {/* Role */}
      <Field>
        <FieldLabel htmlFor="fz-role">Role</FieldLabel>
        <Select
          onValueChange={(v) =>
            register("role").onChange({ target: { value: v } })
          }
        >
          <SelectTrigger
            id="fz-role"
            aria-invalid={!!errors.role}
          >
            <SelectValue placeholder="Select a role..." />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="developer">Developer</SelectItem>
            <SelectItem value="designer">Designer</SelectItem>
            <SelectItem value="manager">Manager</SelectItem>
            <SelectItem value="other">Other</SelectItem>
          </SelectContent>
        </Select>
        {errors.role && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.role.message}
          </FieldError>
        )}
      </Field>

      {/* Bio */}
      <Field>
        <FieldLabel htmlFor="fz-bio">Bio</FieldLabel>
        <Textarea
          id="fz-bio"
          placeholder="Tell us about yourself..."
          {...register("bio")}
          aria-invalid={!!errors.bio}
        />
        {errors.bio && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.bio.message}
          </FieldError>
        )}
      </Field>

      {/* Website */}
      <Field>
        <FieldLabel htmlFor="fz-website">Website (optional)</FieldLabel>
        <Input
          id="fz-website"
          type="url"
          placeholder="https://example.com"
          {...register("website")}
          aria-invalid={!!errors.website}
        />
        {errors.website && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.website.message}
          </FieldError>
        )}
      </Field>

      {/* Notifications toggle */}
      <Field orientation="horizontal">
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="fz-notifications" className="pb-0">
            Email Notifications
          </FieldLabel>
          <FieldDescription>Receive updates about your account.</FieldDescription>
        </div>
        <Switch
          id="fz-notifications"
          {...register("notifications")}
        />
      </Field>

      {/* Marketing consent */}
      <Field orientation="horizontal">
        <FieldLabel htmlFor="fz-marketing" className="pb-0">
          Marketing emails
        </FieldLabel>
        <Switch
          id="fz-marketing"
          {...register("marketingConsent")}
        />
      </Field>

      <Button type="submit" disabled={isSubmitting} className="w-full">
        {isSubmitting ? "Submitting..." : submitted ? "✓ Submitted!" : "Submit"}
      </Button>
    </form>
  )
}

function FormWithAsyncValidation() {
  const [isEmailAvailable, setIsEmailAvailable] = React.useState<boolean | null>(null)
  const [isChecking, setIsChecking] = React.useState(false)

  const {
    register,
    formState: { errors },
    watch,
  } = useForm<FormValues>({
    resolver: zodResolver(formSchema),
    defaultValues: { email: "" },
  })

  const email = watch("email")

  // Simulate async email availability check
  React.useEffect(() => {
    if (!email || !email.includes("@")) {
      setIsEmailAvailable(null)
      return
    }

    setIsChecking(true)
    const timer = setTimeout(() => {
      // Simulate: emails containing "taken" are already registered
      setIsEmailAvailable(!email.includes("taken"))
      setIsChecking(false)
    }, 600)

    return () => clearTimeout(timer)
  }, [email])

  return (
    <div className="w-full max-w-sm space-y-4">
      <p className="text-sm text-muted-foreground">
        Try typing an email containing <code className="bg-muted rounded px-1">taken</code> to see async validation.
      </p>
      <Field>
        <FieldLabel htmlFor="fz-async-email">Email</FieldLabel>
        <div className="relative">
          <Input
            id="fz-async-email"
            type="email"
            placeholder="jane@example.com"
            {...register("email")}
            aria-invalid={!!errors.email}
          />
          {isChecking && (
            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground animate-pulse">
              Checking...
            </span>
          )}
          {isEmailAvailable === false && (
            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-destructive">
              Email already taken
            </span>
          )}
          {isEmailAvailable === true && (
            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-green-600 dark:text-green-400">
              <CheckIcon className="size-3 inline" /> Available
            </span>
          )}
        </div>
        {errors.email && (
          <FieldError>
            <AlertCircleIcon className="size-3" /> {errors.email.message}
          </FieldError>
        )}
      </Field>
    </div>
  )
}

export function FormZodSection() {
  return (
    <Section
      id="form-zod"
      title="Form + Zod"
      group="examples"
      description="Comprehensive form validation using react-hook-form and zod."
      fileKey="forms"
    >
      <Demo
        title="Form · Full Validation"
        description="A complete form with react-hook-form and zod schema validation."
        center
      >
        <FormWithValidation />
      </Demo>

      <Demo
        title="Form · Async Validation"
        description="Demonstrates async field-level validation (e.g., email availability check)."
        center
      >
        <FormWithAsyncValidation />
      </Demo>
    </Section>
  )
}
