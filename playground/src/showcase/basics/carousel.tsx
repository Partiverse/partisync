import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
  CarouselDots,
} from "@/components/ui/carousel"
import { Card, CardContent } from "@/components/ui/card"
import { Avatar, AvatarImage, AvatarFallback } from "@/components/ui/avatar"
import { Demo, Section } from "../shared"

function SampleSlide({
  index,
  label,
  description,
  className,
}: {
  index: number
  label: string
  description: string
  className?: string
}) {
  return (
    <Card className={cn("h-64 w-full", className)}>
      <CardContent className="flex h-full flex-col items-center justify-center gap-3 p-6">
        <Avatar className="size-16">
          <AvatarImage src={`https://api.dicebear.com/9.x/lorelei/svg?seed=${index}`} />
          <AvatarFallback>{index}</AvatarFallback>
        </Avatar>
        <div className="text-center">
          <p className="font-medium">{label}</p>
          <p className="text-sm text-muted-foreground">{description}</p>
        </div>
      </CardContent>
    </Card>
  )
}

// Minimal cn helper for this file
import { cn } from "cn"

export function CarouselSection() {
  return (
    <Section
      id="carousel"
      title="Carousel"
      group="installation"
      description="A carousel for cycling through elements."
      fileKey="basics"
    >
      {/* ========================================================= */}
      {/* ===================== CAROUSEL ========================== */}
      {/* ========================================================= */}

      <Demo
        title="Carousel · Default"
        description="A default carousel with previous/next buttons and dots."
        center
      >
        <div className="w-full max-w-xs">
          <Carousel
            opts={{
              align: "start",
              loop: true,
            }}
          >
            <CarouselContent>
              {Array.from({ length: 5 }).map((_, i) => (
                <CarouselItem key={i}>
                  <SampleSlide
                    index={i + 1}
                    label={`Slide ${i + 1}`}
                    description="A brief description"
                  />
                </CarouselItem>
              ))}
            </CarouselContent>
            <div className="flex items-center justify-between gap-4 pt-4">
              <CarouselPrevious />
              <CarouselDots />
              <CarouselNext />
            </div>
          </Carousel>
        </div>
      </Demo>

      <Demo
        title="Carousel · No Loop"
        description="A carousel that stops at the end."
        center
      >
        <div className="w-full max-w-xs">
          <Carousel
            opts={{
              align: "start",
              loop: false,
            }}
          >
            <CarouselContent>
              {Array.from({ length: 3 }).map((_, i) => (
                <CarouselItem key={i}>
                  <SampleSlide
                    index={i + 1}
                    label={`Slide ${i + 1}`}
                    description="No loop mode"
                  />
                </CarouselItem>
              ))}
            </CarouselContent>
            <div className="flex items-center justify-between gap-4 pt-4">
              <CarouselPrevious />
              <CarouselDots />
              <CarouselNext />
            </div>
          </Carousel>
        </div>
      </Demo>

      <Demo
        title="Carousel · Center Align"
        description="Center-aligned carousel."
        center
      >
        <div className="w-full max-w-sm">
          <Carousel
            opts={{
              align: "center",
              loop: true,
            }}
          >
            <CarouselContent>
              {Array.from({ length: 5 }).map((_, i) => (
                <CarouselItem key={i}>
                  <SampleSlide
                    index={i + 1}
                    label={`Slide ${i + 1}`}
                    description="Center mode"
                  />
                </CarouselItem>
              ))}
            </CarouselContent>
            <div className="flex items-center justify-between gap-4 pt-4">
              <CarouselPrevious />
              <CarouselDots />
              <CarouselNext />
            </div>
          </Carousel>
        </div>
      </Demo>

      <Demo
        title="Carousel · Without Dots"
        description="A carousel without pagination dots."
        center
      >
        <div className="w-full max-w-xs">
          <Carousel
            opts={{
              align: "start",
              loop: true,
            }}
          >
            <CarouselContent>
              {Array.from({ length: 4 }).map((_, i) => (
                <CarouselItem key={i}>
                  <SampleSlide
                    index={i + 1}
                    label={`Slide ${i + 1}`}
                    description="No dots"
                  />
                </CarouselItem>
              ))}
            </CarouselContent>
            <div className="flex items-center justify-end gap-2 pt-4">
              <CarouselPrevious />
              <CarouselNext />
            </div>
          </Carousel>
        </div>
      </Demo>
    </Section>
  )
}
