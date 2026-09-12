import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Demo, Section } from "../shared"

export function SonnerSection() {
  return (
<Section
        id="sonner"
        title="Sonner"
        group="examples"
        description="A succinct message that is displayed temporarily."
        fileKey="data"
      >
      {/* ========================================================================= */}
      {/* Sonner                                                                    */}
      {/* ========================================================================= */}
      <Demo
        title="Sonner · Types"
        description="Shows the different toast types."
        center      >
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            onClick={() => toast("Event has been created")}
          >
            Default
          </Button>
          <Button
            variant="outline"
            onClick={() => toast.success("Event has been created")}
          >
            Success
          </Button>
          <Button
            variant="outline"
            onClick={() => toast.info("Event has been created")}
          >
            Info
          </Button>
          <Button
            variant="outline"
            onClick={() => toast.warning("Event has been created")}
          >
            Warning
          </Button>
          <Button
            variant="outline"
            onClick={() => toast.error("Event has not been created")}
          >
            Error
          </Button>
        </div>
      </Demo>

      <Demo
        title="Sonner · Action"
        description="Adds an action button to a toast."
        center      >
        <Button
          variant="outline"
          onClick={() =>
            toast("Event has been created", {
              action: {
                label: "Undo",
                onClick: () => console.log("Undo"),
              },
            })
          }
        >
          Show Toast
        </Button>
      </Demo>

      <Demo
        title="Sonner · Promise"
        description="Renders a promise-backed toast lifecycle."
        center      >
        <Button
          variant="outline"
          onClick={() => {
            const promise = () =>
              new Promise<{ name: string }>((resolve) =>
                setTimeout(() => resolve({ name: "Sonner" }), 2000)
              )

            toast.promise(promise, {
              loading: "Loading...",
              success: (data: { name: string }) => {
                return `${data.name} toast has been added`
              },
              error: "Error",
            })
          }}
        >
          Show Toast
        </Button>
      </Demo>
      </Section>
  )
}
