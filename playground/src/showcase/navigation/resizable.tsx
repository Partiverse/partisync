import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { Demo, Section } from "../shared"

export function ResizableSection() {
  return (
<Section
        id="resizable"
        title="Resizable"
        group="examples"
        description="Accessible resizable panel groups and layouts with keyboard support."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* RESIZABLE                                                                 */}
      {/* ========================================================================= */}

      {/* Resizable · Composition */}
      <Demo
        title="Resizable · Composition"
        description="Nested resizable panels arranged horizontally and vertically."
      >
        <ResizablePanelGroup
          orientation="horizontal"
          className="max-w-md min-h-[200px] rounded-lg border"
        >
          <ResizablePanel defaultSize={50}>
            <div className="flex h-full items-center justify-center p-6">
              <span className="font-semibold text-sm">One</span>
            </div>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel defaultSize={50}>
            <ResizablePanelGroup orientation="vertical">
              <ResizablePanel defaultSize={50}>
                <div className="flex h-full items-center justify-center p-6">
                  <span className="font-semibold text-sm">Two</span>
                </div>
              </ResizablePanel>
              <ResizableHandle />
              <ResizablePanel defaultSize={50}>
                <div className="flex h-full items-center justify-center p-6">
                  <span className="font-semibold text-sm">Three</span>
                </div>
              </ResizablePanel>
            </ResizablePanelGroup>
          </ResizablePanel>
        </ResizablePanelGroup>
      </Demo>

      {/* Resizable · Vertical */}
      <Demo
        title="Resizable · Vertical"
        description="A vertically resizable panel group with header and content areas."
      >
        <ResizablePanelGroup
          orientation="vertical"
          className="max-w-md min-h-[200px] rounded-lg border"
        >
          <ResizablePanel defaultSize={25}>
            <div className="flex h-full items-center justify-center p-6">
              <span className="font-semibold text-sm">Header</span>
            </div>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel defaultSize={75}>
            <div className="flex h-full items-center justify-center p-6">
              <span className="font-semibold text-sm">Content</span>
            </div>
          </ResizablePanel>
        </ResizablePanelGroup>
      </Demo>

      {/* Resizable · Handle */}
      <Demo
        title="Resizable · Handle"
        description="Resizable panels with a visible drag handle between them."
      >
        <ResizablePanelGroup
          orientation="horizontal"
          className="max-w-md min-h-[200px] rounded-lg border"
        >
          <ResizablePanel defaultSize={50}>
            <div className="flex h-full items-center justify-center p-6">
              <span className="font-semibold text-sm">Sidebar</span>
            </div>
          </ResizablePanel>
          <ResizableHandle withHandle />
          <ResizablePanel defaultSize={50}>
            <div className="flex h-full items-center justify-center p-6">
              <span className="font-semibold text-sm">Content</span>
            </div>
          </ResizablePanel>
        </ResizablePanelGroup>
      </Demo>
      </Section>
  )
}
