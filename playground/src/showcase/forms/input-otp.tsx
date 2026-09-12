import * as React from "react"
import { InputOTP, InputOTPGroup, InputOTPSeparator, InputOTPSlot } from "@/components/ui/input-otp"
import { Demo, Section } from "../shared"

export function InputOtpSection() {
  const [otp, setOtp] = React.useState("")
  const [otpControlled, setOtpControlled] = React.useState("123456")

  return (
<Section
        id="input-otp"
        title="Input OTP"
        group="examples"
        description="Accessible one-time password component with copy-paste functionality."
        fileKey="forms"
      >
      {/* ============ InputOTP · Official H2 demos ============ */}
      <Demo
        title="InputOTP · Composition"
        description="InputOTP renders grouped slots with a separator."      >
        <InputOTP maxLength={6} value={otp} onChange={(v: string) => setOtp(v)}>
          <InputOTPGroup>
            <InputOTPSlot index={0} />
            <InputOTPSlot index={1} />
            <InputOTPSlot index={2} />
          </InputOTPGroup>
          <InputOTPSeparator />
          <InputOTPGroup>
            <InputOTPSlot index={3} />
            <InputOTPSlot index={4} />
            <InputOTPSlot index={5} />
          </InputOTPGroup>
        </InputOTP>
        <p className="text-center text-xs text-muted-foreground">
          {otp.length === 6 ? "✓ Complete" : "Enter your 6-digit code"}
        </p>
      </Demo>

      <Demo
        title="InputOTP · Four Digits"
        description="Set maxLength to control the number of slots."      >
        <InputOTP maxLength={4}>
          <InputOTPGroup>
            <InputOTPSlot index={0} />
            <InputOTPSlot index={1} />
            <InputOTPSlot index={2} />
            <InputOTPSlot index={3} />
          </InputOTPGroup>
        </InputOTP>
      </Demo>

      <Demo
        title="InputOTP · Controlled"
        description="The OTP value can be controlled with state."      >
        <InputOTP maxLength={4} value={otpControlled} onChange={(v: string) => setOtpControlled(v)}>
          <InputOTPGroup>
            <InputOTPSlot index={0} />
            <InputOTPSlot index={1} />
            <InputOTPSlot index={2} />
            <InputOTPSlot index={3} />
          </InputOTPGroup>
        </InputOTP>
        <p className="text-xs text-muted-foreground">Value: {otpControlled || "(empty)"}</p>
      </Demo>

      <Demo
        title="InputOTP · Disabled"
        description="Use the disabled attribute to prevent interaction."      >
        <InputOTP maxLength={4} disabled>
          <InputOTPGroup>
            <InputOTPSlot index={0} />
            <InputOTPSlot index={1} />
            <InputOTPSlot index={2} />
            <InputOTPSlot index={3} />
          </InputOTPGroup>
        </InputOTP>
      </Demo>

      <Demo
        title="InputOTP · Separator"
        description="InputOTPSeparator divides groups of slots."      >
        <InputOTP maxLength={6}>
          <InputOTPGroup>
            <InputOTPSlot index={0} />
            <InputOTPSlot index={1} />
            <InputOTPSlot index={2} />
          </InputOTPGroup>
          <InputOTPSeparator />
          <InputOTPGroup>
            <InputOTPSlot index={3} />
            <InputOTPSlot index={4} />
            <InputOTPSlot index={5} />
          </InputOTPGroup>
        </InputOTP>
      </Demo>

      </Section>
  )
}
