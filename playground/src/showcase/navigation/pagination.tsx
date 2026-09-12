import { Pagination, PaginationContent, PaginationEllipsis, PaginationItem, PaginationLink, PaginationNext, PaginationPrevious } from "@/components/ui/pagination"
import { Demo, Section } from "../shared"

export function PaginationSection() {
  return (
<Section
        id="pagination"
        title="Pagination"
        group="examples"
        description="Pagination with page navigation, next and previous links."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* PAGINATION                                                                */}
      {/* ========================================================================= */}

      {/* Pagination · Composition */}
      <Demo
        title="Pagination · Composition"
        description="A pagination composed with previous/next links, page numbers, and an ellipsis."
      >
        <Pagination>
          <PaginationContent>
            <PaginationItem>
              <PaginationPrevious href="#" />
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#">1</PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#" isActive>
                2
              </PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#">3</PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationEllipsis />
            </PaginationItem>
            <PaginationItem>
              <PaginationNext href="#" />
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </Demo>

      {/* Pagination · Simple */}
      <Demo
        title="Pagination · Simple"
        description="A minimal pagination with only previous and next controls."
      >
        <Pagination>
          <PaginationContent>
            <PaginationItem>
              <PaginationPrevious href="#" />
            </PaginationItem>
            <PaginationItem>
              <PaginationNext href="#" />
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </Demo>

      {/* Pagination · Icons Only */}
      <Demo
        title="Pagination · Icons Only"
        description="A pagination using icon-only links with accessible aria labels."
      >
        <Pagination>
          <PaginationContent>
            <PaginationItem>
              <PaginationLink href="#" aria-label="First page">
                «
              </PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#" aria-label="Previous page">
                ‹
              </PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#" isActive>
                3
              </PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#" aria-label="Next page">
                ›
              </PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#" aria-label="Last page">
                »
              </PaginationLink>
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </Demo>

      {/* Pagination · Next.js */}
      <Demo
        title="Pagination · Next.js"
        description="A pagination with custom link text, ready for a Next.js router."
      >
        <Pagination>
          <PaginationContent>
            <PaginationItem>
              <PaginationPrevious href="#page=1" text="Previous" />
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#page=1">1</PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#page=2" isActive>
                2
              </PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationLink href="#page=3">3</PaginationLink>
            </PaginationItem>
            <PaginationItem>
              <PaginationEllipsis />
            </PaginationItem>
            <PaginationItem>
              <PaginationNext href="#page=3" text="Next" />
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </Demo>

      </Section>
  )
}
