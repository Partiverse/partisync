export { ButtonSection } from "./button"
export { ButtonGroupSection } from "./button-group"
export { BadgeSection } from "./badge"
export { CardSection } from "./card"
export { SeparatorSection } from "./separator"
export { SkeletonSection } from "./skeleton"
export { SpinnerSection } from "./spinner"
export { KbdSection } from "./kbd"
export { AspectRatioSection } from "./aspect-ratio"
export { EmptySection } from "./empty"
export { AvatarSection } from "./avatar"
export { AlertSection } from "./alert"
export { CarouselSection } from "./carousel"
export { TypographySection } from "./typography"
export { ItemSection } from "./item"
import { ButtonSection } from "./button"
import { ButtonGroupSection } from "./button-group"
import { BadgeSection } from "./badge"
import { CardSection } from "./card"
import { SeparatorSection } from "./separator"
import { SkeletonSection } from "./skeleton"
import { SpinnerSection } from "./spinner"
import { KbdSection } from "./kbd"
import { AspectRatioSection } from "./aspect-ratio"
import { EmptySection } from "./empty"
import { AvatarSection } from "./avatar"
import { AlertSection } from "./alert"
import { CarouselSection } from "./carousel"
import { TypographySection } from "./typography"
import { ItemSection } from "./item"

export default function BasicsSection() {
  return (
    <div className="flex flex-col gap-12">
      <ButtonSection />
      <ButtonGroupSection />
      <BadgeSection />
      <CardSection />
      <SeparatorSection />
      <SkeletonSection />
      <SpinnerSection />
      <KbdSection />
      <AspectRatioSection />
      <EmptySection />
      <AvatarSection />
      <AlertSection />
      <CarouselSection />
      <TypographySection />
      <ItemSection />
    </div>
  )
}
