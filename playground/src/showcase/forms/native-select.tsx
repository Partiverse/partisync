import {
  NativeSelect,
  NativeSelectOption,
  NativeSelectOptGroup,
} from "@/components/ui/native-select"
import { Demo, Section } from "../shared"

export function NativeSelectSection() {
  return (
    <Section
      id="native-select"
      title="Native Select"
      group="examples"
      description="A native select element with consistent styling."
      fileKey="forms"
    >
      <Demo
        title="Native Select · Basic"
        description="A basic native select with options."
        center
      >
        <div className="w-full max-w-xs">
          <NativeSelect>
            <NativeSelectOption value="">Select a framework...</NativeSelectOption>
            <NativeSelectOption value="react">React</NativeSelectOption>
            <NativeSelectOption value="vue">Vue</NativeSelectOption>
            <NativeSelectOption value="angular">Angular</NativeSelectOption>
            <NativeSelectOption value="svelte">Svelte</NativeSelectOption>
          </NativeSelect>
        </div>
      </Demo>

      <Demo
        title="Native Select · With OptGroup"
        description="A select with grouped options."
        center
      >
        <div className="w-full max-w-xs">
          <NativeSelect>
            <NativeSelectOptGroup label="Frontend">
              <NativeSelectOption value="react">React</NativeSelectOption>
              <NativeSelectOption value="vue">Vue</NativeSelectOption>
              <NativeSelectOption value="angular">Angular</NativeSelectOption>
            </NativeSelectOptGroup>
            <NativeSelectOptGroup label="Backend">
              <NativeSelectOption value="node">Node.js</NativeSelectOption>
              <NativeSelectOption value="python">Python</NativeSelectOption>
              <NativeSelectOption value="go">Go</NativeSelectOption>
            </NativeSelectOptGroup>
          </NativeSelect>
        </div>
      </Demo>

      <Demo
        title="Native Select · Disabled"
        description="A disabled select."
        center
      >
        <div className="w-full max-w-xs">
          <NativeSelect disabled>
            <NativeSelectOption value="">Select...</NativeSelectOption>
            <NativeSelectOption value="react">React</NativeSelectOption>
          </NativeSelect>
        </div>
      </Demo>
    </Section>
  )
}
