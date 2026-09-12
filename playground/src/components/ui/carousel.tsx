"use client"

import * as React from "react"
import useEmblaCarousel from "embla-carousel-react"
import type {
  EmblaCarouselType,
  EmblaOptionsType,
  EmblaPluginType,
} from "embla-carousel"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ArrowLeftIcon, ArrowRightIcon } from "lucide-react"

type CarouselApi = EmblaCarouselType

interface CarouselContextValue {
  api: CarouselApi | undefined
  selectedIndex: number
  scrollSnaps: number[]
  canScrollNext: boolean
  canScrollPrev: boolean
  scrollNext: () => void
  scrollPrev: () => void
  scrollTo: (index: number) => void
}

const CarouselContext = React.createContext<CarouselContextValue | null>(null)

function useCarouselContext() {
  const context = React.useContext(CarouselContext)
  if (!context) {
    throw new Error("useCarousel must be used within a CarouselProvider")
  }
  return context
}

function CarouselProvider({
  children,
  api,
}: {
  children: React.ReactNode
  api: CarouselApi | undefined
}) {
  const [selectedIndex, setSelectedIndex] = React.useState(0)
  const [scrollSnaps, setScrollSnaps] = React.useState<number[]>([])
  const [canScrollNext, setCanScrollNext] = React.useState(false)
  const [canScrollPrev, setCanScrollPrev] = React.useState(false)

  const scrollTo = React.useCallback(
    (index: number) => {
      api?.scrollTo(index)
    },
    [api]
  )

  const scrollNext = React.useCallback(() => {
    api?.scrollNext()
  }, [api])

  const scrollPrev = React.useCallback(() => {
    api?.scrollPrev()
  }, [api])

  React.useEffect(() => {
    if (!api) return

    const onSelect = () => {
      setSelectedIndex(api.selectedScrollSnap())
      setCanScrollNext(api.canScrollNext())
      setCanScrollPrev(api.canScrollPrev())
    }

    const onReInit = () => {
      setScrollSnaps(api.scrollSnapList())
      setCanScrollNext(api.canScrollNext())
      setCanScrollPrev(api.canScrollPrev())
    }

    api.on("select", onSelect)
    api.on("reInit", onReInit)

    // initial
    setScrollSnaps(api.scrollSnapList())
    setSelectedIndex(api.selectedScrollSnap())
    setCanScrollNext(api.canScrollNext())
    setCanScrollPrev(api.canScrollPrev())

    return () => {
      api.off("select", onSelect)
      api.off("reInit", onReInit)
    }
  }, [api])

  return (
    <CarouselContext.Provider
      value={{
        api,
        selectedIndex,
        scrollSnaps,
        canScrollNext,
        canScrollPrev,
        scrollNext,
        scrollPrev,
        scrollTo,
      }}
    >
      {children}
    </CarouselContext.Provider>
  )
}

function Carousel({
  className,
  opts,
  plugins,
  children,
}: {
  className?: string
  opts?: EmblaOptionsType
  plugins?: EmblaPluginType[]
  children: React.ReactNode
}) {
  const [emblaRef, api] = useEmblaCarousel(opts, plugins)

  return (
    <CarouselProvider api={api}>
      <div
        ref={emblaRef as unknown as React.Ref<HTMLDivElement>}
        className={cn("relative", className)}
      >
        {children}
      </div>
    </CarouselProvider>
  )
}

function CarouselContent({
  className,
  children,
}: {
  className?: string
  children: React.ReactNode
}) {
  return (
    <div
      data-slot="carousel-content"
      className={cn("flex touch-none", className)}
    >
      {children}
    </div>
  )
}

function CarouselItem({
  className,
  children,
}: {
  className?: string
  children: React.ReactNode
}) {
  return (
    <div
      data-slot="carousel-item"
      className={cn(
        "min-w-0 shrink-0 grow-0 basis-full pl-4 first:pl-0",
        className
      )}
    >
      {children}
    </div>
  )
}

function CarouselPrevious({
  className,
  variant = "outline",
  size = "icon",
  ...props
}: React.ComponentProps<typeof Button> & {
  variant?: React.ComponentProps<typeof Button>["variant"]
  size?: React.ComponentProps<typeof Button>["size"]
}) {
  const { canScrollPrev, scrollPrev } = useCarouselContext()

  return (
    <Button
      data-slot="carousel-previous"
      variant={variant}
      size={size}
      className={cn(
        "absolute left-2 top-1/2 z-10 -translate-y-1/2",
        className
      )}
      disabled={!canScrollPrev}
      onClick={scrollPrev}
      {...props}
    >
      <ArrowLeftIcon className="size-4" />
      <span className="sr-only">Previous slide</span>
    </Button>
  )
}

function CarouselNext({
  className,
  variant = "outline",
  size = "icon",
  ...props
}: React.ComponentProps<typeof Button> & {
  variant?: React.ComponentProps<typeof Button>["variant"]
  size?: React.ComponentProps<typeof Button>["size"]
}) {
  const { canScrollNext, scrollNext } = useCarouselContext()

  return (
    <Button
      data-slot="carousel-next"
      variant={variant}
      size={size}
      className={cn(
        "absolute right-2 top-1/2 z-10 -translate-y-1/2",
        className
      )}
      disabled={!canScrollNext}
      onClick={scrollNext}
      {...props}
    >
      <ArrowRightIcon className="size-4" />
      <span className="sr-only">Next slide</span>
    </Button>
  )
}

function CarouselDots({ className }: { className?: string }) {
  const { scrollSnaps, selectedIndex, scrollTo } = useCarouselContext()

  return (
    <div
      data-slot="carousel-dots"
      className={cn("flex items-center justify-center gap-1.5", className)}
    >
      {scrollSnaps.map((_, index) => (
        <button
          key={index}
          data-slot="carousel-dot"
          className={cn(
            "h-1.5 w-1.5 rounded-full transition-all",
            index === selectedIndex
              ? "w-4 bg-foreground"
              : "bg-muted-foreground/30 hover:bg-muted-foreground/50"
          )}
          onClick={() => scrollTo(index)}
          aria-label={`Go to slide ${index + 1}`}
        />
      ))}
    </div>
  )
}

export {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselPrevious,
  CarouselNext,
  CarouselDots,
}
