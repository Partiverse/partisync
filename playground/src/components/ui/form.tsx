"use client"

import * as React from "react"
import { Form as FormPrimitive } from "@base-ui/react/form"
import { Field } from "@base-ui/react/field"
import { cn } from "cn"

function Form({
  className,
  ...props
}: FormPrimitive.Props) {
  return (
    <FormPrimitive
      data-slot="form"
      className={cn("flex flex-col gap-4", className)}
      {...props}
    />
  )
}

function FormField({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <Field.Root
      data-slot="form-field"
      className={cn("flex flex-col gap-1.5", className)}
      {...props}
    />
  )
}

function FormLabel({
  className,
  ...props
}: Field.Label.Props) {
  return (
    <Field.Label
      data-slot="form-label"
      className={cn("text-sm font-medium leading-snug", className)}
      {...props}
    />
  )
}

function FormControl({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="form-control"
      className={cn("flex flex-col gap-1", className)}
      {...props}
    />
  )
}

function FormDescription({
  className,
  ...props
}: Field.Description.Props) {
  return (
    <Field.Description
      data-slot="form-description"
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  )
}

function FormMessage({
  className,
  children,
  ...props
}: Field.Error.Props & { children?: React.ReactNode }) {
  return (
    <Field.Error
      data-slot="form-message"
      className={cn("text-sm text-destructive", className)}
      {...props}
    >
      {children}
    </Field.Error>
  )
}

function FormSubmit({
  className,
  render,
  ...props
}: React.ComponentProps<"button"> & { render?: React.ReactElement }) {
  if (render) {
    return React.cloneElement(render, {
      type: "submit",
      "data-slot": "form-submit",
      className: cn("text-sm font-medium", className, render.props.className),
      ...props,
    })
  }
  return (
    <button
      type="submit"
      data-slot="form-submit"
      className={cn("text-sm font-medium", className)}
      {...props}
    />
  )
}

export {
  Form,
  FormField,
  FormLabel,
  FormControl,
  FormDescription,
  FormMessage,
  FormSubmit,
}
