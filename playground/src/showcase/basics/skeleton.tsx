import { Skeleton } from "@/components/ui/skeleton"
import { Demo, Section } from "../shared"

export function SkeletonSection() {
  return (
<Section
        id="skeleton"
        title="Skeleton"
        group="installation"
        description="Use to show a placeholder while content is loading."
        fileKey="basics"
      >

      <Demo
        title="Skeleton · Avatar"
        description="Skeletons can represent an avatar with text lines."      >
        <div className="flex w-full items-center gap-4">
          <Skeleton className="size-12 rounded-full" />
          <div className="w-full space-y-2">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-1/2" />
          </div>
        </div>
      </Demo>

      <Demo
        title="Skeleton · Card"
        description="Skeletons can represent the layout of a card."      >
        <div className="flex w-full flex-col gap-3">
          <Skeleton className="h-32 w-full rounded-lg" />
          <div className="space-y-2">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-1/2" />
          </div>
        </div>
      </Demo>

      <Demo
        title="Skeleton · Text"
        description="Skeletons can represent lines of text."      >
        <div className="w-full space-y-2">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-5/6" />
          <Skeleton className="h-4 w-4/6" />
          <Skeleton className="h-4 w-3/6" />
        </div>
      </Demo>

      <Demo
        title="Skeleton · Form"
        description="Skeletons can represent a form while it loads."      >
        <div className="w-full space-y-3">
          <div className="space-y-2">
            <Skeleton className="h-4 w-20" />
            <Skeleton className="h-9 w-full" />
          </div>
          <div className="space-y-2">
            <Skeleton className="h-4 w-20" />
            <Skeleton className="h-9 w-full" />
          </div>
          <Skeleton className="h-9 w-24" />
        </div>
      </Demo>

      <Demo
        title="Skeleton · Table"
        description="Skeletons can represent the rows of a table."      >
        <div className="w-full space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-8 w-full" />
          ))}
        </div>
      </Demo>
      </Section>
  )
}
