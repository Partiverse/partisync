import { Check, Plus } from "lucide-react"
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Avatar, AvatarBadge, AvatarFallback, AvatarGroup, AvatarGroupCount, AvatarImage } from "@/components/ui/avatar"
import { Demo, Section } from "../shared"

export function AvatarSection() {
  return (
<Section
        id="avatar"
        title="Avatar"
        group="installation"
        description="An image element with a fallback for representing the user."
        fileKey="basics"
      >

      <Demo
        title="Avatar · Basic"
        description="Avatar shows an image with a fallback for missing sources."
        center      >
        <div className="flex flex-wrap items-center gap-4">
          <Avatar>
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarFallback>PS</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarFallback>FA</AvatarFallback>
          </Avatar>
        </div>
      </Demo>

      <Demo
        title="Avatar · Badge"
        description="AvatarBadge marks an avatar with a status indicator."
        center      >
        <div className="flex flex-wrap items-center gap-4">
          <Avatar>
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
            <AvatarBadge />
          </Avatar>
          <Avatar>
            <AvatarImage src="https://github.com/vercel.png" alt="@vercel" />
            <AvatarFallback>VR</AvatarFallback>
            <AvatarBadge />
          </Avatar>
        </div>
      </Demo>

      <Demo
        title="Avatar · Badge with Icon"
        description="AvatarBadge can display an icon."
        center      >
        <div className="flex flex-wrap items-center gap-4">
          <Avatar>
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
            <AvatarBadge>
              <Plus />
            </AvatarBadge>
          </Avatar>
          <Avatar>
            <AvatarFallback>PS</AvatarFallback>
            <AvatarBadge>
              <Check />
            </AvatarBadge>
          </Avatar>
        </div>
      </Demo>

      <Demo
        title="Avatar · Sizes（sm / default / lg）"
        description="Avatar supports small, default, and large sizes."
        center      >
        <div className="flex flex-wrap items-center gap-4">
          <Avatar size="sm">
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
          <Avatar size="default">
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
          <Avatar size="lg">
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
        </div>
      </Demo>

      <Demo
        title="Avatar · Avatar Group"
        description="AvatarGroup stacks multiple avatars with overlap."
        center      >
        <AvatarGroup>
          <Avatar>
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarImage src="https://github.com/vercel.png" alt="@vercel" />
            <AvatarFallback>VR</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarImage src="https://github.com/rauchg.png" alt="@rauchg" />
            <AvatarFallback>RG</AvatarFallback>
          </Avatar>
        </AvatarGroup>
      </Demo>

      <Demo
        title="Avatar · Avatar Group Count"
        description="AvatarGroupCount renders the number of hidden avatars."
        center      >
        <AvatarGroup>
          <Avatar>
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarImage src="https://github.com/vercel.png" alt="@vercel" />
            <AvatarFallback>VR</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarImage src="https://github.com/rauchg.png" alt="@rauchg" />
            <AvatarFallback>RG</AvatarFallback>
          </Avatar>
          <AvatarGroupCount>+5</AvatarGroupCount>
        </AvatarGroup>
      </Demo>

      <Demo
        title="Avatar · Avatar Group with Icon"
        description="AvatarGroupCount can display an icon."
        center      >
        <AvatarGroup>
          <Avatar>
            <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
            <AvatarFallback>CN</AvatarFallback>
          </Avatar>
          <Avatar>
            <AvatarImage src="https://github.com/vercel.png" alt="@vercel" />
            <AvatarFallback>VR</AvatarFallback>
          </Avatar>
          <AvatarGroupCount>
            <Plus />
          </AvatarGroupCount>
        </AvatarGroup>
      </Demo>

      <Demo
        title="Avatar · Dropdown"
        description="An avatar can trigger a dropdown menu on click."
        center      >
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <button>
                <Avatar>
                  <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
                  <AvatarFallback>CN</AvatarFallback>
                </Avatar>
              </button>
            }
          />
          <DropdownMenuContent align="end" className="w-40">
            <DropdownMenuGroup>
              <DropdownMenuItem>Profile</DropdownMenuItem>
              <DropdownMenuItem>Settings</DropdownMenuItem>
              <DropdownMenuItem>Billing</DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem>Logout</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </Demo>
      </Section>
  )
}
