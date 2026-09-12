import * as React from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { Drawer, DrawerClose, DrawerContent, DrawerDescription, DrawerFooter, DrawerHeader, DrawerTitle, DrawerTrigger } from "@/components/ui/drawer"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Field, FieldContent, FieldDescription, FieldLabel, FieldTitle } from "@/components/ui/field"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Badge } from "@/components/ui/badge"
import { Demo, Section } from "../shared"

const SNAP_POINTS = ["31rem", 1]

const deliveryTimes = [
  {
    value: "asap",
    id: "delivery-asap",
    label: "Standard delivery",
    description: "25–35 min · Driver assigned now",
    badge: "Fastest",
  },
  {
    value: "5-00",
    id: "delivery-5-00",
    label: "5:00 PM – 5:15 PM",
    description: "Prep starts at 4:45 PM",
  },
  {
    value: "5-30",
    id: "delivery-5-30",
    label: "5:30 PM – 5:45 PM",
    description: "Good if you're heading home",
  },
  {
    value: "6-00",
    id: "delivery-6-00",
    label: "6:00 PM – 6:15 PM",
    description: "Most popular · High demand",
  },
  {
    value: "6-30",
    id: "delivery-6-30",
    label: "6:30 PM – 6:45 PM",
    description: "Last slot before kitchen closes",
  },
]

function ResponsiveProfileForm({ className }: React.ComponentProps<"form">) {
  return (
    <form className={cn("grid items-start gap-6", className)}>
      <div className="grid gap-3">
        <Label htmlFor="responsive-email">Email</Label>
        <Input type="email" id="responsive-email" defaultValue="shadcn@example.com" />
      </div>
      <div className="grid gap-3">
        <Label htmlFor="responsive-username">Username</Label>
        <Input id="responsive-username" defaultValue="@shadcn" />
      </div>
      <Button type="submit">Save changes</Button>
    </form>
  )
}

export function DrawerSection() {
  const [drawerDemoOpen, setDrawerDemoOpen] = React.useState(false)
  const [deliveryTime, setDeliveryTime] = React.useState("asap")
  const isMobile = false
  const isDesktop = true
  const swipeDirection: any = undefined
  const [responsiveOpen, setResponsiveOpen] = React.useState(false)
  const handleConfirmDelivery = () => { setDrawerDemoOpen(false) }

  return (
<Section
        id="drawer"
        title="Drawer"
        group="examples"
        description="A drawer component for React."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ======================== DRAWER ========================= */}
      {/* ========================================================= */}
      <Demo
        title="Drawer · Composition"
        description="A drawer for picking a delivery time with a radio group and confirm action."
        center
      >
        <Drawer
          open={drawerDemoOpen}
          onOpenChange={setDrawerDemoOpen}
          showSwipeHandle={isMobile}
          swipeDirection={isMobile ? "down" : "right"}
        >
          <DrawerTrigger render={<Button variant="secondary">Open Drawer</Button>} />
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Pick a delivery time</DrawerTitle>
              <DrawerDescription>
                We'll prepare your order as soon as possible.
              </DrawerDescription>
            </DrawerHeader>
            <div className="flex-1 scroll-fade overflow-y-auto p-4">
              <RadioGroup
                value={deliveryTime}
                onValueChange={setDeliveryTime}
                className="gap-2"
              >
                {deliveryTimes.map((time) => (
                  <FieldLabel key={time.value} htmlFor={time.id}>
                    <Field orientation="horizontal">
                      <FieldContent>
                        <FieldTitle className="flex items-center gap-2">
                          {time.label}
                          {time.badge ? (
                            <Badge variant="secondary">{time.badge}</Badge>
                          ) : null}
                        </FieldTitle>
                        <FieldDescription>{time.description}</FieldDescription>
                      </FieldContent>
                      <RadioGroupItem value={time.value} id={time.id} />
                    </Field>
                  </FieldLabel>
                ))}
              </RadioGroup>
            </div>
            <DrawerFooter>
              <Button onClick={handleConfirmDelivery} className="h-[34px]">
                Confirm Delivery Time
              </Button>
              <DrawerClose render={<Button variant="outline">Cancel</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Custom Sizes"
        description="A vertical drawer with a custom height set via height utilities on DrawerContent."
        center
      >
        <Drawer swipeDirection="down">
          <DrawerTrigger render={<Button variant="outline">Custom Height (50vh)</Button>} />
          <DrawerContent className="h-[50vh]">
            <DrawerHeader>
              <DrawerTitle>Custom Height Drawer</DrawerTitle>
              <DrawerDescription>
                This vertical drawer has a custom height of 50vh.
              </DrawerDescription>
            </DrawerHeader>
            <div className="flex-1 overflow-y-auto p-4">
              <p className="text-sm text-muted-foreground">
                To customize the height of a vertical drawer, use the h-* and max-h-* utilities on DrawerContent.
              </p>
            </div>
            <DrawerFooter>
              <DrawerClose render={<Button variant="outline">Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Styling"
        description="A drawer customized with CSS variables such as --drawer-inset."
        center
      >
        <Drawer swipeDirection="down">
          <DrawerTrigger render={<Button variant="outline">Inset Styling Drawer</Button>} />
          <DrawerContent className="[--drawer-inset:16px] rounded-xl border">
            <DrawerHeader>
              <DrawerTitle>Styled Drawer</DrawerTitle>
              <DrawerDescription>
                Customized using CSS variables such as --drawer-inset.
              </DrawerDescription>
            </DrawerHeader>
            <div className="p-4">
              <p className="text-sm text-muted-foreground">
                The drawer exposes CSS variables for style-level customization like --drawer-inset, --drawer-bleed-background, and --drawer-overlay-min-opacity.
              </p>
            </div>
            <DrawerFooter>
              <DrawerClose render={<Button variant="outline">Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Position"
        description="A drawer that opens from the left side of the viewport."
        center
      >
        <Drawer swipeDirection="left">
          <DrawerTrigger render={<Button variant="secondary">Open Left Drawer</Button>} />
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Move Goal</DrawerTitle>
              <DrawerDescription>Set your daily activity goal.</DrawerDescription>
            </DrawerHeader>
            <div className="flex-1 p-4">
              <div className="size-full rounded-2xl bg-muted" />
            </div>
            <DrawerFooter>
              <DrawerClose render={<Button>Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Swipe Handle"
        description="A drawer rendered with a visible swipe handle."
        center
      >
        <Drawer showSwipeHandle>
          <DrawerTrigger render={<Button variant="secondary">Open Drawer</Button>} />
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Drawer</DrawerTitle>
              <DrawerDescription>Drawer with a swipe handle.</DrawerDescription>
            </DrawerHeader>
            <div className="flex-1 p-4">
              <div className="rounded-2xl bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:h-80 group-data-[swipe-axis=y]/drawer-popup:w-full" />
            </div>
            <DrawerFooter>
              <DrawerClose render={<Button>Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Nested"
        description="Drawers stacked on top of each other, opened from the same direction."
        center
      >
        <Drawer showSwipeHandle={isMobile} swipeDirection={swipeDirection}>
          <DrawerTrigger render={<Button variant="secondary">Open Drawer</Button>} />
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Drawer</DrawerTitle>
              <DrawerDescription>
                Open another drawer from the same direction.
              </DrawerDescription>
            </DrawerHeader>
            <div className="flex-1 p-4">
              <div className="bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:aspect-video group-data-[swipe-axis=y]/drawer-popup:w-full" />
            </div>
            <DrawerFooter>
              <Drawer showSwipeHandle={isMobile} swipeDirection={swipeDirection}>
                <DrawerTrigger render={<Button variant="outline">Open Nested Drawer</Button>} />
                <DrawerContent>
                  <DrawerHeader>
                    <DrawerTitle>Nested Drawer</DrawerTitle>
                    <DrawerDescription>
                      The parent drawer stays mounted behind this one.
                    </DrawerDescription>
                  </DrawerHeader>
                  <div className="flex-1 p-4">
                    <div className="bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:aspect-video group-data-[swipe-axis=y]/drawer-popup:w-full" />
                  </div>
                  <DrawerFooter>
                    <Drawer
                      showSwipeHandle={isMobile}
                      swipeDirection={swipeDirection}
                    >
                      <DrawerTrigger render={<Button variant="outline">Open Third Drawer</Button>} />
                      <DrawerContent>
                        <DrawerHeader>
                          <DrawerTitle>Third Drawer</DrawerTitle>
                          <DrawerDescription>
                            Two drawers are stacked behind this one.
                          </DrawerDescription>
                        </DrawerHeader>
                        <div className="flex-1 p-4">
                          <div className="bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:aspect-video group-data-[swipe-axis=y]/drawer-popup:w-full" />
                        </div>
                        <DrawerFooter>
                          <Drawer
                            showSwipeHandle={isMobile}
                            swipeDirection={swipeDirection}
                          >
                            <DrawerTrigger render={<Button variant="outline">Open Fourth Drawer</Button>} />
                            <DrawerContent>
                              <DrawerHeader>
                                <DrawerTitle>Fourth Drawer</DrawerTitle>
                                <DrawerDescription>
                                  This is the frontmost drawer in the stack.
                                </DrawerDescription>
                              </DrawerHeader>
                              <div className="flex-1 p-4">
                                <div className="bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:aspect-video group-data-[swipe-axis=y]/drawer-popup:w-full" />
                              </div>
                              <DrawerFooter>
                                <DrawerClose render={<Button variant="outline">Close</Button>} />
                              </DrawerFooter>
                            </DrawerContent>
                          </Drawer>
                          <DrawerClose render={<Button variant="outline">Close</Button>} />
                        </DrawerFooter>
                      </DrawerContent>
                    </Drawer>
                    <DrawerClose render={<Button variant="outline">Close</Button>} />
                  </DrawerFooter>
                </DrawerContent>
              </Drawer>
              <DrawerClose render={<Button variant="outline">Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Non Modal"
        description="A non-modal drawer that lets the rest of the page stay interactive."
        center
      >
        <Drawer modal={false} swipeDirection="right">
          <DrawerTrigger render={<Button variant="outline">Non Modal</Button>} />
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Non Modal Drawer</DrawerTitle>
            </DrawerHeader>
            <div className="flex-1 p-4">
              <div className="rounded-2xl bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:h-80 group-data-[swipe-axis=y]/drawer-popup:w-full" />
            </div>
            <DrawerFooter>
              <DrawerClose render={<Button>Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Snap Points"
        description="A drawer that snaps between a compact peek and a near full-height view."
        center
      >
        <Drawer snapPoints={SNAP_POINTS} showSwipeHandle>
          <DrawerTrigger render={<Button variant="outline">Open Snap Drawer</Button>} />
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Snap points</DrawerTitle>
              <DrawerDescription>
                Drag the drawer to snap between a compact peek and a near full-height view.
              </DrawerDescription>
            </DrawerHeader>
            <div className="flex-1 p-4">
              <div className="rounded-2xl bg-muted group-data-[swipe-axis=x]/drawer-popup:size-full group-data-[swipe-axis=y]/drawer-popup:h-80 group-data-[swipe-axis=y]/drawer-popup:w-full" />
            </div>
            <DrawerFooter>
              <DrawerClose render={<Button>Close</Button>} />
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </Demo>

      <Demo
        title="Drawer · Responsive"
        description="A responsive edit-profile form that renders as a dialog on desktop and a drawer on mobile."
        center
      >
        {isDesktop ? (
          <Dialog open={responsiveOpen} onOpenChange={setResponsiveOpen}>
            <DialogTrigger render={<Button variant="outline">Edit Profile</Button>} />
            <DialogContent className="sm:max-w-[425px]">
              <DialogHeader>
                <DialogTitle>Edit profile</DialogTitle>
                <DialogDescription>
                  Make changes to your profile here. Click save when you're done.
                </DialogDescription>
              </DialogHeader>
              <ResponsiveProfileForm />
            </DialogContent>
          </Dialog>
        ) : (
          <Drawer open={responsiveOpen} onOpenChange={setResponsiveOpen}>
            <DrawerTrigger render={<Button variant="outline">Edit Profile</Button>} />
            <DrawerContent>
              <DrawerHeader className="text-left">
                <DrawerTitle>Edit profile</DrawerTitle>
                <DrawerDescription>
                  Make changes to your profile here. Click save when you're done.
                </DrawerDescription>
              </DrawerHeader>
              <ResponsiveProfileForm className="p-4" />
            </DrawerContent>
          </Drawer>
        )}
      </Demo>

      </Section>
  )
}
