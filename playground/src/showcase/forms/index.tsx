export { FieldSection } from "./field"
export { InputSection } from "./input"
export { InputGroupSection } from "./input-group"
export { SelectSection } from "./select"
export { CheckboxSection } from "./checkbox"
export { RadioGroupSection } from "./radio-group"
export { SwitchSection } from "./switch"
export { SliderSection } from "./slider"
export { TextareaSection } from "./textarea"
export { InputOtpSection } from "./input-otp"
export { ComboboxSection } from "./combobox"
export { FormSection } from "./form"
export { FormZodSection } from "./form-zod"
export { NativeSelectSection } from "./native-select"
import { FieldSection } from "./field"
import { InputSection } from "./input"
import { InputGroupSection } from "./input-group"
import { SelectSection } from "./select"
import { CheckboxSection } from "./checkbox"
import { RadioGroupSection } from "./radio-group"
import { SwitchSection } from "./switch"
import { SliderSection } from "./slider"
import { TextareaSection } from "./textarea"
import { InputOtpSection } from "./input-otp"
import { ComboboxSection } from "./combobox"
import { FormSection } from "./form"
import { FormZodSection } from "./form-zod"
import { NativeSelectSection } from "./native-select"

export default function FormsSection() {
  return (
    <div className="flex flex-col gap-12">
      <FieldSection />
      <InputSection />
      <InputGroupSection />
      <SelectSection />
      <CheckboxSection />
      <RadioGroupSection />
      <SwitchSection />
      <SliderSection />
      <TextareaSection />
      <InputOtpSection />
      <ComboboxSection />
      <FormSection />
      <FormZodSection />
      <NativeSelectSection />
    </div>
  )
}
