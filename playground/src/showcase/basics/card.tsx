import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Demo, Section } from "../shared"

export function CardSection() {
  return (
<Section
        id="card"
        title="Card"
        group="examples"
        description="Displays a card with header, content, and footer."
        fileKey="basics"
      >

      <Demo
        title="Card · Composition"
        description="Cards are composed from header, content, and footer parts."      >
        <div className="w-full space-y-3">
          <Card>
            <CardHeader>
              <CardTitle>Create a new project</CardTitle>
              <CardDescription>Deploy your new project in one-click.</CardDescription>
              <CardAction>
                <Badge variant="secondary">Beta</Badge>
              </CardAction>
            </CardHeader>
            <CardContent>
              <p>Card content area.</p>
            </CardContent>
            <CardFooter className="justify-end gap-2">
              <Button variant="ghost" size="sm">Cancel</Button>
              <Button size="sm">Deploy</Button>
            </CardFooter>
          </Card>
        </div>
      </Demo>

      <Demo
        title="Card · Size"
        description="Card supports default and small sizes."      >
        <div className="w-full space-y-3">
          <Card>
            <CardHeader>
              <CardTitle>Default size</CardTitle>
              <CardDescription>Standard padding.</CardDescription>
            </CardHeader>
            <CardContent>
              <p>Content with --card-spacing=4.</p>
            </CardContent>
            <CardFooter>
              <Button size="sm">Action</Button>
            </CardFooter>
          </Card>
          <Card size="sm">
            <CardHeader>
              <CardTitle>Small size</CardTitle>
              <CardDescription>Compact padding.</CardDescription>
            </CardHeader>
            <CardContent>
              <p>Content with --card-spacing=3.</p>
            </CardContent>
            <CardFooter>
              <Button size="sm">Action</Button>
            </CardFooter>
          </Card>
        </div>
      </Demo>

      <Demo
        title="Card · Spacing"
        description="Card padding is controlled by the --card-spacing variable."      >
        <div className="w-full space-y-3">
          <Card className="[--card-spacing:--spacing(2)]">
            <CardHeader>
              <CardTitle>Tight</CardTitle>
              <CardDescription>spacing=2 (0.5rem)</CardDescription>
            </CardHeader>
            <CardContent>
              <p>Compact layout.</p>
            </CardContent>
            <CardFooter>
              <Button size="sm">Action</Button>
            </CardFooter>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Default</CardTitle>
              <CardDescription>spacing=4 (1rem)</CardDescription>
            </CardHeader>
            <CardContent>
              <p>Standard layout.</p>
            </CardContent>
            <CardFooter>
              <Button size="sm">Action</Button>
            </CardFooter>
          </Card>
          <Card className="[--card-spacing:--spacing(6)]">
            <CardHeader>
              <CardTitle>Loose</CardTitle>
              <CardDescription>spacing=6 (1.5rem)</CardDescription>
            </CardHeader>
            <CardContent>
              <p>Spacious layout.</p>
            </CardContent>
            <CardFooter>
              <Button size="sm">Action</Button>
            </CardFooter>
          </Card>
        </div>
      </Demo>

      <Demo
        title="Card · Image"
        description="Cards can display an image on top."      >
        <div className="w-full space-y-3">
          <Card>
            <img
              src="https://images.unsplash.com/photo-1506905925346-21bda4d32df4?w=800&h=400&fit=crop"
              alt="Mountain landscape"
              className="aspect-video w-full object-cover"
            />
            <CardHeader>
              <CardTitle>Mountain Escape</CardTitle>
              <CardDescription>A breathtaking view of the peaks.</CardDescription>
            </CardHeader>
            <CardFooter className="justify-end gap-2">
              <Button variant="ghost" size="sm">Later</Button>
              <Button size="sm">View</Button>
            </CardFooter>
          </Card>
        </div>
      </Demo>
      </Section>
  )
}
